/**
 * @pmcp/widget-runtime - Browser entry point
 *
 * This entry point excludes React hooks so the bundle can run
 * in browsers without a module bundler. React hooks are available
 * via the full package entry point (index.ts) for bundled apps.
 *
 * @packageDocumentation
 */

// Core runtime
export { WidgetRuntime } from './runtime';

// MCP Apps protocol classes
export { App } from './app';
export { AppBridge } from './app-bridge';
export { PostMessageTransport } from './transport';
export { installCompat } from './compat';

// Types
export type {
  HostType,
  HostInfo,
  HostCapabilities,
  ToolResult,
  CallToolOptions,
  ResourceResult,
  ResourceContent,
  PromptResult,
  PromptMessage,
  PromptContent,
  WidgetState,
  StateUpdateHandler,
  NotifyLevel,
  WidgetEvents,
  McpNotification,
  McpBridge,
  DisplayMode,
  DisplayModeOptions,
  FileUploadResult,
  FileDownloadOptions,
  FileDownloadResult,
  SafeArea,
  WidgetView,
  Theme,
  OpenAiEnvironment,
  ToolContext,
  OpenAiGlobals,
  SetGlobalsEvent,
  // MCP Apps protocol types
  CallToolParams,
  CallToolResult,
  HostContext,
  AppOptions,
  AppBridgeOptions,
} from './types';

// Transport types
export type { JsonRpcRequest, JsonRpcResponse, JsonRpcNotification, TransportOptions } from './transport';

// Event type constant
export { SET_GLOBALS_EVENT_TYPE } from './types';

// Utilities (no React dependency)
export {
  detectHost,
  isWidget,
  isChatGPT,
  isMcpApps,
  isMcpUI,
  getBridge,
  waitForBridge,
  createMessageId,
  serializeState,
  mergeState,
  debounce,
  throttle,
  log,
} from './utils';

// Default export for convenience
export { WidgetRuntime as default } from './runtime';
