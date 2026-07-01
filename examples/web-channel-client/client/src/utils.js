// Helper to create a JS Error with optional structured `code` and `data`
// fields, used by the wasm `to_js_error` path to surface pmcp::Error details
// (JSON-RPC code + data) to the browser console / UI.
export function newError(message, code, data) {
  const err = new Error(message);
  if (code !== undefined && code !== null) err.code = code;
  if (data !== undefined && data !== null) err.data = data;
  return err;
}
