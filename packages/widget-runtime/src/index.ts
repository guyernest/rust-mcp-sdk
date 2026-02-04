/**
 * @pmcp/widget-runtime
 *
 * Universal MCP Widget Runtime - A type-safe SDK for building widgets
 * that work across ChatGPT Apps, MCP Apps, and MCP-UI hosts.
 *
 * @example Basic Usage
 * ```typescript
 * import { WidgetRuntime } from '@pmcp/widget-runtime';
 *
 * const runtime = new WidgetRuntime();
 *
 * // Wait for the bridge to be ready
 * await runtime.ready();
 *
 * // Call an MCP tool
 * const result = await runtime.callTool('my_tool', { arg: 'value' });
 * if (result.success) {
 *   console.log('Result:', result.data);
 * }
 *
 * // Manage state (ChatGPT)
 * runtime.setState({ count: 1 });
 * console.log('Current state:', runtime.getState());
 *
 * // Listen for state updates
 * runtime.on('stateUpdate', (state) => {
 *   console.log('State updated:', state);
 * });
 * ```
 *
 * @example React Usage
 * ```tsx
 * import { useWidgetRuntime, useWidgetState, useToolCall } from '@pmcp/widget-runtime';
 *
 * function MyWidget() {
 *   const runtime = useWidgetRuntime();
 *   const [state, setState] = useWidgetState({ count: 0 });
 *   const { call, loading, result } = useToolCall<number>('increment');
 *
 *   const handleClick = async () => {
 *     await call({ current: state.count });
 *     setState({ count: state.count + 1 });
 *   };
 *
 *   return (
 *     <div>
 *       <p>Count: {state.count}</p>
 *       <button onClick={handleClick} disabled={loading}>
 *         {loading ? 'Loading...' : 'Increment'}
 *       </button>
 *     </div>
 *   );
 * }
 * ```
 *
 * @packageDocumentation
 */

// Core runtime
export { WidgetRuntime } from './runtime';

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
  // New types for ChatGPT Apps alignment
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
} from './types';

// Event type constant
export { SET_GLOBALS_EVENT_TYPE } from './types';

// React hooks (tree-shakeable)
export {
  // Core hooks
  useWidgetRuntime,
  useWidgetReady,
  useWidgetHost,
  useWidgetState,
  useToolCall,
  useLazyToolCall,
  useWidgetNotification,
  // OpenAI global hook (ChatGPT)
  useOpenAiGlobal,
  // Tool context hooks (ChatGPT)
  useToolInput,
  useToolOutput,
  useToolResponseMetadata,
  // Environment hooks (ChatGPT)
  useTheme,
  useLocale,
  useDisplayMode,
  useMaxHeight,
  useSafeArea,
  useWidgetView,
  // File hooks (ChatGPT)
  useFileUpload,
  // Display mode hook (ChatGPT)
  useDisplayModeRequest,
} from './hooks';

// Utilities
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
