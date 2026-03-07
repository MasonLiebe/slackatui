use chrono::Utc;
use rand::RngCore;
use serde::Deserialize;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio_rustls::TlsAcceptor;

use super::StoredTokens;

const SLACK_AUTHORIZE_URL: &str = "https://slack.com/oauth/v2/authorize";
const SLACK_TOKEN_URL: &str = "https://slack.com/api/oauth.v2.access";

/// Default user token scopes required by slackatui.
pub const DEFAULT_USER_SCOPES: &[&str] = &[
    "channels:read",
    "channels:history",
    "channels:write",
    "groups:read",
    "groups:history",
    "groups:write",
    "im:read",
    "im:history",
    "im:write",
    "mpim:read",
    "mpim:history",
    "mpim:write",
    "chat:write",
    "users:read",
    "users:write",
];

/// Configuration for the OAuth v2 flow.
#[derive(Debug, Clone)]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
    pub user_scopes: Vec<String>,
    pub port: u16,
}

/// Raw JSON response from Slack's oauth.v2.access endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct TokenResponse {
    pub ok: bool,
    #[serde(default)]
    pub error: String,
    #[serde(default)]
    pub access_token: String,
    #[serde(default)]
    pub token_type: String,
    #[serde(default)]
    pub scope: String,
    #[serde(default)]
    pub bot_user_id: String,
    #[serde(default)]
    pub app_id: String,
    #[serde(default)]
    pub team: TeamInfo,
    #[serde(default)]
    pub authed_user: AuthedUser,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct TeamInfo {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub id: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AuthedUser {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub scope: String,
    #[serde(default)]
    pub access_token: String,
    #[serde(default)]
    pub token_type: String,
}

#[derive(Debug)]
pub enum OAuthError {
    StateFailed(String),
    ServerFailed(String),
    ExchangeFailed(String),
    SlackError(String),
    Denied(String),
    Timeout,
    StateMismatch,
    MissingCode,
}

impl std::fmt::Display for OAuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OAuthError::StateFailed(msg) => write!(f, "failed to generate state: {}", msg),
            OAuthError::ServerFailed(msg) => write!(f, "callback server error: {}", msg),
            OAuthError::ExchangeFailed(msg) => write!(f, "token exchange failed: {}", msg),
            OAuthError::SlackError(msg) => write!(f, "Slack OAuth error: {}", msg),
            OAuthError::Denied(msg) => write!(f, "authorization denied: {}", msg),
            OAuthError::Timeout => write!(f, "timed out waiting for authorization (5 minutes)"),
            OAuthError::StateMismatch => write!(f, "state mismatch: possible CSRF attack"),
            OAuthError::MissingCode => write!(f, "no authorization code in callback"),
        }
    }
}

impl std::error::Error for OAuthError {}

/// Generate a cryptographically random state parameter for CSRF protection.
pub fn generate_state() -> Result<String, OAuthError> {
    let mut buf = [0u8; 16];
    rand::rng().fill_bytes(&mut buf);
    Ok(hex::encode(&buf))
}

mod hex {
    const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";

    pub fn encode(bytes: &[u8]) -> String {
        let mut s = String::with_capacity(bytes.len() * 2);
        for &b in bytes {
            s.push(HEX_CHARS[(b >> 4) as usize] as char);
            s.push(HEX_CHARS[(b & 0x0f) as usize] as char);
        }
        s
    }
}

/// Build the Slack OAuth v2 authorization URL.
pub fn build_authorize_url(cfg: &OAuthConfig, state: &str) -> String {
    let mut params = url::form_urlencoded::Serializer::new(String::new());
    params.append_pair("client_id", &cfg.client_id);
    params.append_pair("state", state);
    params.append_pair("redirect_uri", &cfg.redirect_uri);

    if !cfg.scopes.is_empty() {
        params.append_pair("scope", &cfg.scopes.join(","));
    }
    if !cfg.user_scopes.is_empty() {
        params.append_pair("user_scope", &cfg.user_scopes.join(","));
    }

    format!("{}?{}", SLACK_AUTHORIZE_URL, params.finish())
}

/// Exchange an authorization code for access tokens via oauth.v2.access.
pub async fn exchange_code(cfg: &OAuthConfig, code: &str) -> Result<TokenResponse, OAuthError> {
    let client = reqwest::Client::new();
    let params = [
        ("client_id", cfg.client_id.as_str()),
        ("client_secret", cfg.client_secret.as_str()),
        ("code", code),
        ("redirect_uri", cfg.redirect_uri.as_str()),
    ];

    let resp = client
        .post(SLACK_TOKEN_URL)
        .form(&params)
        .send()
        .await
        .map_err(|e| OAuthError::ExchangeFailed(e.to_string()))?;

    let token_resp: TokenResponse = resp
        .json()
        .await
        .map_err(|e| OAuthError::ExchangeFailed(e.to_string()))?;

    if !token_resp.ok {
        return Err(OAuthError::SlackError(token_resp.error));
    }

    Ok(token_resp)
}

/// Convert a raw TokenResponse into StoredTokens for persistence.
pub fn parse_token_response(resp: &TokenResponse) -> StoredTokens {
    let mut tokens = StoredTokens {
        team_id: resp.team.id.clone(),
        team_name: resp.team.name.clone(),
        saved_at: Some(Utc::now()),
        ..Default::default()
    };

    if !resp.access_token.is_empty() {
        tokens.bot_token = resp.access_token.clone();
        tokens.bot_user_id = resp.bot_user_id.clone();
        tokens.bot_scope = resp.scope.clone();
    }

    if !resp.authed_user.access_token.is_empty() {
        tokens.user_token = resp.authed_user.access_token.clone();
        tokens.user_id = resp.authed_user.id.clone();
        tokens.user_scope = resp.authed_user.scope.clone();
    }

    tokens
}

/// Generate a self-signed TLS certificate for localhost.
fn generate_self_signed_cert() -> Result<(Vec<u8>, Vec<u8>), OAuthError> {
    let subject_alt_names = vec!["localhost".to_string(), "127.0.0.1".to_string()];
    let certified_key = rcgen::generate_simple_self_signed(subject_alt_names)
        .map_err(|e| OAuthError::ServerFailed(format!("TLS cert generation failed: {}", e)))?;
    let cert_pem = certified_key
        .cert
        .pem()
        .into_bytes();
    let key_pem = certified_key
        .key_pair
        .serialize_pem()
        .into_bytes();
    Ok((cert_pem, key_pem))
}

/// Build a TLS acceptor from PEM-encoded cert and key.
fn build_tls_acceptor(cert_pem: &[u8], key_pem: &[u8]) -> Result<TlsAcceptor, OAuthError> {
    let certs: Vec<_> = rustls_pemfile::certs(&mut &*cert_pem)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| OAuthError::ServerFailed(format!("cert parse error: {}", e)))?;

    let key = rustls_pemfile::private_key(&mut &*key_pem)
        .map_err(|e| OAuthError::ServerFailed(format!("key parse error: {}", e)))?
        .ok_or_else(|| OAuthError::ServerFailed("no private key found".to_string()))?;

    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| OAuthError::ServerFailed(format!("TLS config error: {}", e)))?;

    Ok(TlsAcceptor::from(Arc::new(config)))
}

/// Run the complete OAuth v2 authorization flow:
/// 1. Start a local HTTPS callback server with a self-signed cert
/// 2. Open the browser to https://localhost first so the user accepts the cert
/// 3. That page auto-redirects to Slack's OAuth authorize URL
/// 4. Slack redirects back to https://localhost with the auth code (cert already trusted)
/// 5. Exchange the code for access tokens
pub async fn run_oauth_flow(cfg: &mut OAuthConfig) -> Result<TokenResponse, OAuthError> {
    let state = generate_state()?;

    if cfg.user_scopes.is_empty() && cfg.scopes.is_empty() {
        cfg.user_scopes = DEFAULT_USER_SCOPES.iter().map(|s| s.to_string()).collect();
    }

    // Generate self-signed TLS cert for localhost
    let (cert_pem, key_pem) = generate_self_signed_cert()?;
    let tls_acceptor = build_tls_acceptor(&cert_pem, &key_pem)?;

    // Bind callback server
    let addr: SocketAddr = format!("127.0.0.1:{}", cfg.port)
        .parse()
        .map_err(|e: std::net::AddrParseError| OAuthError::ServerFailed(e.to_string()))?;

    let listener = TcpListener::bind(addr)
        .await
        .map_err(|e| OAuthError::ServerFailed(e.to_string()))?;

    let port = listener
        .local_addr()
        .map_err(|e| OAuthError::ServerFailed(e.to_string()))?
        .port();

    if cfg.redirect_uri.is_empty() {
        cfg.redirect_uri = format!("https://localhost:{}/callback", port);
    }

    let auth_url = build_authorize_url(cfg, &state);

    // Step 1: Open browser to our local HTTPS server.
    // The user will see a certificate warning and click through it.
    // Our server responds with a page that auto-redirects to Slack's OAuth URL.
    let local_url = format!("https://localhost:{}/start", port);

    println!("\nOpening browser for Slack authorization...");
    println!("\nYour browser will show a certificate warning for localhost.");
    println!("This is expected — click \"Advanced\" then \"Proceed to localhost\" to continue.\n");
    println!(
        "If the browser doesn't open, visit:\n  {}\n",
        local_url
    );

    let _ = open::that(&local_url);

    // Step 2: Serve the redirect page (first connection — user accepting the cert)
    let redirect_html = format!(
        "<html><head><meta http-equiv=\"refresh\" content=\"0;url={}\"></head>\
         <body><p>Redirecting to Slack... <a href=\"{}\">Click here</a> if not redirected.</p></body></html>",
        auth_url, auth_url
    );

    // Serve requests in a loop. The browser may make multiple connections that fail
    // TLS (before the user accepts the self-signed cert), plus favicon requests, etc.
    // We keep looping until we get the actual OAuth callback with an auth code.
    let mut showed_waiting = false;
    let code = tokio::select! {
        result = async {
            loop {
                // Accept a TCP connection
                let (tcp_stream, _) = match listener.accept().await {
                    Ok(conn) => conn,
                    Err(_) => continue,
                };

                // Attempt TLS handshake — this will fail until the user accepts the cert
                let tls_stream = match tls_acceptor.accept(tcp_stream).await {
                    Ok(s) => s,
                    Err(_) => continue, // browser preflight, favicon, cert not yet accepted — retry
                };

                // TLS succeeded — serve an HTTP request
                let io = hyper_util::rt::TokioIo::new(tls_stream);
                let (uri_tx, uri_rx) = oneshot::channel::<String>();
                let uri_tx = std::sync::Mutex::new(Some(uri_tx));

                // Decide which page to serve: redirect page or success page
                let body = if !showed_waiting {
                    redirect_html.clone()
                } else {
                    "<html><body><h2>Authorization successful!</h2>\
                     <p>You can close this tab and return to the terminal.</p></body></html>"
                        .to_string()
                };

                let body_clone = body.clone();
                let service = hyper::service::service_fn(move |req: hyper::Request<hyper::body::Incoming>| {
                    let uri = req.uri().to_string();
                    if let Some(tx) = uri_tx.lock().unwrap().take() {
                        let _ = tx.send(uri);
                    }
                    let body = body_clone.clone();
                    async move {
                        Ok::<_, hyper::Error>(
                            hyper::Response::builder()
                                .status(200)
                                .header("Content-Type", "text/html")
                                .body(full_body(&body))
                                .unwrap(),
                        )
                    }
                });

                let _ = hyper::server::conn::http1::Builder::new()
                    .keep_alive(false)
                    .serve_connection(io, service)
                    .await;

                let uri = match uri_rx.await {
                    Ok(u) => u,
                    Err(_) => continue,
                };

                // First successful TLS request serves the redirect page
                if !showed_waiting {
                    showed_waiting = true;
                    println!("Waiting for Slack authorization...");
                    continue;
                }

                // Subsequent requests: check if this is the OAuth callback
                match handle_callback(&uri, &state) {
                    Ok(code) => return Ok(code),
                    Err(OAuthError::Denied(msg)) => return Err(OAuthError::Denied(msg)),
                    Err(_) => continue, // favicon, state mismatch, etc. — keep waiting
                }
            }
        } => result,
        _ = tokio::time::sleep(std::time::Duration::from_secs(300)) => {
            Err(OAuthError::Timeout)
        }
    }?;

    println!("Exchanging authorization code for tokens...");
    exchange_code(cfg, &code).await
}

/// Parse a callback URI and extract the authorization code.
fn handle_callback(uri_path: &str, expected_state: &str) -> Result<String, OAuthError> {
    let parsed = url::Url::parse(&format!("http://localhost{}", uri_path))
        .map_err(|e| OAuthError::ServerFailed(e.to_string()))?;

    let params: std::collections::HashMap<String, String> = parsed.query_pairs().into_owned().collect();

    // Check state
    let received_state = params.get("state").map(|s| s.as_str()).unwrap_or("");
    if received_state != expected_state {
        return Err(OAuthError::StateMismatch);
    }

    // Check for error
    if let Some(err) = params.get("error") {
        return Err(OAuthError::Denied(err.clone()));
    }

    // Extract code
    let code = params
        .get("code")
        .filter(|c| !c.is_empty())
        .ok_or(OAuthError::MissingCode)?;

    Ok(code.clone())
}

fn full_body(s: &str) -> http_body_util::Full<hyper::body::Bytes> {
    http_body_util::Full::new(hyper::body::Bytes::from(s.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_state_length() {
        let state = generate_state().unwrap();
        assert_eq!(state.len(), 32); // 16 bytes = 32 hex chars
    }

    #[test]
    fn test_generate_state_uniqueness() {
        let s1 = generate_state().unwrap();
        let s2 = generate_state().unwrap();
        assert_ne!(s1, s2);
    }

    #[test]
    fn test_generate_state_hex_only() {
        let state = generate_state().unwrap();
        for c in state.chars() {
            assert!(
                c.is_ascii_hexdigit() && !c.is_ascii_uppercase(),
                "unexpected char: {}",
                c
            );
        }
    }

    #[test]
    fn test_build_authorize_url_basic() {
        let cfg = OAuthConfig {
            client_id: "123.456".to_string(),
            client_secret: String::new(),
            redirect_uri: "http://127.0.0.1:8080/callback".to_string(),
            scopes: vec![],
            user_scopes: vec!["channels:read".to_string(), "chat:write".to_string()],
            port: 0,
        };

        let url = build_authorize_url(&cfg, "teststate123");
        assert!(url.starts_with(SLACK_AUTHORIZE_URL));
        assert!(url.contains("client_id=123.456"));
        assert!(url.contains("state=teststate123"));
        assert!(url.contains("redirect_uri="));
        assert!(url.contains("user_scope=channels"));
    }

    #[test]
    fn test_build_authorize_url_no_bot_scope_when_empty() {
        let cfg = OAuthConfig {
            client_id: "123.456".to_string(),
            client_secret: String::new(),
            redirect_uri: "http://127.0.0.1:8080/callback".to_string(),
            scopes: vec![],
            user_scopes: vec!["channels:read".to_string()],
            port: 0,
        };

        let url = build_authorize_url(&cfg, "state");
        // "scope=" should not appear (only "user_scope=")
        // We check that scope= doesn't appear without "user_" prefix
        let without_user = url.replace("user_scope", "USR");
        assert!(!without_user.contains("scope="));
    }

    #[test]
    fn test_build_authorize_url_with_bot_scopes() {
        let cfg = OAuthConfig {
            client_id: "123.456".to_string(),
            client_secret: String::new(),
            redirect_uri: "http://127.0.0.1:8080/callback".to_string(),
            scopes: vec!["chat:write".to_string()],
            user_scopes: vec!["channels:read".to_string()],
            port: 0,
        };

        let url = build_authorize_url(&cfg, "state");
        assert!(url.contains("scope=chat"));
        assert!(url.contains("user_scope=channels"));
    }

    #[test]
    fn test_parse_token_response_full() {
        let resp = TokenResponse {
            ok: true,
            error: String::new(),
            access_token: "xoxb-bot-token".to_string(),
            token_type: "bot".to_string(),
            scope: "channels:read,chat:write".to_string(),
            bot_user_id: "U123BOT".to_string(),
            app_id: "A123".to_string(),
            team: TeamInfo {
                name: "Test Team".to_string(),
                id: "T123".to_string(),
            },
            authed_user: AuthedUser {
                id: "U456USER".to_string(),
                scope: "channels:read,channels:history".to_string(),
                access_token: "xoxp-user-token".to_string(),
                token_type: "user".to_string(),
            },
        };

        let stored = parse_token_response(&resp);
        assert_eq!(stored.bot_token, "xoxb-bot-token");
        assert_eq!(stored.user_token, "xoxp-user-token");
        assert_eq!(stored.team_id, "T123");
        assert_eq!(stored.team_name, "Test Team");
        assert_eq!(stored.bot_user_id, "U123BOT");
        assert_eq!(stored.user_id, "U456USER");
        assert_eq!(stored.bot_scope, "channels:read,chat:write");
        assert_eq!(stored.user_scope, "channels:read,channels:history");
        assert!(stored.saved_at.is_some());
    }

    #[test]
    fn test_parse_token_response_user_only() {
        let resp = TokenResponse {
            ok: true,
            error: String::new(),
            access_token: String::new(),
            token_type: String::new(),
            scope: String::new(),
            bot_user_id: String::new(),
            app_id: "A123".to_string(),
            team: TeamInfo {
                name: "My Team".to_string(),
                id: "T789".to_string(),
            },
            authed_user: AuthedUser {
                id: "U999".to_string(),
                scope: "channels:read".to_string(),
                access_token: "xoxp-only-user".to_string(),
                token_type: "user".to_string(),
            },
        };

        let stored = parse_token_response(&resp);
        assert!(stored.bot_token.is_empty());
        assert_eq!(stored.user_token, "xoxp-only-user");
        assert_eq!(stored.team_name, "My Team");
    }

    #[test]
    fn test_parse_token_response_bot_only() {
        let resp = TokenResponse {
            ok: true,
            error: String::new(),
            access_token: "xoxb-bot-only".to_string(),
            token_type: "bot".to_string(),
            scope: "chat:write".to_string(),
            bot_user_id: "UBOT".to_string(),
            app_id: "A123".to_string(),
            team: TeamInfo {
                name: "Bot Team".to_string(),
                id: "TBOT".to_string(),
            },
            authed_user: AuthedUser {
                id: "UHUMAN".to_string(),
                scope: String::new(),
                access_token: String::new(),
                token_type: String::new(),
            },
        };

        let stored = parse_token_response(&resp);
        assert_eq!(stored.bot_token, "xoxb-bot-only");
        assert!(stored.user_token.is_empty());
        assert_eq!(stored.bot_user_id, "UBOT");
    }

    #[test]
    fn test_default_user_scopes_not_empty() {
        assert!(!DEFAULT_USER_SCOPES.is_empty());
    }

    #[test]
    fn test_default_user_scopes_has_required() {
        let required = ["channels:read", "channels:history", "chat:write", "users:read"];
        for scope in &required {
            assert!(
                DEFAULT_USER_SCOPES.contains(scope),
                "missing required scope: {}",
                scope
            );
        }
    }

    #[test]
    fn test_token_response_deserialization() {
        let json = r#"{
            "ok": true,
            "access_token": "xoxb-bot-token-value",
            "token_type": "bot",
            "scope": "channels:read,chat:write",
            "bot_user_id": "U123BOT",
            "app_id": "A123APP",
            "team": {"name": "Test Team", "id": "T123TEAM"},
            "authed_user": {
                "id": "U456USER",
                "scope": "channels:read,channels:history,chat:write",
                "access_token": "xoxp-user-token-value",
                "token_type": "user"
            }
        }"#;

        let resp: TokenResponse = serde_json::from_str(json).unwrap();
        assert!(resp.ok);
        assert_eq!(resp.access_token, "xoxb-bot-token-value");
        assert_eq!(resp.team.id, "T123TEAM");
        assert_eq!(resp.authed_user.access_token, "xoxp-user-token-value");
    }

    #[test]
    fn test_token_response_error_deserialization() {
        let json = r#"{"ok": false, "error": "invalid_code"}"#;
        let resp: TokenResponse = serde_json::from_str(json).unwrap();
        assert!(!resp.ok);
        assert_eq!(resp.error, "invalid_code");
    }

    #[test]
    fn test_hex_encode() {
        assert_eq!(hex::encode(&[0x00]), "00");
        assert_eq!(hex::encode(&[0xff]), "ff");
        assert_eq!(hex::encode(&[0xde, 0xad, 0xbe, 0xef]), "deadbeef");
        assert_eq!(hex::encode(&[]), "");
    }

    #[test]
    fn test_callback_handler_success() {
        let result = handle_callback("/callback?code=test_code_123&state=mystate", "mystate");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test_code_123");
    }

    #[test]
    fn test_callback_handler_state_mismatch() {
        let result = handle_callback("/callback?code=test&state=wrong", "expected");
        assert!(matches!(result, Err(OAuthError::StateMismatch)));
    }

    #[test]
    fn test_callback_handler_denied() {
        let result = handle_callback("/callback?error=access_denied&state=mystate", "mystate");
        assert!(matches!(result, Err(OAuthError::Denied(_))));
    }

    #[test]
    fn test_callback_handler_missing_code() {
        let result = handle_callback("/callback?state=mystate", "mystate");
        assert!(matches!(result, Err(OAuthError::MissingCode)));
    }
}
