//! MCP message handlers for POST requests
//!
//! Handles different MCP message types:
//! - Requests (method invocations)
//! - Notifications (one-way messages)
//! - Results (responses from client)
//! - Errors (error responses from client)

use crate::bindings::wasi::http::types::ResponseOutparam;
use crate::bindings::wasi::io::streams::OutputStream;
use crate::bindings::wasmcp::mcp_v20260728::mcp::{
    ClientNotification, ClientRequest, ClientResult, ErrorCode, RequestId,
};
use crate::bindings::wasmcp::mcp_v20260728::server_handler::handle;
use crate::bindings::wasmcp::mcp_v20260728::server_io::RequestMeta;
use crate::common;
use crate::error::TransportError;
use crate::http::response;
use crate::send_error;

/// Handle MCP request message
///
/// - `server/discover` and `subscriptions/listen` are answered by the transport
/// - All others are delegated to middleware
pub fn handle_mcp_request(
    request_id: RequestId,
    client_request: ClientRequest,
    meta: &RequestMeta,
    identity: Option<&crate::bindings::wasmcp::mcp_v20260728::mcp::Identity>,
    output_stream: &OutputStream,
    frame: &common::MessageFrame,
    http_context: Option<crate::bindings::wasmcp::mcp_v20260728::server_auth::HttpContext>,
) -> Result<(), TransportError> {
    match client_request {
        ClientRequest::Discover => {
            let result = common::handle_discover(frame);
            common::write_mcp_result(output_stream, request_id, result, frame)?;
            Ok(())
        }
        ClientRequest::SubscriptionsListen(_) => {
            common::acknowledge_subscriptions(output_stream, &request_id, frame)?;
            Ok(())
        }
        _ => {
            let result = common::delegate_to_middleware(
                request_id.clone(),
                client_request,
                meta,
                identity,
                output_stream,
                frame,
                http_context,
            )
            .map_err(|e| {
                TransportError::protocol(format!("Middleware delegation failed: {:?}", e))
            })?;

            common::write_mcp_result(output_stream, request_id, result, frame)?;
            Ok(())
        }
    }
}

/// Handle MCP notification message
///
/// Notifications are one-way messages that don't expect a response.
/// Delegates to middleware for processing.
pub fn handle_mcp_notification(
    client_notification: ClientNotification,
    meta: &RequestMeta,
    http_context: Option<crate::bindings::wasmcp::mcp_v20260728::server_auth::HttpContext>,
) -> Result<(), TransportError> {
    common::delegate_notification(
        client_notification,
        meta,
        &common::plain_json_frame(),
        http_context,
    )
    .map_err(|e| TransportError::protocol(format!("Notification handling failed: {:?}", e)))?;

    Ok(())
}

/// Handle result response from client (bidirectional MCP)
///
/// In bidirectional MCP, clients can send responses to server-initiated requests.
pub fn handle_mcp_result(
    result_id: RequestId,
    client_result: ClientResult,
    meta: &RequestMeta,
) -> Result<(), TransportError> {
    // Create message context (no client-stream for results - client sending to server)
    let ctx = common::create_message_context(None, meta, None, &common::plain_json_frame(), None);

    // Create client message
    let message = crate::bindings::wasmcp::mcp_v20260728::mcp::ClientMessage::Result((
        result_id,
        client_result,
    ));

    // Delegate to imported server-handler (should return None for results from client)
    handle(&ctx, message);
    Ok(())
}

/// Handle error response from client (bidirectional MCP)
///
/// In bidirectional MCP, clients can send error responses to server-initiated requests.
pub fn handle_mcp_error(
    error_id: Option<RequestId>,
    error_code: ErrorCode,
    meta: &RequestMeta,
) -> Result<(), TransportError> {
    // Create message context (no client-stream for errors - client sending to server)
    let ctx = common::create_message_context(None, meta, None, &common::plain_json_frame(), None);

    // Create client message
    let message =
        crate::bindings::wasmcp::mcp_v20260728::mcp::ClientMessage::Error((error_id, error_code));

    // Delegate to imported server-handler (should return None for errors from client)
    handle(&ctx, message);
    Ok(())
}

/// Send result or error response for notifications, results, and errors
///
/// Used for one-way messages that return 202 Accepted on success.
pub fn respond_with_result(result: Result<(), TransportError>, response_out: ResponseOutparam) {
    match result {
        Ok(_) => match response::create_accepted_response() {
            Ok(response) => {
                crate::bindings::wasi::http::types::ResponseOutparam::set(
                    response_out,
                    Ok(response),
                );
            }
            Err(e) => send_error!(response_out, TransportError::internal(e)),
        },
        Err(e) => send_error!(response_out, e),
    }
}
