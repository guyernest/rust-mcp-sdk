//! Main page handler

use axum::response::Html;

/// Serve the main preview page
pub async fn index() -> Html<&'static str> {
    Html(include_str!("../../assets/index.html"))
}
