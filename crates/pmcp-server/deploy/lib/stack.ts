import * as cdk from 'aws-cdk-lib';
import * as lambda from 'aws-cdk-lib/aws-lambda';
import * as logs from 'aws-cdk-lib/aws-logs';
import * as iam from 'aws-cdk-lib/aws-iam';
import { Construct } from 'constructs';

/**
 * MCP Server Stack for pmcp.run deployment
 *
 * This stack deploys only the Lambda function. The API Gateway is managed
 * by the shared pmcp.run infrastructure at https://api.pmcp.run/{serverId}/mcp
 */
export class McpServerStack extends cdk.Stack {
  constructor(scope: Construct, id: string, props?: cdk.StackProps) {
    super(scope, id, props);

    // ========================================================================
    // MCP METADATA (for pmcp.run platform enrichment)
    // Read from CDK context, passed by `cargo pmcp deploy`
    // Platforms read this metadata to provision secrets, add IAM, etc.
    // ========================================================================
    const mcpVersion = this.node.tryGetContext('mcp:version') || '1.0';
    const mcpServerType = this.node.tryGetContext('mcp:serverType') || 'custom';
    const mcpServerId = this.node.tryGetContext('mcp:serverId');
    const mcpTemplateId = this.node.tryGetContext('mcp:templateId');
    const mcpTemplateVersion = this.node.tryGetContext('mcp:templateVersion');
    const mcpResources = this.node.tryGetContext('mcp:resources');
    const mcpCapabilities = this.node.tryGetContext('mcp:capabilities');

    // Set CloudFormation template metadata
    // This is ignored by vanilla CloudFormation but read by pmcp.run
    const metadata: Record<string, any> = {
      'mcp:version': mcpVersion,
      'mcp:serverType': mcpServerType,
    };
    if (mcpServerId) metadata['mcp:serverId'] = mcpServerId;
    if (mcpTemplateId) metadata['mcp:templateId'] = mcpTemplateId;
    if (mcpTemplateVersion) metadata['mcp:templateVersion'] = mcpTemplateVersion;
    if (mcpResources) {
      try {
        metadata['mcp:resources'] = typeof mcpResources === 'string'
          ? JSON.parse(mcpResources)
          : mcpResources;
      } catch (e) {
        metadata['mcp:resources'] = mcpResources;
      }
    }
    if (mcpCapabilities) {
      try {
        metadata['mcp:capabilities'] = typeof mcpCapabilities === 'string'
          ? JSON.parse(mcpCapabilities)
          : mcpCapabilities;
      } catch (e) {
        metadata['mcp:capabilities'] = mcpCapabilities;
      }
    }
    this.templateOptions.metadata = metadata;

    // Get configuration from context or environment
    // These can be overridden via CDK context: -c serverId=myserver
    const serverId = this.node.tryGetContext('serverId') || 'pmcp-server';
    const organizationId = this.node.tryGetContext('organizationId') || process.env.PMCP_ORGANIZATION_ID || 'default-org';
    const mcpServersTable = this.node.tryGetContext('mcpServersTable') || process.env.MCP_SERVERS_TABLE || 'McpServer';

    // Lambda function (ARM64 for better price/performance)
    const mcpFunction = new lambda.Function(this, 'McpFunction', {
      functionName: serverId,
      runtime: lambda.Runtime.PROVIDED_AL2023,
      handler: 'bootstrap',
      code: lambda.Code.fromAsset('.build'),
      architecture: lambda.Architecture.ARM_64,
      memorySize: 256,
      timeout: cdk.Duration.seconds(30),
      environment: {
        RUST_LOG: 'info',
        // Composition configuration for domain servers calling foundation servers
        PMCP_ORGANIZATION_ID: organizationId,
        PMCP_SERVER_ID: serverId,
        MCP_SERVERS_TABLE: mcpServersTable,
      },
      tracing: lambda.Tracing.ACTIVE,
      // Structured JSON logging so CloudWatch correctly parses log levels
      loggingFormat: lambda.LoggingFormat.JSON,
    });

    // Log group with 7-day retention (cost optimization)
    new logs.LogGroup(this, 'LogGroup', {
      logGroupName: `/aws/lambda/${mcpFunction.functionName}`,
      retention: logs.RetentionDays.ONE_WEEK,
      removalPolicy: cdk.RemovalPolicy.DESTROY,
    });

    // IAM permissions for domain server composition
    // These permissions allow domain servers to call foundation servers via Lambda
    // 1. Read from DynamoDB McpServer table to discover foundation servers
    mcpFunction.addToRolePolicy(new iam.PolicyStatement({
      effect: iam.Effect.ALLOW,
      actions: [
        'dynamodb:GetItem',
        'dynamodb:Query',
      ],
      resources: [
        `arn:aws:dynamodb:${this.region}:${this.account}:table/${mcpServersTable}`,
        `arn:aws:dynamodb:${this.region}:${this.account}:table/${mcpServersTable}/*`,
      ],
    }));

    // 2. Invoke other Lambda functions (foundation servers)
    mcpFunction.addToRolePolicy(new iam.PolicyStatement({
      effect: iam.Effect.ALLOW,
      actions: ['lambda:InvokeFunction'],
      resources: [
        `arn:aws:lambda:${this.region}:${this.account}:function:*`,
      ],
    }));

    // Outputs
    new cdk.CfnOutput(this, 'LambdaArn', {
      value: mcpFunction.functionArn,
      description: 'MCP Server Lambda ARN',
    });

    new cdk.CfnOutput(this, 'LambdaName', {
      value: mcpFunction.functionName,
      description: 'MCP Server Lambda Name',
    });

    // ApiUrl output for backward compatibility with pmcp.run workflow
    // The actual URL is constructed from serverId: https://api.pmcp.run/{serverId}/mcp
    // This placeholder is used until pmcp.run workflow is updated to use LambdaArn
    new cdk.CfnOutput(this, 'ApiUrl', {
      value: 'https://api.pmcp.run/{use-deployment-id}/mcp',
      description: 'MCP endpoint (construct from deployment ID)',
    });

    new cdk.CfnOutput(this, 'DashboardUrl', {
      value: `https://console.aws.amazon.com/cloudwatch/home?region=${this.region}`,
      description: 'CloudWatch Console',
    });
  }
}
