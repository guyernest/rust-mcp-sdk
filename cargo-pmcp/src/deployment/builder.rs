use anyhow::{bail, Context, Result};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

pub struct BinaryBuilder {
    project_root: PathBuf,
    oauth_enabled: bool,
    /// Whether to build local OAuth lambdas (false for pmcp-run which uses shared OAuth)
    build_oauth_lambdas: bool,
}

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
        let lambda_package = format!("{}-lambda", config.server.name);
        // AWS Lambda Custom Runtime requires the binary to be named "bootstrap"
        let lambda_binary = "bootstrap";

        // Use cargo lambda build - it handles all cross-compilation
        // ARM64 is cheaper and faster on Lambda than x86_64
        let status = Command::new("cargo")
            .args(&[
                "lambda",
                "build",
                "--release",
                "--package",
                &lambda_package,
                "--bin",
                lambda_binary,
                "--output-format",
                "binary",
                "--target",
                "aarch64-unknown-linux-gnu",
            ])
            .current_dir(&self.project_root)
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

        // cargo-lambda outputs to target/lambda/{binary-name}/bootstrap
        let src = self
            .project_root
            .join(format!("target/lambda/{}/bootstrap", package_name));

        if !src.exists() {
            println!(" ❌");
            bail!("Binary not found at: {}", src.display());
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
        let src = self.project_root.join("target/lambda/bootstrap/bootstrap");

        if !src.exists() {
            // Try alternative path
            let alt_src = self
                .project_root
                .join(format!("target/lambda/{}/bootstrap", package_name));
            if !alt_src.exists() {
                println!(" ❌");
                bail!(
                    "{} binary not found at {} or {}",
                    lambda_type,
                    src.display(),
                    alt_src.display()
                );
            }
        }

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

        // Check if any assets are configured
        if !config.assets.has_assets() {
            return Ok(None);
        }

        println!("📦 Bundling assets...");

        // Resolve asset files
        let asset_files = config.assets.resolve_files(&self.project_root)?;

        if asset_files.is_empty() {
            println!("   No asset files found matching patterns");
            return Ok(None);
        }

        println!("   Found {} asset file(s)", asset_files.len());

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

        // Add assets to assets/ subdirectory in the zip
        // Lambda will extract to $LAMBDA_TASK_ROOT/assets/
        let base_dir = config
            .assets
            .base_dir
            .as_ref()
            .map(|d| self.project_root.join(d))
            .unwrap_or_else(|| self.project_root.clone());

        let asset_options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o644);

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
            zip.start_file(&zip_path, asset_options)
                .context(format!("Failed to add {} to zip", zip_path))?;
            zip.write_all(&asset_data)
                .context(format!("Failed to write {} to zip", zip_path))?;

            println!(" ✅");
        }

        zip.finish().context("Failed to finalize zip file")?;

        let zip_size = std::fs::metadata(&zip_path)
            .context("Failed to get zip size")?
            .len();

        println!(
            "✅ Deployment package created: {:.2} MB ({} files)",
            zip_size as f64 / 1_048_576.0,
            asset_files.len() + 1 // +1 for bootstrap
        );

        Ok(Some(zip_path))
    }
}
