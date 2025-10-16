//! Configuration file management for Apollo MCP Server

use crate::errors::McpError;
use rmcp::model::ErrorCode;
use std::fs;
use std::path::Path;
use tracing::{debug, error, info};

pub struct ConfigManager {
    config_path: String,
}

impl ConfigManager {
    pub fn new(config_path: String) -> Self {
        Self { config_path }
    }

    /// Update the authorization token in the config file
    pub fn update_auth_token(&self, new_token: &str) -> Result<(), McpError> {
        info!("🔧 Updating config file with new token...");

        // Read current config
        let config_content = fs::read_to_string(&self.config_path).map_err(|e| {
            error!("Failed to read config file: {}", e);
            McpError::new(
                ErrorCode::INTERNAL_ERROR,
                format!("Failed to read config file: {}", e),
                None,
            )
        })?;

        // Update authorization header
        let updated_content = config_content
            .lines()
            .map(|line| {
                if line.contains("Authorization: Bearer") {
                    // Preserve leading whitespace (indentation)
                    let indent = line
                        .chars()
                        .take_while(|c| c.is_whitespace())
                        .collect::<String>();
                    format!("{}Authorization: Bearer {}", indent, new_token)
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        // Write updated config
        fs::write(&self.config_path, updated_content).map_err(|e| {
            error!("Failed to write updated config file: {}", e);
            McpError::new(
                ErrorCode::INTERNAL_ERROR,
                format!("Failed to write updated config file: {}", e),
                None,
            )
        })?;

        info!("✅ Config file updated with new token");
        Ok(())
    }

    /// Read the current authorization token from config file
    pub fn get_current_token(&self) -> Result<Option<String>, McpError> {
        let config_content = fs::read_to_string(&self.config_path).map_err(|e| {
            error!("Failed to read config file: {}", e);
            McpError::new(
                ErrorCode::INTERNAL_ERROR,
                format!("Failed to read config file: {}", e),
                None,
            )
        })?;

        for line in config_content.lines() {
            if line.contains("Authorization: Bearer")
                && let Some(token) = line.split("Bearer ").nth(1)
            {
                return Ok(Some(token.trim().to_string()));
            }
        }

        Ok(None)
    }

    /// Verify config file exists and is readable
    pub fn verify_config(&self) -> Result<(), McpError> {
        if !Path::new(&self.config_path).exists() {
            return Err(McpError::new(
                ErrorCode::INTERNAL_ERROR,
                format!("Config file does not exist: {}", self.config_path),
                None,
            ));
        }

        fs::read_to_string(&self.config_path).map_err(|e| {
            error!("Config file is not readable: {}", e);
            McpError::new(
                ErrorCode::INTERNAL_ERROR,
                format!("Config file is not readable: {}", e),
                None,
            )
        })?;

        debug!("Config file verified: {}", self.config_path);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Test config file creation and reading
    #[test]
    fn test_config_file_operations() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.yaml");

        // Create initial config
        let initial_config = r#"
endpoint: "https://api.example.com/graphql"
headers:
  Authorization: Bearer initial_token
  Content-Type: "application/json"
"#;
        fs::write(&config_path, initial_config).unwrap();

        let config_manager = ConfigManager::new(config_path.to_string_lossy().to_string());

        // Test reading current token
        let token = config_manager.get_current_token().unwrap();
        assert_eq!(token, Some("initial_token".to_string()));

        // Test updating token
        config_manager.update_auth_token("new_token").unwrap();

        // Verify token was updated
        let updated_token = config_manager.get_current_token().unwrap();
        assert_eq!(updated_token, Some("new_token".to_string()));

        // Verify config file content
        let config_content = fs::read_to_string(&config_path).unwrap();
        assert!(config_content.contains("Authorization: Bearer new_token"));
    }

    /// Test config file token update
    #[test]
    fn test_config_token_update() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.yaml");

        let initial_config = r#"
endpoint: "https://api.example.com/graphql"
headers:
  Authorization: Bearer old_token
"#;
        fs::write(&config_path, initial_config).unwrap();

        let config_manager = ConfigManager::new(config_path.to_string_lossy().to_string());

        // Update token
        config_manager.update_auth_token("new_token").unwrap();

        // Verify token was updated in the config file
        let updated_config = fs::read_to_string(&config_path).unwrap();
        assert!(updated_config.contains("Authorization: Bearer new_token"));
        assert!(!updated_config.contains("old_token"));
    }

    /// Test config file verification
    #[test]
    fn test_config_verification() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.yaml");

        let config_manager = ConfigManager::new(config_path.to_string_lossy().to_string());

        // Test with non-existent file
        let result = config_manager.verify_config();
        assert!(result.is_err());

        // Create valid config file
        let valid_config = r#"
endpoint: "https://api.example.com/graphql"
headers:
  Authorization: Bearer test_token
"#;
        fs::write(&config_path, valid_config).unwrap();

        // Should now pass verification
        let result = config_manager.verify_config();
        assert!(result.is_ok());
    }
}
