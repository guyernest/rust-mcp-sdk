//! Example showing completable arguments for prompts.

use async_trait::async_trait;
use pmcp::error::Result as PmcpResult;
use pmcp::server::{PromptHandler, Server};
use pmcp::types::capabilities::ServerCapabilities;
use pmcp::types::completable::completable;
use pmcp::types::{Content, GetPromptResult, PromptArgument, PromptMessage};
use pmcp::RequestHandlerExtra;
use serde_json::json;
use std::collections::HashMap;
use tracing::info;

/// Database query prompt with completable arguments
struct DatabaseQueryPrompt;

#[async_trait]
impl PromptHandler for DatabaseQueryPrompt {
    async fn handle(
        &self,
        args: HashMap<String, String>,
        _extra: RequestHandlerExtra,
    ) -> PmcpResult<GetPromptResult> {
        let database = args.get("database").unwrap_or(&"main".to_string()).clone();
        let table = args.get("table").unwrap_or(&"users".to_string()).clone();
        let operation = args
            .get("operation")
            .unwrap_or(&"select".to_string())
            .clone();

        let query = match operation.as_str() {
            "select" => format!("SELECT * FROM {} LIMIT 10", table),
            "count" => format!("SELECT COUNT(*) FROM {}", table),
            "describe" => format!("DESCRIBE {}", table),
            _ => format!("-- Unknown operation: {}", operation),
        };

        Ok(GetPromptResult::new(
            vec![
                PromptMessage::system(Content::text(format!("You are a database assistant. Execute the following query on database '{}' table '{}':", database, table))),
                PromptMessage::user(Content::text(query)),
            ],
            Some(format!(
                "Database query for {} on {}.{}",
                operation, database, table
            )),
        ))
    }
}

/// Deployment configuration prompt with environment completions
struct DeploymentPrompt;

#[async_trait]
impl PromptHandler for DeploymentPrompt {
    async fn handle(
        &self,
        args: HashMap<String, String>,
        _extra: RequestHandlerExtra,
    ) -> PmcpResult<GetPromptResult> {
        let environment = args
            .get("environment")
            .unwrap_or(&"development".to_string())
            .clone();
        let service = args.get("service").unwrap_or(&"api".to_string()).clone();
        let version = args.get("version").unwrap_or(&"latest".to_string()).clone();

        let config = match environment.as_str() {
            "development" => json!({
                "replicas": 1,
                "resources": { "cpu": "500m", "memory": "512Mi" },
                "debug": true
            }),
            "staging" => json!({
                "replicas": 2,
                "resources": { "cpu": "1000m", "memory": "1Gi" },
                "debug": false
            }),
            "production" => json!({
                "replicas": 4,
                "resources": { "cpu": "2000m", "memory": "2Gi" },
                "debug": false,
                "monitoring": true
            }),
            _ => json!({}),
        };

        Ok(GetPromptResult::new(
            vec![
                PromptMessage::system(Content::text("You are a deployment assistant. Generate a deployment configuration based on the following parameters:")),
                PromptMessage::user(Content::text(format!("Deploy service '{}' version '{}' to '{}' environment with configuration: {}",
                    service, version, environment, serde_json::to_string_pretty(&config).unwrap()))),
            ],
            Some(format!(
                "Deploy {} version {} to {}",
                service, version, environment
            )),
        ))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // Create server
    let server = Server::builder()
        .name("completable-prompts-example")
        .version("1.0.0")
        .capabilities(ServerCapabilities::prompts_only())
        // Add database query prompt
        .prompt("database_query", DatabaseQueryPrompt)
        // Add deployment prompt
        .prompt("deployment_config", DeploymentPrompt)
        .build()?;

    info!("Starting server with completable prompts...");
    info!("\nAvailable prompts:");
    info!("1. database_query - Generate database queries");
    info!("   Arguments:");
    info!("   - database: Target database (completions: main, analytics, archive)");
    info!("   - table: Table name (completions: users, orders, products, logs)");
    info!("   - operation: Query type (completions: select, count, describe)");
    info!("\n2. deployment_config - Generate deployment configurations");
    info!("   Arguments:");
    info!("   - environment: Target env (completions: development, staging, production)");
    info!("   - service: Service name (completions: api, web, worker, scheduler)");
    info!("   - version: Version tag (e.g., v1.0.0, latest)");

    info!("\nNote: The completable arguments feature allows clients to:");
    info!("- Get suggestions for argument values");
    info!("- Validate input against allowed values");
    info!("- Provide better UX with autocomplete");

    // Run server
    server.run_stdio().await?;

    Ok(())
}

/// Helper to create prompt info with completable arguments.
/// This would be used during server registration to define prompts with completions.
#[allow(dead_code)]
fn create_database_prompt_info() -> pmcp::types::PromptInfo {
    let mut db_arg = PromptArgument::new("database")
        .with_description("Target database")
        .required();
    db_arg.completion = Some(
        completable("database")
            .static_completions(vec![
                "main".to_string(),
                "analytics".to_string(),
                "archive".to_string(),
            ])
            .build()
            .completion
            .unwrap(),
    );

    let mut table_arg = PromptArgument::new("table")
        .with_description("Table name")
        .required();
    table_arg.completion = Some(
        completable("table")
            .static_completions(vec![
                "users".to_string(),
                "orders".to_string(),
                "products".to_string(),
                "logs".to_string(),
            ])
            .build()
            .completion
            .unwrap(),
    );

    let mut op_arg = PromptArgument::new("operation").with_description("Query operation");
    op_arg.completion = Some(
        completable("operation")
            .static_completions(vec![
                "select".to_string(),
                "count".to_string(),
                "describe".to_string(),
            ])
            .build()
            .completion
            .unwrap(),
    );

    pmcp::types::PromptInfo::new("database_query")
        .with_description("Generate database queries with auto-completion")
        .with_arguments(vec![db_arg, table_arg, op_arg])
}
