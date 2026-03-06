use anyhow::Result;
use tracing::info;
use nix_query_link::{
    infrastructure::config::settings::Settings,
    interfaces::http::router::create_router,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    let settings = Settings::load()?;

    let format = settings.logging.format.clone();
    if format == "json" {
        tracing_subscriber::fmt()
            .json()
            .with_env_filter(&settings.logging.level)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(&settings.logging.level)
            .init();
    }

    info!(
        version = env!("CARGO_PKG_VERSION"),
        host = %settings.server.host,
        port = settings.server.port,
        "NiXQueryLink360 starting"
    );

    let addr = format!("{}:{}", settings.server.host, settings.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    info!("Listening on {}", addr);

    let router = create_router(settings)?;

    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install Ctrl+C handler");
    tracing::info!("Shutdown signal received");
}
