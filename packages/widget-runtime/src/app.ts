/**
 * App class -- Widget-side MCP Apps protocol client
 *
 * Aligns with the `@modelcontextprotocol/ext-apps` App class API surface.
 * Widgets use this to communicate with the host via postMessage JSON-RPC.
 */

import { PostMessageTransport } from './transport';
import type { CallToolParams, CallToolResult, HostContext, AppOptions } from './types';

// =============================================================================
// Lifecycle callback types
// =============================================================================

type ToolInputCallback = (params: Record<string, unknown>) => void;
type ToolResultCallback = (result: CallToolResult) => void;
type ToolCancelledCallback = () => void;
type HostContextChangedCallback = (ctx: HostContext) => void;
type TeardownCallback = () => void;

// =============================================================================
// App Class
// =============================================================================

/**
 * MCP Apps protocol-aligned widget-side client.
 *
 * Manages the postMessage transport to the host and provides methods for
 * calling server tools, sending messages, opening links, and logging.
 *
 * @example
 * ```typescript
 * import { App } from '@pmcp/widget-runtime';
 *
 * const app = new App({ name: 'chess-widget', version: '1.0.0' });
 * await app.connect();
 *
 * const result = await app.callServerTool({
 *   name: 'chess_move',
 *   arguments: { from: 'e2', to: 'e4' },
 * });
 * ```
 */
export class App {
  private _name: string;
  private _version: string;
  private _transport: PostMessageTransport | null = null;
  private _hostContext: HostContext | undefined = undefined;
  private _connected = false;

  // Lifecycle callbacks (setter-based, matching MCP Apps spec)
  ontoolinput: ToolInputCallback | null = null;
  ontoolresult: ToolResultCallback | null = null;
  ontoolcancelled: ToolCancelledCallback | null = null;
  onhostcontextchanged: HostContextChangedCallback | null = null;
  onteardown: TeardownCallback | null = null;

  constructor(options: AppOptions) {
    this._name = options.name;
    this._version = options.version;
  }

  /**
   * Connect to the host by creating a PostMessageTransport to window.parent
   * and sending the `ui/initialize` handshake.
   *
   * Gracefully degrades: if connect() times out (2 seconds), logs a warning
   * and resolves anyway to allow standalone usage without a host.
   */
  async connect(): Promise<void> {
    if (this._connected) {
      return;
    }

    if (typeof window === 'undefined') {
      console.warn('[App] No window object -- running outside browser. Skipping connect.');
      return;
    }

    const targetOrigin = this._resolveTargetOrigin();

    this._transport = new PostMessageTransport({
      targetWindow: window.parent,
      targetOrigin,
      timeout: 30000,
    });

    // Listen for incoming notifications from host
    this._transport.onNotification((method, params) => {
      this._handleNotification(method, params);
    });

    // Attempt initialization handshake with a 2s timeout
    try {
      const result = await Promise.race([
        this._transport.send('ui/initialize', {
          name: this._name,
          version: this._version,
        }),
        new Promise<null>((resolve) => setTimeout(() => resolve(null), 2000)),
      ]);

      if (result && typeof result === 'object') {
        this._hostContext = result as HostContext;
      } else {
        console.warn(
          '[App] Host did not respond to ui/initialize within 2s. ' +
          'Running in standalone mode (no host bridge).'
        );
      }
    } catch (err) {
      console.warn('[App] ui/initialize failed:', err);
    }

    this._connected = true;
  }

  /**
   * Call a server-side MCP tool through the host bridge.
   *
   * @param params - Tool name and optional arguments
   * @returns The tool call result
   */
  async callServerTool(params: CallToolParams): Promise<CallToolResult> {
    if (!this._transport) {
      console.warn('[App] Not connected. Call connect() first.');
      return { content: [], isError: true };
    }

    try {
      const result = await this._transport.send('tools/call', {
        name: params.name,
        arguments: params.arguments,
      });
      return (result as CallToolResult) ?? { content: [] };
    } catch (err) {
      console.error('[App] callServerTool failed:', err);
      return { content: [], isError: true };
    }
  }

  /**
   * Send a follow-up message to the AI conversation.
   *
   * @param params - Message parameters
   */
  async sendMessage(params: { message: string }): Promise<void> {
    if (!this._transport) {
      console.warn('[App] Not connected. Call connect() first.');
      return;
    }

    try {
      await this._transport.send('ui/sendMessage', params);
    } catch (err) {
      console.warn('[App] sendMessage not supported by host:', err);
    }
  }

  /**
   * Open an external URL via the host.
   *
   * @param params - URL parameters
   */
  async openLink(params: { url: string }): Promise<void> {
    if (!this._transport) {
      console.warn('[App] Not connected. Call connect() first.');
      return;
    }

    try {
      await this._transport.send('ui/openLink', params);
    } catch (err) {
      console.warn('[App] openLink not supported by host:', err);
    }
  }

  /**
   * Send a log entry to the host for debugging.
   *
   * @param params - Log parameters
   */
  sendLog(params: { level: string; message: string; data?: unknown }): void {
    if (!this._transport) {
      return;
    }

    // Notifications are fire-and-forget
    this._transport.notify('ui/log', params as Record<string, unknown>);
  }

  /**
   * Get the host context received during initialization.
   *
   * @returns The host context or undefined if not yet initialized
   */
  getHostContext(): HostContext | undefined {
    return this._hostContext;
  }

  /**
   * Whether the App is connected to a host.
   */
  get connected(): boolean {
    return this._connected;
  }

  /**
   * The app name.
   */
  get name(): string {
    return this._name;
  }

  /**
   * The app version.
   */
  get version(): string {
    return this._version;
  }

  /**
   * Disconnect from the host and clean up resources.
   */
  destroy(): void {
    if (this.onteardown) {
      this.onteardown();
    }
    if (this._transport) {
      this._transport.destroy();
      this._transport = null;
    }
    this._connected = false;
    this._hostContext = undefined;
  }

  // ===========================================================================
  // Private
  // ===========================================================================

  private _resolveTargetOrigin(): string {
    // In an iframe, the parent's origin is what we need to validate.
    // Use document.referrer to extract origin when available, otherwise
    // fall back to '*' for local development (preview server).
    try {
      if (document.referrer) {
        const url = new URL(document.referrer);
        return url.origin;
      }
    } catch {
      // Ignore URL parsing errors
    }

    // Fallback: same origin or wildcard for dev
    return window.location.origin;
  }

  private _handleNotification(method: string, params?: Record<string, unknown>): void {
    switch (method) {
      case 'ui/toolInput':
        if (this.ontoolinput && params) {
          this.ontoolinput(params);
        }
        break;
      case 'ui/toolResult':
        if (this.ontoolresult && params) {
          this.ontoolresult(params as unknown as CallToolResult);
        }
        break;
      case 'ui/toolCancelled':
        if (this.ontoolcancelled) {
          this.ontoolcancelled();
        }
        break;
      case 'ui/hostContextChanged':
        if (params) {
          this._hostContext = params as unknown as HostContext;
          if (this.onhostcontextchanged) {
            this.onhostcontextchanged(this._hostContext);
          }
        }
        break;
      case 'ui/teardown':
        this.destroy();
        break;
    }
  }
}
