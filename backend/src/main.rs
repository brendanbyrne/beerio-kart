use axum::{Json, Router, routing::get};
use serde::Serialize;

#[derive(Serialize)]
struct HelloResponse {
    message: String,
}

#[tokio::main]
async fn main() {
    let app = Router::new().route("/api/v1/hello", get(hello));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Listening on http://localhost:3000");
    axum::serve(listener, app).await.unwrap();
}

async fn hello() -> Json<HelloResponse> {
    Json(HelloResponse {
        message: "Hello from Beerio Kart!".to_string(),
    })
}
