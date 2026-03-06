//! NiXQueryLink360 — Databricks SQL Endpoint proxy.
//!
//! Entry point that:
//! 1. Loads [`Settings`] from `config.toml` and `NQL__*` environment variables
//! 2. Initialises structured logging (pretty or JSON format)
//! 3. Binds a TCP listener on the configured address
//! 4. Constructs the Axum router via dependency injection
//! 5. Runs until a `SIGINT` / `Ctrl-C` signal is received, then shuts down
//!    gracefully

use anyhow::Result;
use tracing::info;
use nix_query_link::{
    infrastructure::config::settings::Settings,
    interfaces::http::router::create_router,
};

#[tokio::main]
async fn main() -> Result<()> {
    // ── Configuration ─────────────────────────────────────────────────────────
    // Load settings first so we can initialise the logger at the configured level.
    let settings = Settings::load()?;

    // ── Logging ───────────────────────────────────────────────────────────────
    // `json` format is recommended for production / log aggregators.
    // `pretty` format is human-readable and colour-coded for local development.
    if settings.logging.format == "json" {
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
        version    = env!("CARGO_PKG_VERSION"),
        host       = %settings.server.host,
        port       = settings.server.port,
        log_level  = %settings.logging.level,
        log_format = %settings.logging.format,
        default_warehouse = %settings.upstream.default_warehouse_id,
        warehouse_count   = settings.upstream.warehouses.len(),
        "NiXQueryLink360 starting"
    );

    // ── TCP listener ──────────────────────────────────────────────────────────
    let addr = format!("{}:{}", settings.server.host, settings.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    info!(addr = %addr, "Listening for connections");

    // ── Router ────────────────────────────────────────────────────────────────
    let router = create_router(settings)?;

    // ── Serve ─────────────────────────────────────────────────────────────────
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("NiXQueryLink360 stopped");
    Ok(())
}

/// Wait for `SIGINT` (`Ctrl-C`) and emit a shutdown log line.
///
/// `axum::serve::with_graceful_shutdown` calls this future after all
/// in-flight requests have completed.
async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install Ctrl-C signal handler");
    info!("Shutdown signal received — draining in-flight requests");
}
