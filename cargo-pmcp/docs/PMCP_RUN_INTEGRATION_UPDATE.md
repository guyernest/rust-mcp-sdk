# pmcp.run Service Integration Update Required

## Summary

The cargo-pmcp SDK has been updated to generate Lambda-only CDK templates (no API Gateway) for pmcp-run deployments. However, the pmcp.run Step Functions workflow still expects the old template format with `ApiUrl` output.

## Current Error

```
Deployment failed: Failed to extract stack outputs
```

This occurs because the workflow tries to extract `ApiUrl` from CloudFormation outputs, but the Lambda-only template doesn't have an API Gateway.

## cargo-pmcp Changes (Completed)

1. **CDK Template for pmcp-run** now only deploys:
   - Lambda function
   - IAM execution role (implicit)
   - Log group

2. **Outputs from CDK template**:
   - `LambdaArn` - The Lambda function ARN
   - `LambdaName` - The Lambda function name
   - `ApiUrl` - Placeholder: `https://api.pmcp.run/{use-deployment-id}/mcp` (for backward compat)
   - `DashboardUrl` - CloudWatch console URL

3. **URL Construction**: cargo-pmcp constructs the MCP endpoint URL from the deployment ID:
   ```
   https://api.pmcp.run/{deployment_id}/mcp
   ```

## pmcp.run Service Changes Required

### 1. Update Step Functions Workflow

**File**: `amplify/functions/deployment-workflow/state-machine.json`

**Current** (line 320):
```json
"apiUrl": "{% $states.result.Stacks[0].Outputs[OutputKey='ApiUrl'].OutputValue %}"
```

**New**:
```json
"lambdaArn": "{% $states.result.Stacks[0].Outputs[OutputKey='LambdaArn'].OutputValue %}",
"lambdaName": "{% $states.result.Stacks[0].Outputs[OutputKey='LambdaName'].OutputValue %}",
"apiUrl": "{% 'https://api.pmcp.run/' + $deploymentId + '/mcp' %}"
```

### 2. Wire Lambda to Shared API Gateway

After CloudFormation deployment succeeds, the workflow needs to:

1. **Create Lambda integration** on the shared API Gateway:
   ```typescript
   const integration = new apigatewayv2.CfnIntegration(this, 'Integration', {
     apiId: SHARED_API_GATEWAY_ID,
     integrationType: 'AWS_PROXY',
     integrationUri: lambdaArn,
     payloadFormatVersion: '2.0',
   });
   ```

2. **Create route** for `/{serverId}/mcp`:
   ```typescript
   new apigatewayv2.CfnRoute(this, 'Route', {
     apiId: SHARED_API_GATEWAY_ID,
     routeKey: `POST /${serverId}/mcp`,
     target: `integrations/${integration.ref}`,
   });
   ```

3. **Grant API Gateway permission** to invoke the Lambda:
   ```typescript
   new lambda.CfnPermission(this, 'ApiGatewayPermission', {
     action: 'lambda:InvokeFunction',
     functionName: lambdaName,
     principal: 'apigateway.amazonaws.com',
     sourceArn: `arn:aws:execute-api:${region}:${account}:${SHARED_API_GATEWAY_ID}/*/*`,
   });
   ```

### 3. Update DynamoDB Record

Store `lambdaArn` in the deployment record:
```json
{
  "id": "dep_xxxxx",
  "projectName": "chess",
  "status": "success",
  "lambdaArn": "arn:aws:lambda:us-west-2:123456789:function:chess",
  "lambdaName": "chess",
  "url": "https://api.pmcp.run/dep_xxxxx/mcp"
}
```

## Alternative: Quick Fix for Backward Compatibility

If the service-side changes take time, cargo-pmcp can temporarily generate templates with a per-server API Gateway (the old way) for pmcp-run target. However, this defeats the cost optimization benefits of the shared API Gateway architecture.

## Testing

Once pmcp.run service is updated:

```bash
cd ~/Development/mcp/pmcp/test-chess/chess
rm -rf .pmcp deploy
cargo pmcp deploy init --oauth cognito --target pmcp-run
cargo pmcp deploy --target pmcp-run
```

Expected output:
```
🎉 Deployment successful!

📊 Deployment Details:
   Name: chess
   ID: dep_xxxxx

🔌 MCP Endpoint:
   URL: https://api.pmcp.run/dep_xxxxx/mcp

🏥 Health Check:
   URL: https://api.pmcp.run/dep_xxxxx/health
```
