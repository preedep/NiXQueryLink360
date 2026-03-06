//! Runtime configuration — loaded from `config.toml` and `NQL__*` environment variables.
//!
//! Settings are resolved in the following priority order (highest first):
//! 1. **Environment variables** prefixed with `NQL__`, e.g. `NQL__SERVER__PORT=9000`
//! 2. **`config.toml`** file in the working directory (optional — skipped if absent)
//! 3. **Built-in defaults** compiled into the binary
//!
//! The double-underscore separator (`__`) allows nested keys, e.g.:
//! ```shell
//! NQL__UPSTREAM__DEFAULT_WAREHOUSE_ID=wh-prod
//! NQL__LOGGING__LEVEL=debug
//! ```

use anyhow::Result;
use serde::Deserialize;

// ── Server ───────────────────────────────────────────────────────────────────

/// HTTP server bind settings.
#[derive(Debug, Clone, Deserialize)]
pub struct ServerSettings {
    /// IP address to bind the listener to. Use `"0.0.0.0"` for all interfaces.
    /// Default: `"0.0.0.0"`.
    pub host: String,

    /// TCP port to listen on. Default: `8360`.
    pub port: u16,

    /// Maximum seconds to wait for a complete request before returning 408.
    /// Default: `30`.
    pub request_timeout_secs: u64,
}

impl Default for ServerSettings {
    fn default() -> Self {
        ServerSettings {
            host: "0.0.0.0".to_string(),
            port: 8360,
            request_timeout_secs: 30,
        }
    }
}

// ── Upstream ─────────────────────────────────────────────────────────────────

/// Per-warehouse connection details (mirrors [`crate::domain::entities::warehouse::WarehouseConfig`]).
///
/// Populated from the `[[upstream.warehouses]]` array in `config.toml`.
#[derive(Debug, Clone, Deserialize)]
pub struct WarehouseSettings {
    /// Logical warehouse identifier used for request routing.
    pub id: String,

    /// Databricks workspace hostname, e.g. `"adb-xxx.azuredatabricks.net"`.
    pub host: String,

    /// HTTP path to the warehouse SQL endpoint.
    pub http_path: String,

    /// Environment variable name that holds the Bearer token for this warehouse.
    /// The token is resolved at request time; it is **never** stored in config files.
    pub token_env: String,
}

/// Upstream Databricks routing configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct UpstreamSettings {
    /// Warehouse ID used when the client omits `warehouse_id` in their request.
    /// Must match an entry in `warehouses`.
    pub default_warehouse_id: String,

    /// List of available warehouses.  At least one entry is required.
    pub warehouses: Vec<WarehouseSettings>,
}

// ── Connection pool ───────────────────────────────────────────────────────────

/// `reqwest` connection pool settings.
#[derive(Debug, Clone, Deserialize)]
pub struct PoolSettings {
    /// Maximum idle connections to keep open per host. Default: `50`.
    pub max_connections: u32,

    /// Seconds to wait when establishing a new TCP connection. Default: `10`.
    pub connection_timeout_secs: u64,

    /// Seconds before an idle pooled connection is closed. Default: `300` (5 min).
    pub idle_timeout_secs: u64,
}

impl Default for PoolSettings {
    fn default() -> Self {
        PoolSettings {
            max_connections: 50,
            connection_timeout_secs: 10,
            idle_timeout_secs: 300,
        }
    }
}

// ── Retry ─────────────────────────────────────────────────────────────────────

/// Exponential-backoff retry settings for upstream HTTP requests.
///
/// Retries are attempted on status codes `429`, `500`, `502`, `503`, `504`
/// and on network-level failures.
#[derive(Debug, Clone, Deserialize)]
pub struct RetrySettings {
    /// Total number of attempts (including the first try). Default: `3`.
    pub max_attempts: u32,

    /// Base delay in milliseconds for the first retry.
    /// Subsequent retries double the delay: `base * 2^attempt`. Default: `500`.
    pub base_delay_ms: u64,
}

impl Default for RetrySettings {
    fn default() -> Self {
        RetrySettings {
            max_attempts: 3,
            base_delay_ms: 500,
        }
    }
}

// ── Logging ───────────────────────────────────────────────────────────────────

/// Structured logging settings.
#[derive(Debug, Clone, Deserialize)]
pub struct LoggingSettings {
    /// `tracing` filter directive, e.g. `"info"`, `"debug"`, or
    /// `"nix_query_link=debug,tower_http=warn"`. Default: `"info"`.
    pub level: String,

    /// Log output format.
    /// - `"pretty"` — human-readable coloured output (recommended for development)
    /// - `"json"`   — newline-delimited JSON (recommended for production / log aggregators)
    ///
    /// Default: `"pretty"`.
    pub format: String,
}

impl Default for LoggingSettings {
    fn default() -> Self {
        LoggingSettings {
            level: "info".to_string(),
            format: "pretty".to_string(),
        }
    }
}

// ── Root ──────────────────────────────────────────────────────────────────────

/// Complete application settings loaded at startup.
///
/// Obtain an instance via [`Settings::load`] before constructing any
/// infrastructure or interface objects.
#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    /// HTTP server bind address and timeout.
    pub server: ServerSettings,
    /// Upstream Databricks warehouse routing config.
    pub upstream: UpstreamSettings,
    /// HTTP connection pool tunables.
    pub pool: PoolSettings,
    /// Retry policy for upstream requests.
    pub retry: RetrySettings,
    /// Log level and output format.
    pub logging: LoggingSettings,
}

impl Settings {
    /// Load settings from `config.toml` and `NQL__*` environment variables.
    ///
    /// Missing keys fall back to safe built-in defaults so the binary can
    /// start without a config file (useful for local development).
    ///
    /// # Errors
    /// Returns an [`anyhow::Error`] if:
    /// - `config.toml` exists but cannot be parsed
    /// - An `NQL__*` environment variable contains a value that cannot be
    ///   coerced to the expected type
    pub fn load() -> Result<Self> {
        let cfg = config::Config::builder()
            .add_source(config::File::with_name("config").required(false))
            .add_source(
                config::Environment::with_prefix("NQL")
                    .separator("__")
                    .try_parsing(true),
            )
            .set_default("server.host", "0.0.0.0")?
            .set_default("server.port", 8360i64)?
            .set_default("server.request_timeout_secs", 30i64)?
            .set_default("upstream.default_warehouse_id", "")?
            .set_default("upstream.warehouses", Vec::<String>::new())?
            .set_default("pool.max_connections", 50i64)?
            .set_default("pool.connection_timeout_secs", 10i64)?
            .set_default("pool.idle_timeout_secs", 300i64)?
            .set_default("retry.max_attempts", 3i64)?
            .set_default("retry.base_delay_ms", 500i64)?
            .set_default("logging.level", "info")?
            .set_default("logging.format", "pretty")?
            .build()?;

        Ok(cfg.try_deserialize()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_defaults() {
        let s = ServerSettings::default();
        assert_eq!(s.host, "0.0.0.0");
        assert_eq!(s.port, 8360);
        assert_eq!(s.request_timeout_secs, 30);
    }

    #[test]
    fn test_pool_defaults() {
        let p = PoolSettings::default();
        assert_eq!(p.max_connections, 50);
        assert_eq!(p.connection_timeout_secs, 10);
    }

    #[test]
    fn test_retry_defaults() {
        let r = RetrySettings::default();
        assert_eq!(r.max_attempts, 3);
        assert_eq!(r.base_delay_ms, 500);
    }

    #[test]
    fn test_logging_defaults() {
        let l = LoggingSettings::default();
        assert_eq!(l.level, "info");
        assert_eq!(l.format, "pretty");
    }
}
