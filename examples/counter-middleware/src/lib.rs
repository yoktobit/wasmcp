//! Counter Middleware Component
//!
//! A server middleware that counts tool invocations across requests.
//! Demonstrates both the middleware pattern and key-value storage
//! (the protocol is stateless, so the counter is global rather than per-session).

mod bindings {
    wit_bindgen::generate!({
        world: "counter-middleware",
        generate_all,
    });
}

use bindings::exports::wasmcp::mcp_v20260728::server_handler::{Guest, MessageContext};
use bindings::wasmcp::keyvalue::store::TypedValue;
use bindings::wasmcp::mcp_v20260728::mcp::*;
use bindings::wasmcp::mcp_v20260728::server_handler as downstream;
use bindings::wasmcp::mcp_v20260728::server_io;
use bindings::wasmcp::keyvalue::store as kv;

struct Counter;

impl Guest for Counter {
    fn handle(
        ctx: MessageContext,
        message: ClientMessage,
    ) -> Option<Result<ServerResult, ErrorCode>> {
        // Only handle request messages
        let ClientMessage::Request((_request_id, request)) = &message else {
            // Not a request - delegate to downstream
            let downstream_ctx = downstream::MessageContext {
                client_stream: ctx.client_stream,
                protocol_version: ctx.protocol_version,
                client_capabilities: ctx.client_capabilities,
                client_info: ctx.client_info,
                log_level: ctx.log_level,
                identity: ctx.identity,
                frame: ctx.frame,
                http_context: ctx.http_context,
            };
            return downstream::handle(&downstream_ctx, message);
        };

        match request {
            ClientRequest::ToolsList(req) => {
                Some(handle_list_tools(&ctx, req.clone()).map(ServerResult::ToolsList))
            }
            ClientRequest::ToolsCall(req) => {
                Some(handle_call_tool(&ctx, req.clone()).map(ServerResult::ToolsCall))
            }
            // All other requests - delegate to downstream handler
            _ => {
                let downstream_ctx = downstream::MessageContext {
                    client_stream: ctx.client_stream,
                    protocol_version: ctx.protocol_version,
                    client_capabilities: ctx.client_capabilities,
                client_info: ctx.client_info,
                log_level: ctx.log_level,
                    identity: ctx.identity,
                    frame: ctx.frame,
                    http_context: ctx.http_context,
                };
                downstream::handle(&downstream_ctx, message)
            }
        }
    }
}

fn handle_list_tools(
    ctx: &MessageContext,
    _request: ListToolsRequest,
) -> Result<ListToolsResult, ErrorCode> {
    // Get our own tool
    let our_tool = Tool {
        name: "get-count".to_string(),
        input_schema: r#"{
            "type": "object",
            "properties": {}
        }"#
        .to_string(),
        options: Some(ToolOptions {
            meta: None,
            annotations: None,
            description: Some(
                "Get the total tool call count.".to_string(),
            ),
            output_schema: None,
            title: Some("Get Call Count".to_string()),
            icons: None,
        }),
    };

    // Get tools from downstream
    let downstream_ctx = downstream::MessageContext {
        client_stream: ctx.client_stream,
        protocol_version: ctx.protocol_version.clone(),
        client_capabilities: ctx.client_capabilities.clone(),
                client_info: ctx.client_info.clone(),
                log_level: ctx.log_level,
        identity: ctx.identity.clone(),
        frame: ctx.frame.clone(),
        http_context: ctx.http_context.clone(),
    };

    let downstream_msg = ClientMessage::Request((
        RequestId::Number(0),
        ClientRequest::ToolsList(_request.clone()),
    ));

    match downstream::handle(&downstream_ctx, downstream_msg) {
        Some(Ok(ServerResult::ToolsList(downstream_result))) => {
            // Merge our tool with downstream tools
            let mut all_tools = vec![our_tool];
            all_tools.extend(downstream_result.tools);

            Ok(ListToolsResult {
                tools: all_tools,
                next_cursor: downstream_result.next_cursor,
                cache: downstream_result.cache,
                meta: downstream_result.meta,
            })
        }
        _ => {
            // Just return our tool if downstream fails
            Ok(ListToolsResult {
                tools: vec![our_tool],
                next_cursor: None,
                cache: None,
                meta: None,
            })
        }
    }
}

fn handle_call_tool(
    ctx: &MessageContext,
    request: CallToolRequest,
) -> Result<CallToolResult, ErrorCode> {
    // Handle our own tool
    if request.name == "get-count" {
        return Ok(execute_get_count(ctx));
    }

    // Delegate to downstream for all other tools
    let downstream_ctx = downstream::MessageContext {
        client_stream: ctx.client_stream,
        protocol_version: ctx.protocol_version.clone(),
        client_capabilities: ctx.client_capabilities.clone(),
                client_info: ctx.client_info.clone(),
                log_level: ctx.log_level,
        identity: ctx.identity.clone(),
        frame: ctx.frame.clone(),
        http_context: ctx.http_context.clone(),
    };

    let downstream_msg = ClientMessage::Request((
        RequestId::Number(0),
        ClientRequest::ToolsCall(request),
    ));

    match downstream::handle(&downstream_ctx, downstream_msg) {
        Some(Ok(ServerResult::ToolsCall(result))) => {
            // Downstream tool executed successfully - increment counter
            increment_counter(ctx);
            Ok(result)
        }
        Some(Err(e)) => Err(e),
        _ => Err(ErrorCode::MethodNotFound(Error {
            code: -32601,
            message: "Tool not found".to_string(),
            data: None,
        })),
    }
}

/// Send a log notification to the client if a stream is available
fn log_notification(ctx: &MessageContext, message: String, level: LogLevel) {
    if let Some(stream) = &ctx.client_stream {
        let notification = ServerNotification::Log(LoggingMessageNotification {
            data: message,
            level,
            logger: Some("counter".to_string()),
        });
        let msg = ServerMessage::Notification(notification);
        let _ = server_io::send_message(stream, msg, &ctx.frame);
    }
}

const COUNTER_KEY: &str = "tool_call_count";

fn read_count(bucket: &kv::Bucket) -> u64 {
    match bucket.get(COUNTER_KEY) {
        Ok(Some(TypedValue::AsBytes(bytes))) => String::from_utf8(bytes)
            .unwrap_or_default()
            .parse::<u64>()
            .unwrap_or(0),
        Ok(Some(TypedValue::AsString(s))) => s.parse::<u64>().unwrap_or(0),
        Ok(Some(TypedValue::AsU64(n))) => n,
        _ => 0,
    }
}

fn increment_counter(ctx: &MessageContext) {
    if let Ok(bucket) = kv::open("") {
        let new_count = read_count(&bucket) + 1;
        let _ = bucket.set(COUNTER_KEY, &TypedValue::AsU64(new_count));

        // Send notification about the counter increment
        log_notification(
            ctx,
            format!("Tool call counter incremented to {}", new_count),
            LogLevel::Info,
        );
    }
}

fn execute_get_count(ctx: &MessageContext) -> CallToolResult {
    let message = match kv::open("") {
        Ok(bucket) => {
            let count = read_count(&bucket);
            log_notification(
                ctx,
                format!("Retrieving counter: {} tool calls", count),
                LogLevel::Info,
            );
            format!("Total tool calls: {}", count)
        }
        Err(_) => {
            log_notification(
                ctx,
                "Key-value store unavailable - counter not available".to_string(),
                LogLevel::Warning,
            );
            "Key-value store unavailable - counter not available".to_string()
        }
    };

    success_result(message)
}

fn success_result(result: String) -> CallToolResult {
    CallToolResult {
        content: vec![ContentBlock::Text(TextContent {
            text: TextData::Text(result),
            options: None,
        })],
        is_error: None,
        meta: None,
        structured_content: None,
    }
}


bindings::export!(Counter with_types_in bindings);
