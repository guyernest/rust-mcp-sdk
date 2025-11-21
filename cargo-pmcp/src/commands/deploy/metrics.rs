use anyhow::Result;
use std::path::PathBuf;

pub struct MetricsCommand {
    project_root: PathBuf,
    period: String,
}

impl MetricsCommand {
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            project_root,
            period: "24h".to_string(),
        }
    }

    pub fn with_period(mut self, period: &str) -> Self {
        self.period = period.to_string();
        self
    }

    pub fn execute(&self) -> Result<()> {
        println!("📊 Metrics feature coming in Phase 2!");
        println!();
        println!("For now, view metrics in AWS Console:");
        println!("1. Go to CloudWatch Metrics");
        println!("2. Select Lambda → By Function Name");
        println!("3. Find your server");
        println!();
        println!("Or check the dashboard from deployment outputs:");
        println!("  cargo pmcp deploy outputs");

        Ok(())
    }
}
