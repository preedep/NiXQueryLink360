use serde::{Deserialize, Serialize};

/// Configuration for a single upstream Databricks warehouse
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarehouseConfig {
    pub id: String,
    pub host: String,
    pub http_path: String,
    /// Environment variable name that holds the access token
    pub token_env: String,
}

impl WarehouseConfig {
    pub fn new(id: String, host: String, http_path: String, token_env: String) -> Self {
        WarehouseConfig {
            id,
            host,
            http_path,
            token_env,
        }
    }

    /// Build the base URL for this warehouse
    pub fn base_url(&self) -> String {
        format!("https://{}", self.host)
    }

    /// Validate the config fields
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
