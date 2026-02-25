//! WASM bridge API handlers
//!
//! Provides endpoints for triggering WASM builds, querying build status,
//! and serving compiled WASM artifacts with correct MIME types.

use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::IntoResponse,
    Json,
};
use serde_json::json;
use std::sync::Arc;

use crate::server::AppState;

/// Trigger a WASM build (or return cached result if already built).
///
/// **POST /api/wasm/build**
///
/// Returns JSON with the current build status:
/// - `{"status": "ready"}` on success
/// - `{"status": "error", "message": "..."}` on failure
pub async fn trigger_build(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.wasm_builder.ensure_built().await {
        Ok(_) => Json(json!({"status": "ready"})),
        Err(msg) => Json(json!({"status": "error", "message": msg})),
    }
}

/// Query the current WASM build status without triggering a build.
///
/// **GET /api/wasm/status**
///
/// Returns JSON: `{"status": "not_built"|"building"|"ready"|"failed: ..."}`
pub async fn build_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let status = state.wasm_builder.status().await;
    Json(json!({"status": status}))
}

/// Serve a compiled WASM artifact by filename.
///
/// **GET /wasm/:filename**
///
/// Sets `Content-Type` based on the file extension:
/// - `.wasm` -> `application/wasm` (required for streaming compilation)
/// - `.js`   -> `application/javascript`
/// - `.d.ts` -> `application/typescript`
///
/// Returns 404 if the WASM build is not ready or the file does not exist.
pub async fn serve_artifact(
    State(state): State<Arc<AppState>>,
    Path(filename): Path<String>,
) -> impl IntoResponse {
    let artifact_dir = match state.wasm_builder.artifact_dir().await {
        Some(dir) => dir,
        None => {
            return (
                StatusCode::NOT_FOUND,
                "WASM build not ready. Trigger a build first via POST /api/wasm/build",
            )
                .into_response();
        }
    };

    let file_path = artifact_dir.join(&filename);

    // Prevent path traversal
    if !file_path.starts_with(&artifact_dir) {
        return (StatusCode::BAD_REQUEST, "Invalid filename").into_response();
    }

    match tokio::fs::read(&file_path).await {
        Ok(contents) => {
            let content_type = mime_for_extension(&filename);
            ([(header::CONTENT_TYPE, content_type)], contents).into_response()
        }
        Err(_) => (
            StatusCode::NOT_FOUND,
            format!("Artifact not found: {filename}"),
        )
            .into_response(),
    }
}

/// Determine the MIME type for a WASM artifact based on file extension.
fn mime_for_extension(filename: &str) -> &'static str {
    if filename.ends_with(".wasm") {
        "application/wasm"
    } else if filename.ends_with(".d.ts") {
        "application/typescript"
    } else if filename.ends_with(".js") {
        "application/javascript"
    } else {
        "application/octet-stream"
    }
}
