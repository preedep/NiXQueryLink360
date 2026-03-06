use anyhow::Result;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ServerSettings {
    pub host: String,
    pub port: u16,
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

#[derive(Debug, Clone, Deserialize)]
pub struct WarehouseSettings {
    pub id: String,
    pub host: String,
    pub http_path: String,
    pub token_env: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpstreamSettings {
    pub default_warehouse_id: String,
    pub warehouses: Vec<WarehouseSettings>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PoolSettings {
    pub max_connections: u32,
    pub connection_timeout_secs: u64,
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

#[derive(Debug, Clone, Deserialize)]
pub struct RetrySettings {
    pub max_attempts: u32,
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

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingSettings {
    pub level: String,
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

#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    pub server: ServerSettings,
    pub upstream: UpstreamSettings,
    pub pool: PoolSettings,
    pub retry: RetrySettings,
    pub logging: LoggingSettings,
}

impl Settings {
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
