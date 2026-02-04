/**
 * MCP Widget Runtime Types
 *
 * Type definitions for the universal MCP widget bridge API.
 */

// =============================================================================
// Host Platform Types
// =============================================================================

/**
 * Known MCP host platforms.
 */
export type HostType = 'chatgpt' | 'mcp-apps' | 'mcp-ui' | 'generic' | 'unknown';

/**
 * Detected host platform information.
 */
export interface HostInfo {
  /** The detected host type */
  type: HostType;
  /** Host name (e.g., "ChatGPT", "Claude", "Nanobot") */
  name: string;
  /** Whether the bridge is ready */
  ready: boolean;
  /** Available capabilities */
  capabilities: HostCapabilities;
}

/**
 * Host capabilities vary by platform.
 */
export interface HostCapabilities {
  /** Can call MCP tools */
  callTool: boolean;
  /** Can read MCP resources */
  readResource: boolean;
  /** Can get MCP prompts */
  getPrompt: boolean;
  /** Can manage widget state (ChatGPT) */
  state: boolean;
  /** Can send follow-up messages (ChatGPT) */
  sendMessage: boolean;
  /** Can open external URLs */
  openExternal: boolean;
  /** Can send notifications (MCP-UI) */
  notify: boolean;
  /** Can send intents (MCP-UI) */
  sendIntent: boolean;
  /** Can upload files (ChatGPT) */
  uploadFile: boolean;
  /** Can download files (ChatGPT) */
  downloadFile: boolean;
  /** Can request display mode changes (ChatGPT) */
  displayModes: boolean;
  /** Can report intrinsic height (ChatGPT) */
  intrinsicHeight: boolean;
}

// =============================================================================
// Tool Types
// =============================================================================

/**
 * Result from calling an MCP tool.
 */
export interface ToolResult<T = unknown> {
  /** Whether the call succeeded */
  success: boolean;
  /** Result data (if success) */
  data?: T;
  /** Error message (if failure) */
  error?: string;
  /** Whether this was an error result from the tool */
  isError?: boolean;
}

/**
 * Options for tool calls.
 */
export interface CallToolOptions {
  /** Timeout in milliseconds (default: 30000) */
  timeout?: number;
  /** Message ID for correlation */
  messageId?: string;
  /** Signal for cancellation */
  signal?: AbortSignal;
}

// =============================================================================
// Resource Types
// =============================================================================

/**
 * Result from reading an MCP resource.
 */
export interface ResourceResult {
  /** Resource contents */
  contents: ResourceContent[];
}

/**
 * Individual resource content item.
 */
export interface ResourceContent {
  /** Resource URI */
  uri: string;
  /** MIME type */
  mimeType?: string;
  /** Text content */
  text?: string;
  /** Binary content (base64) */
  blob?: string;
}

// =============================================================================
// Prompt Types
// =============================================================================

/**
 * Result from getting an MCP prompt.
 */
export interface PromptResult {
  /** Prompt description */
  description?: string;
  /** Prompt messages */
  messages: PromptMessage[];
}

/**
 * Individual prompt message.
 */
export interface PromptMessage {
  /** Message role */
  role: 'user' | 'assistant';
  /** Message content */
  content: PromptContent;
}

/**
 * Prompt content types.
 */
export type PromptContent =
  | { type: 'text'; text: string }
  | { type: 'image'; data: string; mimeType: string }
  | { type: 'resource'; uri: string; text?: string; blob?: string; mimeType?: string };

// =============================================================================
// State Types (ChatGPT)
// =============================================================================

/**
 * Widget state object.
 * Can be any serializable JSON value.
 */
export type WidgetState = Record<string, unknown>;

/**
 * State update handler.
 */
export type StateUpdateHandler = (state: WidgetState) => void;

// =============================================================================
// Notification Types (MCP-UI)
// =============================================================================

/**
 * Notification severity levels.
 */
export type NotifyLevel = 'info' | 'success' | 'warning' | 'error';

// =============================================================================
// Display Mode Types (ChatGPT)
// =============================================================================

/**
 * Widget display modes.
 * - inline: Rendered inline with the conversation (default)
 * - pip: Picture-in-picture floating window
 * - fullscreen: Full screen takeover
 */
export type DisplayMode = 'inline' | 'pip' | 'fullscreen';

/**
 * Options for requesting a display mode change.
 */
export interface DisplayModeOptions {
  /** The display mode to request */
  mode: DisplayMode;
}

// =============================================================================
// File Operation Types (ChatGPT)
// =============================================================================

/**
 * Result from uploading a file.
 */
export interface FileUploadResult {
  /** Unique identifier for the uploaded file */
  fileId: string;
}

/**
 * Options for getting a file download URL.
 */
export interface FileDownloadOptions {
  /** The file ID to get the download URL for */
  fileId: string;
}

/**
 * Result from getting a file download URL.
 */
export interface FileDownloadResult {
  /** Temporary download URL for the file */
  downloadUrl: string;
}

// =============================================================================
// Environment Context Types (ChatGPT)
// =============================================================================

/**
 * Safe area insets for the widget viewport.
 * Accounts for device notches, navigation bars, etc.
 */
export interface SafeArea {
  /** Top inset in pixels */
  top: number;
  /** Bottom inset in pixels */
  bottom: number;
  /** Left inset in pixels */
  left: number;
  /** Right inset in pixels */
  right: number;
}

/**
 * Widget view type.
 * - default: Normal view
 * - compact: Compact/minimal view
 */
export type WidgetView = 'default' | 'compact';

/**
 * Theme setting for the widget.
 */
export type Theme = 'light' | 'dark';

/**
 * Complete environment context available from the host.
 */
export interface OpenAiEnvironment {
  /** Current theme setting */
  theme: Theme;
  /** Current display mode */
  displayMode: DisplayMode;
  /** Maximum height available for the widget in pixels */
  maxHeight: number;
  /** Safe area insets */
  safeArea: SafeArea;
  /** Widget view type */
  view: WidgetView;
  /** User agent string of the host */
  userAgent: string;
  /** Locale string (e.g., "en-US") */
  locale: string;
}

/**
 * Tool context provided when a tool is invoked.
 */
export interface ToolContext {
  /** Arguments supplied when the tool was invoked */
  toolInput: Record<string, unknown>;
  /** The structuredContent returned by the tool */
  toolOutput: unknown;
  /** The _meta payload (widget-only, never sent to model) */
  toolResponseMetadata: Record<string, unknown>;
}

// =============================================================================
// OpenAI Globals Type (for React hooks)
// =============================================================================

/**
 * All OpenAI globals that can be subscribed to reactively.
 * Used with useOpenAiGlobal hook.
 */
export interface OpenAiGlobals {
  /** Arguments supplied when the tool was invoked */
  toolInput: Record<string, unknown>;
  /** The structuredContent returned by the tool */
  toolOutput: unknown;
  /** The _meta payload (widget-only) */
  toolResponseMetadata: Record<string, unknown>;
  /** Current widget state */
  widgetState: WidgetState;
  /** Current theme */
  theme: Theme;
  /** Current display mode */
  displayMode: DisplayMode;
  /** Maximum height in pixels */
  maxHeight: number;
  /** Safe area insets */
  safeArea: SafeArea;
  /** Widget view type */
  view: WidgetView;
  /** User agent string */
  userAgent: string;
  /** Locale string */
  locale: string;
}

// =============================================================================
// Event Types
// =============================================================================

/**
 * Events emitted by the widget runtime.
 */
export interface WidgetEvents {
  /** Bridge is ready */
  ready: void;
  /** State was updated (ChatGPT) */
  stateUpdate: WidgetState;
  /** MCP notification received */
  notification: McpNotification;
  /** Bridge error occurred */
  error: Error;
}

/**
 * MCP notification from host.
 */
export interface McpNotification {
  /** Notification method */
  method: string;
  /** Notification params */
  params?: unknown;
}

// =============================================================================
// Bridge Interface
// =============================================================================

/**
 * The universal MCP bridge API available on window.mcpBridge.
 *
 * This is the low-level bridge injected by the adapters.
 * Use the WidgetRuntime class for a higher-level API.
 */
export interface McpBridge {
  /** Call an MCP tool */
  callTool(name: string, args?: Record<string, unknown>): Promise<unknown>;
  /** Read an MCP resource */
  readResource?(uri: string): Promise<unknown>;
  /** Get an MCP prompt */
  getPrompt?(name: string, args?: Record<string, unknown>): Promise<unknown>;
  /** Get current widget state (ChatGPT) */
  getState?(): WidgetState;
  /** Set widget state (ChatGPT) */
  setState?(state: WidgetState): void;
  /** Send follow-up message (ChatGPT) */
  sendMessage?(message: string): void;
  /** Open external URL */
  openExternal?(url: string): void;
  /** Send notification (MCP-UI) */
  notify?(level: NotifyLevel, message: string): void;
  /** Send intent (MCP-UI) */
  sendIntent?(action: string, data?: unknown): Promise<unknown>;
  /** Open link (MCP-UI) */
  openLink?(url: string): void;

  // ChatGPT Tool Context (NEW)
  /** Arguments supplied when the tool was invoked */
  readonly toolInput?: Record<string, unknown>;
  /** The structuredContent returned by the tool */
  readonly toolOutput?: unknown;
  /** The _meta payload (widget-only) */
  readonly toolResponseMetadata?: Record<string, unknown>;

  // ChatGPT File Operations (NEW)
  /** Upload a file and get a file ID */
  uploadFile?(file: File): Promise<FileUploadResult>;
  /** Get a temporary download URL for a file */
  getFileDownloadUrl?(fileId: string): Promise<FileDownloadResult>;

  // ChatGPT Display Modes (NEW)
  /** Request a display mode change */
  requestDisplayMode?(mode: DisplayMode): Promise<void>;
  /** Close the widget */
  requestClose?(): void;
  /** Report the widget's intrinsic height */
  notifyIntrinsicHeight?(height: number): void;
  /** Set the URL for the "Open in App" button */
  setOpenInAppUrl?(href: string): void;

  // ChatGPT Environment Context (NEW)
  /** Current theme */
  readonly theme?: Theme;
  /** Current locale */
  readonly locale?: string;
  /** Current display mode */
  readonly displayMode?: DisplayMode;
  /** Maximum widget height */
  readonly maxHeight?: number;
  /** Safe area insets */
  readonly safeArea?: SafeArea;
  /** User agent string */
  readonly userAgent?: string;
  /** Widget view type */
  readonly view?: WidgetView;
}

/**
 * Extend Window interface to include mcpBridge and openai.
 */
declare global {
  interface Window {
    mcpBridge?: McpBridge;
    openai?: {
      // Core operations
      callTool?(name: string, args?: Record<string, unknown>): Promise<unknown>;
      setWidgetState?(state: WidgetState): void;
      sendFollowUpMessage?(options: { prompt: string }): void;
      openExternal?(options: { href: string }): void;

      // Lifecycle callbacks
      onWidgetReady?(): void;
      onStateUpdate?(handler: StateUpdateHandler): void;

      // Tool context (read-only)
      readonly toolInput?: Record<string, unknown>;
      readonly toolOutput?: unknown;
      readonly toolResponseMetadata?: Record<string, unknown>;
      readonly widgetState?: WidgetState;

      // File operations
      uploadFile?(file: File): Promise<FileUploadResult>;
      getFileDownloadUrl?(options: FileDownloadOptions): Promise<FileDownloadResult>;

      // Display modes
      requestDisplayMode?(options: DisplayModeOptions): Promise<void>;
      requestClose?(): void;
      notifyIntrinsicHeight?(height: number): void;
      requestModal?(options: unknown): Promise<unknown>;
      setOpenInAppUrl?(options: { href: string }): void;

      // Environment context (read-only)
      readonly theme?: Theme;
      readonly displayMode?: DisplayMode;
      readonly maxHeight?: number;
      readonly safeArea?: SafeArea;
      readonly view?: WidgetView;
      readonly userAgent?: string;
      readonly locale?: string;
    };
  }
}

/**
 * Event type for OpenAI globals updates.
 * Listen for this on window to reactively update when globals change.
 */
export interface SetGlobalsEvent extends CustomEvent {
  detail: {
    globals: Partial<OpenAiGlobals>;
  };
}

/**
 * Event type name for OpenAI globals updates.
 */
export const SET_GLOBALS_EVENT_TYPE = 'openai:set_globals' as const;
