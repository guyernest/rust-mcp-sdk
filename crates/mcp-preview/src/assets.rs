//! Embedded static assets

use rust_embed::Embed;

/// Embedded assets for the preview UI
#[derive(Embed)]
#[folder = "assets/"]
pub struct Assets;
