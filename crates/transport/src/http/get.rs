//! GET request handler
//!
//! GET is only used for the OAuth 2.0 discovery endpoint
//! (/.well-known/oauth-protected-resource, RFC 9728). MCP itself is POST-only.
//! All other GET requests return 405 Method Not Allowed.

use crate::bindings::wasi::http::types::{IncomingRequest, ResponseOutparam};
use crate::error::TransportError;
use crate::http::{discovery, response};
use crate::send_error;

pub fn handle_get(request: IncomingRequest, response_out: ResponseOutparam) {
    // Both /.well-known/oauth-protected-resource and the /mcp-suffixed variant are served
    let path = request.path_with_query().unwrap_or_default();
    let normalized_path = path.strip_suffix("/mcp").unwrap_or(&path);

    if normalized_path == "/.well-known/oauth-protected-resource" {
        discovery::handle_protected_resource_metadata(&request, response_out);
        return;
    }

    match response::create_method_not_allowed_response() {
        Ok(response) => {
            crate::bindings::wasi::http::types::ResponseOutparam::set(response_out, Ok(response));
        }
        Err(e) => send_error!(response_out, TransportError::internal(e)),
    }
}
