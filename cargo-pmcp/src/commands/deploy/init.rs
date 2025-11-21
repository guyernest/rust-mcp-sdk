use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::Command;

pub struct InitCommand {
    project_root: PathBuf,
    region: String,
    check_credentials: bool,
}

impl InitCommand {
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            project_root,
            region: "us-east-1".to_string(),
            check_credentials: true,
        }
    }

    pub fn with_region(mut self, region: &str) -> Self {
        self.region = region.to_string();
        self
    }

    pub fn with_credentials_check(mut self, check: bool) -> Self {
        self.check_credentials = check;
        self
    }

    pub fn execute(&self) -> Result<()> {
        println!("🚀 Initializing AWS Lambda deployment...");
        println!();

        // 1. Check AWS credentials
        if self.check_credentials {
            self.check_aws_credentials()?;
        }

        // 2. Get server name from Cargo.toml
        let server_name = self.get_server_name()?;

        // 3. Create .pmcp/deploy.toml
        self.create_config(&server_name)?;

        // 4. Create deploy/ directory with CDK templates
        self.create_cdk_project(&server_name)?;

        // 5. Install CDK dependencies
        self.install_cdk_deps()?;

        println!();
        println!("✅ AWS Lambda deployment initialized!");
        println!();
        println!("Next steps:");
        println!("1. (Optional) Edit .pmcp/deploy.toml to customize deployment");
        println!("2. Deploy: cargo pmcp deploy");

        Ok(())
    }

    fn check_aws_credentials(&self) -> Result<()> {
        print!("🔍 Checking AWS credentials...");
        std::io::Write::flush(&mut std::io::stdout())?;

        let output = Command::new("aws")
            .args(&["sts", "get-caller-identity"])
            .output();

        match output {
            Ok(output) if output.status.success() => {
                println!(" ✅");
                Ok(())
            },
            Ok(_) => {
                println!(" ❌");
                anyhow::bail!(
                    "AWS credentials not configured. Run: aws configure\n\
                     Or use --skip-credentials-check to skip this check"
                );
            },
            Err(_) => {
                println!(" ⚠️");
                println!("   AWS CLI not found. Continuing anyway...");
                println!("   Make sure AWS credentials are configured before deploying.");
                Ok(())
            },
        }
    }

    fn get_server_name(&self) -> Result<String> {
        let cargo_toml_path = self.project_root.join("Cargo.toml");
        let cargo_toml_str =
            std::fs::read_to_string(&cargo_toml_path).context("Failed to read Cargo.toml")?;

        let cargo_toml: toml::Value =
            toml::from_str(&cargo_toml_str).context("Failed to parse Cargo.toml")?;

        // First check if this is a package with a name
        if let Some(name) = cargo_toml
            .get("package")
            .and_then(|p| p.get("name"))
            .and_then(|n| n.as_str())
        {
            return Ok(name.to_string());
        }

        // Otherwise, check if this is a workspace
        if let Some(workspace) = cargo_toml.get("workspace") {
            if let Some(members) = workspace.get("members").and_then(|m| m.as_array()) {
                // Look for *-server binaries in the workspace
                for member in members {
                    if let Some(member_str) = member.as_str() {
                        if member_str.ends_with("-server") {
                            // Extract server name from "crates/hello-server" -> "hello"
                            // or "hello-server" -> "hello"
                            let server_name = member_str
                                .split('/')
                                .last()
                                .unwrap_or(member_str)
                                .strip_suffix("-server")
                                .unwrap_or(member_str);
                            return Ok(server_name.to_string());
                        }
                    }
                }

                // If no *-server found, just use the first member
                if let Some(first) = members.first().and_then(|m| m.as_str()) {
                    let name = first.split('/').last().unwrap_or(first);
                    return Ok(name.to_string());
                }
            }
        }

        anyhow::bail!("Could not find package name or workspace members in Cargo.toml")
    }

    fn create_config(&self, server_name: &str) -> Result<()> {
        print!("📝 Creating deployment configuration...");
        std::io::Write::flush(&mut std::io::stdout())?;

        let config = crate::deployment::config::DeployConfig::default_for_server(
            server_name.to_string(),
            self.region.clone(),
        );

        config.save(&self.project_root)?;

        println!(" ✅");
        Ok(())
    }

    fn create_cdk_project(&self, server_name: &str) -> Result<()> {
        print!("📁 Creating CDK project...");
        std::io::Write::flush(&mut std::io::stdout())?;

        let deploy_dir = self.project_root.join("deploy");
        std::fs::create_dir_all(&deploy_dir).context("Failed to create deploy directory")?;

        // Create CDK files
        self.create_cdk_json(&deploy_dir)?;
        self.create_package_json(&deploy_dir, server_name)?;
        self.create_tsconfig(&deploy_dir)?;
        self.create_app_ts(&deploy_dir)?;
        self.create_stack_ts(&deploy_dir, server_name)?;
        self.create_constructs(&deploy_dir)?;

        println!(" ✅");
        Ok(())
    }

    fn create_cdk_json(&self, deploy_dir: &PathBuf) -> Result<()> {
        let cdk_json = serde_json::json!({
            "app": "npx ts-node --prefer-ts-exts bin/app.ts",
            "watch": {
                "include": ["**"],
                "exclude": [
                    "README.md",
                    "cdk*.json",
                    "**/*.d.ts",
                    "**/*.js",
                    "tsconfig.json",
                    "package*.json",
                    "yarn.lock",
                    "node_modules"
                ]
            },
            "context": {
                "@aws-cdk/aws-lambda:recognizeLayerVersion": true,
                "@aws-cdk/core:checkSecretUsage": true,
                "@aws-cdk/core:target-partitions": ["aws", "aws-cn"]
            }
        });

        std::fs::write(
            deploy_dir.join("cdk.json"),
            serde_json::to_string_pretty(&cdk_json)?,
        )?;

        Ok(())
    }

    fn create_package_json(&self, deploy_dir: &PathBuf, server_name: &str) -> Result<()> {
        let package_json = serde_json::json!({
            "name": format!("{}-deploy", server_name),
            "version": "1.0.0",
            "bin": {
                "app": "bin/app.js"
            },
            "scripts": {
                "build": "tsc",
                "cdk": "cdk"
            },
            "devDependencies": {
                "@types/node": "^20.0.0",
                "typescript": "^5.0.0",
                "aws-cdk": "^2.100.0",
                "ts-node": "^10.9.1"
            },
            "dependencies": {
                "aws-cdk-lib": "^2.100.0",
                "constructs": "^10.0.0"
            }
        });

        std::fs::write(
            deploy_dir.join("package.json"),
            serde_json::to_string_pretty(&package_json)?,
        )?;

        Ok(())
    }

    fn create_tsconfig(&self, deploy_dir: &PathBuf) -> Result<()> {
        let tsconfig = serde_json::json!({
            "compilerOptions": {
                "target": "ES2020",
                "module": "commonjs",
                "lib": ["es2020"],
                "declaration": true,
                "strict": true,
                "noImplicitAny": true,
                "strictNullChecks": true,
                "noImplicitThis": true,
                "alwaysStrict": true,
                "noUnusedLocals": false,
                "noUnusedParameters": false,
                "noImplicitReturns": true,
                "noFallthroughCasesInSwitch": false,
                "inlineSourceMap": true,
                "inlineSources": true,
                "experimentalDecorators": true,
                "strictPropertyInitialization": false,
                "typeRoots": ["./node_modules/@types"]
            },
            "exclude": ["node_modules", "cdk.out"]
        });

        std::fs::write(
            deploy_dir.join("tsconfig.json"),
            serde_json::to_string_pretty(&tsconfig)?,
        )?;

        Ok(())
    }

    fn create_app_ts(&self, deploy_dir: &PathBuf) -> Result<()> {
        let bin_dir = deploy_dir.join("bin");
        std::fs::create_dir_all(&bin_dir)?;

        let app_ts = r#"#!/usr/bin/env node
import * as cdk from 'aws-cdk-lib';
import { McpServerStack } from '../lib/stack';
import * as fs from 'fs';

const app = new cdk.App();

// Load configuration from .pmcp/deploy.toml
const configPath = '../.pmcp/deploy.toml';

if (!fs.existsSync(configPath)) {
  throw new Error('Configuration not found: .pmcp/deploy.toml');
}

// For now, we'll read basic config from env or use defaults
// In the future, we can add a TOML parser for TypeScript
const serverName = process.env.SERVER_NAME || 'mcp-server';
const region = process.env.AWS_REGION || process.env.CDK_DEFAULT_REGION || 'us-east-1';

new McpServerStack(app, `${serverName}-stack`, {
  env: {
    account: process.env.CDK_DEFAULT_ACCOUNT,
    region: region,
  },
  description: `MCP Server: ${serverName}`,
});

app.synth();
"#;

        std::fs::write(bin_dir.join("app.ts"), app_ts)?;

        Ok(())
    }

    fn create_stack_ts(&self, deploy_dir: &PathBuf, server_name: &str) -> Result<()> {
        let lib_dir = deploy_dir.join("lib");
        std::fs::create_dir_all(&lib_dir)?;

        // This is a minimal stack for MVP
        // We'll expand this with constructs in the next phase
        let stack_ts = format!(
            r#"import * as cdk from 'aws-cdk-lib';
import * as lambda from 'aws-cdk-lib/aws-lambda';
import * as apigatewayv2 from 'aws-cdk-lib/aws-apigatewayv2';
import * as logs from 'aws-cdk-lib/aws-logs';
import {{ Construct }} from 'constructs';

export class McpServerStack extends cdk.Stack {{
  constructor(scope: Construct, id: string, props?: cdk.StackProps) {{
    super(scope, id, props);

    // Lambda function
    const mcpFunction = new lambda.Function(this, 'McpFunction', {{
      functionName: '{}',
      runtime: lambda.Runtime.PROVIDED_AL2023,
      handler: 'bootstrap',
      code: lambda.Code.fromAsset('.build'),
      memorySize: 512,
      timeout: cdk.Duration.seconds(30),
      environment: {{
        RUST_LOG: 'info',
      }},
      tracing: lambda.Tracing.ACTIVE,
    }});

    // Log group
    const logGroup = new logs.LogGroup(this, 'LogGroup', {{
      logGroupName: `/aws/lambda/${{mcpFunction.functionName}}`,
      retention: logs.RetentionDays.ONE_MONTH,
      removalPolicy: cdk.RemovalPolicy.DESTROY,
    }});

    // HTTP API (will add auth in next phase)
    const httpApi = new apigatewayv2.HttpApi(this, 'HttpApi', {{
      apiName: '{}',
      description: 'MCP Server HTTP API',
      corsPreflight: {{
        allowOrigins: ['*'],
        allowMethods: [
          apigatewayv2.CorsHttpMethod.GET,
          apigatewayv2.CorsHttpMethod.POST,
          apigatewayv2.CorsHttpMethod.OPTIONS,
        ],
        allowHeaders: ['*'],
      }},
    }});

    // Lambda integration
    const integration = new apigatewayv2.CfnIntegration(this, 'Integration', {{
      apiId: httpApi.apiId,
      integrationType: 'AWS_PROXY',
      integrationUri: mcpFunction.functionArn,
      payloadFormatVersion: '2.0',
    }});

    // Route
    new apigatewayv2.CfnRoute(this, 'Route', {{
      apiId: httpApi.apiId,
      routeKey: 'POST /{{proxy+}}',
      target: `integrations/${{integration.ref}}`,
    }});

    // Permission for API Gateway to invoke Lambda
    mcpFunction.addPermission('ApiGatewayInvoke', {{
      principal: new cdk.aws_iam.ServicePrincipal('apigateway.amazonaws.com'),
      sourceArn: `arn:aws:execute-api:${{this.region}}:${{this.account}}:${{httpApi.apiId}}/*/*`,
    }});

    // Outputs
    new cdk.CfnOutput(this, 'ApiUrl', {{
      value: httpApi.apiEndpoint || '',
      description: 'MCP Server API URL',
      exportName: `${{}}-api-url`,
    }});

    // Temporary outputs (will add real OAuth in next phase)
    new cdk.CfnOutput(this, 'OAuthDiscoveryUrl', {{
      value: 'https://oauth-coming-soon',
      description: 'OAuth Discovery URL (coming in next phase)',
    }});

    new cdk.CfnOutput(this, 'ClientId', {{
      value: 'client-id-coming-soon',
      description: 'OAuth Client ID (coming in next phase)',
    }});

    new cdk.CfnOutput(this, 'DashboardUrl', {{
      value: `https://console.aws.amazon.com/cloudwatch/home?region=${{this.region}}`,
      description: 'CloudWatch Console',
    }});

    new cdk.CfnOutput(this, 'UserPoolId', {{
      value: 'user-pool-coming-soon',
      description: 'Cognito User Pool ID (coming in next phase)',
    }});
  }}
}}
"#,
            server_name, server_name
        );

        std::fs::write(lib_dir.join("stack.ts"), stack_ts)?;

        Ok(())
    }

    fn create_constructs(&self, deploy_dir: &PathBuf) -> Result<()> {
        let constructs_dir = deploy_dir.join("lib/constructs");
        std::fs::create_dir_all(&constructs_dir)?;

        // Placeholder files for future constructs
        std::fs::write(constructs_dir.join(".gitkeep"), "")?;

        Ok(())
    }

    fn install_cdk_deps(&self) -> Result<()> {
        print!("📦 Installing CDK dependencies (this may take a minute)...");
        std::io::Write::flush(&mut std::io::stdout())?;

        let deploy_dir = self.project_root.join("deploy");

        let status = Command::new("npm")
            .arg("install")
            .current_dir(&deploy_dir)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .context("Failed to run npm install")?;

        if !status.success() {
            println!(" ❌");
            anyhow::bail!("npm install failed");
        }

        println!(" ✅");
        Ok(())
    }
}
