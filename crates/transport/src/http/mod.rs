//! HTTP transport implementation
//!
//! Handles HTTP-specific protocol concerns:
//! - Origin validation (DNS rebinding protection)
//! - Header validation (Accept, MCP-Protocol-Version, Mcp-Method)
//! - HTTP method routing (POST for MCP; GET only for OAuth discovery)
//! - Request/response lifecycle
//!
//! Delegates I/O to http-server-io via server-io interface

pub mod discovery;
mod get;
pub(crate) mod helpers;
pub mod post;
pub(crate) mod response;
mod validation;

use crate::bindings::exports::wasi::http::incoming_handler::Guest;
use crate::bindings::wasi::http::types::{IncomingRequest, Method, ResponseOutparam};
use crate::config::TransportConfig;
use crate::error::TransportError;
use crate::send_error;

pub struct HttpTransportGuest;

impl Guest for HttpTransportGuest {
    fn handle(request: IncomingRequest, response_out: ResponseOutparam) {
        // Block on async handler - this bridges sync WIT trait to async implementation
        futures::executor::block_on(handle_http_request_async(request, response_out))
    }
}

async fn handle_http_request_async(request: IncomingRequest, response_out: ResponseOutparam) {
    let config = TransportConfig::from_env();

    // DNS rebinding protection
    if let Err(e) = validation::validate_origin(&request) {
        send_error!(response_out, e);
    }

    match request.method() {
        Method::Post => post::handle_post(request, response_out, &config).await,
        Method::Get => get::handle_get(request, response_out),
        _ => match response::create_method_not_allowed_response() {
            Ok(response) => ResponseOutparam::set(response_out, Ok(response)),
            Err(e) => send_error!(response_out, TransportError::internal(e)),
        },
    }
}
