//! reqwest-backed [`HttpConnector`] implementation (OAPI-01).
//!
//! Lifts the pmcp-run reference `HttpClient::execute_with_options` body into a
//! toolkit-owned [`HttpClient`] that implements [`HttpConnector`]. The concrete
//! shape mirrors `crate::sql::sqlite::SqliteConnector` (a concrete connector impl
//! + constructor). Construction is LAZY — `new` parses the base URL but contacts
//! no backend (CF-2). URL building uses the shared [`crate::http::join_url`]
//! helper so an API-Gateway stage prefix (`/v1`) survives (Pitfall 2 — explicit
//! path concatenation, never the RFC-3986 url-crate path merge). Error messages
//! never echo the URL or a credential (Pitfall 5).

use super::auth::HttpAuthProvider;
use super::{join_url, HttpConnector, HttpConnectorError, Operation};
use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// HTTP client configuration (OWNED here in `http`, mirroring [`super::AuthConfig`]
/// ownership so Plan 02 re-exports it rather than redefining).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HttpConfig {
    /// Request timeout in seconds.
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
    /// Number of retry attempts on 5xx / connect / timeout.
    #[serde(default = "default_retries")]
    pub retries: u32,
    /// Base backoff in milliseconds (exponential per attempt).
    #[serde(default = "default_retry_backoff")]
    pub retry_backoff_ms: u64,
    /// `User-Agent` header for all requests.
    #[serde(default = "default_user_agent")]
    pub user_agent: String,
    /// Extra headers applied to every request.
    #[serde(default)]
    pub default_headers: HashMap<String, String>,
}

fn default_timeout() -> u64 {
    30
}
fn default_retries() -> u32 {
    3
}
fn default_retry_backoff() -> u64 {
    1000
}
fn default_user_agent() -> String {
    format!("pmcp-server-toolkit/{}", env!("CARGO_PKG_VERSION"))
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            timeout_seconds: default_timeout(),
            retries: default_retries(),
            retry_backoff_ms: default_retry_backoff(),
            user_agent: default_user_agent(),
            default_headers: HashMap::new(),
        }
    }
}

/// reqwest-backed [`HttpConnector`].
pub struct HttpClient {
    client: reqwest::Client,
    base_url: url::Url,
    auth: Arc<dyn HttpAuthProvider>,
    http_config: HttpConfig,
}

impl HttpClient {
    /// Construct a client. LAZY: parses `base_url` but contacts no backend (CF-2).
    ///
    /// # Errors
    ///
    /// Returns [`HttpConnectorError::Backend`] when `base_url` is unparseable or
    /// the reqwest client cannot be built. The error message does NOT echo the URL.
    pub fn new(
        client: reqwest::Client,
        base_url: String,
        auth: Arc<dyn HttpAuthProvider>,
    ) -> Result<Self, HttpConnectorError> {
        Self::with_config(client, base_url, auth, HttpConfig::default())
    }

    /// Construct a client with an explicit [`HttpConfig`]. LAZY (CF-2).
    ///
    /// # Errors
    ///
    /// As [`HttpClient::new`].
    pub fn with_config(
        client: reqwest::Client,
        base_url: String,
        auth: Arc<dyn HttpAuthProvider>,
        http_config: HttpConfig,
    ) -> Result<Self, HttpConnectorError> {
        let base_url = url::Url::parse(&base_url)
            .map_err(|_| HttpConnectorError::Backend("invalid base URL".to_string()))?;
        Ok(Self {
            client,
            base_url,
            auth,
            http_config,
        })
    }

    /// Build a client from an [`HttpConfig`], constructing the reqwest client with
    /// the configured timeout, user-agent, and default headers. LAZY (CF-2).
    ///
    /// # Errors
    ///
    /// As [`HttpClient::new`].
    pub fn from_config(
        base_url: String,
        auth: Arc<dyn HttpAuthProvider>,
        http_config: HttpConfig,
    ) -> Result<Self, HttpConnectorError> {
        let mut headers = HeaderMap::new();
        if let Ok(ua) = HeaderValue::from_str(&http_config.user_agent) {
            headers.insert(reqwest::header::USER_AGENT, ua);
        }
        for (key, value) in &http_config.default_headers {
            if let (Ok(name), Ok(val)) = (
                HeaderName::try_from(key.as_str()),
                HeaderValue::try_from(value.as_str()),
            ) {
                headers.insert(name, val);
            }
        }
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(http_config.timeout_seconds))
            .default_headers(headers)
            .build()
            .map_err(|_| HttpConnectorError::Backend("failed to build HTTP client".to_string()))?;
        Self::with_config(client, base_url, auth, http_config)
    }

    /// Substitute path parameters into the operation path template.
    fn substitute_path(
        operation: &Operation,
        args: &serde_json::Map<String, serde_json::Value>,
    ) -> String {
        let mut path = operation.path.clone();
        for param in operation.path_parameters() {
            let placeholder = format!("{{{}}}", param.name);
            if let Some(value) = args.get(&param.name) {
                let value_str = value_to_string(value);
                path = path.replace(&placeholder, &value_str);
            }
        }
        path
    }

    /// Build the query map from query-located params present in `args`.
    fn build_query(
        operation: &Operation,
        args: &serde_json::Map<String, serde_json::Value>,
    ) -> HashMap<String, String> {
        let mut query = HashMap::new();
        for param in operation.query_parameters() {
            if let Some(value) = args.get(&param.name) {
                if let serde_json::Value::Array(arr) = value {
                    // Comma-separate array members (OpenAPI `form`/`simple` style).
                    let mut csv = String::new();
                    for (i, member) in arr.iter().enumerate() {
                        if i > 0 {
                            csv.push(',');
                        }
                        csv.push_str(&value_to_string(member));
                    }
                    query.insert(param.name.clone(), csv);
                } else {
                    query.insert(param.name.clone(), value_to_string(value));
                }
            }
        }
        query
    }

    /// Build the header map from header-located params present in `args`.
    fn build_headers(
        operation: &Operation,
        args: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<HeaderMap, HttpConnectorError> {
        let mut headers = HeaderMap::new();
        for param in operation.header_parameters() {
            if let Some(value) = args.get(&param.name) {
                let name = HeaderName::try_from(param.name.as_str()).map_err(|_| {
                    HttpConnectorError::InvalidHeader("invalid header name".to_string())
                })?;
                let val = HeaderValue::try_from(value_to_string(value)).map_err(|_| {
                    HttpConnectorError::InvalidHeader("invalid header value".to_string())
                })?;
                headers.insert(name, val);
            }
        }
        Ok(headers)
    }

    /// Collect the request body: args that are NOT path/query/header params.
    fn build_body(
        operation: &Operation,
        args: &serde_json::Map<String, serde_json::Value>,
    ) -> Option<serde_json::Value> {
        if !operation.has_request_body {
            return None;
        }
        if let Some(body) = args.get("body") {
            return Some(body.clone());
        }
        let declared: std::collections::HashSet<&str> = operation
            .parameters
            .iter()
            .map(|p| p.name.as_str())
            .collect();
        let body: serde_json::Map<String, serde_json::Value> = args
            .iter()
            .filter(|(k, _)| !declared.contains(k.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        if body.is_empty() {
            None
        } else {
            Some(serde_json::Value::Object(body))
        }
    }

    fn convert_method(method: &str) -> Result<reqwest::Method, HttpConnectorError> {
        match method.to_uppercase().as_str() {
            "GET" => Ok(reqwest::Method::GET),
            "POST" => Ok(reqwest::Method::POST),
            "PUT" => Ok(reqwest::Method::PUT),
            "PATCH" => Ok(reqwest::Method::PATCH),
            "DELETE" => Ok(reqwest::Method::DELETE),
            "HEAD" => Ok(reqwest::Method::HEAD),
            "OPTIONS" => Ok(reqwest::Method::OPTIONS),
            _ => Err(HttpConnectorError::Backend(
                "unknown HTTP method".to_string(),
            )),
        }
    }

    /// Send the request, retrying on 5xx / connect / timeout with exponential backoff.
    async fn send_with_retries(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<reqwest::Response, HttpConnectorError> {
        let max_retries = self.http_config.retries;
        let mut last_status: Option<u16> = None;
        for attempt in 0..=max_retries {
            if attempt > 0 {
                let delay = self.http_config.retry_backoff_ms * (1u64 << (attempt - 1));
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }
            let Some(attempt_request) = request.try_clone() else {
                return Err(HttpConnectorError::Request(
                    "request body is not retryable".to_string(),
                ));
            };
            match attempt_request.send().await {
                Ok(response) => {
                    let status = response.status();
                    if status.is_server_error() && attempt < max_retries {
                        last_status = Some(status.as_u16());
                        continue;
                    }
                    return Ok(response);
                },
                Err(e) => {
                    let retryable = e.is_connect() || e.is_timeout();
                    if retryable && attempt < max_retries {
                        continue;
                    }
                    // Redacted: never forward the reqwest error Display (echoes URL).
                    return Err(HttpConnectorError::Request(
                        "transport error contacting backend".to_string(),
                    ));
                },
            }
        }
        Err(HttpConnectorError::Status {
            status: last_status.unwrap_or(0),
        })
    }
}

/// Stringify a JSON scalar for use in a path / query / header position.
fn value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        other => other.to_string(),
    }
}

#[async_trait]
impl HttpConnector for HttpClient {
    async fn execute(
        &self,
        operation: &Operation,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value, HttpConnectorError> {
        let empty = serde_json::Map::new();
        let args_map = args.as_object().unwrap_or(&empty);

        // Build URL via the shared join_url helper (explicit concat, never the
        // url-crate RFC-3986 path merge) — preserves a stage prefix like /v1
        // (Pitfall 2 / T-90-01-05).
        let substituted = Self::substitute_path(operation, args_map);
        let joined = join_url(self.base_url.as_str(), &substituted);
        let mut url = url::Url::parse(&joined)
            .map_err(|_| HttpConnectorError::Backend("constructed URL is invalid".to_string()))?;

        let mut query = Self::build_query(operation, args_map);
        let mut headers = Self::build_headers(operation, args_map)?;

        // Single-call tools have no per-request passthrough token (Plan 04/06 carry
        // it through HttpCodeExecutor); pass None here.
        self.auth.apply(&mut headers, &mut query, None).await?;

        // Why: reqwest 0.13 gates `RequestBuilder::query` behind a `query` feature
        // (verified in reqwest-0.13.2 request.rs:`#[cfg(feature = "query")]`). The
        // toolkit deliberately does NOT enable that feature (Pitfall 4 / lean
        // build), so query params are appended to the URL via `url`'s built-in,
        // percent-encoding query-pair serializer instead.
        if !query.is_empty() {
            let mut pairs = url.query_pairs_mut();
            for (key, value) in &query {
                pairs.append_pair(key, value);
            }
            drop(pairs);
        }

        let method = Self::convert_method(&operation.method)?;
        let mut request = self.client.request(method, url);
        request = request.headers(headers);
        if let Some(body) = Self::build_body(operation, args_map) {
            request = request.json(&body);
        }

        let response = self.send_with_retries(request).await?;
        let status = response.status();
        if !status.is_success() {
            return Err(HttpConnectorError::Status {
                status: status.as_u16(),
            });
        }
        let body = response
            .text()
            .await
            .map_err(|_| HttpConnectorError::Request("failed to read response body".to_string()))?;
        if body.is_empty() {
            return Ok(serde_json::Value::Null);
        }
        serde_json::from_str(&body).map_err(|_| {
            HttpConnectorError::Backend("response body was not valid JSON".to_string())
        })
    }

    fn base_url(&self) -> &str {
        self.base_url.as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::auth::NoAuth;
    use crate::http::{Parameter, ParameterLocation};

    fn get_user_op() -> Operation {
        Operation {
            method: "GET".to_string(),
            path: "/users/{id}".to_string(),
            parameters: vec![
                Parameter::new("id", ParameterLocation::Path, true),
                Parameter::new("verbose", ParameterLocation::Query, false),
            ],
            has_request_body: false,
            base_url: None,
        }
    }

    #[test]
    fn test_build_url_with_path_prefix() {
        // Regression: an API-Gateway stage prefix /v1 survives via join_url.
        let client = HttpClient::new(
            reqwest::Client::new(),
            "https://xxx.execute-api.eu-west-1.amazonaws.com/v1/".to_string(),
            Arc::new(NoAuth),
        )
        .unwrap();
        let op = get_user_op();
        let mut args = serde_json::Map::new();
        args.insert("id".to_string(), serde_json::json!("42"));
        let substituted = HttpClient::substitute_path(&op, &args);
        let joined = join_url(client.base_url(), &substituted);
        assert_eq!(
            joined,
            "https://xxx.execute-api.eu-west-1.amazonaws.com/v1/users/42"
        );
    }

    #[test]
    fn test_substitute_path_replaces_placeholder() {
        let op = get_user_op();
        let mut args = serde_json::Map::new();
        args.insert("id".to_string(), serde_json::json!(7));
        assert_eq!(HttpClient::substitute_path(&op, &args), "/users/7");
    }

    #[test]
    fn test_build_query_skips_path_params() {
        let op = get_user_op();
        let mut args = serde_json::Map::new();
        args.insert("id".to_string(), serde_json::json!("42"));
        args.insert("verbose".to_string(), serde_json::json!(true));
        let query = HttpClient::build_query(&op, &args);
        assert_eq!(query.get("verbose"), Some(&"true".to_string()));
        assert!(!query.contains_key("id"));
    }

    #[test]
    fn test_new_is_lazy_and_rejects_bad_url() {
        // Lazy: a bad URL fails synchronously without any network (CF-2).
        let err = HttpClient::new(
            reqwest::Client::new(),
            "not a url".to_string(),
            Arc::new(NoAuth),
        )
        .err()
        .expect("bad URL should error");
        assert!(matches!(err, HttpConnectorError::Backend(_)));
        let rendered = err.to_string();
        assert!(!rendered.contains("not a url"), "must not echo the bad URL");
    }

    #[tokio::test]
    async fn http_connector_get_returns_json() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/42"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"id": 42, "name": "Ada"})),
            )
            .mount(&server)
            .await;

        let client =
            HttpClient::new(reqwest::Client::new(), server.uri(), Arc::new(NoAuth)).unwrap();
        let op = get_user_op();
        let args = serde_json::json!({"id": "42"});
        let result = client.execute(&op, &args).await.unwrap();
        assert_eq!(result["id"], 42);
        assert_eq!(result["name"], "Ada");
    }

    #[tokio::test]
    async fn http_connector_post_sends_body_and_auth() {
        use wiremock::matchers::{body_json, header, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/items"))
            .and(header("authorization", "Bearer tok"))
            .and(body_json(serde_json::json!({"name": "widget"})))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({"ok": true})))
            .mount(&server)
            .await;

        let auth = crate::http::auth::create_auth_provider(&crate::http::AuthConfig::Bearer {
            token: "tok".to_string(),
            required: true,
        })
        .unwrap();
        let client = HttpClient::new(reqwest::Client::new(), server.uri(), auth).unwrap();
        let op = Operation {
            method: "POST".to_string(),
            path: "/items".to_string(),
            parameters: vec![],
            has_request_body: true,
            base_url: None,
        };
        let args = serde_json::json!({"name": "widget"});
        let result = client.execute(&op, &args).await.unwrap();
        assert_eq!(result["ok"], true);
    }

    #[tokio::test]
    async fn http_connector_maps_non_2xx_to_status_without_url() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/42"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let client =
            HttpClient::new(reqwest::Client::new(), server.uri(), Arc::new(NoAuth)).unwrap();
        let op = get_user_op();
        let args = serde_json::json!({"id": "42"});
        let err = client.execute(&op, &args).await.unwrap_err();
        assert!(matches!(err, HttpConnectorError::Status { status: 404 }));
        let rendered = err.to_string();
        assert!(rendered.contains("404"));
        assert!(
            !rendered.contains("http://"),
            "status error must not echo the URL"
        );
    }
}
