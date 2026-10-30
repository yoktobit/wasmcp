# Counter Middleware

Server middleware that tracks downstream tool invocations across requests using the `wasmcp:keyvalue/store` interface (the 2026-07-28 protocol has no sessions, so the counter is global).

## Overview

This example demonstrates **two important patterns** in wasmcp:

- **Middleware Pattern**: Intercepting and transforming MCP requests
- **Key-Value Storage**: Persisting state across requests via key-value storage

Counter-middleware sits between the client and your capability providers, counting every tool call that flows through it while transparently passing requests downstream.

## What is Middleware?

In wasmcp, middleware components:

- **Intercept** specific requests they care about
- **Delegate** all other requests to the downstream handler unchanged
- **Can add** new capabilities (like our `get-count` tool)

## How It Works

### 1. Intercept tools/list

When a client requests the list of available tools:

1. Add our own `get-count` tool to the response
2. Call downstream handler to get their tools
3. Merge both lists and return

This is how middleware can **add capabilities** to existing servers.

### 2. Intercept tools/call

When a client calls a tool:

1. Check if it's calling our `get-count` tool → handle it directly
2. Otherwise, delegate to downstream handler
3. If downstream succeeds, increment the counter
4. Return the downstream result

This is how middleware can **observe and count** operations.

### 3. Delegate Everything Else

For all other requests (initialize, resources/list, prompts/list, etc.):

- Pass through to downstream unchanged
- Middleware is transparent for requests it doesn't care about

## Building

```bash
make build
```

This produces `target/wasm32-wasip2/release/counter_middleware.wasm`.

## Composition Examples

Middleware can be placed **anywhere** in the composition chain. Components are composed left-to-right, so middleware wraps everything that comes after it.

### Count Calculator Invocations

```bash
wasmcp compose server \
  target/wasm32-wasip2/release/counter_middleware.wasm \
  ../calculator-rs/target/wasm32-wasip2/release/calculator.wasm \
  -o server.wasm
```

The counter will track how many times calculator tools (`add`, `subtract`, `factorial`) are called.

### Count Multiple Tool Providers

```bash
wasmcp compose server \
  target/wasm32-wasip2/release/counter_middleware.wasm \
  ../calculator-rs/target/wasm32-wasip2/release/calculator.wasm \
  ../weather-ts/dist/weather.wasm \
  -o server.wasm
```

The counter tracks invocations across ALL downstream tools.

### Multiple Middleware Layers

```bash
wasmcp compose server \
  middleware1.wasm \
  middleware2.wasm \
  tools.wasm \
  -o server.wasm
```

Middleware composes naturally - each layer wraps the next.

## The get-count Tool

Counter-middleware adds a `get-count` tool that retrieves the current count:

```bash
# Using wasmcp CLI
wasmcp mcp call-tool get-count

# Response
{
  "content": [{
    "type": "text",
    "text": "Total tool calls : 5"
  }]
}
```

This tool demonstrates:
- Middleware adding new capabilities
- Reading from key-value storage
- Sending notifications via server-io

## Code Walkthrough

### Request Flow

```rust
impl Guest for Counter {
    fn handle(
        ctx: MessageContext,
        message: ClientMessage,
    ) -> Option<Result<ServerResult, ErrorCode>> {
        match message {
            // Intercept tools/list and tools/call
            ClientMessage::Request((_, ClientRequest::ToolsList(req))) => {
                Some(handle_list_tools(&ctx, req).map(ServerResult::ToolsList))
            }
            ClientMessage::Request((_, ClientRequest::ToolsCall(req))) => {
                Some(handle_call_tool(&ctx, req).map(ServerResult::ToolsCall))
            }

            // Delegate everything else downstream
            _ => downstream::handle(&downstream_ctx, message)
        }
    }
}
```

### Counter

```rust
fn increment_counter() {
    if let Ok(bucket) = kv::open("") {
        let current = bucket.get_u64("tool_call_count").ok().flatten().unwrap_or(0);
        let _ = bucket.set_u64("tool_call_count", current + 1);
    }
}
```

## Key Takeaways

- **Middleware pattern**: Import + export server-handler to sit in the middle
- **Composition order**: Left-to-right wrapping (middleware before providers)
- **Key-value storage**: Persistent state across requests
- **Transparency**: Delegate what you don't care about
- **Extensibility**: Add new tools while preserving downstream capabilities

## Testing

After building and composing:

```bash
# Start the server (example with Spin)
spin up -f server.wasm

# In another terminal, call some tools
wasmcp mcp call-tool add '{"a": 5, "b": 3}'
wasmcp mcp call-tool subtract '{"a": 10, "b": 4}'

# Check the count
wasmcp mcp call-tool get-count
# Output: "Total tool calls : 2"
```

Each tool invocation increments the counter, and `get-count` retrieves the current value from key-value storage.
