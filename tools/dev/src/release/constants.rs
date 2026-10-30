pub const COMPONENTS: &[&str] = &[
    "authorization",
    "filter-middleware",
    "kv-store",
    "method-not-found",
    "prompts-middleware",
    "resources-middleware",
    "server-io",
    "tools-middleware",
    "transport",
];

pub const MCP_WIT_NAMESPACE: &str = "wasmcp:mcp-v20260728";
pub const MCP_WIT_TAG_PREFIX: &str = "mcp-v2026-07-28-v";
pub const GH_RELEASE_FETCH_LIMIT: &str = "1000";

// Mapping for components where workflow name differs from component name
pub fn get_workflow_name(component: &str) -> String {
    format!("release-{}.yml", component)
}
