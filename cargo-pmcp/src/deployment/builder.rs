use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
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

        // 1. Ensure musl target is installed
        self.ensure_musl_target()?;

        // 2. Build release binary with musl target
        self.build_musl_binary()?;

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

    fn ensure_musl_target(&self) -> Result<()> {
        print!("   Checking musl target...");
        std::io::Write::flush(&mut std::io::stdout())?;

        let output = Command::new("rustup")
            .args(&["target", "list", "--installed"])
            .output()
            .context("Failed to run rustup")?;

        let installed = String::from_utf8_lossy(&output.stdout);

        if !installed.contains("x86_64-unknown-linux-musl") {
            print!(" installing...");
            std::io::Write::flush(&mut std::io::stdout())?;

            let status = Command::new("rustup")
                .args(&["target", "add", "x86_64-unknown-linux-musl"])
                .status()
                .context("Failed to add musl target")?;

            if !status.success() {
                println!(" ❌");
                bail!("Failed to install musl target");
            }
        }

        println!(" ✅");
        Ok(())
    }

    fn build_musl_binary(&self) -> Result<()> {
        print!("   Building release binary...");
        std::io::Write::flush(&mut std::io::stdout())?;

        // Load config to get server name
        let config = crate::deployment::config::DeployConfig::load(&self.project_root)?;
        let server_binary = format!("{}-server", config.server.name);

        let status = Command::new("cargo")
            .args(&[
                "build",
                "--release",
                "--target",
                "x86_64-unknown-linux-musl",
                "--bin",
                &server_binary,
            ])
            .current_dir(&self.project_root)
            .stdout(std::process::Stdio::null())
            .status()
            .context("Failed to run cargo build")?;

        if !status.success() {
            println!(" ❌");
            bail!("Failed to build binary");
        }

        println!(" ✅");
        Ok(())
    }

    fn copy_to_bootstrap(&self) -> Result<PathBuf> {
        print!("   Copying to Lambda bootstrap...");
        std::io::Write::flush(&mut std::io::stdout())?;

        // Get package name from Cargo.toml
        let package_name = self.get_package_name()?;

        // Source binary path
        let src = self.project_root.join(format!(
            "target/x86_64-unknown-linux-musl/release/{}",
            package_name
        ));

        if !src.exists() {
            println!(" ❌");
            bail!("Binary not found at: {}", src.display());
        }

        // Destination path
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
