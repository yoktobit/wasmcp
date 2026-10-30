//! Transport error types
//!
//! Provides a unified error type for all transport-layer operations,
//! eliminating the need for repeated error handling boilerplate.

use crate::bindings::wasmcp::mcp_v20260728::mcp::ErrorCode;
use crate::bindings::wasmcp::mcp_v20260728::server_io::IoError;

/// Unified transport error type
#[derive(Debug)]
pub enum TransportError {
    /// Validation error (origin, headers, protocol version, etc.)
    Validation(String),

    /// OAuth authentication error (invalid/missing token)
    /// Includes WWW-Authenticate header value
    Unauthorized {
        message: String,
        www_authenticate: Option<String>,
    },

    /// OAuth authorization error (valid token, insufficient permissions)
    ///
    /// NOTE(architecture): Transport does NOT perform authorization
    /// - Transport layer: Authentication only (validates JWT is valid)
    /// - Middleware layer: Authorization (per-tool/resource permission checks)
    /// - Each middleware component enforces its own authorization requirements
    /// - This error variant exists for middleware to return 403 responses via transport
    /// - MCP spec requires 403 Forbidden for authorization failures (mpc-auth.md:286)
    #[allow(dead_code)]
    Forbidden(String),

    /// I/O error from server-io operations
    Io(IoError),

    /// Protocol-level error (MCP protocol violations)
    Protocol(String),

    /// Error with an explicit JSON-RPC code (e.g. -32020 header mismatch, -32022 unsupported version)
    Coded { code: i32, message: String },

    /// Internal error (should not happen in normal operation)
    Internal(String),
}

impl TransportError {
    /// Create a validation error
    pub fn validation(msg: impl Into<String>) -> Self {
        Self::Validation(msg.into())
    }

    /// Create an unauthorized error (401) with optional WWW-Authenticate header
    /// TODO: Currently unused - use unauthorized_with_challenge instead
    /// Kept for completeness when authorization is implemented
    #[allow(dead_code)]
    pub fn unauthorized(msg: impl Into<String>) -> Self {
        Self::Unauthorized {
            message: msg.into(),
            www_authenticate: None,
        }
    }

    /// Create an unauthorized error with WWW-Authenticate header
    pub fn unauthorized_with_challenge(msg: impl Into<String>, www_authenticate: String) -> Self {
        Self::Unauthorized {
            message: msg.into(),
            www_authenticate: Some(www_authenticate),
        }
    }

    /// Create a forbidden error (403)
    ///
    /// Used when middleware authorization checks fail. Transport validates JWT (authentication),
    /// but middleware enforces permissions (authorization) per-tool/resource.
    #[allow(dead_code)]
    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self::Forbidden(msg.into())
    }

    /// Create a protocol error
    pub fn protocol(msg: impl Into<String>) -> Self {
        Self::Protocol(msg.into())
    }

    /// Create a 400 error with an explicit JSON-RPC code
    pub fn coded(code: i32, msg: impl Into<String>) -> Self {
        Self::Coded {
            code,
            message: msg.into(),
        }
    }

    /// Create an internal error
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }

    /// Get HTTP status code for this error
    pub fn http_status_code(&self) -> u16 {
        match self {
            Self::Validation(_) => 400,
            Self::Unauthorized { .. } => 401,
            Self::Forbidden(_) => 403,
            Self::Protocol(_) => 400,
            Self::Coded { .. } => 400,
            Self::Io(_) => 500,
            Self::Internal(_) => 500,
        }
    }

    /// JSON-RPC error code for the response body
    pub fn json_rpc_code(&self) -> i32 {
        match self {
            Self::Coded { code, .. } => *code,
            _ => -32700,
        }
    }

    /// Get WWW-Authenticate header value if present
    pub fn www_authenticate_header(&self) -> Option<&str> {
        match self {
            Self::Unauthorized {
                www_authenticate: Some(header),
                ..
            } => Some(header),
            _ => None,
        }
    }

    /// Get error message
    pub fn message(&self) -> String {
        match self {
            Self::Validation(msg) => msg.clone(),
            Self::Unauthorized { message, .. } => message.clone(),
            Self::Forbidden(msg) => msg.clone(),
            Self::Protocol(msg) => msg.clone(),
            Self::Coded { message, .. } => message.clone(),
            Self::Io(e) => format!("I/O error: {:?}", e),
            Self::Internal(msg) => msg.clone(),
        }
    }
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message())
    }
}

impl std::error::Error for TransportError {}

// Automatic conversions from other error types

impl From<IoError> for TransportError {
    fn from(e: IoError) -> Self {
        Self::Io(e)
    }
}

impl From<ErrorCode> for TransportError {
    fn from(e: ErrorCode) -> Self {
        Self::Protocol(format!("{:?}", e))
    }
}

impl From<String> for TransportError {
    fn from(s: String) -> Self {
        Self::Internal(s)
    }
}

impl From<&str> for TransportError {
    fn from(s: &str) -> Self {
        Self::Internal(s.to_string())
    }
}
