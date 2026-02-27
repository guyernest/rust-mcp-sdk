/**
 * Backward-compatibility shim
 *
 * Maps the legacy `window.mcpBridge` and `window.openai` APIs to the new
 * App class so existing widgets continue working without code changes.
 */

import { App } from './app';
import type { CallToolResult, HostContext } from './types';

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
      // Not JSON — return raw text
      return textItem.text;
    }
  }

  // Fallback: return content array or structured content
  return result.content ?? result.structuredContent;
}

/**
 * Install the backward-compatibility shim that maps `window.mcpBridge` and
 * `window.openai` to the provided App instance.
 *
 * Existing widgets using `window.mcpBridge.callTool(name, args)` will
 * continue working, with calls routed through the App's postMessage transport.
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
 * // Legacy code still works:
 * const result = await window.mcpBridge.callTool('my_tool', { key: 'value' });
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

  // Build the mcpBridge facade
  const mcpBridge = {
    callTool: async (name: string, args?: Record<string, unknown>): Promise<unknown> => {
      warnDeprecation();
      const result = await app.callServerTool({ name, arguments: args });
      return unwrapToolResult(result);
    },

    getState: (): Record<string, unknown> => {
      warnDeprecation();
      return {};
    },

    setState: (_state: Record<string, unknown>): void => {
      warnDeprecation();
      // State management is not implemented in the new App class;
      // this is a no-op shim for forward compatibility
    },

    sendMessage: (message: string): void => {
      warnDeprecation();
      app.sendMessage({ message });
    },

    openExternal: (url: string): void => {
      warnDeprecation();
      app.openLink({ url });
    },

    openLink: (url: string): void => {
      warnDeprecation();
      app.openLink({ url });
    },

    get theme(): string | undefined {
      return getCtx().theme;
    },

    get locale(): string | undefined {
      return getCtx().locale;
    },

    get displayMode(): string | undefined {
      return getCtx().displayMode;
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
