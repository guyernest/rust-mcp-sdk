/**
 * MCP Widget Runtime
 *
 * High-level API for MCP widgets to interact with host platforms.
 * Provides a unified interface that works across ChatGPT Apps, MCP Apps, and MCP-UI.
 */

import type {
  HostType,
  HostInfo,
  HostCapabilities,
  ToolResult,
  CallToolOptions,
  ResourceResult,
  PromptResult,
  WidgetState,
  NotifyLevel,
  WidgetEvents,
  McpBridge,
  DisplayMode,
  FileUploadResult,
  SafeArea,
  Theme,
  WidgetView,
} from './types';

/**
 * Event emitter for widget events.
 */
type EventHandler<T> = (data: T) => void;

/**
 * MCP Widget Runtime class.
 *
 * Provides a high-level, type-safe API for interacting with MCP hosts.
 *
 * @example
 * ```typescript
 * import { WidgetRuntime } from '@pmcp/widget-runtime';
 *
 * const runtime = new WidgetRuntime();
 *
 * // Wait for bridge to be ready
 * await runtime.ready();
 *
 * // Call an MCP tool
 * const result = await runtime.callTool('chess_move', { from: 'e2', to: 'e4' });
 *
 * // Handle state updates (ChatGPT)
 * runtime.on('stateUpdate', (state) => {
 *   console.log('New state:', state);
 * });
 * ```
 */
export class WidgetRuntime {
  private _ready = false;
  private _readyPromise: Promise<void>;
  private _resolveReady!: () => void;
  private _eventHandlers: Map<keyof WidgetEvents, Set<EventHandler<unknown>>> = new Map();
  private _state: WidgetState = {};
  private _hostInfo: HostInfo | null = null;

  constructor() {
    this._readyPromise = new Promise((resolve) => {
      this._resolveReady = resolve;
    });

    // Initialize when DOM is ready
    if (typeof window !== 'undefined') {
      if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', () => this._initialize());
      } else {
        this._initialize();
      }
    }
  }

  // ===========================================================================
  // Initialization
  // ===========================================================================

  private _initialize(): void {
    // Detect host platform
    this._hostInfo = this._detectHost();

    // Setup event listeners
    this._setupEventListeners();

    // Mark as ready if bridge is available
    if (this._getBridge()) {
      this._markReady();
    } else {
      // Retry after a short delay (bridge might be injected async)
      setTimeout(() => {
        if (this._getBridge()) {
          this._markReady();
        }
      }, 100);
    }
  }

  private _detectHost(): HostInfo {
    const capabilities = this._detectCapabilities();
    let type: HostType = 'unknown';
    let name = 'Unknown';

    if (typeof window !== 'undefined') {
      if (window.openai) {
        type = 'chatgpt';
        name = 'ChatGPT';
      } else if (window.mcpBridge) {
        // Determine if MCP Apps or MCP-UI based on capabilities
        if (window.mcpBridge.sendIntent) {
          type = 'mcp-ui';
          name = 'MCP-UI Host';
        } else {
          type = 'mcp-apps';
          name = 'MCP Apps Host';
        }
      }
    }

    return {
      type,
      name,
      ready: this._ready,
      capabilities,
    };
  }

  private _detectCapabilities(): HostCapabilities {
    const bridge = this._getBridge();
    return {
      callTool: !!bridge?.callTool,
      readResource: !!bridge?.readResource,
      getPrompt: !!bridge?.getPrompt,
      state: !!bridge?.getState && !!bridge?.setState,
      sendMessage: !!bridge?.sendMessage,
      openExternal: !!bridge?.openExternal,
      notify: !!bridge?.notify,
      sendIntent: !!bridge?.sendIntent,
      uploadFile: !!bridge?.uploadFile,
      downloadFile: !!bridge?.getFileDownloadUrl,
      displayModes: !!bridge?.requestDisplayMode,
      intrinsicHeight: !!bridge?.notifyIntrinsicHeight,
    };
  }

  private _getBridge(): McpBridge | undefined {
    if (typeof window === 'undefined') return undefined;
    return window.mcpBridge;
  }

  private _markReady(): void {
    if (this._ready) return;
    this._ready = true;
    if (this._hostInfo) {
      this._hostInfo.ready = true;
      this._hostInfo.capabilities = this._detectCapabilities();
    }
    this._emit('ready', undefined);
    this._resolveReady();
  }

  private _setupEventListeners(): void {
    if (typeof window === 'undefined') return;

    // Listen for widget state updates (ChatGPT)
    window.addEventListener('widgetStateUpdate', ((event: CustomEvent<WidgetState>) => {
      this._state = event.detail;
      this._emit('stateUpdate', event.detail);
    }) as EventListener);

    // Listen for MCP notifications
    window.addEventListener('mcpNotification', ((event: CustomEvent) => {
      this._emit('notification', event.detail);
    }) as EventListener);
  }

  // ===========================================================================
  // Public API - Lifecycle
  // ===========================================================================

  /**
   * Wait for the widget runtime to be ready.
   *
   * @returns Promise that resolves when the bridge is available
   *
   * @example
   * ```typescript
   * await runtime.ready();
   * console.log('Bridge is ready!');
   * ```
   */
  ready(): Promise<void> {
    return this._readyPromise;
  }

  /**
   * Check if the runtime is ready.
   */
  get isReady(): boolean {
    return this._ready;
  }

  /**
   * Get information about the host platform.
   */
  get host(): HostInfo {
    return this._hostInfo ?? {
      type: 'unknown',
      name: 'Unknown',
      ready: false,
      capabilities: {
        callTool: false,
        readResource: false,
        getPrompt: false,
        state: false,
        sendMessage: false,
        openExternal: false,
        notify: false,
        sendIntent: false,
        uploadFile: false,
        downloadFile: false,
        displayModes: false,
        intrinsicHeight: false,
      },
    };
  }

  // ===========================================================================
  // Public API - Tools
  // ===========================================================================

  /**
   * Call an MCP tool.
   *
   * @param name - Tool name
   * @param args - Tool arguments
   * @param options - Call options
   * @returns Tool result
   *
   * @example
   * ```typescript
   * const result = await runtime.callTool('get_weather', { city: 'Seattle' });
   * if (result.success) {
   *   console.log('Weather:', result.data);
   * }
   * ```
   */
  async callTool<T = unknown>(
    name: string,
    args?: Record<string, unknown>,
    options?: CallToolOptions
  ): Promise<ToolResult<T>> {
    const bridge = this._getBridge();
    if (!bridge?.callTool) {
      return { success: false, error: 'Tool calls not supported on this host' };
    }

    const timeout = options?.timeout ?? 30000;
    const controller = new AbortController();
    const signal = options?.signal ?? controller.signal;

    const timeoutId = setTimeout(() => controller.abort(), timeout);

    try {
      const result = await Promise.race([
        bridge.callTool(name, args),
        new Promise((_, reject) => {
          signal.addEventListener('abort', () => reject(new Error('Request aborted')));
        }),
      ]);

      clearTimeout(timeoutId);

      // Handle different result formats
      if (typeof result === 'object' && result !== null) {
        const r = result as { error?: boolean; content?: unknown; isError?: boolean };
        if (r.error || r.isError) {
          return { success: false, error: String(r.content ?? 'Unknown error'), isError: true };
        }
        return { success: true, data: result as T };
      }

      return { success: true, data: result as T };
    } catch (error) {
      clearTimeout(timeoutId);
      return { success: false, error: error instanceof Error ? error.message : String(error) };
    }
  }

  // ===========================================================================
  // Public API - Resources
  // ===========================================================================

  /**
   * Read an MCP resource.
   *
   * @param uri - Resource URI
   * @returns Resource contents
   *
   * @example
   * ```typescript
   * const result = await runtime.readResource('file:///config.json');
   * if (result) {
   *   console.log('Config:', result.contents);
   * }
   * ```
   */
  async readResource(uri: string): Promise<ResourceResult | null> {
    const bridge = this._getBridge();
    if (!bridge?.readResource) {
      console.warn('Resource reading not supported on this host');
      return null;
    }

    try {
      const result = await bridge.readResource(uri);
      return result as ResourceResult;
    } catch (error) {
      console.error('Failed to read resource:', error);
      return null;
    }
  }

  // ===========================================================================
  // Public API - Prompts
  // ===========================================================================

  /**
   * Get an MCP prompt.
   *
   * @param name - Prompt name
   * @param args - Prompt arguments
   * @returns Prompt result
   *
   * @example
   * ```typescript
   * const prompt = await runtime.getPrompt('analyze_code', { language: 'rust' });
   * if (prompt) {
   *   console.log('Prompt messages:', prompt.messages);
   * }
   * ```
   */
  async getPrompt(
    name: string,
    args?: Record<string, unknown>
  ): Promise<PromptResult | null> {
    const bridge = this._getBridge();
    if (!bridge?.getPrompt) {
      console.warn('Prompts not supported on this host');
      return null;
    }

    try {
      const result = await bridge.getPrompt(name, args);
      return result as PromptResult;
    } catch (error) {
      console.error('Failed to get prompt:', error);
      return null;
    }
  }

  // ===========================================================================
  // Public API - State (ChatGPT)
  // ===========================================================================

  /**
   * Get current widget state.
   *
   * State is preserved across tool calls in ChatGPT Apps.
   *
   * @returns Current state object
   *
   * @example
   * ```typescript
   * const state = runtime.getState();
   * console.log('Current turn:', state.turn);
   * ```
   */
  getState(): WidgetState {
    const bridge = this._getBridge();
    if (bridge?.getState) {
      return bridge.getState();
    }
    return this._state;
  }

  /**
   * Update widget state.
   *
   * Merges new state with existing state.
   *
   * @param state - State updates to apply
   *
   * @example
   * ```typescript
   * runtime.setState({ turn: 'white', moveCount: 10 });
   * ```
   */
  setState(state: Partial<WidgetState>): void {
    const bridge = this._getBridge();
    const newState = { ...this._state, ...state };
    this._state = newState;

    if (bridge?.setState) {
      bridge.setState(newState);
    }
  }

  // ===========================================================================
  // Public API - Communication
  // ===========================================================================

  /**
   * Send a follow-up message to the AI.
   *
   * Available on ChatGPT Apps.
   *
   * @param message - Message text
   *
   * @example
   * ```typescript
   * runtime.sendMessage('Show me the next move');
   * ```
   */
  sendMessage(message: string): void {
    const bridge = this._getBridge();
    if (bridge?.sendMessage) {
      bridge.sendMessage(message);
    } else {
      console.warn('sendMessage not supported on this host');
    }
  }

  /**
   * Show a notification to the user.
   *
   * Available on MCP-UI hosts.
   *
   * @param level - Notification level
   * @param message - Notification message
   *
   * @example
   * ```typescript
   * runtime.notify('success', 'Move completed!');
   * ```
   */
  notify(level: NotifyLevel, message: string): void {
    const bridge = this._getBridge();
    if (bridge?.notify) {
      bridge.notify(level, message);
    } else {
      // Fallback: log to console
      console[level === 'error' ? 'error' : level === 'warning' ? 'warn' : 'log'](
        `[${level.toUpperCase()}] ${message}`
      );
    }
  }

  /**
   * Open an external URL.
   *
   * @param url - URL to open
   *
   * @example
   * ```typescript
   * runtime.openExternal('https://chess.com/learn');
   * ```
   */
  openExternal(url: string): void {
    const bridge = this._getBridge();
    if (bridge?.openExternal) {
      bridge.openExternal(url);
    } else if (bridge?.openLink) {
      bridge.openLink(url);
    } else {
      // Fallback: open in new tab
      window.open(url, '_blank', 'noopener,noreferrer');
    }
  }

  /**
   * Send an intent action.
   *
   * Available on MCP-UI hosts.
   *
   * @param action - Intent action name
   * @param data - Intent data
   * @returns Intent result
   *
   * @example
   * ```typescript
   * await runtime.sendIntent('navigate', { route: '/settings' });
   * ```
   */
  async sendIntent(action: string, data?: unknown): Promise<unknown> {
    const bridge = this._getBridge();
    if (!bridge?.sendIntent) {
      console.warn('sendIntent not supported on this host');
      return null;
    }

    try {
      return await bridge.sendIntent(action, data);
    } catch (error) {
      console.error('Failed to send intent:', error);
      return null;
    }
  }

  // ===========================================================================
  // Public API - Tool Context (ChatGPT)
  // ===========================================================================

  /**
   * Get the arguments that were passed when the tool was invoked.
   *
   * Available on ChatGPT Apps.
   *
   * @returns Tool input arguments
   *
   * @example
   * ```typescript
   * const input = runtime.getToolInput();
   * console.log('Tool was called with:', input);
   * ```
   */
  getToolInput(): Record<string, unknown> {
    const bridge = this._getBridge();
    return bridge?.toolInput ?? window.openai?.toolInput ?? {};
  }

  /**
   * Get the structured content returned by the tool.
   *
   * Available on ChatGPT Apps.
   *
   * @returns Tool output (structuredContent)
   *
   * @example
   * ```typescript
   * const output = runtime.getToolOutput();
   * console.log('Tool returned:', output);
   * ```
   */
  getToolOutput(): unknown {
    const bridge = this._getBridge();
    return bridge?.toolOutput ?? window.openai?.toolOutput;
  }

  /**
   * Get the tool response metadata (_meta payload).
   *
   * This is widget-only data that is never sent to the model.
   *
   * Available on ChatGPT Apps.
   *
   * @returns Tool response metadata
   *
   * @example
   * ```typescript
   * const meta = runtime.getToolResponseMetadata();
   * console.log('Widget session ID:', meta['openai/widgetSessionId']);
   * ```
   */
  getToolResponseMetadata(): Record<string, unknown> {
    const bridge = this._getBridge();
    return bridge?.toolResponseMetadata ?? window.openai?.toolResponseMetadata ?? {};
  }

  // ===========================================================================
  // Public API - File Operations (ChatGPT)
  // ===========================================================================

  /**
   * Upload a file and get a file ID.
   *
   * Supported file types: image/png, image/jpeg, image/webp
   *
   * Available on ChatGPT Apps.
   *
   * @param file - File to upload
   * @returns Upload result with file ID, or null if not supported
   *
   * @example
   * ```typescript
   * const input = document.querySelector('input[type="file"]');
   * const file = input.files[0];
   * const result = await runtime.uploadFile(file);
   * if (result) {
   *   console.log('Uploaded file ID:', result.fileId);
   * }
   * ```
   */
  async uploadFile(file: File): Promise<FileUploadResult | null> {
    const bridge = this._getBridge();
    if (bridge?.uploadFile) {
      try {
        return await bridge.uploadFile(file);
      } catch (error) {
        console.error('Failed to upload file:', error);
        return null;
      }
    }
    if (window.openai?.uploadFile) {
      try {
        return await window.openai.uploadFile(file);
      } catch (error) {
        console.error('Failed to upload file:', error);
        return null;
      }
    }
    console.warn('File upload not supported on this host');
    return null;
  }

  /**
   * Get a temporary download URL for a file.
   *
   * Available on ChatGPT Apps.
   *
   * @param fileId - The file ID to get download URL for
   * @returns Download URL, or null if not supported
   *
   * @example
   * ```typescript
   * const url = await runtime.getFileDownloadUrl('file-123');
   * if (url) {
   *   const img = document.createElement('img');
   *   img.src = url;
   * }
   * ```
   */
  async getFileDownloadUrl(fileId: string): Promise<string | null> {
    const bridge = this._getBridge();
    if (bridge?.getFileDownloadUrl) {
      try {
        const result = await bridge.getFileDownloadUrl(fileId);
        return result.downloadUrl;
      } catch (error) {
        console.error('Failed to get file download URL:', error);
        return null;
      }
    }
    if (window.openai?.getFileDownloadUrl) {
      try {
        const result = await window.openai.getFileDownloadUrl({ fileId });
        return result.downloadUrl;
      } catch (error) {
        console.error('Failed to get file download URL:', error);
        return null;
      }
    }
    console.warn('File download not supported on this host');
    return null;
  }

  // ===========================================================================
  // Public API - Display Modes (ChatGPT)
  // ===========================================================================

  /**
   * Request a display mode change.
   *
   * Available on ChatGPT Apps. On mobile, PiP may be coerced to fullscreen.
   *
   * @param mode - The display mode to request
   * @returns Whether the request was successful
   *
   * @example
   * ```typescript
   * // Go fullscreen for a better experience
   * const success = await runtime.requestDisplayMode('fullscreen');
   * if (success) {
   *   console.log('Now in fullscreen mode');
   * }
   * ```
   */
  async requestDisplayMode(mode: DisplayMode): Promise<boolean> {
    const bridge = this._getBridge();
    if (bridge?.requestDisplayMode) {
      try {
        await bridge.requestDisplayMode(mode);
        return true;
      } catch (error) {
        console.error('Failed to request display mode:', error);
        return false;
      }
    }
    if (window.openai?.requestDisplayMode) {
      try {
        await window.openai.requestDisplayMode({ mode });
        return true;
      } catch (error) {
        console.error('Failed to request display mode:', error);
        return false;
      }
    }
    console.warn('Display mode changes not supported on this host');
    return false;
  }

  /**
   * Close the widget.
   *
   * Available on ChatGPT Apps.
   *
   * @example
   * ```typescript
   * // Close when user clicks "Done"
   * doneButton.onclick = () => runtime.requestClose();
   * ```
   */
  requestClose(): void {
    const bridge = this._getBridge();
    if (bridge?.requestClose) {
      bridge.requestClose();
    } else if (window.openai?.requestClose) {
      window.openai.requestClose();
    } else {
      console.warn('Widget close not supported on this host');
    }
  }

  /**
   * Report the widget's intrinsic height to avoid scroll clipping.
   *
   * Available on ChatGPT Apps.
   *
   * @param height - Height in pixels
   *
   * @example
   * ```typescript
   * // Report height after content changes
   * const height = document.body.scrollHeight;
   * runtime.notifyIntrinsicHeight(height);
   * ```
   */
  notifyIntrinsicHeight(height: number): void {
    const bridge = this._getBridge();
    if (bridge?.notifyIntrinsicHeight) {
      bridge.notifyIntrinsicHeight(height);
    } else if (window.openai?.notifyIntrinsicHeight) {
      window.openai.notifyIntrinsicHeight(height);
    }
    // Silent fail - not all hosts need this
  }

  /**
   * Set the URL for the "Open in App" button in fullscreen mode.
   *
   * Available on ChatGPT Apps.
   *
   * @param href - URL to open
   *
   * @example
   * ```typescript
   * runtime.setOpenInAppUrl('https://myapp.com/game/123');
   * ```
   */
  setOpenInAppUrl(href: string): void {
    const bridge = this._getBridge();
    if (bridge?.setOpenInAppUrl) {
      bridge.setOpenInAppUrl(href);
    } else if (window.openai?.setOpenInAppUrl) {
      window.openai.setOpenInAppUrl({ href });
    }
    // Silent fail - not all hosts need this
  }

  // ===========================================================================
  // Public API - Environment Context (ChatGPT)
  // ===========================================================================

  /**
   * Get the current theme setting.
   *
   * @returns 'light' or 'dark'
   *
   * @example
   * ```typescript
   * const theme = runtime.theme;
   * document.body.classList.add(`theme-${theme}`);
   * ```
   */
  get theme(): Theme {
    const bridge = this._getBridge();
    return bridge?.theme ?? window.openai?.theme ?? 'light';
  }

  /**
   * Get the current locale.
   *
   * @returns Locale string (e.g., 'en-US')
   *
   * @example
   * ```typescript
   * const locale = runtime.locale;
   * const formatter = new Intl.NumberFormat(locale);
   * ```
   */
  get locale(): string {
    const bridge = this._getBridge();
    return bridge?.locale ?? window.openai?.locale ?? 'en-US';
  }

  /**
   * Get the current display mode.
   *
   * @returns 'inline', 'pip', or 'fullscreen'
   */
  get currentDisplayMode(): DisplayMode {
    const bridge = this._getBridge();
    return bridge?.displayMode ?? window.openai?.displayMode ?? 'inline';
  }

  /**
   * Get the maximum height available for the widget.
   *
   * @returns Height in pixels, or undefined if not available
   */
  get maxHeight(): number | undefined {
    const bridge = this._getBridge();
    return bridge?.maxHeight ?? window.openai?.maxHeight;
  }

  /**
   * Get the safe area insets.
   *
   * @returns Safe area insets, or undefined if not available
   */
  get safeArea(): SafeArea | undefined {
    const bridge = this._getBridge();
    return bridge?.safeArea ?? window.openai?.safeArea;
  }

  /**
   * Get the widget view type.
   *
   * @returns 'default' or 'compact'
   */
  get view(): WidgetView {
    const bridge = this._getBridge();
    return bridge?.view ?? window.openai?.view ?? 'default';
  }

  /**
   * Get the user agent string of the host.
   *
   * @returns User agent string, or undefined if not available
   */
  get userAgent(): string | undefined {
    const bridge = this._getBridge();
    return bridge?.userAgent ?? window.openai?.userAgent;
  }

  // ===========================================================================
  // Public API - Events
  // ===========================================================================

  /**
   * Subscribe to runtime events.
   *
   * @param event - Event name
   * @param handler - Event handler
   * @returns Unsubscribe function
   *
   * @example
   * ```typescript
   * const unsubscribe = runtime.on('stateUpdate', (state) => {
   *   console.log('State updated:', state);
   * });
   *
   * // Later: unsubscribe();
   * ```
   */
  on<K extends keyof WidgetEvents>(
    event: K,
    handler: EventHandler<WidgetEvents[K]>
  ): () => void {
    if (!this._eventHandlers.has(event)) {
      this._eventHandlers.set(event, new Set());
    }
    this._eventHandlers.get(event)!.add(handler as EventHandler<unknown>);

    return () => {
      this._eventHandlers.get(event)?.delete(handler as EventHandler<unknown>);
    };
  }

  /**
   * Subscribe to an event once.
   *
   * @param event - Event name
   * @param handler - Event handler
   */
  once<K extends keyof WidgetEvents>(
    event: K,
    handler: EventHandler<WidgetEvents[K]>
  ): void {
    const wrapper = ((data: WidgetEvents[K]) => {
      this._eventHandlers.get(event)?.delete(wrapper as EventHandler<unknown>);
      handler(data);
    }) as EventHandler<WidgetEvents[K]>;
    this.on(event, wrapper);
  }

  private _emit<K extends keyof WidgetEvents>(event: K, data: WidgetEvents[K]): void {
    this._eventHandlers.get(event)?.forEach((handler) => {
      try {
        handler(data);
      } catch (error) {
        console.error(`Error in ${event} handler:`, error);
      }
    });
  }
}
