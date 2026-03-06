//! Warehouse configuration entity.
//!
//! A [`WarehouseConfig`] describes a single upstream Databricks SQL warehouse
//! that NiXQueryLink360 can route statements to.  All connectivity details
//! (hostname, HTTP path, authentication token source) are encapsulated here,
//! keeping infrastructure concerns out of the application layer.

use serde::{Deserialize, Serialize};

/// Configuration for a single upstream Databricks SQL warehouse.
///
/// One proxy instance can serve multiple warehouses simultaneously.
/// Each warehouse entry must have a unique `id` that clients pass in
/// the `warehouse_id` field of their SQL statement request.
///
/// # Example (`config.toml`)
/// ```toml
/// [[upstream.warehouses]]
/// id        = "wh-prod"
/// host      = "adb-1234567890.1.azuredatabricks.net"
/// http_path = "/sql/1.0/warehouses/abc123"
/// token_env = "DATABRICKS_TOKEN_PROD"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarehouseConfig {
    /// Logical identifier used for routing (e.g. `"wh-prod"`, `"wh-dev"`).
    ///
    /// Must match the `warehouse_id` sent by API clients.
    pub id: String,

    /// Databricks workspace hostname without scheme, e.g.
    /// `"adb-1234567890.1.azuredatabricks.net"`.
    pub host: String,

    /// HTTP path to the warehouse's SQL endpoint, e.g.
    /// `"/sql/1.0/warehouses/abc123def456"`.
    pub http_path: String,

    /// Name of the environment variable that holds the Databricks PAT or
    /// OAuth token for this warehouse.  The token is resolved at request
    /// time via [`std::env::var`]; it is **never** stored in config files.
    pub token_env: String,
}

impl WarehouseConfig {
    /// Create a new `WarehouseConfig` from owned strings.
    pub fn new(id: String, host: String, http_path: String, token_env: String) -> Self {
        WarehouseConfig { id, host, http_path, token_env }
    }

    /// Build the HTTPS base URL for this warehouse.
    ///
    /// Used by the infrastructure layer to construct full endpoint URLs.
    ///
    /// # Example
    /// ```
    /// // base_url() → "https://adb-xxx.azuredatabricks.net"
    /// ```
    pub fn base_url(&self) -> String {
        format!("https://{}", self.host)
    }

    /// Validate that all required fields are non-empty.
    ///
    /// Called at startup to catch misconfigured warehouses before any
    /// request is attempted.
    ///
    /// # Errors
    /// Returns a human-readable `String` describing the first violation found.
    pub fn validate(&self) -> Result<(), String> {
        if self.id.trim().is_empty() {
            return Err("Warehouse id cannot be empty".to_string());
        }
        if self.host.trim().is_empty() {
            return Err("Warehouse host cannot be empty".to_string());
        }
        if self.http_path.trim().is_empty() {
            return Err("Warehouse http_path cannot be empty".to_string());
        }
        if self.token_env.trim().is_empty() {
            return Err("Warehouse token_env cannot be empty".to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config() -> WarehouseConfig {
        WarehouseConfig::new(
            "wh1".to_string(),
            "adb-xxx.azuredatabricks.net".to_string(),
            "/sql/1.0/warehouses/wh1".to_string(),
            "DATABRICKS_TOKEN".to_string(),
        )
    }

    #[test]
    fn test_base_url_format() {
        let cfg = make_config();
        assert_eq!(cfg.base_url(), "https://adb-xxx.azuredatabricks.net");
    }

    #[test]
    fn test_validate_valid_config() {
        assert!(make_config().validate().is_ok());
    }

    #[test]
    fn test_validate_empty_id_fails() {
        let mut cfg = make_config();
        cfg.id = "".to_string();
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_validate_empty_host_fails() {
        let mut cfg = make_config();
        cfg.host = "".to_string();
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_validate_empty_http_path_fails() {
        let mut cfg = make_config();
        cfg.http_path = "".to_string();
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_validate_empty_token_env_fails() {
        let mut cfg = make_config();
        cfg.token_env = "".to_string();
        assert!(cfg.validate().is_err());
    }
}
