/**
 * React Hooks for MCP Widget Runtime
 *
 * Provides React hooks for easy integration with the widget runtime.
 * These hooks work with any React-based widget framework.
 */

import { useCallback, useEffect, useRef, useState, useSyncExternalStore } from 'react';
import { WidgetRuntime } from './runtime';
import type {
  WidgetState,
  ToolResult,
  CallToolOptions,
  HostInfo,
  OpenAiGlobals,
  Theme,
  DisplayMode,
  SafeArea,
  WidgetView,
} from './types';
import { SET_GLOBALS_EVENT_TYPE } from './types';

// Singleton runtime instance
let globalRuntime: WidgetRuntime | null = null;

function getRuntime(): WidgetRuntime {
  if (!globalRuntime) {
    globalRuntime = new WidgetRuntime();
  }
  return globalRuntime;
}

/**
 * Hook to access the widget runtime.
 *
 * @returns The widget runtime instance
 *
 * @example
 * ```tsx
 * function MyWidget() {
 *   const runtime = useWidgetRuntime();
 *
 *   const handleClick = async () => {
 *     const result = await runtime.callTool('my_tool', { arg: 'value' });
 *     console.log(result);
 *   };
 *
 *   return <button onClick={handleClick}>Call Tool</button>;
 * }
 * ```
 */
export function useWidgetRuntime(): WidgetRuntime {
  return getRuntime();
}

/**
 * Hook to track runtime ready state.
 *
 * @returns Whether the runtime is ready
 *
 * @example
 * ```tsx
 * function MyWidget() {
 *   const isReady = useWidgetReady();
 *
 *   if (!isReady) {
 *     return <div>Loading...</div>;
 *   }
 *
 *   return <div>Widget is ready!</div>;
 * }
 * ```
 */
export function useWidgetReady(): boolean {
  const runtime = getRuntime();
  const [isReady, setIsReady] = useState(runtime.isReady);

  useEffect(() => {
    if (runtime.isReady) {
      setIsReady(true);
      return;
    }

    runtime.ready().then(() => setIsReady(true));
  }, [runtime]);

  return isReady;
}

/**
 * Hook to get host platform information.
 *
 * @returns Host information
 *
 * @example
 * ```tsx
 * function MyWidget() {
 *   const host = useWidgetHost();
 *
 *   return (
 *     <div>
 *       <p>Running on: {host.name}</p>
 *       <p>Can call tools: {host.capabilities.callTool ? 'Yes' : 'No'}</p>
 *     </div>
 *   );
 * }
 * ```
 */
export function useWidgetHost(): HostInfo {
  const runtime = getRuntime();
  const [host, setHost] = useState<HostInfo>(runtime.host);

  useEffect(() => {
    const update = () => setHost(runtime.host);

    // Update when ready
    runtime.ready().then(update);

    return runtime.on('ready', update);
  }, [runtime]);

  return host;
}

/**
 * Hook for managing widget state with React.
 *
 * @param initialState - Initial state (used only if no state exists)
 * @returns [state, setState] tuple
 *
 * @example
 * ```tsx
 * function ChessWidget() {
 *   const [state, setState] = useWidgetState({
 *     turn: 'white',
 *     board: initialBoard,
 *   });
 *
 *   const makeMove = (move: Move) => {
 *     setState({
 *       ...state,
 *       turn: state.turn === 'white' ? 'black' : 'white',
 *       board: applyMove(state.board, move),
 *     });
 *   };
 *
 *   return <Board board={state.board} onMove={makeMove} />;
 * }
 * ```
 */
export function useWidgetState<T extends WidgetState>(
  initialState: T
): [T, (newState: Partial<T>) => void] {
  const runtime = getRuntime();

  // Get initial state, preferring existing runtime state
  const getInitialState = useCallback((): T => {
    const existing = runtime.getState();
    if (Object.keys(existing).length > 0) {
      return existing as T;
    }
    // Initialize with provided initial state
    runtime.setState(initialState);
    return initialState;
  }, [runtime, initialState]);

  const [state, setLocalState] = useState<T>(getInitialState);

  // Subscribe to state updates
  useEffect(() => {
    return runtime.on('stateUpdate', (newState) => {
      setLocalState(newState as T);
    });
  }, [runtime]);

  // Update both local and runtime state
  const setState = useCallback(
    (newState: Partial<T>) => {
      setLocalState((prev: T) => {
        const merged = { ...prev, ...newState };
        runtime.setState(merged);
        return merged;
      });
    },
    [runtime]
  );

  return [state, setState];
}

/**
 * Hook for calling MCP tools with loading and error states.
 *
 * @param toolName - Tool name
 * @param options - Call options
 * @returns Object with call function, loading state, error, and result
 *
 * @example
 * ```tsx
 * function WeatherWidget() {
 *   const { call, loading, error, result } = useToolCall<WeatherData>('get_weather');
 *
 *   const handleSubmit = (city: string) => {
 *     call({ city });
 *   };
 *
 *   if (loading) return <Spinner />;
 *   if (error) return <Error message={error} />;
 *   if (result) return <WeatherDisplay data={result} />;
 *
 *   return <CityInput onSubmit={handleSubmit} />;
 * }
 * ```
 */
export function useToolCall<T = unknown>(
  toolName: string,
  options?: CallToolOptions
): {
  call: (args?: Record<string, unknown>) => Promise<ToolResult<T>>;
  loading: boolean;
  error: string | null;
  result: T | null;
  reset: () => void;
} {
  const runtime = getRuntime();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<T | null>(null);

  const call = useCallback(
    async (args?: Record<string, unknown>): Promise<ToolResult<T>> => {
      setLoading(true);
      setError(null);

      const toolResult = await runtime.callTool<T>(toolName, args, options);

      setLoading(false);

      if (toolResult.success && toolResult.data !== undefined) {
        setResult(toolResult.data);
      } else if (!toolResult.success && toolResult.error) {
        setError(toolResult.error);
      }

      return toolResult;
    },
    [runtime, toolName, options]
  );

  const reset = useCallback(() => {
    setLoading(false);
    setError(null);
    setResult(null);
  }, []);

  return { call, loading, error, result, reset };
}

/**
 * Hook for lazy tool calls (call on demand with caching).
 *
 * @param toolName - Tool name
 * @returns Object with fetch function and cached result
 *
 * @example
 * ```tsx
 * function ConfigWidget() {
 *   const { fetch, data, loading } = useLazyToolCall<Config>('get_config');
 *
 *   useEffect(() => {
 *     fetch(); // Fetch once on mount
 *   }, []);
 *
 *   if (loading || !data) return <Spinner />;
 *   return <ConfigDisplay config={data} />;
 * }
 * ```
 */
export function useLazyToolCall<T = unknown>(
  toolName: string
): {
  fetch: (args?: Record<string, unknown>) => Promise<T | null>;
  data: T | null;
  loading: boolean;
  error: string | null;
} {
  const runtime = getRuntime();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [data, setData] = useState<T | null>(null);

  const fetch = useCallback(
    async (args?: Record<string, unknown>): Promise<T | null> => {
      setLoading(true);
      setError(null);

      const result = await runtime.callTool<T>(toolName, args);

      setLoading(false);

      if (result.success && result.data !== undefined) {
        setData(result.data);
        return result.data;
      } else if (!result.success && result.error) {
        setError(result.error);
      }

      return null;
    },
    [runtime, toolName]
  );

  return { fetch, data, loading, error };
}

/**
 * Hook for subscribing to MCP notifications.
 *
 * @param handler - Notification handler
 *
 * @example
 * ```tsx
 * function NotificationListener() {
 *   useWidgetNotification((notification) => {
 *     console.log('Received:', notification.method, notification.params);
 *   });
 *
 *   return null;
 * }
 * ```
 */
export function useWidgetNotification(
  handler: (notification: { method: string; params?: unknown }) => void
): void {
  const runtime = getRuntime();
  const handlerRef = useRef(handler);
  handlerRef.current = handler;

  useEffect(() => {
    return runtime.on('notification', (notification) => {
      handlerRef.current(notification);
    });
  }, [runtime]);
}

// =============================================================================
// OpenAI Global Hooks (ChatGPT)
// =============================================================================

/**
 * Hook for reactive subscription to OpenAI globals.
 *
 * This follows the pattern from OpenAI's Apps SDK documentation.
 * Use this when you need to reactively update when globals change.
 *
 * @param key - The global key to subscribe to
 * @returns The current value of the global
 *
 * @example
 * ```tsx
 * function ThemeAwareWidget() {
 *   const theme = useOpenAiGlobal('theme');
 *   const locale = useOpenAiGlobal('locale');
 *
 *   return (
 *     <div className={`theme-${theme}`}>
 *       <p>Locale: {locale}</p>
 *     </div>
 *   );
 * }
 * ```
 */
export function useOpenAiGlobal<K extends keyof OpenAiGlobals>(
  key: K
): OpenAiGlobals[K] | undefined {
  const runtime = getRuntime();

  const subscribe = useCallback(
    (onChange: () => void) => {
      const handleSetGlobal = (event: Event) => {
        const customEvent = event as CustomEvent<{ globals: Partial<OpenAiGlobals> }>;
        const value = customEvent.detail?.globals?.[key];
        if (value !== undefined) {
          onChange();
        }
      };

      window.addEventListener(SET_GLOBALS_EVENT_TYPE, handleSetGlobal, { passive: true });
      return () => {
        window.removeEventListener(SET_GLOBALS_EVENT_TYPE, handleSetGlobal);
      };
    },
    [key]
  );

  const getSnapshot = useCallback((): OpenAiGlobals[K] | undefined => {
    // Access the value based on the key
    switch (key) {
      case 'toolInput':
        return runtime.getToolInput() as OpenAiGlobals[K];
      case 'toolOutput':
        return runtime.getToolOutput() as OpenAiGlobals[K];
      case 'toolResponseMetadata':
        return runtime.getToolResponseMetadata() as OpenAiGlobals[K];
      case 'widgetState':
        return runtime.getState() as OpenAiGlobals[K];
      case 'theme':
        return runtime.theme as OpenAiGlobals[K];
      case 'displayMode':
        return runtime.currentDisplayMode as OpenAiGlobals[K];
      case 'maxHeight':
        return runtime.maxHeight as OpenAiGlobals[K];
      case 'safeArea':
        return runtime.safeArea as OpenAiGlobals[K];
      case 'view':
        return runtime.view as OpenAiGlobals[K];
      case 'userAgent':
        return runtime.userAgent as OpenAiGlobals[K];
      case 'locale':
        return runtime.locale as OpenAiGlobals[K];
      default:
        return undefined;
    }
  }, [runtime, key]);

  return useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
}

// =============================================================================
// Tool Context Hooks (ChatGPT)
// =============================================================================

/**
 * Hook to get the tool input arguments.
 *
 * @returns The arguments passed when the tool was invoked
 *
 * @example
 * ```tsx
 * function MyWidget() {
 *   const input = useToolInput();
 *   return <div>Called with: {JSON.stringify(input)}</div>;
 * }
 * ```
 */
export function useToolInput(): Record<string, unknown> {
  const runtime = getRuntime();
  const [input, setInput] = useState<Record<string, unknown>>(() => runtime.getToolInput());

  useEffect(() => {
    const handleUpdate = () => setInput(runtime.getToolInput());
    window.addEventListener(SET_GLOBALS_EVENT_TYPE, handleUpdate, { passive: true });
    return () => window.removeEventListener(SET_GLOBALS_EVENT_TYPE, handleUpdate);
  }, [runtime]);

  return input;
}

/**
 * Hook to get the tool output (structuredContent).
 *
 * @returns The structured content returned by the tool
 *
 * @example
 * ```tsx
 * function DataWidget() {
 *   const output = useToolOutput<MyDataType>();
 *   if (!output) return <Loading />;
 *   return <DataDisplay data={output} />;
 * }
 * ```
 */
export function useToolOutput<T = unknown>(): T | undefined {
  const runtime = getRuntime();
  const [output, setOutput] = useState<T | undefined>(() => runtime.getToolOutput() as T);

  useEffect(() => {
    const handleUpdate = () => setOutput(runtime.getToolOutput() as T);
    window.addEventListener(SET_GLOBALS_EVENT_TYPE, handleUpdate, { passive: true });
    return () => window.removeEventListener(SET_GLOBALS_EVENT_TYPE, handleUpdate);
  }, [runtime]);

  return output;
}

/**
 * Hook to get the tool response metadata.
 *
 * @returns The _meta payload (widget-only, never sent to model)
 *
 * @example
 * ```tsx
 * function SessionAwareWidget() {
 *   const meta = useToolResponseMetadata();
 *   const sessionId = meta['openai/widgetSessionId'];
 *   return <div>Session: {sessionId}</div>;
 * }
 * ```
 */
export function useToolResponseMetadata(): Record<string, unknown> {
  const runtime = getRuntime();
  const [meta, setMeta] = useState<Record<string, unknown>>(() =>
    runtime.getToolResponseMetadata()
  );

  useEffect(() => {
    const handleUpdate = () => setMeta(runtime.getToolResponseMetadata());
    window.addEventListener(SET_GLOBALS_EVENT_TYPE, handleUpdate, { passive: true });
    return () => window.removeEventListener(SET_GLOBALS_EVENT_TYPE, handleUpdate);
  }, [runtime]);

  return meta;
}

// =============================================================================
// Environment Hooks (ChatGPT)
// =============================================================================

/**
 * Hook to get the current theme.
 *
 * @returns 'light' or 'dark'
 *
 * @example
 * ```tsx
 * function ThemeWidget() {
 *   const theme = useTheme();
 *   return <div className={theme === 'dark' ? 'dark-mode' : 'light-mode'}>...</div>;
 * }
 * ```
 */
export function useTheme(): Theme {
  const runtime = getRuntime();
  const [theme, setTheme] = useState<Theme>(() => runtime.theme);

  useEffect(() => {
    const handleUpdate = () => setTheme(runtime.theme);
    window.addEventListener(SET_GLOBALS_EVENT_TYPE, handleUpdate, { passive: true });
    return () => window.removeEventListener(SET_GLOBALS_EVENT_TYPE, handleUpdate);
  }, [runtime]);

  return theme;
}

/**
 * Hook to get the current locale.
 *
 * @returns Locale string (e.g., 'en-US')
 *
 * @example
 * ```tsx
 * function LocalizedWidget() {
 *   const locale = useLocale();
 *   const formatter = new Intl.NumberFormat(locale);
 *   return <div>{formatter.format(12345.67)}</div>;
 * }
 * ```
 */
export function useLocale(): string {
  const runtime = getRuntime();
  const [locale, setLocale] = useState<string>(() => runtime.locale);

  useEffect(() => {
    const handleUpdate = () => setLocale(runtime.locale);
    window.addEventListener(SET_GLOBALS_EVENT_TYPE, handleUpdate, { passive: true });
    return () => window.removeEventListener(SET_GLOBALS_EVENT_TYPE, handleUpdate);
  }, [runtime]);

  return locale;
}

/**
 * Hook to get the current display mode.
 *
 * @returns 'inline', 'pip', or 'fullscreen'
 *
 * @example
 * ```tsx
 * function ResponsiveWidget() {
 *   const displayMode = useDisplayMode();
 *   return displayMode === 'fullscreen' ? <FullView /> : <CompactView />;
 * }
 * ```
 */
export function useDisplayMode(): DisplayMode {
  const runtime = getRuntime();
  const [mode, setMode] = useState<DisplayMode>(() => runtime.currentDisplayMode);

  useEffect(() => {
    const handleUpdate = () => setMode(runtime.currentDisplayMode);
    window.addEventListener(SET_GLOBALS_EVENT_TYPE, handleUpdate, { passive: true });
    return () => window.removeEventListener(SET_GLOBALS_EVENT_TYPE, handleUpdate);
  }, [runtime]);

  return mode;
}

/**
 * Hook to get the maximum height.
 *
 * @returns Maximum height in pixels, or undefined
 */
export function useMaxHeight(): number | undefined {
  const runtime = getRuntime();
  const [height, setHeight] = useState<number | undefined>(() => runtime.maxHeight);

  useEffect(() => {
    const handleUpdate = () => setHeight(runtime.maxHeight);
    window.addEventListener(SET_GLOBALS_EVENT_TYPE, handleUpdate, { passive: true });
    return () => window.removeEventListener(SET_GLOBALS_EVENT_TYPE, handleUpdate);
  }, [runtime]);

  return height;
}

/**
 * Hook to get the safe area insets.
 *
 * @returns Safe area insets, or undefined
 */
export function useSafeArea(): SafeArea | undefined {
  const runtime = getRuntime();
  const [safeArea, setSafeArea] = useState<SafeArea | undefined>(() => runtime.safeArea);

  useEffect(() => {
    const handleUpdate = () => setSafeArea(runtime.safeArea);
    window.addEventListener(SET_GLOBALS_EVENT_TYPE, handleUpdate, { passive: true });
    return () => window.removeEventListener(SET_GLOBALS_EVENT_TYPE, handleUpdate);
  }, [runtime]);

  return safeArea;
}

/**
 * Hook to get the widget view type.
 *
 * @returns 'default' or 'compact'
 */
export function useWidgetView(): WidgetView {
  const runtime = getRuntime();
  const [view, setView] = useState<WidgetView>(() => runtime.view);

  useEffect(() => {
    const handleUpdate = () => setView(runtime.view);
    window.addEventListener(SET_GLOBALS_EVENT_TYPE, handleUpdate, { passive: true });
    return () => window.removeEventListener(SET_GLOBALS_EVENT_TYPE, handleUpdate);
  }, [runtime]);

  return view;
}

// =============================================================================
// File Upload Hook (ChatGPT)
// =============================================================================

/**
 * Hook for uploading files.
 *
 * @returns Object with upload function, loading state, error, and result
 *
 * @example
 * ```tsx
 * function ImageUploader() {
 *   const { upload, loading, error, fileId } = useFileUpload();
 *
 *   const handleChange = async (e: React.ChangeEvent<HTMLInputElement>) => {
 *     const file = e.target.files?.[0];
 *     if (file) {
 *       await upload(file);
 *     }
 *   };
 *
 *   return (
 *     <div>
 *       <input type="file" accept="image/*" onChange={handleChange} disabled={loading} />
 *       {error && <p className="error">{error}</p>}
 *       {fileId && <p>Uploaded: {fileId}</p>}
 *     </div>
 *   );
 * }
 * ```
 */
export function useFileUpload(): {
  upload: (file: File) => Promise<string | null>;
  loading: boolean;
  error: string | null;
  fileId: string | null;
  reset: () => void;
} {
  const runtime = getRuntime();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [fileId, setFileId] = useState<string | null>(null);

  const upload = useCallback(
    async (file: File): Promise<string | null> => {
      setLoading(true);
      setError(null);

      const result = await runtime.uploadFile(file);

      setLoading(false);

      if (result) {
        setFileId(result.fileId);
        return result.fileId;
      } else {
        setError('Failed to upload file');
        return null;
      }
    },
    [runtime]
  );

  const reset = useCallback(() => {
    setLoading(false);
    setError(null);
    setFileId(null);
  }, []);

  return { upload, loading, error, fileId, reset };
}

// =============================================================================
// Display Mode Hook (ChatGPT)
// =============================================================================

/**
 * Hook for requesting display mode changes.
 *
 * @returns Object with request function and current mode
 *
 * @example
 * ```tsx
 * function FullscreenToggle() {
 *   const { request, current } = useDisplayModeRequest();
 *
 *   const toggleFullscreen = () => {
 *     request(current === 'fullscreen' ? 'inline' : 'fullscreen');
 *   };
 *
 *   return (
 *     <button onClick={toggleFullscreen}>
 *       {current === 'fullscreen' ? 'Exit Fullscreen' : 'Go Fullscreen'}
 *     </button>
 *   );
 * }
 * ```
 */
export function useDisplayModeRequest(): {
  request: (mode: DisplayMode) => Promise<boolean>;
  current: DisplayMode;
  loading: boolean;
} {
  const runtime = getRuntime();
  const [loading, setLoading] = useState(false);
  const current = useDisplayMode();

  const request = useCallback(
    async (mode: DisplayMode): Promise<boolean> => {
      setLoading(true);
      const success = await runtime.requestDisplayMode(mode);
      setLoading(false);
      return success;
    },
    [runtime]
  );

  return { request, current, loading };
}
