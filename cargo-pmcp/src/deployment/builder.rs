use anyhow::{bail, Context, Result};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

#[allow(dead_code)]
pub struct BinaryBuilder {
    project_root: PathBuf,
    oauth_enabled: bool,
    /// Whether to build local OAuth lambdas (false for pmcp-run which uses shared OAuth)
    build_oauth_lambdas: bool,
}

#[allow(dead_code)]
pub struct BuildResult {
    pub binary_path: PathBuf,
    pub binary_size: u64,
    pub oauth_proxy_path: Option<PathBuf>,
    pub authorizer_path: Option<PathBuf>,
    /// Path to deployment package (zip) if assets were bundled
    pub deployment_package: Option<PathBuf>,
}

impl BinaryBuilder {
    pub fn new(project_root: PathBuf) -> Self {
        // Check if OAuth is enabled in config and if we should build local OAuth lambdas
        let config = crate::deployment::config::DeployConfig::load(&project_root);
        let oauth_enabled = config.as_ref().is_ok_and(|c| c.auth.enabled);

        // For pmcp-run target, OAuth lambdas are shared on the service side
        // Only build local OAuth lambdas for aws-lambda target
        let build_oauth_lambdas = config
            .as_ref()
            .is_ok_and(|c| c.auth.enabled && c.target.target_type == "aws-lambda");

        Self {
            project_root,
            oauth_enabled,
            build_oauth_lambdas,
        }
    }

    pub fn build(&self) -> Result<BuildResult> {
        println!("🔨 Building Rust binary for AWS Lambda...");

        // 1. Check for cargo-lambda
        self.ensure_cargo_lambda()?;

        // 2. Build release binary with cargo-lambda
        self.build_lambda_binary()?;

        // 3. Copy to deploy/.build/bootstrap
        let binary_path = self.copy_to_bootstrap()?;

        let binary_size = std::fs::metadata(&binary_path)
            .context("Failed to get binary size")?
            .len();

        println!(
            "✅ Binary built: {:.2} MB",
            binary_size as f64 / 1_048_576.0
        );

        // 4. Build OAuth Lambdas if enabled AND target requires local OAuth lambdas
        // (pmcp-run uses shared OAuth infrastructure, so we skip building local OAuth lambdas)
        let (oauth_proxy_path, authorizer_path) = if self.build_oauth_lambdas {
            println!("🔐 Building OAuth Lambdas...");
            let proxy_path = self.build_and_copy_oauth_lambda("oauth-proxy")?;
            let authorizer_path = self.build_and_copy_oauth_lambda("authorizer")?;
            (Some(proxy_path), Some(authorizer_path))
        } else {
            (None, None)
        };

        // 5. Bundle assets and create deployment package if assets are configured
        let deployment_package = self.bundle_assets_if_configured(&binary_path)?;

        Ok(BuildResult {
            binary_path,
            binary_size,
            oauth_proxy_path,
            authorizer_path,
            deployment_package,
        })
    }

    fn ensure_cargo_lambda(&self) -> Result<()> {
        print!("   Checking cargo-lambda...");
        std::io::Write::flush(&mut std::io::stdout())?;

        let output = Command::new("cargo")
            .args(&["lambda", "--version"])
            .output();

        match output {
            Ok(output) if output.status.success() => {
                println!(" ✅");
                Ok(())
            },
            _ => {
                println!(" ❌");
                println!();
                println!("cargo-lambda is required for building Lambda binaries.");
                println!("Install with:");
                println!("  cargo install cargo-lambda");
                println!();
                bail!("cargo-lambda not installed");
            },
        }
    }

    fn build_lambda_binary(&self) -> Result<()> {
        print!("   Building Lambda binary (this may take a few minutes)...");
        std::io::Write::flush(&mut std::io::stdout())?;

        // Load config to get server name
        let config = crate::deployment::config::DeployConfig::load(&self.project_root)?;

        // Use cargo lambda build with --arm64 for cross-compilation.
        // ARM64 is cheaper and faster on Lambda than x86_64.
        //
        // We cd into the lambda package directory and run without --package/--bin
        // to ensure cargo-lambda configures Zig wrappers correctly for build scripts.
        //
        // Three env vars fix Zig 0.15+ cross-compilation of C dependencies:
        // - AWS_LC_SYS_CMAKE_BUILDER=1: aws-lc-sys uses cmake (not cc crate)
        // - AWS_LC_SYS_NO_JITTER_ENTROPY=1: skip jitterentropy (Zig rejects -U flag)
        // - CC_aarch64_unknown_linux_gnu=zigcc wrapper: ring's cc crate uses the
        //   wrapper which handles the --target triple Zig can't parse
        let lambda_pkg_dir = self.find_lambda_package_dir(&config.server.name)?;

        let status = Command::new("cargo")
            .args(["lambda", "build", "--release", "--arm64"])
            .current_dir(&lambda_pkg_dir)
            // Force aws-lc-sys to use cmake builder (bypasses cc crate target conflict)
            .env("AWS_LC_SYS_CMAKE_BUILDER", "1")
            // Disable jitter entropy: Zig rejects -U_FORTIFY_SOURCE preprocessor flag
            // that cmake adds. Lambda uses OS-provided entropy, not CPU jitter.
            .env("AWS_LC_SYS_NO_JITTER_ENTROPY", "1")
            // Set CC to cargo-lambda's zigcc wrapper for ring and other cc-crate
            // based builds. The cc crate appends --target=aarch64-unknown-linux-gnu
            // which bare zig rejects (UnknownOperatingSystem), but the zigcc wrapper
            // handles it by routing through `cargo-lambda zig cc` first.
            .envs(Self::find_zigcc_env_vars())
            .status()
            .context("Failed to run cargo lambda build")?;

        if !status.success() {
            println!(" ❌");
            bail!("Failed to build Lambda binary");
        }

        println!(" ✅");
        Ok(())
    }

    fn copy_to_bootstrap(&self) -> Result<PathBuf> {
        print!("   Copying to Lambda bootstrap...");
        std::io::Write::flush(&mut std::io::stdout())?;

        // Get package name from config
        let package_name = self.get_package_name()?;

        // Resolve the target directory: respect CARGO_TARGET_DIR, .cargo/config.toml,
        // and workspace target-dir settings. Falls back to {project_root}/target.
        let target_dir = self.resolve_target_dir();

        // cargo-lambda outputs to {target_dir}/lambda/{binary-name}/bootstrap
        let src = target_dir.join(format!("lambda/{}/bootstrap", package_name));

        if !src.exists() {
            println!(" ❌");
            bail!(
                "Binary not found at: {}\n\
                 Hint: If using a shared target directory (CARGO_TARGET_DIR or \
                 [build] target-dir in .cargo/config.toml), the binary will be \
                 in that directory, not in the project root.",
                src.display()
            );
        }

        // Destination path for CDK
        let deploy_build_dir = self.project_root.join("deploy/.build");
        std::fs::create_dir_all(&deploy_build_dir)
            .context("Failed to create deploy/.build directory")?;

        let dst = deploy_build_dir.join("bootstrap");

        // Copy binary
        std::fs::copy(&src, &dst).context("Failed to copy binary to deploy/.build/bootstrap")?;

        // Make executable (Unix only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&dst)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&dst, perms)?;
        }

        println!(" ✅");
        Ok(dst)
    }

    fn get_package_name(&self) -> Result<String> {
        // cargo-lambda outputs to target/lambda/{binary-name}/bootstrap
        // Since AWS Lambda requires binary name "bootstrap", the output is in target/lambda/bootstrap/
        Ok("bootstrap".to_string())
    }

    /// Resolve the effective target directory.
    ///
    /// Checks (in priority order):
    /// 1. `CARGO_TARGET_DIR` env var
    /// 2. `CARGO_BUILD_TARGET_DIR` env var
    /// 3. Workspace root's target/ (walks up to 5 ancestors looking for Cargo.toml with [workspace])
    /// 4. `{project_root}/target`
    fn resolve_target_dir(&self) -> PathBuf {
        // Env vars take priority (matches Cargo's own resolution)
        if let Ok(dir) = std::env::var("CARGO_TARGET_DIR") {
            return PathBuf::from(dir);
        }
        if let Ok(dir) = std::env::var("CARGO_BUILD_TARGET_DIR") {
            return PathBuf::from(dir);
        }

        // Walk up to find workspace root (Cargo.toml with [workspace])
        let mut dir = self.project_root.clone();
        for _ in 0..5 {
            let cargo_toml = dir.join("Cargo.toml");
            if cargo_toml.exists() {
                if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
                    if content.contains("[workspace]") {
                        return dir.join("target");
                    }
                }
            }
            if let Some(parent) = dir.parent() {
                dir = parent.to_path_buf();
            } else {
                break;
            }
        }

        // Fallback: project root
        self.project_root.join("target")
    }

    /// Find the Lambda wrapper package in the workspace.
    /// First tries {server_name}-lambda, then looks for any *-lambda package with bootstrap binary.
    /// Find the directory of the Lambda package for cd-based builds.
    ///
    /// Returns the absolute path to the Lambda package directory.
    /// Find cargo-zigbuild's zigcc wrapper script and return CC/AR env vars.
    ///
    /// cargo-lambda caches zigcc wrappers in ~/Library/Caches/cargo-zigbuild/
    /// (macOS) or ~/.cache/cargo-zigbuild/ (Linux). The wrapper correctly
    /// handles the cc crate's --target flag that bare zig rejects.
    fn find_zigcc_env_vars() -> Vec<(String, String)> {
        let mut vars = Vec::new();

        // dirs::cache_dir() resolves to ~/Library/Caches (macOS) or ~/.cache (Linux)
        let cache_dirs: Vec<PathBuf> = dirs::cache_dir()
            .map(|d| vec![d.join("cargo-zigbuild")])
            .unwrap_or_default();

        for cache_base in &cache_dirs {
            if !cache_base.exists() {
                continue;
            }
            // Find the most recent version directory
            if let Ok(entries) = std::fs::read_dir(cache_base) {
                let mut versions: Vec<_> = entries
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().is_dir())
                    .collect();
                versions.sort_by_key(|e| e.file_name());

                if let Some(version_dir) = versions.last() {
                    let dir = version_dir.path();
                    // Find zigcc-aarch64-unknown-linux-gnu-*.sh
                    if let Ok(files) = std::fs::read_dir(&dir) {
                        for file in files.filter_map(|f| f.ok()) {
                            let name = file.file_name().to_string_lossy().to_string();
                            if name.starts_with("zigcc-aarch64-unknown-linux-gnu-")
                                && name.ends_with(".sh")
                            {
                                vars.push((
                                    "CC_aarch64_unknown_linux_gnu".to_string(),
                                    file.path().to_string_lossy().to_string(),
                                ));
                            }
                        }
                    }
                    // AR is just the plain 'ar' in the same directory
                    let ar = dir.join("ar");
                    if ar.exists() {
                        vars.push((
                            "AR_aarch64_unknown_linux_gnu".to_string(),
                            ar.to_string_lossy().to_string(),
                        ));
                    }
                }
            }
        }

        vars
    }

    fn find_lambda_package_dir(&self, server_name: &str) -> Result<PathBuf> {
        let preferred_package = format!("{}-lambda", server_name);

        // Check if the preferred package exists as a direct subdirectory
        let preferred_dir = self.project_root.join(&preferred_package);
        if preferred_dir.exists() && preferred_dir.join("Cargo.toml").exists() {
            return Ok(preferred_dir);
        }

        // Search workspace members for *-lambda packages
        let binaries = crate::deployment::naming::detect_workspace_binaries(&self.project_root)?;

        for binary in &binaries {
            if binary.binary_name == "bootstrap" && binary.package_name.ends_with("-lambda") {
                // Find the Cargo.toml for this package using cargo_metadata
                let metadata = cargo_metadata::MetadataCommand::new()
                    .current_dir(&self.project_root)
                    .exec()
                    .context("Failed to read workspace metadata")?;

                if let Some(pkg) = metadata
                    .packages
                    .iter()
                    .find(|p| p.name == binary.package_name)
                {
                    let pkg_dir = pkg.manifest_path.parent().expect("manifest has parent");
                    return Ok(pkg_dir.into());
                }
            }
        }

        bail!(
            "No Lambda wrapper package found. Expected '{}' or any '*-lambda' package with 'bootstrap' binary.\n\
             Run 'cargo pmcp deploy init --target pmcp-run' to create one.",
            preferred_package
        );
    }

    fn find_lambda_package(&self, server_name: &str) -> Result<String> {
        let preferred_package = format!("{}-lambda", server_name);

        // Check if the preferred package exists
        let preferred_dir = self.project_root.join(&preferred_package);
        if preferred_dir.exists() {
            return Ok(preferred_package);
        }

        // Look for any *-lambda package with bootstrap binary in the workspace
        let binaries = crate::deployment::naming::detect_workspace_binaries(&self.project_root)?;

        for binary in binaries {
            if binary.binary_name == "bootstrap" && binary.package_name.ends_with("-lambda") {
                println!();
                println!(
                    "   ℹ️  Using existing Lambda wrapper: {}",
                    binary.package_name
                );
                return Ok(binary.package_name);
            }
        }

        // No Lambda wrapper found
        bail!(
            "No Lambda wrapper package found. Expected '{}' or any '*-lambda' package with 'bootstrap' binary.\n\
             Run 'cargo pmcp deploy init --target pmcp-run' to create one.",
            preferred_package
        );
    }

    /// Build and copy an OAuth Lambda (proxy or authorizer)
    fn build_and_copy_oauth_lambda(&self, lambda_type: &str) -> Result<PathBuf> {
        let config = crate::deployment::config::DeployConfig::load(&self.project_root)?;
        let package_name = format!("{}-{}", config.server.name, lambda_type);
        let output_dir = format!(".build-{}", lambda_type);

        print!("   Building {} Lambda...", lambda_type);
        std::io::Write::flush(&mut std::io::stdout())?;

        // Build with cargo-lambda
        let status = Command::new("cargo")
            .args([
                "lambda",
                "build",
                "--release",
                "--package",
                &package_name,
                "--output-format",
                "binary",
                "--target",
                "aarch64-unknown-linux-gnu",
            ])
            .current_dir(&self.project_root)
            .status()
            .context(format!(
                "Failed to run cargo lambda build for {}",
                lambda_type
            ))?;

        if !status.success() {
            println!(" ❌");
            bail!("Failed to build {} Lambda binary", lambda_type);
        }

        // Copy to deploy/{output_dir}/bootstrap
        let target_dir = self.resolve_target_dir();
        let src = target_dir.join("lambda/bootstrap/bootstrap");

        let src = if src.exists() {
            src
        } else {
            let alt_src = target_dir.join(format!("lambda/{}/bootstrap", package_name));
            if alt_src.exists() {
                alt_src
            } else {
                println!(" ❌");
                bail!(
                    "{} binary not found at {} or {}",
                    lambda_type,
                    src.display(),
                    alt_src.display()
                );
            }
        };

        let deploy_build_dir = self.project_root.join(format!("deploy/{}", output_dir));
        std::fs::create_dir_all(&deploy_build_dir)
            .context(format!("Failed to create deploy/{} directory", output_dir))?;

        let dst = deploy_build_dir.join("bootstrap");
        std::fs::copy(&src, &dst).context(format!(
            "Failed to copy {} binary to deploy/{}/bootstrap",
            lambda_type, output_dir
        ))?;

        // Make executable (Unix only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&dst)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&dst, perms)?;
        }

        println!(" ✅");
        Ok(dst)
    }

    /// Bundle assets and create a deployment package (zip) if assets are configured.
    ///
    /// Returns the path to the zip file if assets were bundled, None otherwise.
    fn bundle_assets_if_configured(&self, binary_path: &Path) -> Result<Option<PathBuf>> {
        let config = crate::deployment::config::DeployConfig::load(&self.project_root)?;

        // Resolve asset files (empty vec if no assets configured)
        let asset_files = if config.assets.has_assets() {
            config.assets.resolve_files(&self.project_root)?
        } else {
            Vec::new()
        };

        // Check if config.toml exists (for Code Mode operation extraction)
        let has_config_toml = self.project_root.join("config.toml").exists()
            || self.find_single_instance_toml().is_some();

        // Only create ZIP if there's something to bundle beyond bootstrap
        if asset_files.is_empty() && !has_config_toml {
            return Ok(None);
        }

        println!("📦 Bundling deployment package...");
        if !asset_files.is_empty() {
            println!("   Found {} asset file(s)", asset_files.len());
        }

        // Create deployment package directory
        let package_dir = self.project_root.join("deploy/.build");
        std::fs::create_dir_all(&package_dir)
            .context("Failed to create deployment package directory")?;

        // Create zip file
        let zip_path = package_dir.join("deployment.zip");
        let zip_file =
            std::fs::File::create(&zip_path).context("Failed to create deployment.zip")?;
        let mut zip = ZipWriter::new(zip_file);

        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o755);

        // Add bootstrap binary
        print!("   Adding bootstrap binary...");
        std::io::Write::flush(&mut std::io::stdout())?;

        let bootstrap_data =
            std::fs::read(binary_path).context("Failed to read bootstrap binary")?;
        zip.start_file("bootstrap", options)
            .context("Failed to add bootstrap to zip")?;
        zip.write_all(&bootstrap_data)
            .context("Failed to write bootstrap to zip")?;
        println!(" ✅");

        let file_options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o644);

        // Add config.toml if found (for Code Mode operation extraction by pmcp.run)
        let config_toml_included = self.add_config_toml_to_zip(&mut zip, file_options)?;

        // Add assets to assets/ subdirectory in the zip
        // Lambda will extract to $LAMBDA_TASK_ROOT/assets/
        let base_dir = config
            .assets
            .base_dir
            .as_ref()
            .map(|d| self.project_root.join(d))
            .unwrap_or_else(|| self.project_root.clone());

        for asset_path in &asset_files {
            // Get relative path from base directory
            let relative_path = pathdiff::diff_paths(asset_path, &base_dir)
                .unwrap_or_else(|| asset_path.file_name().unwrap().into());

            // Put assets in assets/ subdirectory
            let zip_path = format!("assets/{}", relative_path.display());

            print!("   Adding {}...", relative_path.display());
            std::io::Write::flush(&mut std::io::stdout())?;

            let asset_data = std::fs::read(asset_path)
                .context(format!("Failed to read {}", asset_path.display()))?;
            zip.start_file(&zip_path, file_options)
                .context(format!("Failed to add {} to zip", zip_path))?;
            zip.write_all(&asset_data)
                .context(format!("Failed to write {} to zip", zip_path))?;

            println!(" ✅");
        }

        zip.finish().context("Failed to finalize zip file")?;

        let zip_size = std::fs::metadata(&zip_path)
            .context("Failed to get zip size")?
            .len();

        let extra_files = 1 + if config_toml_included { 1 } else { 0 }; // bootstrap + optional config.toml
        println!(
            "✅ Deployment package created: {:.2} MB ({} files)",
            zip_size as f64 / 1_048_576.0,
            asset_files.len() + extra_files
        );

        Ok(Some(zip_path))
    }

    /// Find and add config.toml to the deploy ZIP for Code Mode operation extraction.
    ///
    /// Uses `resolve_config_toml` to find the config file, then writes it to the ZIP
    /// root as `config.toml`. Returns true if a config file was found and added.
    fn add_config_toml_to_zip<W: std::io::Write + std::io::Seek>(
        &self,
        zip: &mut ZipWriter<W>,
        options: SimpleFileOptions,
    ) -> Result<bool> {
        let config_path = match self.resolve_config_toml() {
            Some(path) => path,
            None => return Ok(false),
        };

        print!("   Adding config.toml...");
        std::io::Write::flush(&mut std::io::stdout())?;

        let config_data =
            std::fs::read(&config_path).context("Failed to read config.toml")?;
        zip.start_file("config.toml", options)
            .context("Failed to add config.toml to zip")?;
        zip.write_all(&config_data)
            .context("Failed to write config.toml to zip")?;
        println!(" ✅ ({})", config_path.display());
        Ok(true)
    }

    /// Resolve the server's config.toml path.
    ///
    /// Resolution order (same as Rust servers using `include_str!()`):
    /// 1. `config.toml` in the server crate root
    /// 2. Single file in `instances/*.toml`
    fn resolve_config_toml(&self) -> Option<PathBuf> {
        let direct = self.project_root.join("config.toml");
        if direct.exists() {
            return Some(direct);
        }
        self.find_single_instance_toml()
    }

    /// Find a single .toml file in the `instances/` directory.
    fn find_single_instance_toml(&self) -> Option<PathBuf> {
        let instances_dir = self.project_root.join("instances");
        if !instances_dir.is_dir() {
            return None;
        }
        let toml_files: Vec<_> = std::fs::read_dir(&instances_dir)
            .ok()?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext == "toml")
                    .unwrap_or(false)
            })
            .collect();
        if toml_files.len() == 1 {
            Some(toml_files[0].path())
        } else {
            None
        }
    }
}
