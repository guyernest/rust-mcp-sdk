# widget-runtime

TypeScript bridge library for MCP Apps widgets. Provides the communication layer between widget iframes and MCP servers.

## Classes

### `App` (Widget Side)
Widget-side client for MCP Apps protocol communication.

```typescript
import { App, installCompat } from 'widget-runtime';

const app = new App();
await app.connect();

// Modern API
const result = await app.callServerTool('get_weather', { city: 'London' });

// Legacy compatibility (window.mcpBridge)
installCompat(app);
```

### `AppBridge` (Host Side)
Host-side iframe bridge manager that routes tool calls to the MCP server.

```typescript
import { AppBridge } from 'widget-runtime';

const bridge = new AppBridge(iframe, {
  toolCallHandler: async (name, args) => {
    return await mcpClient.callTool(name, args);
  }
});
```

### `PostMessageTransport`
JSON-RPC 2.0 over postMessage with correlation IDs and origin validation.

## Type Definitions

TypeScript declarations ship at `dist/index.d.ts` providing autocomplete for:
- `callTool(name, args)` — Call an MCP tool
- `getState()` / `setState(state)` — Widget state persistence
- `theme`, `locale`, `displayMode` — Host context
- Lifecycle events: `mcpBridgeReady`, `mcpBridgeError`

## Build

```bash
cd packages/widget-runtime
npm run build    # Compiles to dist/index.mjs + dist/index.js + dist/index.d.ts
```

The compiled ESM is copied to `crates/mcp-preview/assets/widget-runtime.mjs` for embedding.
