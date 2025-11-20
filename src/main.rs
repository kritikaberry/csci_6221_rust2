mod config;
mod elo;
mod matchmaking;
mod storage;
mod tcp_server;

#[tokio::main]
async fn main() {
    // Initialize tracing (env RUST_LOG controls level, default info)
    use tracing_subscriber::{fmt, EnvFilter};
    use tracing_subscriber::prelude::*; // for .with()
    let _ = tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .try_init();

    let cfg = config::AppCfg::default();
    tracing::info!("🚀 CSCI 6221 – Matchmaking Server Running…");
    tcp_server::start_tcp_server(&cfg.tcp_addr).await;
}
