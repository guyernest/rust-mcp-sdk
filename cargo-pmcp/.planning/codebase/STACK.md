# Technology Stack

**Analysis Date:** 2026-02-26

## Languages

**Primary:**
- Rust 2021 edition - Entire CLI tool and all modules

## Runtime

**Environment:**
- Tokio 1.46+ - Async runtime for concurrent operations
- Rust 1.82.0+ - Minimum supported Rust version

**Package Manager:**
- Cargo - Standard Rust package manager
- Lockfile: `Cargo.lock` (present)

## Frameworks & Core Libraries

**CLI Framework:**
- Clap 4.x - Command-line argument parsing with derive macros
  - Features: `derive`, `env` for environment variable support

**HTTP & Networking:**
- Tokio 1.46 with `full` features - Async runtime with all features enabled
- Reqwest 0.12 - HTTP client library with JSON/multipart support (rustls-tls backend)
- Tiny_http 0.12 - Lightweight HTTP server for OAuth 2.0 callback handling
- Hyper 1.6 - Low-level HTTP server/client (optional, feature-gated)
- Hyper-util 0.1 - Hyper utilities (optional, feature-gated)
- Axum 0.8.5 - Modern async web framework (optional, feature-gated)

**Authentication & OAuth:**
- OAuth2 5.0 - OAuth 2.0 client implementation with custom Cognito token fields
- Jsonwebtoken 9.3 - JWT validation and generation (optional, feature-gated)

**Serialization & Parsing:**
- Serde 1.x - Serialization/deserialization framework with derive macros
- Serde_json 1.x - JSON serialization
- TOML 0.9 - TOML configuration file parsing
- Serde_json with `rename_all = "camelCase"` for API compatibility

**Data Structures:**
- Indexmap 2.10 - Ordered hash maps (feature: `serde`)
- Smallvec 1.13 - Small vector optimization (features: `serde`, `union`)
- Parking_lot 0.12 - Faster synchronization primitives (RwLock)
- Dashmap 6.1 - Concurrent hash map implementation
- Bytes 1.10 - Byte buffer types (optional, feature-gated)

**Time & Chrono:**
- Chrono 0.4 - Date/time handling with RFC3339 serialization
- Used for OAuth token expiration handling and timestamps

**File & Path Handling:**
- Walkdir 2.x - Recursive directory traversal
- Glob 0.3 - Glob pattern matching
- Dirs 6.x - Platform-specific directory paths (home, config)
- Pathdiff 0.2.3 - Path difference/relative path computation
- Zip 7.0 - Zip file creation for landing page deployment packages

**Utilities:**
- Anyhow 1.x - Flexible error handling
- Thiserror 2.x - Error type derivation for custom error types
- Colored 3.x - Terminal color output
- Indicatif 0.18 - Progress bar indicators
- Console 0.16 - Advanced terminal styling
- Regex 1.x - Pattern matching for secret reference parsing
- Urlencoding 2.x - URL encoding/decoding
- Async-trait 0.1.89 - Async trait implementations

**Secrets & Security:**
- Secrecy 0.10 - Secure secret handling with zeroization (feature: `serde`)
- Zeroize 1.x - Secure memory clearing
- Rand 0.8 - Random secret generation
- Rpassword 7.3 - Secure password input (hidden terminal input)

**Metadata & Introspection:**
- Cargo_metadata 0.19 - Access to Cargo.toml metadata programmatically

**Local Crate Dependencies:**
- `mcp-tester` 0.2.0 - MCP server testing library (path: `../crates/mcp-tester`)
- `mcp-preview` 0.1.0 - MCP Apps preview server (path: `../crates/mcp-preview`)

## Optional Features

**Feature Flags:**
- `aws-secrets` - AWS Secrets Manager integration
  - `aws-config` 1.x - AWS SDK configuration
  - `aws-sdk-secretsmanager` 1.x - AWS Secrets Manager client

**Build/Deployment Features:**
- WebAssembly support (feature-gated)
- File watching with `notify` and `glob-match` (optional)
- JSON schema validation with `jsonschema` (optional)
- Input validation with `garde` (optional)
- SIMD support with `rayon` (optional)

## Configuration Files

**Build Configuration:**
- `Cargo.toml` - Package manifest with all dependencies
- Edition: 2021

**Environment Configuration:**
- `.pmcp/` directory - Configuration cache directory
- `pmcp-landing.toml` - Landing page configuration (parsed by `LandingConfig`)
- `.pmcp/deploy.toml` - Deployment configuration
- `.pmcp/pmcp-run-config.json` - Cached pmcp.run service discovery config
- `.pmcp/pmcp-run-credentials.json` - Cached OAuth credentials

**Development:**
- Project follows Cargo workspace structure with path dependencies

## Key Technology Decisions

**CLI Architecture:**
- Uses Clap derive macros for subcommand organization
- Supports cargo subcommand pattern (`cargo pmcp ...`)
- Global `--verbose` flag propagated via environment variable

**Async Programming:**
- Tokio full runtime for all async operations
- Async-trait for trait implementations
- `block_on` for bridging sync main to async operations

**OAuth Flow:**
- Custom Cognito token fields to capture `id_token`
- Local OAuth callback server on port 8787
- Token caching with 1-hour expiration
- PKCE flow support for security

**Configuration Management:**
- TOML for structured configuration
- Home directory config cache via `dirs` crate
- Server ID-based namespacing for secrets

**Error Handling:**
- Anyhow for flexibility in error types
- Thiserror for custom error definitions
- Context chains for debugging

---

*Stack analysis: 2026-02-26*
