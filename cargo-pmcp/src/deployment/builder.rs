use anyhow::{bail, Context, Result};
use std::path::PathBuf;
use std::process::Command;

pub struct BinaryBuilder {
    project_root: PathBuf,
}

pub struct BuildResult {
    pub binary_path: PathBuf,
    pub binary_size: u64,
}

impl BinaryBuilder {
    pub fn new(project_root: PathBuf) -> Self {
        Self { project_root }
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

        Ok(BuildResult {
            binary_path,
            binary_size,
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
        let server_binary = format!("{}-server", config.server.name);
        let lambda_package = format!("{}-lambda", config.server.name);

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
                &server_binary,
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
        // Load config to get server name
        let config = crate::deployment::config::DeployConfig::load(&self.project_root)?;
        Ok(format!("{}-server", config.server.name))
    }
}
