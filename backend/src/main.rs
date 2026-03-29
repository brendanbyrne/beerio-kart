use axum::{Router, response::Html, routing::get};

#[tokio::main]
async fn main() {
    let app = Router::new().route("/", get(hello));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Listening on http://localhost:3000");
    axum::serve(listener, app).await.unwrap();
}

async fn hello() -> Html<&'static str> {
    Html("<h1>Hello from Beerio Kart!</h1>")
}
