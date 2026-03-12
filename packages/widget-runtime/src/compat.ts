/**
 * Backward-compatibility shim
 *
 * Maps the legacy `window.mcpBridge` and `window.openai` APIs to the new
 * App class so existing widgets continue working without code changes.
 *
 * When running inside ChatGPT (window.openai detected), ChatGPT-specific
 * methods are populated under mcpBridge.extensions.chatgpt.
 * When running in standard MCP Apps hosts, extensions.chatgpt is undefined.
 */

import { App } from './app';
import type { CallToolResult, ChatGptExtensions, HostContext, McpBridgeExtensions } from './types';

// Track whether the deprecation warning has been logged
let deprecationWarned = false;

/**
 * Extract the raw tool result from an MCP CallToolResult.
 *
 * Tool handlers return JSON values that get wrapped in MCP content format:
 *   { content: [{ type: "text", text: '{"board":...}' }] }
 *
 * The legacy bridge returned the raw parsed value, so we unwrap it here
 * for backward compatibility with existing widgets.
 */
function unwrapToolResult(result: CallToolResult): unknown {
  if (result.isError) {
    const errorText = result.content?.[0]?.text ?? 'Unknown error';
    return { success: false, error: errorText };
  }

  // Extract first text content item and parse as JSON
  const textItem = result.content?.find(
    (c: Record<string, unknown>) => c.type === 'text' && typeof c.text === 'string'
  );
  if (textItem) {
    try {
      return JSON.parse(textItem.text as string);
    } catch {
      // Not JSON -- return raw text
      return textItem.text;
    }
  }

  // Fallback: return content array or structured content
  return result.content ?? result.structuredContent;
}

/**
 * Build the ChatGPT extensions object when window.openai is available.
 *
 * Delegates all ChatGPT-specific operations to the native window.openai API.
 * Returns undefined when not running inside ChatGPT.
 */
function buildChatGptExtensions(): ChatGptExtensions | undefined {
  if (typeof window === 'undefined' || !window.openai) {
    return undefined;
  }

  const openai = window.openai;

  return {
    getState: (): Record<string, unknown> => openai.widgetState ?? {},
    setState: (state: Record<string, unknown>): void => {
      openai.setWidgetState?.(state);
    },
    sendMessage: (message: string): void => {
      openai.sendFollowUpMessage?.({ prompt: message });
    },
    uploadFile: async (file: File) => {
      if (!openai.uploadFile) {
        throw new Error('uploadFile not available');
      }
      return openai.uploadFile(file);
    },
    getFileDownloadUrl: async (fileId: string) => {
      if (!openai.getFileDownloadUrl) {
        throw new Error('getFileDownloadUrl not available');
      }
      return openai.getFileDownloadUrl({ fileId });
    },
    requestDisplayMode: async (mode) => {
      if (!openai.requestDisplayMode) {
        throw new Error('requestDisplayMode not available');
      }
      await openai.requestDisplayMode({ mode });
    },
    requestClose: (): void => {
      openai.requestClose?.();
    },
    notifyIntrinsicHeight: (height: number): void => {
      openai.notifyIntrinsicHeight?.(height);
    },
    setOpenInAppUrl: (href: string): void => {
      openai.setOpenInAppUrl?.({ href });
    },
    get toolInput() { return openai.toolInput; },
    get toolOutput() { return openai.toolOutput; },
    get toolResponseMetadata() { return openai.toolResponseMetadata; },
    get theme() { return openai.theme; },
    get locale() { return openai.locale; },
    get displayMode() { return openai.displayMode; },
    get maxHeight() { return openai.maxHeight; },
    get safeArea() { return openai.safeArea; },
    get view() { return openai.view; },
    get userAgent() { return openai.userAgent; },
  };
}

/**
 * Install the backward-compatibility shim that maps `window.mcpBridge` and
 * `window.openai` to the provided App instance.
 *
 * Standard MCP methods (callTool, readResource, etc.) are on the root bridge.
 * ChatGPT-specific methods are under mcpBridge.extensions.chatgpt (only
 * populated when window.openai is detected).
 *
 * @param app - A connected App instance
 *
 * @example
 * ```typescript
 * import { App, installCompat } from '@pmcp/widget-runtime';
 *
 * const app = new App({ name: 'legacy-widget', version: '1.0.0' });
 * await app.connect();
 * installCompat(app);
 *
 * // Standard MCP call works on any host:
 * const result = await window.mcpBridge.callTool('my_tool', { key: 'value' });
 *
 * // ChatGPT-specific (only when running in ChatGPT):
 * if (window.mcpBridge.extensions?.chatgpt) {
 *   window.mcpBridge.extensions.chatgpt.sendMessage('hello');
 * }
 * ```
 */
export function installCompat(app: App): void {
  if (typeof window === 'undefined') {
    return;
  }

  const warnDeprecation = (): void => {
    if (!deprecationWarned) {
      deprecationWarned = true;
      console.warn(
        "[widget-runtime] window.mcpBridge is deprecated. " +
        "Use `import { App } from 'widget-runtime.js'` instead."
      );
    }
  };

  // Resolve host context for environment properties
  const getCtx = (): HostContext => app.getHostContext() ?? {};

  // Build ChatGPT extensions (undefined when not in ChatGPT)
  const chatgptExt = buildChatGptExtensions();

  // Build extensions namespace
  const extensions: McpBridgeExtensions = {};
  if (chatgptExt) {
    extensions.chatgpt = chatgptExt;
  }

  // Build the mcpBridge facade with standard methods at root
  const mcpBridge = {
    // Standard MCP methods (root level)
    callTool: async (name: string, args?: Record<string, unknown>): Promise<unknown> => {
      warnDeprecation();
      const result = await app.callServerTool({ name, arguments: args });
      return unwrapToolResult(result);
    },

    openExternal: (url: string): void => {
      warnDeprecation();
      app.openLink({ url });
    },

    openLink: (url: string): void => {
      warnDeprecation();
      app.openLink({ url });
    },

    // Host-specific extensions namespace
    extensions,

    // Legacy flat methods for backward compat (deprecated)
    getState: (): Record<string, unknown> => {
      warnDeprecation();
      return chatgptExt?.getState() ?? {};
    },

    setState: (state: Record<string, unknown>): void => {
      warnDeprecation();
      chatgptExt?.setState(state);
    },

    sendMessage: (message: string): void => {
      warnDeprecation();
      if (chatgptExt) {
        chatgptExt.sendMessage(message);
      } else {
        app.sendMessage({ message });
      }
    },

    get theme(): string | undefined {
      return chatgptExt?.theme ?? getCtx().theme;
    },

    get locale(): string | undefined {
      return chatgptExt?.locale ?? getCtx().locale;
    },

    get displayMode(): string | undefined {
      return chatgptExt?.displayMode ?? getCtx().displayMode;
    },
  };

  // Install window.mcpBridge
  (window as unknown as Record<string, unknown>).mcpBridge = mcpBridge;

  // Install window.openai ChatGPT compatibility alias
  (window as unknown as Record<string, unknown>).openai = {
    callTool: mcpBridge.callTool,
    setWidgetState: mcpBridge.setState,
    sendFollowUpMessage: (options: { prompt: string }) => {
      warnDeprecation();
      app.sendMessage({ message: options.prompt });
    },
    openExternal: (options: { href: string }) => {
      warnDeprecation();
      app.openLink({ url: options.href });
    },
  };
}
