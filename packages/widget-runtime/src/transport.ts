/**
 * PostMessageTransport
 *
 * JSON-RPC 2.0 transport layer over window.postMessage for communication
 * between widget iframes and host windows.
 */

// =============================================================================
// JSON-RPC 2.0 Types
// =============================================================================

/**
 * JSON-RPC 2.0 request object.
 */
export interface JsonRpcRequest {
  jsonrpc: '2.0';
  id: number;
  method: string;
  params?: Record<string, unknown>;
}

/**
 * JSON-RPC 2.0 response object.
 */
export interface JsonRpcResponse {
  jsonrpc: '2.0';
  id: number;
  result?: unknown;
  error?: {
    code: number;
    message: string;
    data?: unknown;
  };
}

/**
 * JSON-RPC 2.0 notification (no id -- no response expected).
 */
export interface JsonRpcNotification {
  jsonrpc: '2.0';
  method: string;
  params?: Record<string, unknown>;
}

// =============================================================================
// Transport Options
// =============================================================================

/**
 * Configuration for PostMessageTransport.
 */
export interface TransportOptions {
  /** The window to send postMessage calls to (e.g., window.parent or iframe.contentWindow). */
  targetWindow: Window;
  /** The expected origin for incoming messages. Used for security validation. */
  targetOrigin: string;
  /** Request timeout in milliseconds. Defaults to 30000 (30 seconds). */
  timeout?: number;
}

// =============================================================================
// Internal Types
// =============================================================================

interface PendingRequest {
  resolve: (value: unknown) => void;
  reject: (reason: Error) => void;
  timer: ReturnType<typeof setTimeout>;
}

type NotificationHandler = (method: string, params?: Record<string, unknown>) => void;

// =============================================================================
// PostMessageTransport
// =============================================================================

/**
 * PostMessageTransport handles JSON-RPC 2.0 communication over postMessage.
 *
 * Supports both request/response patterns (with correlation IDs) and
 * one-way notifications. Validates message origins for security.
 */
export class PostMessageTransport {
  private _nextId = 1;
  private _pending: Map<number, PendingRequest> = new Map();
  private _targetWindow: Window;
  private _targetOrigin: string;
  private _timeout: number;
  private _messageHandler: ((event: MessageEvent) => void) | null = null;
  private _notificationHandler: NotificationHandler | null = null;
  private _requestHandler: ((method: string, params?: Record<string, unknown>) => Promise<unknown>) | null = null;

  constructor(options: TransportOptions) {
    this._targetWindow = options.targetWindow;
    this._targetOrigin = options.targetOrigin;
    this._timeout = options.timeout ?? 30000;

    this._messageHandler = this._handleMessage.bind(this);
    window.addEventListener('message', this._messageHandler);
  }

  /**
   * Send a JSON-RPC request and wait for the response.
   *
   * @param method - The RPC method name (e.g., 'tools/call')
   * @param params - Optional parameters for the method
   * @returns A promise that resolves with the result or rejects on error/timeout
   */
  send(method: string, params?: Record<string, unknown>): Promise<unknown> {
    const id = this._nextId++;
    const request: JsonRpcRequest = {
      jsonrpc: '2.0',
      id,
      method,
      params,
    };

    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this._pending.delete(id);
        reject(new Error(`JSON-RPC request '${method}' timed out after ${this._timeout}ms`));
      }, this._timeout);

      this._pending.set(id, { resolve, reject, timer });
      this._targetWindow.postMessage(request, this._targetOrigin);
    });
  }

  /**
   * Send a JSON-RPC response to an incoming request.
   *
   * @param id - The request ID to respond to
   * @param result - The result value (mutually exclusive with error)
   * @param error - The error value (mutually exclusive with result)
   */
  respond(id: number, result?: unknown, error?: { code: number; message: string; data?: unknown }): void {
    const response: JsonRpcResponse = {
      jsonrpc: '2.0',
      id,
    };

    if (error) {
      response.error = error;
    } else {
      response.result = result;
    }

    this._targetWindow.postMessage(response, this._targetOrigin);
  }

  /**
   * Send a JSON-RPC notification (fire-and-forget, no response expected).
   *
   * @param method - The notification method name
   * @param params - Optional notification parameters
   */
  notify(method: string, params?: Record<string, unknown>): void {
    const notification: JsonRpcNotification = {
      jsonrpc: '2.0',
      method,
      params,
    };
    this._targetWindow.postMessage(notification, this._targetOrigin);
  }

  /**
   * Set a handler for incoming JSON-RPC notifications (messages without an id).
   *
   * @param handler - Function called with the notification method and params
   */
  onNotification(handler: NotificationHandler): void {
    this._notificationHandler = handler;
  }

  /**
   * Set a handler for incoming JSON-RPC requests (messages with an id and method).
   * The handler should return a promise that resolves with the result.
   *
   * @param handler - Async function that processes the request and returns a result
   */
  onRequest(handler: (method: string, params?: Record<string, unknown>) => Promise<unknown>): void {
    this._requestHandler = handler;
  }

  /**
   * Clean up event listeners and reject all pending requests.
   */
  destroy(): void {
    if (this._messageHandler) {
      window.removeEventListener('message', this._messageHandler);
      this._messageHandler = null;
    }

    for (const [id, pending] of this._pending) {
      clearTimeout(pending.timer);
      pending.reject(new Error('Transport destroyed'));
      this._pending.delete(id);
    }

    this._notificationHandler = null;
    this._requestHandler = null;
  }

  // ===========================================================================
  // Private
  // ===========================================================================

  private _handleMessage(event: MessageEvent): void {
    // Origin validation: reject messages from unexpected origins
    if (event.origin !== this._targetOrigin) {
      return;
    }

    const data = event.data;
    if (!data || data.jsonrpc !== '2.0') {
      return;
    }

    // Response to one of our pending requests (has id, has result or error, no method)
    if (typeof data.id === 'number' && !data.method) {
      this._handleResponse(data as JsonRpcResponse);
      return;
    }

    // Incoming request (has id and method) -- route to request handler
    if (typeof data.id === 'number' && typeof data.method === 'string') {
      this._handleIncomingRequest(data as JsonRpcRequest);
      return;
    }

    // Notification (has method, no id)
    if (typeof data.method === 'string' && data.id === undefined) {
      if (this._notificationHandler) {
        this._notificationHandler(data.method, data.params);
      }
    }
  }

  private _handleResponse(response: JsonRpcResponse): void {
    const pending = this._pending.get(response.id);
    if (!pending) {
      return;
    }

    clearTimeout(pending.timer);
    this._pending.delete(response.id);

    if (response.error) {
      pending.reject(new Error(response.error.message));
    } else {
      pending.resolve(response.result);
    }
  }

  private _handleIncomingRequest(request: JsonRpcRequest): void {
    if (!this._requestHandler) {
      // Auto-respond with "method not found" if no handler is registered
      this.respond(request.id, undefined, {
        code: -32601,
        message: `Method not found: ${request.method}`,
      });
      return;
    }

    this._requestHandler(request.method, request.params)
      .then((result) => {
        this.respond(request.id, result);
      })
      .catch((err: unknown) => {
        const message = err instanceof Error ? err.message : String(err);
        this.respond(request.id, undefined, {
          code: -32000,
          message,
        });
      });
  }
}
