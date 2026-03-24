use watcher::config;

#[tokio::main]
async fn main() -> Result<(), String> {
    let config = config::Config::from_env()?;
    watcher::run(config).await
}
