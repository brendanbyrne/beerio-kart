mod seed;

use std::{net::SocketAddr, sync::Arc, time::Duration};

use anyhow::Context;
use axum::{
    BoxError, Json, Router,
    error_handling::HandleErrorLayer,
    http::StatusCode,
    routing::{get, post, put},
};
use beerio_kart::{
    ARGON2_MAX_CONCURRENT, AppState,
    config::{self, Config},
    db, routes, services,
};
use migration::{Migrator, MigratorTrait};
use serde::Serialize;
use tokio::sync::Semaphore;
use tower::{ServiceBuilder, limit::ConcurrencyLimitLayer, load_shed::LoadShedLayer};
use tower_governor::{GovernorLayer, governor::GovernorConfigBuilder};
// `tower::timeout::TimeoutLayer` (cited in tokio.md § 12) produces a service
// whose error type is `BoxError`, which `axum::Router::layer` refuses because
// it needs `Into<Infallible>`. `tower_http::timeout::TimeoutLayer` is the
// HTTP-aware sibling: on elapsed it returns a real `408 Request Timeout`
// response, which is the right user-facing behavior anyway. Functionally
// satisfies the same "cap per-request wall time" rule.
use tower_http::{
    limit::RequestBodyLimitLayer,
    services::{ServeDir, ServeFile},
    timeout::TimeoutLayer,
    trace::TraceLayer,
};
use tracing_subscriber::EnvFilter;

#[derive(Serialize)]
struct HelloResponse {
    message: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file if present (non-fatal if missing)
    dotenvy::dotenv().ok();

    // Initialize structured logging. Defaults to `info` level; override with
    // the RUST_LOG env var (e.g., RUST_LOG=debug or RUST_LOG=beerio_kart=debug).
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn,sea_orm_migration=warn")),
        )
        .init();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:../data/db/beerio-kart.db?mode=rwc".to_string());

    // Load config from env vars (errors if JWT_SECRET is missing)
    let config = Config::from_env()?;

    // Build the SQLx pool with per-connection PRAGMAs (foreign_keys, busy_timeout,
    // synchronous, journal_mode), then wrap as a SeaORM DatabaseConnection. See
    // src/db.rs for the rationale and seaorm.md § 8 for the rule.
    let db = db::connect(&database_url)
        .await
        .context("Connecting to database")?;

    // Run all pending migrations. On a fresh database this creates every table.
    Migrator::up(&db, None)
        .await
        .context("Running database migrations")?;

    // Seed static game data (characters, tracks, cups, etc.) from JSON files.
    // Only inserts into empty tables — safe to call on every startup.
    tracing::info!("Seeding static data...");
    seed::run(&db).await.context("Seeding database")?;
    tracing::info!("Seeding complete");

    // Snapshot the limit knobs before `config` is moved into `state`. These
    // power the Tower middleware stack below.
    let request_timeout = Duration::from_secs(config.request_timeout_seconds);
    let concurrency_limit = config.request_concurrency_limit;
    let max_body_bytes = config.max_request_body_bytes;
    let rate_limit_per_minute = config.rate_limit_per_minute;

    let state = AppState {
        db,
        config,
        argon2_limit: Arc::new(Semaphore::new(ARGON2_MAX_CONCURRENT)),
    };

    // Clone the DB connection for the background cleanup task before `state`
    // is moved into the router.
    let cleanup_db = state.db.clone();

    // Per-peer-IP rate limit. The default `PeerIpKeyExtractor` reads the
    // remote address from the `ConnectInfo<SocketAddr>` extension, so the
    // server below must be served via `into_make_service_with_connect_info`.
    // If we ever sit behind a *trusted* reverse proxy, swap to
    // `SmartIpKeyExtractor` so the limit keys on the real client IP via
    // `X-Forwarded-For` / `X-Real-IP` instead of the proxy's address.
    //
    // Period (not rate) goes into `per_millisecond` — see
    // `config::governor_period_ms` for the math + unit tests.
    let governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .per_millisecond(config::governor_period_ms(rate_limit_per_minute))
            .burst_size(rate_limit_per_minute)
            .finish()
            .context("Invalid rate-limit config")?,
    );
    // The governor caches one entry per observed IP. Without periodic
    // pruning, that map grows unbounded across the process lifetime; the
    // standard `retain_recent()` cleanup loop is from `tower-governor`'s
    // README. Uses `tokio::time::interval` with `MissedTickBehavior::Skip`
    // (tokio.md § 8) — same pattern as the stale-session loop below, and
    // a real OS thread is overkill for a once-a-minute janitor.
    let governor_limiter = governor_conf.limiter().clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(60));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tick.tick().await;
            governor_limiter.retain_recent();
        }
    });

    // STATIC_DIR defaults to ../frontend/dist for local dev (running from backend/).
    // In Docker, set to /app/static where the built frontend is copied.
    let static_dir = std::env::var("STATIC_DIR").unwrap_or_else(|_| "../frontend/dist".to_string());

    let app = Router::new()
        .route("/api/v1/hello", get(hello))
        // Auth
        .route("/api/v1/auth/register", post(routes::auth::register))
        .route("/api/v1/auth/login", post(routes::auth::login))
        .route("/api/v1/auth/refresh", post(routes::auth::refresh))
        .route("/api/v1/auth/logout", post(routes::auth::logout))
        .route("/api/v1/auth/password", put(routes::auth::change_password))
        // Game data (pre-seeded, read-only)
        .route(
            "/api/v1/characters",
            get(routes::game_data::list_characters),
        )
        .route("/api/v1/bodies", get(routes::game_data::list_bodies))
        .route("/api/v1/wheels", get(routes::game_data::list_wheels))
        .route("/api/v1/gliders", get(routes::game_data::list_gliders))
        .route("/api/v1/cups", get(routes::game_data::list_cups))
        .route("/api/v1/cups/{id}", get(routes::game_data::get_cup))
        .route("/api/v1/tracks", get(routes::game_data::list_tracks))
        .route("/api/v1/tracks/{id}", get(routes::game_data::get_track))
        // Users
        .route("/api/v1/users", get(routes::users::list_users))
        .route(
            "/api/v1/users/{id}",
            get(routes::users::get_user).put(routes::users::update_user),
        )
        // Drink types
        .route(
            "/api/v1/drink-types",
            get(routes::drink_types::list_drink_types).post(routes::drink_types::create_drink_type),
        )
        .route(
            "/api/v1/drink-types/{id}",
            get(routes::drink_types::get_drink_type),
        )
        // Sessions
        .route(
            "/api/v1/sessions",
            get(routes::sessions::list_sessions).post(routes::sessions::create_session),
        )
        .route("/api/v1/sessions/mine", get(routes::sessions::my_session))
        .route("/api/v1/sessions/{id}", get(routes::sessions::get_session))
        .route(
            "/api/v1/sessions/{id}/join",
            post(routes::sessions::join_session),
        )
        .route(
            "/api/v1/sessions/{id}/leave",
            post(routes::sessions::leave_session),
        )
        .route(
            "/api/v1/sessions/{id}/next-track",
            post(routes::sessions::next_track),
        )
        .route(
            "/api/v1/sessions/{id}/skip-turn",
            post(routes::sessions::skip_turn),
        )
        .route(
            "/api/v1/sessions/{id}/races",
            get(routes::sessions::list_races),
        )
        .route(
            "/api/v1/sessions/{id}/races/{race_id}/skip",
            post(routes::sessions::skip_pending_race),
        )
        // Runs — /defaults before /{id} so literal matches first
        .route(
            "/api/v1/runs",
            get(routes::runs::list_runs).post(routes::runs::create_run),
        )
        .route("/api/v1/runs/defaults", get(routes::runs::get_defaults))
        .route(
            "/api/v1/runs/{id}",
            get(routes::runs::get_run).delete(routes::runs::delete_run),
        )
        // Request-shape limits (tokio.md § 12). Order is request-flow:
        // trace wraps everything (so rejections are still logged), governor
        // sheds abusive IPs, body-size rejects huge POSTs, then the
        // concurrency cap + load-shed pair turn "would queue" into a 503,
        // and timeout is innermost so it budgets the handler itself.
        // `.layer()` adds layers as outer wrappers, so the last call is
        // the outermost: in code order, innermost first.
        //
        // Saturation trade-off: `ConcurrencyLimitLayer` alone *queues* on
        // saturation — the 101st in-flight request hangs until the client
        // disconnects. `LoadShedLayer` makes the inner service return
        // `Overloaded` when no permit is available; `HandleErrorLayer`
        // maps that to 503. Net: under load, request #101 sees an
        // immediate 503 instead of an indefinite wait. Matches the Issue's
        // "503/429" verification contract.
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            request_timeout,
        ))
        // ConcurrencyLimit alone *queues* on saturation. Wrap it with
        // LoadShed (returns `Overloaded` when no permit is available) and
        // HandleError (maps `Overloaded` to 503). ServiceBuilder composes
        // the three into one stack whose top-level error type is `Infallible`
        // — axum's `Router::layer` requires that, which is why this group
        // is bundled into a ServiceBuilder instead of three separate
        // `.layer()` calls.
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(handle_load_shed_error))
                .layer(LoadShedLayer::new())
                .layer(ConcurrencyLimitLayer::new(concurrency_limit)),
        )
        .layer(RequestBodyLimitLayer::new(max_body_bytes))
        .layer(GovernorLayer {
            config: governor_conf,
        })
        .layer(TraceLayer::new_for_http())
        .with_state(state)
        // Serve frontend static files. If no API route or static file matches,
        // fall back to index.html so React Router can handle client-side routing.
        // Using .fallback() instead of .not_found_service() returns 200 (not 404).
        .fallback_service(
            ServeDir::new(&static_dir).fallback(ServeFile::new(format!("{static_dir}/index.html"))),
        );

    // Spawn background task to close stale sessions (no activity for 1 hour).
    // Runs every 5 minutes.
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(300)).await;
            match services::sessions::close_stale_sessions(&cleanup_db).await {
                Ok(0) => {}
                Ok(n) => tracing::info!("Closed {n} stale session(s)"),
                Err(_) => tracing::error!("Stale session cleanup failed"),
            }
        }
    });

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .context("Binding TCP listener on 0.0.0.0:3000")?;
    tracing::info!("Listening on http://localhost:3000");
    // `into_make_service_with_connect_info::<SocketAddr>` populates the
    // `ConnectInfo<SocketAddr>` request extension that `tower-governor`'s
    // `PeerIpKeyExtractor` reads to bucket rate-limit counters per IP.
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .context("HTTP server returned an error")?;

    Ok(())
}

async fn hello() -> Json<HelloResponse> {
    Json(HelloResponse {
        message: "Hello from Beerio Kart!".to_string(),
    })
}

/// Translate the only error type expected to reach `HandleErrorLayer` —
/// `tower::load_shed::error::Overloaded` from `LoadShedLayer` — into a 503.
/// Anything else is a programming bug (no other layer above this one errors),
/// so map it to 500 rather than panic.
async fn handle_load_shed_error(err: BoxError) -> (StatusCode, &'static str) {
    if err.is::<tower::load_shed::error::Overloaded>() {
        (StatusCode::SERVICE_UNAVAILABLE, "Service overloaded")
    } else {
        tracing::error!(error = %err, "unexpected error reached load-shed handler");
        (StatusCode::INTERNAL_SERVER_ERROR, "Internal error")
    }
}
