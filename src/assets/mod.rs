//! Platform-agnostic asset loading for MCP servers.
//!
//! This module provides a unified API for loading assets (files, resources, databases)
//! that works across all deployment targets: local development, AWS Lambda,
//! Google Cloud Run, and Cloudflare Workers.
//!
//! # Overview
//!
//! Assets are files bundled with your MCP server deployment. Common use cases:
//! - SQLite databases
//! - Markdown resource files
//! - Configuration files
//! - Prompt templates
//! - Static data files
//!
//! # Configuration
//!
//! Assets are configured in `pmcp.toml`:
//!
//! ```toml
//! [deploy.assets]
//! include = [
//!     "chinook.db",
//!     "resources/**/*.md",
//!     "config/*.toml",
//! ]
//! exclude = ["**/*.tmp"]
//! ```
//!
//! # Usage
//!
//! ```rust,ignore
//! use pmcp::assets;
//!
//! // Load asset as bytes
//! let db_bytes = assets::load("chinook.db")?;
//!
//! // Load asset as string (UTF-8)
//! let guide = assets::load_string("resources/guide.md")?;
//!
//! // Get filesystem path (extracts to temp on serverless platforms)
//! let db_path = assets::path("chinook.db")?;
//! let conn = Connection::open(&db_path)?;
//!
//! // Check if asset exists
//! if assets::exists("config/override.toml") {
//!     let config = assets::load_string("config/override.toml")?;
//! }
//!
//! // List assets matching a pattern
//! for asset in assets::list("resources/**/*.md")? {
//!     println!("Found: {}", asset);
//! }
//! ```
//!
//! # Platform Behavior
//!
//! | Platform | Asset Location | `path()` Behavior |
//! |----------|----------------|-------------------|
//! | Local dev | Workspace root | Returns original path |
//! | AWS Lambda | `$LAMBDA_TASK_ROOT` | Returns path in package |
//! | Cloud Run | `/app/assets/` | Returns path in image |
//! | Cloudflare | Bundled/KV | Extracts to memory/temp |
//!
//! # Performance
//!
//! - Assets loaded via `load()` are cached in memory after first access
//! - The `path()` function extracts embedded assets to a temp directory once
//! - Use `load()` for small files, `path()` for large files or when a path is required

mod loader;

pub use loader::{
    exists, list, load, load_string, path, AssetConfig, AssetError, AssetLoader, Platform,
};

/// Result type for asset operations.
pub type Result<T> = std::result::Result<T, AssetError>;
