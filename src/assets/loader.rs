//! Asset loader implementation with platform detection.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};
use thiserror::Error;

/// Asset loading errors.
#[derive(Debug, Error)]
pub enum AssetError {
    /// Asset not found at the expected location.
    #[error("Asset not found: {0}")]
    NotFound(String),

    /// Failed to read asset from filesystem.
    #[error("Failed to read asset '{path}': {source}")]
    ReadError {
        /// The asset path that failed to read.
        path: String,
        /// The underlying IO error.
        #[source]
        source: std::io::Error,
    },

    /// Asset content is not valid UTF-8.
    #[error("Asset '{0}' is not valid UTF-8")]
    InvalidUtf8(String),

    /// Failed to extract asset to temporary location.
    #[error("Failed to extract asset '{path}': {source}")]
    ExtractionError {
        /// The asset path that failed to extract.
        path: String,
        /// The underlying IO error.
        #[source]
        source: std::io::Error,
    },

    /// Invalid glob pattern.
    #[error("Invalid glob pattern '{0}': {1}")]
    InvalidPattern(String, String),
}

/// Result type for asset operations.
pub(super) type Result<T> = std::result::Result<T, AssetError>;

/// Detected runtime platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    /// Local development environment.
    Local,
    /// AWS Lambda.
    Lambda,
    /// Google Cloud Run.
    CloudRun,
    /// Cloudflare Workers.
    CloudflareWorkers,
    /// Generic containerized environment.
    Container,
}

impl Platform {
    /// Detect the current platform from environment variables.
    pub fn detect() -> Self {
        // AWS Lambda
        if std::env::var("AWS_LAMBDA_FUNCTION_NAME").is_ok() {
            return Self::Lambda;
        }

        // Google Cloud Run
        if std::env::var("K_SERVICE").is_ok() || std::env::var("CLOUD_RUN_JOB").is_ok() {
            return Self::CloudRun;
        }

        // Cloudflare Workers (detected via special env or lack of filesystem)
        if std::env::var("CF_WORKER").is_ok() {
            return Self::CloudflareWorkers;
        }

        // Generic container detection
        if std::env::var("CONTAINER").is_ok()
            || Path::new("/.dockerenv").exists()
            || Path::new("/run/.containerenv").exists()
        {
            return Self::Container;
        }

        Self::Local
    }

    /// Get the base path for assets on this platform.
    pub fn assets_base_path(&self) -> PathBuf {
        match self {
            Self::Lambda => {
                // Lambda packages are extracted to LAMBDA_TASK_ROOT
                // Assets are placed in assets/ subdirectory
                std::env::var("LAMBDA_TASK_ROOT")
                    .map_or_else(|_| PathBuf::from("/var/task"), PathBuf::from)
                    .join("assets")
            },
            Self::CloudRun | Self::Container => {
                // Docker images typically use /app/assets
                PathBuf::from("/app/assets")
            },
            Self::CloudflareWorkers => {
                // Workers use in-memory or KV - temp path for extraction
                PathBuf::from("/tmp/assets")
            },
            Self::Local => {
                // Local dev uses workspace root or ASSETS_DIR env var
                std::env::var("PMCP_ASSETS_DIR").map_or_else(
                    |_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
                    PathBuf::from,
                )
            },
        }
    }

    /// Get the temp extraction path for this platform.
    pub fn temp_path(&self) -> PathBuf {
        match self {
            Self::Lambda => PathBuf::from("/tmp/pmcp-assets"),
            Self::CloudflareWorkers => PathBuf::from("/tmp/pmcp-assets"),
            _ => std::env::temp_dir().join("pmcp-assets"),
        }
    }
}

/// Asset configuration for deployment.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AssetConfig {
    /// Glob patterns for files to include.
    #[serde(default)]
    pub include: Vec<String>,

    /// Glob patterns for files to exclude.
    #[serde(default)]
    pub exclude: Vec<String>,

    /// Base directory for assets (relative to workspace root).
    #[serde(default)]
    pub base_dir: Option<String>,
}

impl AssetConfig {
    /// Create a new asset configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an include pattern.
    pub fn include(mut self, pattern: impl Into<String>) -> Self {
        self.include.push(pattern.into());
        self
    }

    /// Add an exclude pattern.
    pub fn exclude(mut self, pattern: impl Into<String>) -> Self {
        self.exclude.push(pattern.into());
        self
    }

    /// Set the base directory.
    pub fn base_dir(mut self, dir: impl Into<String>) -> Self {
        self.base_dir = Some(dir.into());
        self
    }
}

/// Global asset loader instance.
static LOADER: OnceLock<AssetLoader> = OnceLock::new();

/// Get or initialize the global asset loader.
fn get_loader() -> &'static AssetLoader {
    LOADER.get_or_init(AssetLoader::new)
}

/// Asset loader with caching and platform-aware loading.
pub struct AssetLoader {
    platform: Platform,
    base_path: PathBuf,
    temp_path: PathBuf,
    cache: Arc<RwLock<HashMap<String, Arc<Vec<u8>>>>>,
    extracted: Arc<RwLock<HashMap<String, PathBuf>>>,
}

impl std::fmt::Debug for AssetLoader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AssetLoader")
            .field("platform", &self.platform)
            .field("base_path", &self.base_path)
            .field("temp_path", &self.temp_path)
            .finish()
    }
}

impl AssetLoader {
    /// Create a new asset loader with platform detection.
    pub fn new() -> Self {
        let platform = Platform::detect();
        Self {
            base_path: platform.assets_base_path(),
            temp_path: platform.temp_path(),
            platform,
            cache: Arc::new(RwLock::new(HashMap::new())),
            extracted: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create an asset loader with a custom base path.
    pub fn with_base_path(base_path: impl Into<PathBuf>) -> Self {
        let platform = Platform::detect();
        Self {
            base_path: base_path.into(),
            temp_path: platform.temp_path(),
            platform,
            cache: Arc::new(RwLock::new(HashMap::new())),
            extracted: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get the detected platform.
    pub fn platform(&self) -> Platform {
        self.platform
    }

    /// Get the base path for assets.
    pub fn base_path(&self) -> &Path {
        &self.base_path
    }

    /// Resolve an asset path to a filesystem path.
    fn resolve_path(&self, asset_path: &str) -> PathBuf {
        // Normalize path separators
        let normalized = asset_path.replace('\\', "/");

        // Check if it's an absolute path
        if Path::new(&normalized).is_absolute() {
            return PathBuf::from(normalized);
        }

        // Join with base path
        self.base_path.join(normalized)
    }

    /// Load an asset as bytes.
    pub fn load(&self, asset_path: &str) -> Result<Arc<Vec<u8>>> {
        // Check cache first
        {
            let cache = self.cache.read().unwrap();
            if let Some(data) = cache.get(asset_path) {
                return Ok(Arc::clone(data));
            }
        }

        // Load from filesystem
        let full_path = self.resolve_path(asset_path);
        let data = std::fs::read(&full_path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                AssetError::NotFound(asset_path.to_string())
            } else {
                AssetError::ReadError {
                    path: asset_path.to_string(),
                    source: e,
                }
            }
        })?;

        let data = Arc::new(data);

        // Cache the result
        {
            let mut cache = self.cache.write().unwrap();
            cache.insert(asset_path.to_string(), Arc::clone(&data));
        }

        Ok(data)
    }

    /// Load an asset as a UTF-8 string.
    pub fn load_string(&self, asset_path: &str) -> Result<String> {
        let data = self.load(asset_path)?;
        String::from_utf8((*data).clone())
            .map_err(|_| AssetError::InvalidUtf8(asset_path.to_string()))
    }

    /// Get a filesystem path to an asset.
    ///
    /// On serverless platforms, this may extract the asset to a temp directory.
    pub fn path(&self, asset_path: &str) -> Result<PathBuf> {
        // Check if already extracted
        {
            let extracted = self.extracted.read().unwrap();
            if let Some(path) = extracted.get(asset_path) {
                if path.exists() {
                    return Ok(path.clone());
                }
            }
        }

        let source_path = self.resolve_path(asset_path);

        // On local platform, just return the source path if it exists
        if self.platform == Platform::Local && source_path.exists() {
            return Ok(source_path);
        }

        // Check if source exists
        if !source_path.exists() {
            return Err(AssetError::NotFound(asset_path.to_string()));
        }

        // For serverless platforms, we might need to copy to temp if the
        // asset needs to be writable (e.g., SQLite)
        if matches!(
            self.platform,
            Platform::Lambda | Platform::CloudflareWorkers
        ) {
            // Extract to temp path
            let temp_asset_path = self.temp_path.join(asset_path);

            // Create parent directories
            if let Some(parent) = temp_asset_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| AssetError::ExtractionError {
                    path: asset_path.to_string(),
                    source: e,
                })?;
            }

            // Copy if not already extracted
            if !temp_asset_path.exists() {
                std::fs::copy(&source_path, &temp_asset_path).map_err(|e| {
                    AssetError::ExtractionError {
                        path: asset_path.to_string(),
                        source: e,
                    }
                })?;
            }

            // Cache the extracted path
            {
                let mut extracted = self.extracted.write().unwrap();
                extracted.insert(asset_path.to_string(), temp_asset_path.clone());
            }

            return Ok(temp_asset_path);
        }

        // For other platforms, return the source path
        Ok(source_path)
    }

    /// Check if an asset exists.
    pub fn exists(&self, asset_path: &str) -> bool {
        let full_path = self.resolve_path(asset_path);
        full_path.exists()
    }

    /// List assets matching a glob pattern.
    pub fn list(&self, pattern: &str) -> Result<Vec<String>> {
        let full_pattern = self.base_path.join(pattern);
        let pattern_str = full_pattern.to_string_lossy();

        let paths = glob::glob(&pattern_str)
            .map_err(|e| AssetError::InvalidPattern(pattern.to_string(), e.to_string()))?;

        let base_path_len = self.base_path.to_string_lossy().len();
        let results: Vec<String> = paths
            .filter_map(|entry| entry.ok())
            .filter(|path| path.is_file())
            .map(|path| {
                let path_str = path.to_string_lossy();
                // Return path relative to base
                if path_str.len() > base_path_len {
                    path_str[base_path_len..]
                        .trim_start_matches('/')
                        .to_string()
                } else {
                    path_str.to_string()
                }
            })
            .collect();

        Ok(results)
    }

    /// Clear the asset cache.
    pub fn clear_cache(&self) {
        let mut cache = self.cache.write().unwrap();
        cache.clear();
    }
}

impl Default for AssetLoader {
    fn default() -> Self {
        Self::new()
    }
}

// Public API functions that use the global loader

/// Load an asset as bytes.
///
/// # Example
///
/// ```rust,ignore
/// use pmcp::assets;
///
/// let db_bytes = assets::load("chinook.db")?;
/// ```
pub fn load(asset_path: &str) -> Result<Arc<Vec<u8>>> {
    get_loader().load(asset_path)
}

/// Load an asset as a UTF-8 string.
///
/// # Example
///
/// ```rust,ignore
/// use pmcp::assets;
///
/// let guide = assets::load_string("resources/guide.md")?;
/// println!("{}", guide);
/// ```
pub fn load_string(asset_path: &str) -> Result<String> {
    get_loader().load_string(asset_path)
}

/// Get a filesystem path to an asset.
///
/// On serverless platforms (Lambda, Workers), this extracts the asset
/// to a temp directory and returns that path. This is necessary for
/// libraries that require a filesystem path (e.g., `SQLite`).
///
/// # Example
///
/// ```rust,ignore
/// use pmcp::assets;
/// use rusqlite::Connection;
///
/// let db_path = assets::path("chinook.db")?;
/// let conn = Connection::open(&db_path)?;
/// ```
pub fn path(asset_path: &str) -> Result<PathBuf> {
    get_loader().path(asset_path)
}

/// Check if an asset exists.
///
/// # Example
///
/// ```rust,ignore
/// use pmcp::assets;
///
/// if assets::exists("config/override.toml") {
///     let config = assets::load_string("config/override.toml")?;
/// }
/// ```
pub fn exists(asset_path: &str) -> bool {
    get_loader().exists(asset_path)
}

/// List assets matching a glob pattern.
///
/// # Example
///
/// ```rust,ignore
/// use pmcp::assets;
///
/// for asset in assets::list("resources/**/*.md")? {
///     println!("Found markdown: {}", asset);
/// }
/// ```
pub fn list(pattern: &str) -> Result<Vec<String>> {
    get_loader().list(pattern)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup_test_assets() -> TempDir {
        let temp_dir = TempDir::new().unwrap();

        // Create test assets
        fs::write(temp_dir.path().join("test.txt"), "Hello, World!").unwrap();
        fs::create_dir_all(temp_dir.path().join("resources")).unwrap();
        fs::write(
            temp_dir.path().join("resources/guide.md"),
            "# Guide\n\nThis is a guide.",
        )
        .unwrap();
        fs::write(
            temp_dir.path().join("resources/api.md"),
            "# API\n\nAPI documentation.",
        )
        .unwrap();

        temp_dir
    }

    #[test]
    fn test_platform_detection_local() {
        // In test environment without Lambda/Cloud Run env vars
        let platform = Platform::detect();
        // Could be Local or Container depending on test environment
        assert!(matches!(platform, Platform::Local | Platform::Container));
    }

    #[test]
    fn test_asset_loader_load() {
        let temp_dir = setup_test_assets();
        let loader = AssetLoader::with_base_path(temp_dir.path());

        let data = loader.load("test.txt").unwrap();
        assert_eq!(&*data, b"Hello, World!");
    }

    #[test]
    fn test_asset_loader_load_string() {
        let temp_dir = setup_test_assets();
        let loader = AssetLoader::with_base_path(temp_dir.path());

        let content = loader.load_string("resources/guide.md").unwrap();
        assert!(content.contains("# Guide"));
    }

    #[test]
    fn test_asset_loader_exists() {
        let temp_dir = setup_test_assets();
        let loader = AssetLoader::with_base_path(temp_dir.path());

        assert!(loader.exists("test.txt"));
        assert!(loader.exists("resources/guide.md"));
        assert!(!loader.exists("nonexistent.txt"));
    }

    #[test]
    fn test_asset_loader_list() {
        let temp_dir = setup_test_assets();
        let loader = AssetLoader::with_base_path(temp_dir.path());

        let markdown_files = loader.list("resources/*.md").unwrap();
        assert_eq!(markdown_files.len(), 2);
        assert!(markdown_files.iter().any(|f| f.contains("guide.md")));
        assert!(markdown_files.iter().any(|f| f.contains("api.md")));
    }

    #[test]
    fn test_asset_loader_path() {
        let temp_dir = setup_test_assets();
        let loader = AssetLoader::with_base_path(temp_dir.path());

        let path = loader.path("test.txt").unwrap();
        assert!(path.exists());
    }

    #[test]
    fn test_asset_loader_not_found() {
        let temp_dir = setup_test_assets();
        let loader = AssetLoader::with_base_path(temp_dir.path());

        let result = loader.load("nonexistent.txt");
        assert!(matches!(result, Err(AssetError::NotFound(_))));
    }

    #[test]
    fn test_asset_loader_caching() {
        let temp_dir = setup_test_assets();
        let loader = AssetLoader::with_base_path(temp_dir.path());

        // Load twice
        let data1 = loader.load("test.txt").unwrap();
        let data2 = loader.load("test.txt").unwrap();

        // Should be the same Arc (cached)
        assert!(Arc::ptr_eq(&data1, &data2));
    }

    #[test]
    fn test_asset_config_builder() {
        let config = AssetConfig::new()
            .include("*.db")
            .include("resources/**/*.md")
            .exclude("**/*.tmp")
            .base_dir("assets");

        assert_eq!(config.include.len(), 2);
        assert_eq!(config.exclude.len(), 1);
        assert_eq!(config.base_dir, Some("assets".to_string()));
    }
}
