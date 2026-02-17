//! EIP-1193 provider implementation for WASM.
//!
//! This module handles the translation between EIP-1193 JSON-RPC requests
//! and the internal Lumen verification pipeline.
//!
//! For each supported method, the flow is:
//! 1. Parse the request parameters
//! 2. Fetch proof data from any available source (P2P or fallback RPC)
//! 3. Verify the proof cryptographically against our verified state root
//! 4. Return the verified result
//!
//! NEVER return unverified data. If verification fails, return an error.

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

/// EIP-1193 JSON-RPC request.
#[derive(Serialize, Deserialize, Debug)]
pub struct JsonRpcRequest {
    pub method: String,
    pub params: Vec<serde_json::Value>,
    #[serde(default)]
    pub id: serde_json::Value,
}

/// EIP-1193 JSON-RPC response.
#[derive(Serialize, Deserialize, Debug)]
pub struct JsonRpcResponse {
    pub id: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC error object.
#[derive(Serialize, Deserialize, Debug)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// Methods that Lumen fully supports with cryptographic verification.
pub const VERIFIED_METHODS: &[&str] = &[
    "eth_blockNumber",
    "eth_getBalance",
    "eth_getCode",
    "eth_getStorageAt",
    "eth_getTransactionCount",
    "eth_sendRawTransaction",
    "eth_chainId",
    "net_version",
];

/// Methods that require trusted execution (documented clearly).
pub const TRUSTED_METHODS: &[&str] = &[
    "eth_call",
    "eth_estimateGas",
];

/// Methods that are purely informational.
pub const INFO_METHODS: &[&str] = &[
    "eth_chainId",
    "net_version",
    "web3_clientVersion",
];

/// Check if a method is supported.
pub fn is_method_supported(method: &str) -> bool {
    VERIFIED_METHODS.contains(&method)
        || TRUSTED_METHODS.contains(&method)
        || INFO_METHODS.contains(&method)
}

/// Check if a method returns verified data.
pub fn is_method_verified(method: &str) -> bool {
    VERIFIED_METHODS.contains(&method)
}

/// Create an error response for unsupported methods.
pub fn method_not_supported(id: serde_json::Value, method: &str) -> JsonRpcResponse {
    JsonRpcResponse {
        id,
        result: None,
        error: Some(JsonRpcError {
            code: -32601,
            message: format!("Method {} is not supported by Lumen", method),
            data: None,
        }),
    }
}

/// Create an error response for verification failures.
pub fn verification_failed(id: serde_json::Value, reason: &str) -> JsonRpcResponse {
    JsonRpcResponse {
        id,
        result: None,
        error: Some(JsonRpcError {
            code: -32000,
            message: format!(
                "Lumen verification failed: {}. Data was not returned because it could not be verified.",
                reason
            ),
            data: None,
        }),
    }
}

/// Create a success response.
pub fn success_response(id: serde_json::Value, result: serde_json::Value) -> JsonRpcResponse {
    JsonRpcResponse {
        id,
        result: Some(result),
        error: None,
    }
}

/// Handle informational methods that don't require network or verification.
pub fn handle_info_method(request: &JsonRpcRequest) -> Option<JsonRpcResponse> {
    match request.method.as_str() {
        "eth_chainId" => Some(success_response(
            request.id.clone(),
            serde_json::Value::String("0x1".to_string()), // Mainnet
        )),
        "net_version" => Some(success_response(
            request.id.clone(),
            serde_json::Value::String("1".to_string()),
        )),
        "web3_clientVersion" => Some(success_response(
            request.id.clone(),
            serde_json::Value::String(format!("Lumen/{}", env!("CARGO_PKG_VERSION"))),
        )),
        _ => None,
    }
}
