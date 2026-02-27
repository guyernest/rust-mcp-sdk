//! Mock MCP bridge JavaScript generation and injection.
//!
//! Injects a `window.mcpBridge` object into the page via CDP's
//! `addScriptToEvaluateOnNewDocument`, providing canned tool responses
//! without requiring a real MCP server.

use chromiumoxide::cdp::browser_protocol::page::AddScriptToEvaluateOnNewDocumentParams;
use chromiumoxide::page::Page;

/// Inject a mock MCP bridge into the page before navigation.
///
/// The `responses` value is a JSON object mapping tool names to their
/// canned return values. When the widget calls `mcpBridge.callTool(name, args)`,
/// the mock looks up `name` in the responses map and returns a deep clone.
///
/// This function must be called **before** `page.goto()` so that the mock
/// bridge is available when widget scripts execute on page load.
pub async fn inject_mock_bridge(page: &Page, responses: &serde_json::Value) -> anyhow::Result<()> {
    let responses_json = serde_json::to_string(responses)?;

    let script = format!(
        r#"
(function() {{
    const RESPONSES = {responses_json};
    let _internalState = {{}};

    window.mcpBridge = {{
        callTool: async function(name, args) {{
            const handler = RESPONSES[name];
            if (handler) {{
                return JSON.parse(JSON.stringify(handler));
            }}
            return {{ error: "Unknown tool: " + name }};
        }},
        getState: function() {{
            return JSON.parse(JSON.stringify(_internalState));
        }},
        setState: function(s) {{
            Object.assign(_internalState, s);
            window.dispatchEvent(new CustomEvent('widgetStateUpdate', {{ detail: s }}));
        }},
        getHost: function() {{
            return {{
                type: 'standalone',
                capabilities: {{ tools: true, resources: true }}
            }};
        }},
        __toolCallLog: [],
        __getToolCallLog: function() {{
            return this.__toolCallLog;
        }}
    }};

    // Wrap callTool to log every invocation
    const _origCallTool = window.mcpBridge.callTool;
    window.mcpBridge.callTool = async function(name, args) {{
        const result = await _origCallTool.call(window.mcpBridge, name, args);
        window.mcpBridge.__toolCallLog.push({{ name: name, args: args, result: result }});
        return result;
    }};
}})();
"#
    );

    page.evaluate_on_new_document(AddScriptToEvaluateOnNewDocumentParams::new(script))
        .await?;

    Ok(())
}

/// Retrieve the tool call log from the mock bridge.
///
/// Returns a vector of objects, each with `name`, `args`, and `result` fields.
pub async fn get_tool_call_log(page: &Page) -> anyhow::Result<Vec<serde_json::Value>> {
    let result: serde_json::Value = page
        .evaluate("window.mcpBridge.__getToolCallLog()")
        .await?
        .into_value()?;

    match result {
        serde_json::Value::Array(arr) => Ok(arr),
        _ => Ok(vec![]),
    }
}

/// Retrieve the current widget state from the mock bridge.
pub async fn get_widget_state(page: &Page) -> anyhow::Result<serde_json::Value> {
    let result: serde_json::Value = page
        .evaluate("window.mcpBridge.getState()")
        .await?
        .into_value()?;
    Ok(result)
}
