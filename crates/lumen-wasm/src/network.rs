//! Network abstractions for the WASM environment.
//!
//! In the browser, we don't have TCP sockets. Instead we use:
//! - Fetch API for HTTP requests (initial checkpoint fetching only)
//! - WebSocket API for persistent connections (libp2p transport)
//!
//! IMPORTANT: These network primitives are used ONLY for:
//! 1. Fetching initial checkpoint data from beacon APIs
//! 2. Establishing WebSocket connections for libp2p
//!
//! All data received over any transport is cryptographically verified
//! by lumen-core before being trusted. The network layer is untrusted.

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestInit, RequestMode, Response};

/// Errors from network operations.
#[derive(Debug)]
pub enum NetworkError {
    /// Failed to construct the HTTP request.
    RequestFailed(String),
    /// HTTP request returned a non-200 status.
    HttpError(u16, String),
    /// Failed to read the response body.
    BodyReadFailed(String),
    /// WebSocket connection failed.
    WebSocketFailed(String),
}

impl std::fmt::Display for NetworkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NetworkError::RequestFailed(e) => write!(f, "Request failed: {}", e),
            NetworkError::HttpError(status, msg) => {
                write!(f, "HTTP error {}: {}", status, msg)
            }
            NetworkError::BodyReadFailed(e) => write!(f, "Body read failed: {}", e),
            NetworkError::WebSocketFailed(e) => write!(f, "WebSocket failed: {}", e),
        }
    }
}

/// Fetch a URL and return bytes using the browser Fetch API.
///
/// This is used ONLY for initial checkpoint fetching from multiple sources.
/// After P2P is established, this is no longer used.
/// The response data is always verified cryptographically — this function
/// does not trust the source at all.
pub async fn fetch_bytes(url: &str) -> Result<Vec<u8>, NetworkError> {
    let mut opts = RequestInit::new();
    opts.method("GET");
    opts.mode(RequestMode::Cors);

    let request = Request::new_with_str_and_init(url, &opts)
        .map_err(|e| NetworkError::RequestFailed(format!("{:?}", e)))?;

    let window = web_sys::window()
        .ok_or_else(|| NetworkError::RequestFailed("No window object".to_string()))?;

    let resp_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| NetworkError::RequestFailed(format!("{:?}", e)))?;

    let resp: Response = resp_value
        .dyn_into()
        .map_err(|_| NetworkError::RequestFailed("Response is not a Response object".to_string()))?;

    let status = resp.status();
    if status != 200 {
        return Err(NetworkError::HttpError(
            status,
            resp.status_text(),
        ));
    }

    let array_buffer = JsFuture::from(
        resp.array_buffer()
            .map_err(|e| NetworkError::BodyReadFailed(format!("{:?}", e)))?,
    )
    .await
    .map_err(|e| NetworkError::BodyReadFailed(format!("{:?}", e)))?;

    let uint8_array = js_sys::Uint8Array::new(&array_buffer);
    Ok(uint8_array.to_vec())
}

/// Fetch a URL and return the response as a string.
///
/// Same trust model as fetch_bytes — the response is untrusted.
pub async fn fetch_text(url: &str) -> Result<String, NetworkError> {
    let mut opts = RequestInit::new();
    opts.method("GET");
    opts.mode(RequestMode::Cors);

    let request = Request::new_with_str_and_init(url, &opts)
        .map_err(|e| NetworkError::RequestFailed(format!("{:?}", e)))?;

    let window = web_sys::window()
        .ok_or_else(|| NetworkError::RequestFailed("No window object".to_string()))?;

    let resp_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| NetworkError::RequestFailed(format!("{:?}", e)))?;

    let resp: Response = resp_value
        .dyn_into()
        .map_err(|_| NetworkError::RequestFailed("Response is not a Response object".to_string()))?;

    let status = resp.status();
    if status != 200 {
        return Err(NetworkError::HttpError(
            status,
            resp.status_text(),
        ));
    }

    let text = JsFuture::from(
        resp.text()
            .map_err(|e| NetworkError::BodyReadFailed(format!("{:?}", e)))?,
    )
    .await
    .map_err(|e| NetworkError::BodyReadFailed(format!("{:?}", e)))?;

    text.as_string()
        .ok_or_else(|| NetworkError::BodyReadFailed("Response text is not a string".to_string()))
}

/// Post JSON data and return the response as a string.
///
/// Used for JSON-RPC requests to fallback RPC endpoints.
/// The response is NEVER trusted for correctness — all data is verified
/// against our cryptographic chain state.
pub async fn post_json(url: &str, body: &str) -> Result<String, NetworkError> {
    let mut opts = RequestInit::new();
    opts.method("POST");
    opts.mode(RequestMode::Cors);
    opts.body(Some(&JsValue::from_str(body)));

    let headers = web_sys::Headers::new()
        .map_err(|e| NetworkError::RequestFailed(format!("{:?}", e)))?;
    headers
        .set("Content-Type", "application/json")
        .map_err(|e| NetworkError::RequestFailed(format!("{:?}", e)))?;
    opts.headers(&headers);

    let request = Request::new_with_str_and_init(url, &opts)
        .map_err(|e| NetworkError::RequestFailed(format!("{:?}", e)))?;

    let window = web_sys::window()
        .ok_or_else(|| NetworkError::RequestFailed("No window object".to_string()))?;

    let resp_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| NetworkError::RequestFailed(format!("{:?}", e)))?;

    let resp: Response = resp_value
        .dyn_into()
        .map_err(|_| NetworkError::RequestFailed("Response is not a Response object".to_string()))?;

    let text = JsFuture::from(
        resp.text()
            .map_err(|e| NetworkError::BodyReadFailed(format!("{:?}", e)))?,
    )
    .await
    .map_err(|e| NetworkError::BodyReadFailed(format!("{:?}", e)))?;

    text.as_string()
        .ok_or_else(|| NetworkError::BodyReadFailed("Response text is not a string".to_string()))
}
