/**
 * AppBridge -- Host-side bridge for managing widget iframe communication
 *
 * Handles the host side of the MCP Apps postMessage JSON-RPC protocol.
 * Creates and manages the message bridge between the host window and a
 * widget iframe, routing tool calls to the provided handler.
 */

import { PostMessageTransport } from './transport';
import type { CallToolResult, HostContext, AppBridgeOptions } from './types';

// =============================================================================
// AppBridge Class
// =============================================================================

/**
 * Host-side bridge that manages communication with a widget iframe.
 *
 * The AppBridge listens for JSON-RPC requests from the iframe (e.g.,
 * `ui/initialize`, `tools/call`) and dispatches them to the provided
 * handlers. It also supports pushing notifications to the widget.
 *
 * @example
 * ```typescript
 * import { AppBridge } from '@pmcp/widget-runtime';
 *
 * const iframe = document.getElementById('widget') as HTMLIFrameElement;
 *
 * const bridge = new AppBridge({
 *   iframe,
 *   toolCallHandler: async (name, args) => {
 *     const result = await mcpClient.callTool(name, args);
 *     return result;
 *   },
 * });
 *
 * await bridge.initialize();
 * ```
 */
export class AppBridge {
  private _iframe: HTMLIFrameElement;
  private _toolCallHandler: (name: string, args?: Record<string, unknown>) => Promise<CallToolResult>;
  private _origin: string;
  private _transport: PostMessageTransport | null = null;
  private _hostContext: HostContext;
  private _initialized = false;

  constructor(options: AppBridgeOptions) {
    this._iframe = options.iframe;
    this._toolCallHandler = options.toolCallHandler;
    this._origin = options.origin ?? window.location.origin;
    this._hostContext = options.hostContext ?? {};
  }

  /**
   * Start listening for messages from the widget iframe.
   *
   * Sets up the PostMessageTransport and registers request handlers for
   * the MCP Apps protocol methods (ui/initialize, tools/call, etc.).
   */
  initialize(): void {
    if (this._initialized) {
      return;
    }

    const contentWindow = this._iframe.contentWindow;
    if (!contentWindow) {
      console.error('[AppBridge] iframe has no contentWindow. Is it attached to the DOM?');
      return;
    }

    this._transport = new PostMessageTransport({
      targetWindow: contentWindow,
      targetOrigin: this._origin,
    });

    // Handle incoming requests from the widget
    this._transport.onRequest(async (method, params) => {
      return this._handleRequest(method, params);
    });

    this._initialized = true;
  }

  /**
   * Send a tool input notification to the widget.
   *
   * @param params - The tool input arguments
   */
  sendToolInput(params: Record<string, unknown>): void {
    if (!this._transport) {
      console.warn('[AppBridge] Not initialized.');
      return;
    }
    this._transport.notify('ui/toolInput', params);
  }

  /**
   * Send a tool result notification to the widget.
   *
   * @param result - The tool call result
   */
  sendToolResult(result: CallToolResult): void {
    if (!this._transport) {
      console.warn('[AppBridge] Not initialized.');
      return;
    }
    this._transport.notify('ui/toolResult', result as unknown as Record<string, unknown>);
  }

  /**
   * Send a host context changed notification to the widget.
   *
   * @param ctx - The updated host context
   */
  sendHostContextChanged(ctx: HostContext): void {
    this._hostContext = ctx;
    if (!this._transport) {
      console.warn('[AppBridge] Not initialized.');
      return;
    }
    this._transport.notify('ui/hostContextChanged', ctx as unknown as Record<string, unknown>);
  }

  /**
   * Send a teardown notification to the widget.
   */
  sendTeardown(): void {
    if (!this._transport) {
      return;
    }
    this._transport.notify('ui/teardown');
  }

  /**
   * Whether the bridge has been initialized.
   */
  get initialized(): boolean {
    return this._initialized;
  }

  /**
   * The current host context.
   */
  get hostContext(): HostContext {
    return this._hostContext;
  }

  /**
   * Clean up event listeners and transport.
   */
  destroy(): void {
    this.sendTeardown();
    if (this._transport) {
      this._transport.destroy();
      this._transport = null;
    }
    this._initialized = false;
  }

  // ===========================================================================
  // Private
  // ===========================================================================

  private async _handleRequest(method: string, params?: Record<string, unknown>): Promise<unknown> {
    switch (method) {
      case 'ui/initialize':
        return this._hostContext;

      case 'tools/call': {
        const name = params?.name as string | undefined;
        const args = params?.arguments as Record<string, unknown> | undefined;
        if (!name) {
          throw new Error('tools/call requires a "name" parameter');
        }
        return await this._toolCallHandler(name, args);
      }

      case 'ui/sendMessage':
        // Host can handle this or ignore it
        console.log('[AppBridge] Widget sent message:', params);
        return {};

      case 'ui/openLink': {
        const url = params?.url as string | undefined;
        if (url) {
          window.open(url, '_blank', 'noopener,noreferrer');
        }
        return {};
      }

      default:
        throw new Error(`Unknown method: ${method}`);
    }
  }
}
