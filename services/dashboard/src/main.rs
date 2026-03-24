use dashboard::{app::build_app, config::load_config};

#[tokio::main]
async fn main() {
    let _ = dotenvy::from_path("services/dashboard/.env");

    let config = load_config();

    let (app, addr) = build_app(config).await;

    println!("Dashboard listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind listener");

    axum::serve(listener, app)
        .await
        .expect("Server failed");
}
