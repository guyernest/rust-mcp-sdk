/**
 * Utility functions for MCP widgets.
 */

import type { HostType, McpBridge } from './types';

/**
 * Detect the current host platform.
 *
 * @returns The detected host type
 *
 * @example
 * ```typescript
 * const host = detectHost();
 * if (host === 'chatgpt') {
 *   // ChatGPT-specific logic
 * }
 * ```
 */
export function detectHost(): HostType {
  if (typeof window === 'undefined') {
    return 'unknown';
  }

  if (window.openai) {
    return 'chatgpt';
  }

  if (window.mcpBridge) {
    if (window.mcpBridge.sendIntent) {
      return 'mcp-ui';
    }
    return 'mcp-apps';
  }

  return 'unknown';
}

/**
 * Check if running in a widget environment.
 *
 * @returns True if running in an MCP widget
 */
export function isWidget(): boolean {
  return detectHost() !== 'unknown';
}

/**
 * Check if running in ChatGPT Apps.
 *
 * @returns True if running in ChatGPT
 */
export function isChatGPT(): boolean {
  return detectHost() === 'chatgpt';
}

/**
 * Check if running in MCP Apps.
 *
 * @returns True if running in MCP Apps
 */
export function isMcpApps(): boolean {
  return detectHost() === 'mcp-apps';
}

/**
 * Check if running in MCP-UI.
 *
 * @returns True if running in MCP-UI
 */
export function isMcpUI(): boolean {
  return detectHost() === 'mcp-ui';
}

/**
 * Get the bridge instance if available.
 *
 * @returns The bridge or undefined
 */
export function getBridge(): McpBridge | undefined {
  if (typeof window === 'undefined') {
    return undefined;
  }
  return window.mcpBridge;
}

/**
 * Wait for the bridge to be available.
 *
 * @param timeout - Maximum time to wait in ms (default: 5000)
 * @returns Promise that resolves to the bridge or null
 *
 * @example
 * ```typescript
 * const bridge = await waitForBridge();
 * if (bridge) {
 *   const result = await bridge.callTool('my_tool', {});
 * }
 * ```
 */
export async function waitForBridge(timeout = 5000): Promise<McpBridge | null> {
  if (typeof window === 'undefined') {
    return null;
  }

  // Already available
  if (window.mcpBridge) {
    return window.mcpBridge;
  }

  // Wait for it
  return new Promise((resolve) => {
    const startTime = Date.now();

    const check = () => {
      if (window.mcpBridge) {
        resolve(window.mcpBridge);
        return;
      }

      if (Date.now() - startTime > timeout) {
        resolve(null);
        return;
      }

      requestAnimationFrame(check);
    };

    requestAnimationFrame(check);
  });
}

/**
 * Create a unique message ID for correlation.
 *
 * @returns Unique message ID
 */
export function createMessageId(): string {
  return `msg_${Date.now()}_${Math.random().toString(36).substring(2, 9)}`;
}

/**
 * Serialize state for transport.
 *
 * Ensures state is serializable and within size limits.
 *
 * @param state - State object
 * @param maxSize - Maximum size in bytes (default: 100KB)
 * @returns Serialized state or null if too large
 */
export function serializeState(
  state: Record<string, unknown>,
  maxSize = 100 * 1024
): string | null {
  try {
    const json = JSON.stringify(state);
    if (json.length > maxSize) {
      console.warn(`State too large: ${json.length} bytes (max: ${maxSize})`);
      return null;
    }
    return json;
  } catch (error) {
    console.error('Failed to serialize state:', error);
    return null;
  }
}

/**
 * Deep merge two state objects.
 *
 * @param target - Target state
 * @param source - Source state to merge
 * @returns Merged state
 */
export function mergeState<T extends Record<string, unknown>>(
  target: T,
  source: Partial<T>
): T {
  const result = { ...target };

  for (const key of Object.keys(source) as Array<keyof T>) {
    const sourceValue = source[key];
    const targetValue = target[key];

    if (
      sourceValue !== null &&
      typeof sourceValue === 'object' &&
      !Array.isArray(sourceValue) &&
      targetValue !== null &&
      typeof targetValue === 'object' &&
      !Array.isArray(targetValue)
    ) {
      result[key] = mergeState(
        targetValue as Record<string, unknown>,
        sourceValue as Record<string, unknown>
      ) as T[keyof T];
    } else {
      result[key] = sourceValue as T[keyof T];
    }
  }

  return result;
}

/**
 * Debounce a function.
 *
 * @param fn - Function to debounce
 * @param delay - Delay in ms
 * @returns Debounced function
 */
export function debounce<T extends (...args: unknown[]) => unknown>(
  fn: T,
  delay: number
): (...args: Parameters<T>) => void {
  let timeoutId: ReturnType<typeof setTimeout> | null = null;

  return (...args: Parameters<T>) => {
    if (timeoutId) {
      clearTimeout(timeoutId);
    }
    timeoutId = setTimeout(() => {
      fn(...args);
      timeoutId = null;
    }, delay);
  };
}

/**
 * Throttle a function.
 *
 * @param fn - Function to throttle
 * @param limit - Minimum time between calls in ms
 * @returns Throttled function
 */
export function throttle<T extends (...args: unknown[]) => unknown>(
  fn: T,
  limit: number
): (...args: Parameters<T>) => void {
  let inThrottle = false;

  return (...args: Parameters<T>) => {
    if (!inThrottle) {
      fn(...args);
      inThrottle = true;
      setTimeout(() => {
        inThrottle = false;
      }, limit);
    }
  };
}

/**
 * Log a message with widget context.
 *
 * @param level - Log level
 * @param message - Log message
 * @param data - Additional data
 */
export function log(
  level: 'debug' | 'info' | 'warn' | 'error',
  message: string,
  data?: unknown
): void {
  const host = detectHost();
  const prefix = `[Widget:${host}]`;

  const logFn = console[level] || console.log;
  if (data !== undefined) {
    logFn(prefix, message, data);
  } else {
    logFn(prefix, message);
  }
}
