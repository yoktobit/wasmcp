//! Protocol version and message context utilities

use crate::bindings::wasi::io::streams::OutputStream;
use crate::bindings::wasmcp::mcp_v20260728::mcp::ProtocolVersion;
use crate::bindings::wasmcp::mcp_v20260728::server_handler::MessageContext;
use crate::bindings::wasmcp::mcp_v20260728::server_io::{MessageFrame, RequestMeta};

/// The only protocol version this transport speaks
pub const PROTOCOL_VERSION: &str = "2026-07-28";

/// Protocol versions advertised by `server/discover`
pub const SUPPORTED_VERSIONS: &[ProtocolVersion] = &[ProtocolVersion::V20260728];

/// Create MessageContext from the per-request `_meta` values
pub fn create_message_context<'a>(
    client_stream: Option<&'a OutputStream>,
    meta: &RequestMeta,
    identity: Option<&crate::bindings::wasmcp::mcp_v20260728::mcp::Identity>,
    frame: &MessageFrame,
    http_context: Option<crate::bindings::wasmcp::mcp_v20260728::server_auth::HttpContext>,
) -> MessageContext<'a> {
    MessageContext {
        client_stream,
        protocol_version: PROTOCOL_VERSION.to_string(),
        client_capabilities: meta.client_capabilities.clone(),
        client_info: meta.client_info.clone(),
        log_level: meta.log_level,
        identity: identity.cloned(),
        frame: frame.clone(),
        http_context,
    }
}

/// Request metadata for internal calls that have no client request (capability probes)
pub fn empty_meta() -> RequestMeta {
    RequestMeta {
        protocol_version: None,
        client_capabilities: None,
        client_info: None,
        log_level: None,
    }
}
