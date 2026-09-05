//! Shared OAuth 2.0 helpers for Google integrations.
//!
//! Trace uses the installed-app authorization-code flow with PKCE. Desktop
//! applications cannot keep a client secret confidential, so no client secret
//! is embedded in the binary or sent to Google's token endpoint.

use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use rand::RngCore;
use sha2::{Digest, Sha256};
use url::Url;

pub const GOOGLE_TOKEN_URI: &str = "https://oauth2.googleapis.com/token";
const GOOGLE_AUTH_URI: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const MAX_REQUEST_LINE_BYTES: usize = 8 * 1024;

#[derive(Debug, serde::Deserialize)]
pub struct TokenResp {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    pub expires_in: u64,
}

#[derive(Debug, Clone)]
pub struct OAuthFlow {
    pub auth_url: String,
    pub state: String,
    pub code_verifier: String,
}

const DEFAULT_SUCCESS_HTML: &str = "<!doctype html><html lang='en'><head><meta charset='utf-8'><meta name='viewport' content='width=device-width,initial-scale=1'><title>Trace connected</title></head><body><main><h1>Connected to Trace</h1><p>You can close this tab and return to the app.</p></main></body></html>";

pub fn google_client_id() -> Result<String, String> {
    std::env::var("TRACE_GOOGLE_CLIENT_ID")
        .ok()
        .or_else(|| option_env!("TRACE_GOOGLE_CLIENT_ID").map(str::to_owned))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            "Google integration is not configured. Set TRACE_GOOGLE_CLIENT_ID before building or launching Trace."
                .to_string()
        })
}

pub fn google_oauth_flow(redirect_uri: &str, scope: &str) -> Result<OAuthFlow, String> {
    let client_id = google_client_id()?;
    let state = random_urlsafe(32);
    let code_verifier = random_urlsafe(64);
    let code_challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(code_verifier.as_bytes()));

    let mut auth_url =
        Url::parse(GOOGLE_AUTH_URI).map_err(|error| format!("invalid OAuth endpoint: {error}"))?;
    auth_url
        .query_pairs_mut()
        .append_pair("client_id", &client_id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", scope)
        .append_pair("access_type", "offline")
        .append_pair("prompt", "consent")
        .append_pair("state", &state)
        .append_pair("code_challenge", &code_challenge)
        .append_pair("code_challenge_method", "S256");

    Ok(OAuthFlow {
        auth_url: auth_url.to_string(),
        state,
        code_verifier,
    })
}

fn random_urlsafe(byte_count: usize) -> String {
    let mut bytes = vec![0_u8; byte_count];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Wait for one loopback OAuth callback and return its authorization code.
pub fn wait_for_oauth_redirect(
    listener: TcpListener,
    expected_state: &str,
    success_html: Option<&str>,
) -> Result<String, String> {
    let (mut stream, peer) = listener
        .accept()
        .map_err(|error| format!("failed to accept OAuth callback: {error}"))?;
    if !peer.ip().is_loopback() {
        return Err("rejected a non-loopback OAuth callback".to_string());
    }

    let reader = BufReader::new(
        stream
            .try_clone()
            .map_err(|error| format!("failed to clone OAuth callback stream: {error}"))?,
    );
    let mut request_line = String::new();
    reader
        .take((MAX_REQUEST_LINE_BYTES + 1) as u64)
        .read_line(&mut request_line)
        .map_err(|error| format!("failed to read OAuth callback: {error}"))?;

    if request_line.len() > MAX_REQUEST_LINE_BYTES || !request_line.ends_with('\n') {
        write_callback_response(
            &mut stream,
            "400 Bad Request",
            "Invalid OAuth callback.",
            None,
        );
        return Err("OAuth callback request line was invalid or too large".to_string());
    }

    let mut request_parts = request_line.split_whitespace();
    if request_parts.next() != Some("GET") {
        write_callback_response(
            &mut stream,
            "405 Method Not Allowed",
            "Invalid OAuth callback.",
            None,
        );
        return Err("OAuth callback must use GET".to_string());
    }
    let target = request_parts
        .next()
        .ok_or_else(|| "OAuth callback did not include a request target".to_string())?;
    let code = match parse_oauth_callback_target(target, expected_state) {
        Ok(code) => code,
        Err(error) => {
            let message = if error == "Google authorization was not completed" {
                "Google authorization was not completed."
            } else {
                "Invalid OAuth callback."
            };
            write_callback_response(&mut stream, "400 Bad Request", message, None);
            return Err(error);
        }
    };

    write_callback_response(
        &mut stream,
        "200 OK",
        "OAuth connection completed.",
        Some(success_html.unwrap_or(DEFAULT_SUCCESS_HTML)),
    );
    Ok(code)
}

fn parse_oauth_callback_target(target: &str, expected_state: &str) -> Result<String, String> {
    let callback_url = Url::parse(&format!("http://127.0.0.1{target}"))
        .map_err(|_| "OAuth callback URL was invalid".to_string())?;
    let params: HashMap<_, _> = callback_url.query_pairs().collect();

    if params.contains_key("error") {
        return Err("Google authorization was not completed".to_string());
    }

    let returned_state = params
        .get("state")
        .ok_or_else(|| "OAuth callback did not include state".to_string())?;
    if returned_state.as_ref() != expected_state {
        return Err("OAuth callback state did not match".to_string());
    }

    params
        .get("code")
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| "OAuth callback did not include an authorization code".to_string())
}

fn write_callback_response(
    stream: &mut impl Write,
    status: &str,
    message: &str,
    html: Option<&str>,
) {
    let body = html.unwrap_or(message);
    let content_type = if html.is_some() {
        "text/html; charset=utf-8"
    } else {
        "text/plain; charset=utf-8"
    };
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nContent-Security-Policy: default-src 'none'; frame-ancestors 'none'\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

pub async fn exchange_code_for_tokens(
    code: &str,
    redirect_uri: &str,
    client_id: &str,
    code_verifier: &str,
) -> Result<TokenResp, String> {
    let response = reqwest::Client::new()
        .post(GOOGLE_TOKEN_URI)
        .form(&[
            ("code", code),
            ("client_id", client_id),
            ("redirect_uri", redirect_uri),
            ("grant_type", "authorization_code"),
            ("code_verifier", code_verifier),
        ])
        .send()
        .await
        .map_err(|error| format!("token exchange request failed: {error}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "Google token exchange failed with status {}",
            response.status()
        ));
    }

    response
        .json::<TokenResp>()
        .await
        .map_err(|error| format!("failed to parse token response: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_flow_uses_pkce_and_state_without_a_secret() {
        std::env::set_var(
            "TRACE_GOOGLE_CLIENT_ID",
            "example.apps.googleusercontent.com",
        );
        let flow = google_oauth_flow("http://127.0.0.1:1234", "scope-a scope-b").expect("flow");
        let url = Url::parse(&flow.auth_url).expect("url");
        let query: HashMap<_, _> = url.query_pairs().collect();
        assert_eq!(
            query
                .get("code_challenge_method")
                .map(|value| value.as_ref()),
            Some("S256")
        );
        assert_eq!(
            query.get("state").map(|value| value.as_ref()),
            Some(flow.state.as_str())
        );
        assert!(query.contains_key("code_challenge"));
        assert!(!query.contains_key("client_secret"));
        assert!(flow.code_verifier.len() >= 43);
    }

    #[test]
    fn callback_rejects_mismatched_state() {
        assert_eq!(
            parse_oauth_callback_target("/?code=test-code&state=wrong", "expected"),
            Err("OAuth callback state did not match".to_string())
        );
    }
}
