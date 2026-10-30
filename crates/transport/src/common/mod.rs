//! Common transport logic shared between HTTP and stdio implementations

pub mod capability;
pub mod framing;
pub mod protocol;

use crate::bindings::wasi::io::streams::{InputStream, OutputStream};
use crate::bindings::wasmcp::mcp_v20260728::mcp::{
    ClientMessage, ClientNotification, ClientRequest, DiscoverResult, ErrorCode, Implementation,
    NotificationOptions, RequestId, ServerMessage, ServerNotification, ServerResult,
};
use crate::bindings::wasmcp::mcp_v20260728::server_handler::handle;
use crate::bindings::wasmcp::mcp_v20260728::server_io::{self, IoError, ReadLimit, RequestMeta};

// Re-export commonly used items
pub use capability::discover_capabilities;
pub use framing::{
    http_read_limit, http_sse_frame, plain_json_frame, stdio_frame, stdio_read_limit,
};
pub use protocol::{create_message_context, PROTOCOL_VERSION, SUPPORTED_VERSIONS};

// Re-export MessageFrame so it's public
pub use crate::bindings::wasmcp::mcp_v20260728::server_io::MessageFrame;

// =============================================================================
// MESSAGE TYPES
// =============================================================================

/// Parsed MCP message from the wire
#[derive(Debug)]
pub enum McpMessage {
    Request(RequestId, ClientRequest),
    Notification(ClientNotification),
    Result(
        RequestId,
        crate::bindings::wasmcp::mcp_v20260728::mcp::ClientResult,
    ),
    Error(
        Option<RequestId>,
        crate::bindings::wasmcp::mcp_v20260728::mcp::ErrorCode,
    ),
}

// =============================================================================
// MESSAGE PARSING
// =============================================================================

/// Parse incoming MCP message using server-io
///
/// Uses the new unified parse_message() interface with explicit frame parameter.
pub fn parse_mcp_message(
    input: &InputStream,
    limit: ReadLimit,
    frame: &MessageFrame,
) -> Result<(McpMessage, RequestMeta), String> {
    let parsed = server_io::parse_message(input, &limit, frame)
        .map_err(|e| format!("Failed to parse message: {:?}", e))?;

    let message = match parsed.message {
        ClientMessage::Request((request_id, client_request)) => {
            McpMessage::Request(request_id, client_request)
        }
        ClientMessage::Notification(client_notification) => {
            McpMessage::Notification(client_notification)
        }
        ClientMessage::Result((result_id, client_result)) => {
            McpMessage::Result(result_id, client_result)
        }
        ClientMessage::Error((error_id, error_code)) => McpMessage::Error(error_id, error_code),
    };
    Ok((message, parsed.meta))
}

// =============================================================================
// MESSAGE WRITING
// =============================================================================

/// Write MCP result using server-io
///
/// Uses the new unified send_message() interface with explicit frame parameter.
pub fn write_mcp_result(
    output: &OutputStream,
    id: RequestId,
    result: ServerResult,
    frame: &MessageFrame,
) -> Result<(), IoError> {
    let message = ServerMessage::Result((id, result));
    server_io::send_message(output, message, frame)
}

/// Answer `server/discover` from the transport, probing the middleware for capabilities
pub fn handle_discover(frame: &MessageFrame) -> ServerResult {
    ServerResult::Discover(DiscoverResult {
        meta: None,
        server_info: Implementation {
            name: "wasmcp-server".to_string(),
            title: None,
            version: env!("CARGO_PKG_VERSION").to_string(),
            description: None,
            icons: None,
        },
        capabilities: discover_capabilities(frame),
        supported_versions: SUPPORTED_VERSIONS.to_vec(),
        instructions: None,
    })
}

/// Acknowledge `subscriptions/listen` and end the stream.
///
/// shortcut: no pub/sub backend, so the ack honors no subscriptions (empty filter) and the
/// stream closes right away; upgrade when a component can push list-changed/updated events.
pub fn acknowledge_subscriptions(
    output: &OutputStream,
    request_id: &RequestId,
    frame: &MessageFrame,
) -> Result<(), IoError> {
    let id = match request_id {
        RequestId::Number(n) => n.to_string(),
        RequestId::String(s) => s.clone(),
    };
    let ack = NotificationOptions {
        meta: Some(serde_json::json!({ "io.modelcontextprotocol/subscriptionId": id }).to_string()),
        extras: Some("{}".to_string()),
    };
    server_io::send_message(
        output,
        ServerMessage::Notification(ServerNotification::SubscriptionsAcknowledged(ack)),
        frame,
    )
}

/// Delegate non-transport methods to middleware via server-handler
pub fn delegate_to_middleware(
    request_id: RequestId,
    client_request: ClientRequest,
    meta: &RequestMeta,
    identity: Option<&crate::bindings::wasmcp::mcp_v20260728::mcp::Identity>,
    output_stream: &OutputStream,
    frame: &MessageFrame,
    http_context: Option<crate::bindings::wasmcp::mcp_v20260728::server_auth::HttpContext>,
) -> Result<ServerResult, ErrorCode> {
    let ctx = create_message_context(Some(output_stream), meta, identity, frame, http_context);
    let message = ClientMessage::Request((request_id, client_request));

    match handle(&ctx, message) {
        Some(Ok(result)) => Ok(result),
        Some(Err(e)) => Err(e),
        None => Err(ErrorCode::InternalError(
            crate::bindings::wasmcp::mcp_v20260728::mcp::Error {
                code: -32603,
                message: "Handler returned None for request".to_string(),
                data: None,
            },
        )),
    }
}

/// Delegate notification to middleware via server-handler
pub fn delegate_notification(
    client_notification: ClientNotification,
    meta: &RequestMeta,
    frame: &MessageFrame,
    http_context: Option<crate::bindings::wasmcp::mcp_v20260728::server_auth::HttpContext>,
) -> Result<(), ErrorCode> {
    // No client-stream for notifications - they're one-way
    let ctx = create_message_context(None, meta, None, frame, http_context);
    handle(&ctx, ClientMessage::Notification(client_notification));
    Ok(())
}
