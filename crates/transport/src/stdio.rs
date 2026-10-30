//! Stdio transport implementation
//!
//! Handles stdio-specific protocol concerns:
//! - Line-delimited JSON-RPC over stdin/stdout
//! - Process lifecycle via wasi:cli/run
//!
//! The protocol is stateless: every request carries its own `_meta`.
//! Delegates I/O to server-io via common wrappers

use crate::bindings::exports::wasi::cli::run::Guest;
use crate::bindings::wasi::cli::stdin::get_stdin;
use crate::bindings::wasi::cli::stdout::get_stdout;
use crate::bindings::wasmcp::mcp_v20260728::mcp::{ClientMessage, ClientRequest, ErrorCode};
use crate::bindings::wasmcp::mcp_v20260728::server_handler::handle;
use crate::common;

pub struct StdioTransportGuest;

impl Guest for StdioTransportGuest {
    fn run() -> Result<(), ()> {
        let stdin = get_stdin();
        let stdout = get_stdout();

        loop {
            let (message, meta) = match common::parse_mcp_message(
                &stdin,
                common::stdio_read_limit(),
                &common::stdio_frame(),
            ) {
                Ok(msg) => msg,
                Err(e) => {
                    // Stream closed means client disconnected - exit gracefully
                    if e.contains("Stream closed") {
                        return Ok(());
                    }
                    eprintln!("[ERROR] Failed to parse message: {}", e);
                    continue;
                }
            };

            match message {
                common::McpMessage::Request(request_id, client_request) => {
                    let frame = common::stdio_frame();

                    let outcome = match client_request {
                        ClientRequest::Discover => {
                            Ok(Some(common::handle_discover(&frame)))
                        }
                        ClientRequest::SubscriptionsListen(_) => {
                            if let Err(e) =
                                common::acknowledge_subscriptions(&stdout, &request_id, &frame)
                            {
                                eprintln!("[ERROR] Failed to write acknowledgement: {:?}", e);
                            }
                            Ok(None)
                        }
                        other => common::delegate_to_middleware(
                            request_id.clone(),
                            other,
                            &meta,
                            None, // No identity in stdio mode
                            &stdout,
                            &frame,
                            None, // No HTTP context in stdio mode
                        )
                        .map(Some),
                    };

                    match outcome {
                        Ok(Some(result)) => {
                            if let Err(e) =
                                common::write_mcp_result(&stdout, request_id, result, &frame)
                            {
                                eprintln!("[ERROR] Failed to write result: {:?}", e);
                            }
                        }
                        Ok(None) => {}
                        Err(e) => write_error(&stdout, Some(request_id), e),
                    }
                }

                common::McpMessage::Notification(notification) => {
                    if let Err(e) = common::delegate_notification(
                        notification,
                        &meta,
                        &common::stdio_frame(),
                        None,
                    ) {
                        eprintln!("[ERROR] Notification handling failed: {:?}", e);
                    }
                }

                common::McpMessage::Result(result_id, client_result) => {
                    let ctx = common::create_message_context(
                        None,
                        &meta,
                        None,
                        &common::stdio_frame(),
                        None,
                    );
                    handle(&ctx, ClientMessage::Result((result_id, client_result)));
                }

                common::McpMessage::Error(error_id, error_code) => {
                    let ctx = common::create_message_context(
                        None,
                        &meta,
                        None,
                        &common::stdio_frame(),
                        None,
                    );
                    handle(&ctx, ClientMessage::Error((error_id, error_code)));
                }
            }
        }
    }
}

/// Write JSON-RPC error to stdout
fn write_error(
    stdout: &crate::bindings::wasi::io::streams::OutputStream,
    id: Option<crate::bindings::wasmcp::mcp_v20260728::mcp::RequestId>,
    error: ErrorCode,
) {
    use crate::bindings::wasmcp::mcp_v20260728::mcp::ServerMessage;
    use crate::bindings::wasmcp::mcp_v20260728::server_io;

    let message = ServerMessage::Error((id, error));
    if let Err(e) = server_io::send_message(stdout, message, &common::stdio_frame()) {
        eprintln!("[ERROR] Failed to write error: {:?}", e);
    }
}
