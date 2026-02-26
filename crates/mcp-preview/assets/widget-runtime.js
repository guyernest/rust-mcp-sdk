/**
 * MCP Widget Runtime -- Standalone WASM Bridge Loader
 *
 * Creates a mini-host that connects to an MCP server via the WASM client
 * and exposes `window.mcpBridge` for backward compatibility.
 *
 * This is a thin loader that delegates to the shared widget-runtime ESM
 * library for all bridge API implementation. The bridge code is NOT
 * duplicated here -- it comes from the compiled ES module.
 *
 * Usage:
 *   <script type="module" src="widget-runtime.js" data-mcp-url="http://localhost:3000/mcp"></script>
 *   <script>
 *     window.addEventListener('mcpBridgeReady', async () => {
 *       const result = await window.mcpBridge.callTool('my_tool', { arg: 'value' });
 *       console.log(result);
 *     });
 *   </script>
 *
 * Events:
 *   - mcpBridgeReady: Fired when the WASM client is connected and the bridge is ready.
 *   - mcpBridgeError: Fired (CustomEvent) with { detail: { error } } if initialization fails.
 *
 * The bridge exposes the same API as the preview proxy bridge via installCompat():
 *   - callTool(name, args)  -- returns { success, content }
 *   - getState()            -- returns {}
 *   - setState(s)           -- no-op (forward compat)
 *   - sendMessage(msg)      -- sends via App.sendMessage
 *   - openExternal(url)     -- opens via App.openLink
 *   - theme (getter)        -- from host context or 'light'
 *   - locale (getter)       -- from host context or navigator.language
 *   - displayMode (getter)  -- from host context or 'inline'
 */
(async function () {
  'use strict';

  // Locate script tag to read configuration
  const scriptTag = document.currentScript || document.querySelector('script[data-mcp-url]');
  const serverUrl = scriptTag ? scriptTag.getAttribute('data-mcp-url') : null;

  if (!serverUrl) {
    console.error('[widget-runtime] Missing data-mcp-url attribute on script tag.');
    return;
  }

  // Determine artifact URLs relative to this script's location
  const scriptUrl = new URL(scriptTag.src);
  const runtimeUrl = new URL('widget-runtime.mjs', scriptUrl);
  const wasmJsUrl = new URL('mcp_wasm_client.js', scriptUrl);
  const wasmBinaryUrl = new URL('mcp_wasm_client_bg.wasm', scriptUrl);

  try {
    // Import shared library and WASM client in parallel
    const [runtimeModule, wasmModule] = await Promise.all([
      import(runtimeUrl.href),
      import(wasmJsUrl.href),
    ]);

    const { App, AppBridge, installCompat } = runtimeModule;
    const init = wasmModule.default;
    const WasmClient = wasmModule.WasmClient;

    // Initialize WASM binary and connect to MCP server
    await init(wasmBinaryUrl.href);
    const client = new WasmClient();
    await client.connect(serverUrl);

    // Create App instance and install backward-compat shim
    const app = new App({ name: 'StandaloneWidget', version: '1.0.0' });
    installCompat(app);

    // Override the compat shim's callTool to route through the WASM client
    // since we are running standalone (no host iframe to postMessage through)
    window.mcpBridge.callTool = async function (name, args) {
      try {
        const mcpResult = await client.call_tool(name, args || {});
        const success = !mcpResult.isError;
        return { success, content: mcpResult.content || [] };
      } catch (e) {
        return { success: false, content: [], error: e.message };
      }
    };

    // Signal that the bridge is ready
    window.dispatchEvent(new Event('mcpBridgeReady'));
  } catch (e) {
    console.error('[widget-runtime] Initialization failed:', e);
    window.dispatchEvent(
      new CustomEvent('mcpBridgeError', { detail: { error: e.message || String(e) } })
    );
  }
})();
