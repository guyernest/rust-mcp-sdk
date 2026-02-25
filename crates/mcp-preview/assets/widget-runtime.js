/**
 * MCP Widget Runtime -- Standalone WASM Bridge Polyfill
 *
 * Provides `window.mcpBridge` powered by the WASM MCP client for use
 * outside the preview context. Widget authors include this script in
 * their HTML and specify the MCP server URL via a data attribute.
 *
 * Usage:
 *   <script src="widget-runtime.js" data-mcp-url="http://localhost:3000/mcp"></script>
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
 * The bridge exposes the same API as the preview proxy bridge:
 *   - callTool(name, args)  -- returns { success, content }
 *   - getState()            -- returns window.__mcpState or {}
 *   - setState(s)           -- merges into window.__mcpState
 *   - sendMessage(msg)      -- console.log
 *   - openExternal(url)     -- window.open
 *   - theme (getter)        -- reads document.documentElement.dataset.theme or 'light'
 *   - locale (getter)       -- navigator.language
 *   - displayMode (getter)  -- 'inline'
 */
(async function () {
  'use strict';

  // Locate script tag to read configuration
  const scriptTag = document.currentScript || document.querySelector('script[data-mcp-url]');
  const serverUrl = scriptTag ? scriptTag.getAttribute('data-mcp-url') : null;

  if (!serverUrl) {
    console.error('[mcpBridge] Missing data-mcp-url attribute on script tag. Cannot initialize WASM bridge.');
    return;
  }

  // Determine WASM artifact URLs relative to this script's location
  const scriptUrl = new URL(scriptTag.src);
  const wasmJsUrl = new URL('mcp_wasm_client.js', scriptUrl);
  const wasmBinaryUrl = new URL('mcp_wasm_client_bg.wasm', scriptUrl);

  // Shared widget state (standalone -- no preview runtime)
  if (!window.__mcpState) {
    window.__mcpState = {};
  }

  try {
    // Dynamically import the WASM JS module
    const wasmModule = await import(wasmJsUrl.href);
    const init = wasmModule.default;
    const WasmClient = wasmModule.WasmClient;

    // Initialize WASM binary
    await init(wasmBinaryUrl.href);

    // Create WASM client and connect to MCP server
    const client = new WasmClient();
    await client.connect(serverUrl);

    // Expose window.mcpBridge with standalone implementations
    window.mcpBridge = {
      /**
       * Call an MCP tool by name with the given arguments.
       * Returns { success: boolean, content: Array }.
       */
      callTool: async function (name, args) {
        try {
          const mcpResult = await client.call_tool(name, args || {});
          const success = !mcpResult.isError;
          return {
            success: success,
            content: mcpResult.content || []
          };
        } catch (e) {
          return {
            success: false,
            content: [],
            error: e.message
          };
        }
      },

      /** Get the current widget state object. */
      getState: function () {
        return window.__mcpState || {};
      },

      /** Merge partial state into the widget state. */
      setState: function (s) {
        window.__mcpState = Object.assign(window.__mcpState || {}, s);
      },

      /** Log a message (standalone: writes to console). */
      sendMessage: function (msg) {
        console.log('[mcpBridge] sendMessage:', msg);
      },

      /** Open a URL in a new browser tab. */
      openExternal: function (url) {
        window.open(url, '_blank');
      },

      /** Current theme from the document root data-theme attribute. */
      get theme() {
        return document.documentElement.dataset.theme || 'light';
      },

      /** Browser locale. */
      get locale() {
        return navigator.language;
      },

      /** Display mode (always 'inline' in standalone context). */
      get displayMode() {
        return 'inline';
      }
    };

    // ChatGPT compatibility alias
    window.openai = window.mcpBridge;

    // Signal that the bridge is ready
    window.dispatchEvent(new Event('mcpBridgeReady'));
  } catch (e) {
    console.error('[mcpBridge] Initialization failed:', e);
    window.dispatchEvent(
      new CustomEvent('mcpBridgeError', { detail: { error: e.message || String(e) } })
    );
  }
})();
