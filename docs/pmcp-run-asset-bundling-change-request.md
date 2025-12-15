# Change Request: Support Asset Bundling in pmcp.run Deployments

**Date:** 2024-12-15
**Priority:** High
**Requested by:** cargo-pmcp CLI team
**Affects:** pmcp.run deployment backend

## Summary

The cargo-pmcp CLI now supports bundling assets (database files, configuration files, markdown resources) with Lambda deployments. However, the pmcp.run backend doesn't correctly handle zip deployment packages, causing Lambda execution failures.

## Current Behavior (Broken)

When cargo-pmcp uploads a deployment package (zip file) containing:
```
deployment.zip
├── bootstrap          (ARM64 Linux executable)
└── chinook.db         (asset file)
```

The pmcp.run backend appears to wrap this in an additional folder structure:
```
/var/task/
└── {SERVER-NAME}/
    ├── bootstrap
    └── chinook.db
```

This causes Lambda to fail with:
```
/var/task/bootstrap: cannot execute binary file
```

Because Lambda expects the bootstrap executable at `/var/task/bootstrap`, not `/var/task/{SERVER-NAME}/bootstrap`.

## Expected Behavior

When receiving a zip deployment package, the contents should be deployed directly to Lambda's task root:
```
/var/task/
├── bootstrap          (executable)
└── chinook.db         (asset)
```

## Technical Details

### How to Detect Zip Uploads

The cargo-pmcp CLI sets the content-type header when uploading:
- **Raw binary:** `Content-Type: application/octet-stream`
- **Zip package:** `Content-Type: application/zip`

Alternatively, detect by checking the file's magic bytes:
```
ZIP magic bytes: 0x50 0x4B 0x03 0x04 (PK..)
ELF magic bytes: 0x7F 0x45 0x4C 0x46 (.ELF)
```

### Recommended Implementation

```python
# Pseudocode for deployment handler

def deploy_lambda_code(s3_key: str, content_type: str, server_name: str):
    code_bytes = s3.get_object(s3_key)

    if content_type == "application/zip" or is_zip_file(code_bytes):
        # Use zip directly as Lambda code - DO NOT wrap in folder
        lambda_code = {
            'S3Bucket': bucket,
            'S3Key': s3_key
        }
    else:
        # Raw binary - wrap in zip with bootstrap at root
        zip_buffer = create_zip_with_bootstrap(code_bytes)
        new_s3_key = upload_to_s3(zip_buffer)
        lambda_code = {
            'S3Bucket': bucket,
            'S3Key': new_s3_key
        }

    lambda_client.update_function_code(
        FunctionName=server_name,
        **lambda_code
    )

def is_zip_file(data: bytes) -> bool:
    return data[:4] == b'PK\x03\x04'

def create_zip_with_bootstrap(binary_data: bytes) -> bytes:
    """Wrap raw binary in zip - bootstrap at ROOT, not in subfolder"""
    buffer = BytesIO()
    with zipfile.ZipFile(buffer, 'w', zipfile.ZIP_DEFLATED) as zf:
        # IMPORTANT: Use 'bootstrap' not '{server_name}/bootstrap'
        info = zipfile.ZipInfo('bootstrap')
        info.external_attr = 0o755 << 16  # Make executable
        zf.writestr(info, binary_data)
    return buffer.getvalue()
```

### Key Points

1. **Zip uploads should be used as-is** - Lambda natively understands zip files and extracts them to `/var/task/`

2. **Never wrap in a subfolder** - The current behavior of putting files under `{SERVER-NAME}/` breaks Lambda

3. **Bootstrap must be at zip root** - AWS Lambda Custom Runtime requires `/var/task/bootstrap`

4. **Preserve file permissions** - Bootstrap needs execute permission (0o755)

## Testing

### Test Case 1: Raw Binary Upload (existing behavior)
```bash
# Upload raw bootstrap binary
curl -X PUT "$BOOTSTRAP_URL" \
  -H "Content-Type: application/octet-stream" \
  --data-binary @bootstrap

# Lambda should have:
# /var/task/bootstrap (executable)
```

### Test Case 2: Zip Package Upload (new behavior)
```bash
# Create test zip
zip deployment.zip bootstrap chinook.db

# Upload zip package
curl -X PUT "$BOOTSTRAP_URL" \
  -H "Content-Type: application/zip" \
  --data-binary @deployment.zip

# Lambda should have:
# /var/task/bootstrap (executable)
# /var/task/chinook.db (asset file)
```

### Verification
```bash
# Check Lambda code structure
aws lambda get-function --function-name $FUNCTION_NAME \
  --query 'Code.Location' --output text | xargs curl -s | unzip -l -

# Should show:
#   bootstrap
#   chinook.db
# NOT:
#   SERVER-NAME/bootstrap
#   SERVER-NAME/chinook.db
```

## Impact

This change enables:
- **Database-backed MCP servers** - Bundle SQLite databases with deployments
- **Resource files** - Include markdown files, templates, configs
- **Larger deployments** - Assets up to Lambda's 250MB unzipped limit

## Cargo-pmcp CLI Changes (Already Implemented)

The CLI side is ready:
- Creates zip with `bootstrap` + assets at root level
- Sets `Content-Type: application/zip` for zip uploads
- Falls back to raw binary upload when no assets configured

Files changed:
- `cargo-pmcp/src/deployment/builder.rs` - Creates deployment.zip
- `cargo-pmcp/src/deployment/targets/pmcp_run/deploy.rs` - Uploads with correct content-type

## Questions for pmcp.run Team

1. Is the current folder-wrapping behavior intentional? If so, why?
2. Is there a different S3 key or API endpoint we should use for zip packages?
3. Any concerns about accepting user-provided zip files directly?

## References

- [AWS Lambda Custom Runtime](https://docs.aws.amazon.com/lambda/latest/dg/runtimes-custom.html)
- [Lambda Deployment Package](https://docs.aws.amazon.com/lambda/latest/dg/gettingstarted-package.html)
- [cargo-pmcp asset bundling PR](#) (link to PR when ready)
