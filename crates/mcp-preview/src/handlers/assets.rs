//! Static assets handler

use axum::{
    extract::Path,
    http::{header, StatusCode},
    response::IntoResponse,
};

use crate::assets::Assets;

/// Serve embedded static assets
pub async fn serve(Path(path): Path<String>) -> impl IntoResponse {
    let path = path.trim_start_matches('/');

    match Assets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, mime.as_ref())],
                content.data.into_owned(),
            )
                .into_response()
        },
        None => (StatusCode::NOT_FOUND, "Not found").into_response(),
    }
}
