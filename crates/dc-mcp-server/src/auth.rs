use axum::{
    Json, Router,
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
    routing::get,
};
use axum_extra::{
    TypedHeader,
    headers::{Authorization, authorization::Bearer},
};
use http::Method;
use networked_token_validator::NetworkedTokenValidator;
use schemars::JsonSchema;
use serde::Deserialize;
use std::env;
use tower_http::cors::{Any, CorsLayer};
use url::Url;

mod networked_token_validator;
mod protected_resource;
mod valid_token;
mod www_authenticate;

use protected_resource::ProtectedResource;
pub(crate) use valid_token::ValidToken;
use valid_token::ValidateToken;
use www_authenticate::WwwAuthenticate;

/// Auth configuration options
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct Config {
    /// List of upstream OAuth servers to delegate auth
    pub servers: Vec<Url>,

    /// List of accepted audiences for the OAuth tokens
    pub audiences: Vec<String>,

    /// The resource to protect.
    ///
    /// Note: This is usually the publicly accessible URL of this running MCP server
    pub resource: Url,

    /// Link to documentation related to the protected resource
    pub resource_documentation: Option<Url>,

    /// Supported OAuth scopes by this resource server
    pub scopes: Vec<String>,

    /// Whether to disable the auth token passthrough to upstream API
    #[serde(default)]
    pub disable_auth_token_passthrough: bool,
}

impl Config {
    pub fn enable_middleware(&self, router: Router) -> Router {
        /// Simple handler to encode our config into the desired OAuth 2.1 protected
        /// resource format
        async fn protected_resource(State(auth_config): State<Config>) -> Json<ProtectedResource> {
            Json(auth_config.into())
        }

        // Set up auth routes. NOTE: CORs needs to allow for get requests to the
        // metadata information paths.
        let cors = CorsLayer::new()
            .allow_methods([Method::GET])
            .allow_origin(Any);
        let auth_router = Router::new()
            .route(
                "/.well-known/oauth-protected-resource",
                get(protected_resource),
            )
            .with_state(self.clone())
            .layer(cors);

        // Merge with MCP server routes
        Router::new()
            .merge(auth_router)
            .merge(router.layer(axum::middleware::from_fn_with_state(
                self.clone(),
                oauth_validate,
            )))
    }
}

/// Validate that requests made have a corresponding bearer JWT token
#[tracing::instrument(skip_all, fields(status_code, reason))]
async fn oauth_validate(
    State(auth_config): State<Config>,
    token: Option<TypedHeader<Authorization<Bearer>>>,
    mut request: Request,
    next: Next,
) -> Result<Response, (StatusCode, TypedHeader<WwwAuthenticate>)> {
    // Consolidated unauthorized error for use with any fallible step in this process
    let unauthorized_error = || {
        let mut resource = auth_config.resource.clone();
        resource.set_path("/.well-known/oauth-protected-resource");

        (
            StatusCode::UNAUTHORIZED,
            TypedHeader(WwwAuthenticate::Bearer {
                resource_metadata: resource,
            }),
        )
    };

    let validator = NetworkedTokenValidator::new(&auth_config.audiences, &auth_config.servers);
    let token = token.ok_or_else(|| {
        tracing::Span::current().record("reason", "missing_token");
        tracing::Span::current().record("status_code", StatusCode::UNAUTHORIZED.as_u16());
        unauthorized_error()
    })?;

    let valid_token = validator.validate(token.0).await.ok_or_else(|| {
        tracing::Span::current().record("reason", "invalid_token");
        tracing::Span::current().record("status_code", StatusCode::UNAUTHORIZED.as_u16());
        unauthorized_error()
    })?;

    // Insert new context to ensure that handlers only use our enforced token verification
    // for propagation
    request.extensions_mut().insert(valid_token);

    let response = next.run(request).await;
    tracing::Span::current().record("status_code", response.status().as_u16());
    Ok(response)
}

/// Enable customer ID validation middleware if CUSTOMER_ID environment variable is set
/// This middleware validates that the X-Company-ID header matches the CUSTOMER_ID env var
pub fn enable_customer_id_validation(router: Router) -> Router {
    // Check if CUSTOMER_ID environment variable exists and is not empty
    let customer_id_env = match env::var("CUSTOMER_ID") {
        Ok(val) if !val.is_empty() => Some(val),
        _ => None,
    };

    if let Some(expected_customer_id) = customer_id_env {
        tracing::info!(
            "Customer ID validation enabled, expecting: {}",
            expected_customer_id
        );
        router.layer(axum::middleware::from_fn_with_state(
            expected_customer_id,
            customer_id_validate,
        ))
    } else {
        tracing::debug!("Customer ID validation disabled (CUSTOMER_ID env var not set)");
        router
    }
}

/// Validate that the X-Company-ID header matches the expected customer ID from environment
#[tracing::instrument(skip_all, fields(status_code, reason))]
async fn customer_id_validate(
    State(expected_customer_id): State<String>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Extract X-Company-ID header (HTTP header names are case-insensitive)
    let customer_id_header = request.headers().get("x-company-id");

    match customer_id_header {
        None => {
            tracing::Span::current().record("reason", "missing_x_company_id_header");
            tracing::Span::current().record("status_code", StatusCode::UNAUTHORIZED.as_u16());
            tracing::warn!("Request rejected: missing X-Company-ID header");
            Err(StatusCode::UNAUTHORIZED)
        }
        Some(header_value) => {
            // Convert header value to string and compare
            let header_str = match header_value.to_str() {
                Ok(s) => s,
                Err(_) => {
                    tracing::Span::current().record("reason", "invalid_x_company_id_header");
                    tracing::Span::current()
                        .record("status_code", StatusCode::UNAUTHORIZED.as_u16());
                    tracing::warn!(
                        "Request rejected: invalid X-Company-ID header value (not valid UTF-8)"
                    );
                    return Err(StatusCode::UNAUTHORIZED);
                }
            };

            // Case-sensitive comparison
            if header_str == expected_customer_id {
                tracing::debug!("Customer ID validation passed: {}", header_str);
                let response = next.run(request).await;
                tracing::Span::current().record("status_code", response.status().as_u16());
                Ok(response)
            } else {
                tracing::Span::current().record("reason", "customer_id_mismatch");
                tracing::Span::current().record("status_code", StatusCode::UNAUTHORIZED.as_u16());
                tracing::warn!(
                    "Request rejected: customer ID mismatch (expected: {}, got: {})",
                    expected_customer_id,
                    header_str
                );
                Err(StatusCode::UNAUTHORIZED)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::middleware::from_fn_with_state;
    use axum::routing::get;
    use axum::{
        Router,
        body::Body,
        http::{Request, StatusCode},
    };
    use http::header::{AUTHORIZATION, WWW_AUTHENTICATE};
    use tower::ServiceExt; // for .oneshot()
    use url::Url;

    fn test_config() -> Config {
        Config {
            servers: vec![Url::parse("http://localhost:1234").unwrap()],
            audiences: vec!["test-audience".to_string()],
            resource: Url::parse("http://localhost:4000").unwrap(),
            resource_documentation: None,
            scopes: vec!["read".to_string()],
            disable_auth_token_passthrough: false,
        }
    }

    fn test_router(config: Config) -> Router {
        Router::new()
            .route("/test", get(|| async { "ok" }))
            .layer(from_fn_with_state(config, oauth_validate))
    }

    #[tokio::test]
    async fn missing_token_returns_unauthorized() {
        let config = test_config();
        let app = test_router(config.clone());
        let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
        let headers = res.headers();
        let www_auth = headers.get(WWW_AUTHENTICATE).unwrap().to_str().unwrap();
        assert!(www_auth.contains("Bearer"));
        assert!(www_auth.contains("resource_metadata"));
    }

    #[tokio::test]
    async fn invalid_token_returns_unauthorized() {
        let config = test_config();
        let app = test_router(config.clone());
        let req = Request::builder()
            .uri("/test")
            .header(AUTHORIZATION, "Bearer invalidtoken")
            .body(Body::empty())
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
        let headers = res.headers();
        let www_auth = headers.get(WWW_AUTHENTICATE).unwrap().to_str().unwrap();
        assert!(www_auth.contains("Bearer"));
        assert!(www_auth.contains("resource_metadata"));
    }

    // Customer ID validation tests
    fn test_router_with_customer_id(expected_customer_id: String) -> Router {
        Router::new()
            .route("/test", get(|| async { "ok" }))
            .layer(from_fn_with_state(
                expected_customer_id,
                customer_id_validate,
            ))
    }

    #[tokio::test]
    async fn customer_id_matching_returns_ok() {
        let expected = "TestCustomer123".to_string();
        let app = test_router_with_customer_id(expected.clone());
        let req = Request::builder()
            .uri("/test")
            .header("X-Company-ID", expected)
            .body(Body::empty())
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn customer_id_mismatch_returns_unauthorized() {
        let expected = "TestCustomer123".to_string();
        let app = test_router_with_customer_id(expected);
        let req = Request::builder()
            .uri("/test")
            .header("X-Company-ID", "WrongCustomer")
            .body(Body::empty())
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn missing_customer_id_header_returns_unauthorized() {
        let expected = "TestCustomer123".to_string();
        let app = test_router_with_customer_id(expected);
        let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn customer_id_case_sensitive_comparison() {
        let expected = "TestCustomer123".to_string();
        let app = test_router_with_customer_id(expected);

        // Test lowercase version (should fail)
        let req = Request::builder()
            .uri("/test")
            .header("X-Company-ID", "testcustomer123")
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

        // Test exact match (should pass)
        let req = Request::builder()
            .uri("/test")
            .header("X-Company-ID", "TestCustomer123")
            .body(Body::empty())
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn customer_id_header_case_insensitive_name() {
        let expected = "TestCustomer123".to_string();
        let app = test_router_with_customer_id(expected);

        // Test lowercase header name (should still work)
        let req = Request::builder()
            .uri("/test")
            .header("x-company-id", "TestCustomer123")
            .body(Body::empty())
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn empty_customer_id_header_returns_unauthorized() {
        let expected = "TestCustomer123".to_string();
        let app = test_router_with_customer_id(expected);
        let req = Request::builder()
            .uri("/test")
            .header("X-Company-ID", "")
            .body(Body::empty())
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        // Empty string won't match, so should return unauthorized
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn enable_customer_id_validation_with_env_var() {
        // Save original env var if it exists
        let original = env::var("CUSTOMER_ID").ok();

        // Set CUSTOMER_ID env var BEFORE creating router
        unsafe {
            env::set_var("CUSTOMER_ID", "TestCustomer123");
        }

        // Create router and enable validation (env var is read at this point)
        let router = Router::new().route("/test", get(|| async { "ok" }));
        let app = enable_customer_id_validation(router);

        // Test with matching header
        let req = Request::builder()
            .uri("/test")
            .header("X-Company-ID", "TestCustomer123")
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(
            res.status(),
            StatusCode::OK,
            "Matching customer ID should return OK"
        );

        // Test with mismatched header (using same app instance)
        let req = Request::builder()
            .uri("/test")
            .header("X-Company-ID", "WrongCustomer")
            .body(Body::empty())
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(
            res.status(),
            StatusCode::UNAUTHORIZED,
            "Mismatched customer ID should return 401"
        );

        // Restore original env var
        unsafe {
            match original {
                Some(val) => env::set_var("CUSTOMER_ID", val),
                None => env::remove_var("CUSTOMER_ID"),
            }
        }
    }

    #[tokio::test]
    async fn enable_customer_id_validation_without_env_var() {
        // Save original env var if it exists
        let original = env::var("CUSTOMER_ID").ok();

        // Remove CUSTOMER_ID env var (ensure it's not set from previous tests)
        unsafe {
            env::remove_var("CUSTOMER_ID");
        }

        // Verify it's actually removed
        assert!(
            env::var("CUSTOMER_ID").is_err(),
            "CUSTOMER_ID should not be set"
        );

        let router = Router::new().route("/test", get(|| async { "ok" }));
        let app = enable_customer_id_validation(router);

        // Test without header (should pass since validation is disabled)
        let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(
            res.status(),
            StatusCode::OK,
            "Should pass when CUSTOMER_ID env var is not set"
        );

        // Restore original env var
        unsafe {
            match original {
                Some(val) => env::set_var("CUSTOMER_ID", val),
                None => env::remove_var("CUSTOMER_ID"),
            }
        }
    }

    #[tokio::test]
    async fn enable_customer_id_validation_with_empty_env_var() {
        // Save original env var if it exists
        let original = env::var("CUSTOMER_ID").ok();

        // Set empty CUSTOMER_ID env var (should disable validation)
        unsafe {
            env::set_var("CUSTOMER_ID", "");
        }

        // Verify it's set to empty string
        assert_eq!(
            env::var("CUSTOMER_ID").unwrap(),
            "",
            "CUSTOMER_ID should be empty string"
        );

        let router = Router::new().route("/test", get(|| async { "ok" }));
        let app = enable_customer_id_validation(router);

        // Test without header (should pass since validation is disabled for empty env var)
        let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(
            res.status(),
            StatusCode::OK,
            "Should pass when CUSTOMER_ID env var is empty"
        );

        // Restore original env var
        unsafe {
            match original {
                Some(val) => env::set_var("CUSTOMER_ID", val),
                None => env::remove_var("CUSTOMER_ID"),
            }
        }
    }
}
