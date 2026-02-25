//! WASM build orchestration and artifact caching
//!
//! Automates `wasm-pack build` for the WASM client, caches output artifacts,
//! and provides status tracking for the preview server's WASM bridge mode.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio::sync::RwLock;
use tracing::{info, warn};

/// Current state of the WASM build pipeline
#[derive(Debug, Clone)]
pub enum BuildStatus {
    /// No build has been attempted yet
    NotBuilt,
    /// A build is currently in progress
    Building,
    /// Build succeeded; contains the path to the `pkg/` output directory
    Ready(PathBuf),
    /// Build failed with the given error message
    Failed(String),
}

impl BuildStatus {
    /// Return a human-readable status string suitable for JSON responses.
    pub fn as_str(&self) -> String {
        match self {
            Self::NotBuilt => "not_built".to_string(),
            Self::Building => "building".to_string(),
            Self::Ready(_) => "ready".to_string(),
            Self::Failed(msg) => format!("failed: {msg}"),
        }
    }

    /// Whether the build is in the `Ready` state.
    fn is_ready(&self) -> bool {
        matches!(self, Self::Ready(_))
    }
}

/// Orchestrates `wasm-pack build` and caches the resulting artifacts.
///
/// The builder tracks build status behind a `tokio::sync::RwLock` so
/// multiple HTTP handlers can safely query status or trigger builds
/// concurrently.
pub struct WasmBuilder {
    /// Directory containing the WASM client source (e.g. `examples/wasm-client/`)
    source_dir: PathBuf,
    /// Directory where build artifacts are cached (e.g. `target/wasm-bridge/`)
    cache_dir: PathBuf,
    /// Current build status, protected for async-safe access
    build_status: RwLock<BuildStatus>,
}

impl WasmBuilder {
    /// Create a new builder.
    ///
    /// If cached artifacts from a previous build already exist on disk,
    /// the initial status is set to `Ready`; otherwise `NotBuilt`.
    pub fn new(source_dir: PathBuf, cache_dir: PathBuf) -> Self {
        let pkg_dir = cache_dir.join("pkg");
        let initial_status = if Self::artifacts_exist(&pkg_dir) {
            info!("Found cached WASM artifacts at {}", pkg_dir.display());
            BuildStatus::Ready(pkg_dir)
        } else {
            BuildStatus::NotBuilt
        };

        Self {
            source_dir,
            cache_dir,
            build_status: RwLock::new(initial_status),
        }
    }

    /// Ensure the WASM client has been built and return the artifact directory.
    ///
    /// - If `Ready`, returns the cached path immediately.
    /// - If `NotBuilt` or `Failed`, triggers a new build.
    /// - If `Building`, polls until the build completes (short sleep loop).
    pub async fn ensure_built(&self) -> Result<PathBuf, String> {
        // Fast path: already built
        {
            let status = self.build_status.read().await;
            if let BuildStatus::Ready(ref path) = *status {
                return Ok(path.clone());
            }
        }

        // Check if a build is in progress
        {
            let status = self.build_status.read().await;
            if matches!(*status, BuildStatus::Building) {
                return self.wait_for_build().await;
            }
        }

        // Trigger a new build
        self.build().await
    }

    /// Trigger a `wasm-pack build` and return the artifact directory on success.
    pub async fn build(&self) -> Result<PathBuf, String> {
        // Acquire write lock and set Building status
        {
            let mut status = self.build_status.write().await;
            if status.is_ready() {
                if let BuildStatus::Ready(ref path) = *status {
                    return Ok(path.clone());
                }
            }
            *status = BuildStatus::Building;
        }

        // Check that wasm-pack is available
        if !self.wasm_pack_available().await {
            let msg = "WASM mode requires wasm-pack. Install with: \
                       cargo install wasm-pack && rustup target add wasm32-unknown-unknown"
                .to_string();
            let mut status = self.build_status.write().await;
            *status = BuildStatus::Failed(msg.clone());
            return Err(msg);
        }

        // Ensure source directory exists
        if !self.source_dir.exists() {
            let msg = format!(
                "WASM client source not found at {}",
                self.source_dir.display()
            );
            let mut status = self.build_status.write().await;
            *status = BuildStatus::Failed(msg.clone());
            return Err(msg);
        }

        let pkg_dir = self.cache_dir.join("pkg");
        info!(
            "Starting wasm-pack build: source={}, out={}",
            self.source_dir.display(),
            pkg_dir.display()
        );

        let result = tokio::process::Command::new("wasm-pack")
            .arg("build")
            .arg("--target")
            .arg("web")
            .arg("--out-name")
            .arg("mcp_wasm_client")
            .arg("--no-opt")
            .arg("--out-dir")
            .arg(&pkg_dir)
            .current_dir(&self.source_dir)
            .env("CARGO_PROFILE_RELEASE_LTO", "false")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await;

        match result {
            Ok(output) if output.status.success() => {
                info!("wasm-pack build succeeded");
                let mut status = self.build_status.write().await;
                *status = BuildStatus::Ready(pkg_dir.clone());
                Ok(pkg_dir)
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let stdout = String::from_utf8_lossy(&output.stdout);
                let msg = format!(
                    "wasm-pack build failed (exit code {:?}):\n{}\n{}",
                    output.status.code(),
                    stderr,
                    stdout
                );
                warn!("{}", msg);
                let mut status = self.build_status.write().await;
                *status = BuildStatus::Failed(msg.clone());
                Err(msg)
            }
            Err(e) => {
                let msg = format!("Failed to spawn wasm-pack: {e}");
                warn!("{}", msg);
                let mut status = self.build_status.write().await;
                *status = BuildStatus::Failed(msg.clone());
                Err(msg)
            }
        }
    }

    /// Return the current build status as a human-readable string.
    pub async fn status(&self) -> String {
        self.build_status.read().await.as_str()
    }

    /// Return the path to the artifact directory if the build is ready.
    pub async fn artifact_dir(&self) -> Option<PathBuf> {
        let status = self.build_status.read().await;
        match *status {
            BuildStatus::Ready(ref path) => Some(path.clone()),
            _ => None,
        }
    }

    /// Check whether `wasm-pack` is installed and accessible.
    async fn wasm_pack_available(&self) -> bool {
        tokio::process::Command::new("wasm-pack")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .is_ok_and(|s| s.success())
    }

    /// Poll until the build transitions out of the `Building` state.
    async fn wait_for_build(&self) -> Result<PathBuf, String> {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            let status = self.build_status.read().await;
            match &*status {
                BuildStatus::Ready(path) => return Ok(path.clone()),
                BuildStatus::Failed(msg) => return Err(msg.clone()),
                BuildStatus::Building => continue,
                BuildStatus::NotBuilt => {
                    return Err("Build was reset while waiting".to_string());
                }
            }
        }
    }

    /// Check whether the expected WASM artifacts exist on disk.
    fn artifacts_exist(pkg_dir: &Path) -> bool {
        pkg_dir.join("mcp_wasm_client.js").exists()
            && pkg_dir.join("mcp_wasm_client_bg.wasm").exists()
    }
}

/// Locate the workspace root by walking up from `start_dir` and looking
/// for a `Cargo.toml` that contains `[workspace]`.
pub fn find_workspace_root(start_dir: &Path) -> Option<PathBuf> {
    let mut dir = start_dir.to_path_buf();
    loop {
        let cargo_toml = dir.join("Cargo.toml");
        if cargo_toml.exists() {
            if let Ok(contents) = std::fs::read_to_string(&cargo_toml) {
                if contents.contains("[workspace]") {
                    return Some(dir);
                }
            }
        }
        if !dir.pop() {
            return None;
        }
    }
}
