//! Startup and initialization functions for Apollo MCP Server

use crate::config_manager::ConfigManager;
use crate::errors::McpError;
use crate::token_manager::TokenManager;
use rmcp::model::ErrorCode;
use std::env;
use tracing::{debug, info, warn};

/// Initialize the Apollo MCP Server with token refresh and environment setup
pub async fn initialize_with_token_refresh(
    config_path: String,
    refresh_token: String,
    refresh_url: String,
    graphql_endpoint: String,
) -> Result<(), McpError> {
    info!("🎯 Apollo MCP Server initializing with token refresh...");

    // Step 1: Verify config file
    let config_manager = ConfigManager::new(config_path.clone());
    config_manager.verify_config().map_err(|e| {
        warn!("Config verification failed: {}", e);
        e
    })?;

    // Step 2: Initialize token manager
    let mut token_manager = TokenManager::new(refresh_token, refresh_url)?;

    // Step 3: Get fresh token
    let new_token = token_manager.get_valid_token().await.map_err(|e| {
        warn!("Token refresh failed: {}", e);
        e
    })?;

    // Step 4: Update config file with new token
    config_manager.update_auth_token(&new_token).map_err(|e| {
        warn!("Config update failed: {}", e);
        e
    })?;

    // Step 5: Verify the new token
    if !token_manager
        .verify_token(&new_token, &graphql_endpoint)
        .await
        .map_err(|e| {
            warn!("Token verification failed: {}", e);
            e
        })?
    {
        return Err(McpError::new(
            ErrorCode::INTERNAL_ERROR,
            "Token verification failed after refresh".to_string(),
            None,
        ));
    }

    // Step 6: Start background token refresh task
    token_manager
        .start_refresh_task(graphql_endpoint, config_path)
        .await;

    // Step 7: Set up environment variables
    setup_environment_variables();

    info!("✅ Apollo MCP Server initialization complete");
    Ok(())
}

/// Set up environment variables for optimal HTTP client behavior
fn setup_environment_variables() {
    info!("🔧 Setting up environment variables...");

    unsafe {
        // Set environment variables to match curl behavior
        env::set_var("RUSTLS_SYSTEM_CERT_ROOT", "1"); // Use system certificates like curl
        env::set_var("HTTP_PROXY", ""); // Disable proxy
        env::set_var("HTTPS_PROXY", ""); // Disable HTTPS proxy
        env::set_var("NO_PROXY", ""); // Clear no-proxy list
        env::set_var("RUST_LOG", "error"); // Set logging level to error

        // Set reqwest-specific environment variables
        env::set_var("REQWEST_TIMEOUT", "30"); // 30 second timeout
        env::set_var("REQWEST_CONNECT_TIMEOUT", "10"); // 10 second connect timeout
        env::set_var("REQWEST_USER_AGENT", "curl/8.4.0"); // Match curl user agent
        env::set_var("REQWEST_SSL_VERIFY", "true"); // Enable SSL verification
        env::set_var("REQWEST_SSL_VERIFY_HOSTNAME", "true"); // Enable hostname verification
    }

    debug!("Environment variables set:");
    debug!("   RUSTLS_SYSTEM_CERT_ROOT=1");
    debug!("   HTTP_PROXY=");
    debug!("   HTTPS_PROXY=");
    debug!("   RUST_LOG=error");
    debug!("   REQWEST_TIMEOUT=30");
    debug!("   REQWEST_CONNECT_TIMEOUT=10");
    debug!("   REQWEST_USER_AGENT=curl/8.4.0");
    debug!("   REQWEST_SSL_VERIFY=true");
    debug!("   REQWEST_SSL_VERIFY_HOSTNAME=true");
}

/// Check if token refresh is enabled via environment variables
pub fn is_token_refresh_enabled() -> bool {
    env::var("DC_TOKEN_REFRESH_ENABLED")
        .ok()
        .map(|s| s == "true")
        .unwrap_or(false)
}

/// Get refresh token from environment
pub fn get_refresh_token() -> Option<String> {
    env::var("DC_REFRESH_TOKEN").ok()
}

/// Get refresh URL from environment
pub fn get_refresh_url() -> Option<String> {
    env::var("DC_REFRESH_URL").ok()
}

/// Get GraphQL endpoint from environment
pub fn get_graphql_endpoint() -> Option<String> {
    env::var("DC_GRAPHQL_ENDPOINT").ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config_manager::ConfigManager;
    use crate::token_manager::TokenManager;
    use std::fs;
    use tempfile::TempDir;

    /// Test complete initialization flow
    #[tokio::test]
    async fn test_complete_initialization_flow() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.yaml");

        // Create initial config
        let initial_config = r#"
endpoint: "https://api.example.com/graphql"
headers:
  Authorization: "Bearer old_token"
"#;
        fs::write(&config_path, initial_config).unwrap();

        let _config_manager = ConfigManager::new(config_path.to_string_lossy().to_string());

        // Test environment variable setup
        unsafe {
            std::env::set_var("DC_REFRESH_TOKEN", "test_refresh_token");
            std::env::set_var("DC_REFRESH_URL", "https://api.example.com/refresh");
            std::env::set_var("DC_GRAPHQL_ENDPOINT", "https://api.example.com/graphql");
        }

        // Test getting refresh token from environment
        let refresh_token = get_refresh_token();
        assert_eq!(refresh_token, Some("test_refresh_token".to_string()));

        // Test getting refresh URL from environment
        let refresh_url = get_refresh_url();
        assert_eq!(
            refresh_url,
            Some("https://api.example.com/refresh".to_string())
        );

        // Test getting GraphQL endpoint from environment
        let endpoint = get_graphql_endpoint();
        assert_eq!(
            endpoint,
            Some("https://api.example.com/graphql".to_string())
        );

        // Clean up environment variables
        unsafe {
            std::env::remove_var("DC_REFRESH_TOKEN");
            std::env::remove_var("DC_REFRESH_URL");
            std::env::remove_var("DC_GRAPHQL_ENDPOINT");
        }
    }

    /// Test initialization with missing environment variables
    #[tokio::test]
    async fn test_initialization_missing_env_vars() {
        // Ensure environment variables are not set
        unsafe {
            std::env::remove_var("DC_REFRESH_TOKEN");
            std::env::remove_var("DC_REFRESH_URL");
        }

        // Test getting missing refresh token
        let refresh_token = get_refresh_token();
        assert_eq!(refresh_token, None);

        // Test getting missing refresh URL
        let refresh_url = get_refresh_url();
        assert_eq!(refresh_url, None);
    }

    /// Test token manager integration
    #[tokio::test]
    async fn test_token_manager_integration() {
        let refresh_token = "test_refresh_token";
        let refresh_url = "https://api.example.com/refresh";

        // Test creating token manager
        let token_manager = TokenManager::new(refresh_token.to_string(), refresh_url.to_string());
        assert!(token_manager.is_ok());

        let token_manager = token_manager.unwrap();
        assert_eq!(token_manager.refresh_token(), refresh_token);
        assert_eq!(token_manager.refresh_url(), refresh_url);
    }
}
