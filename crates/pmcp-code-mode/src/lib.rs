// Originated from pmcp-run/built-in/shared/pmcp-code-mode (https://github.com/guyernest/pmcp-run)
// Moved into rust-mcp-sdk workspace as a first-class SDK crate for Phase 67.1

//! Code Mode - LLM-generated query validation and execution.
//!
//! This crate provides the infrastructure for "Code Mode", which allows MCP clients
//! to generate and execute structured queries (GraphQL, SQL, REST) with a validation
//! pipeline that ensures security and provides human-readable explanations.
//!
//! ## Architecture
//!
//! ```text
//! describe_schema() → LLM generates code → validate_code() → user approval → execute_code()
//! ```
//!
//! ## Key Components
//!
//! - **Validation Pipeline**: Parse → Policy Check → Security Analysis → Explanation → Token
//! - **Approval Tokens**: HMAC-signed tokens binding code hash to validation result
//! - **Explanations**: Template-based business-language descriptions of queries
//! - **Policy Evaluation**: Pluggable trait for Cedar/AVP/custom policy engines
//!
//! ## Example Usage
//!
//! ```ignore
//! use pmcp_code_mode::{
//!     CodeModeConfig, ValidationPipeline, ValidationContext
//! };
//!
//! // Create a validation pipeline
//! let config = CodeModeConfig::enabled();
//! let pipeline = ValidationPipeline::new(config, b"secret-key".to_vec());
//!
//! // Validate a query
//! let context = ValidationContext::new("user-123", "session-456", "schema-hash", "perms-hash");
//! let result = pipeline.validate_graphql_query("query { users { id name } }", &context)?;
//! ```

pub mod config;
mod explanation;
mod graphql;
pub mod handler;
mod token;
mod types;
pub mod validation;

// Code Mode instruction and policy templates
pub mod templates;

// Schema Exposure Architecture - Three-Layer Schema Model
pub mod schema_exposure;

// Policy evaluation framework
pub mod policy;

// Cedar policy annotation parsing (no AWS dependency)
pub mod policy_annotations;

// Cedar schema and policy validation (test only)
#[cfg(test)]
pub mod cedar_validation;

// JavaScript validation for OpenAPI Code Mode (requires SWC parser)
#[cfg(feature = "openapi-code-mode")]
mod javascript;

// JavaScript execution runtime (AST-based execution in pure Rust)
#[cfg(feature = "js-runtime")]
pub mod executor;

// Shared expression evaluation logic (used by both sync and async executors)
#[cfg(feature = "js-runtime")]
mod eval;

// Re-export public types
pub use config::CodeModeConfig;

pub use explanation::{ExplanationGenerator, TemplateExplanationGenerator};

pub use graphql::{GraphQLOperationType, GraphQLQueryInfo, GraphQLValidator};

// JavaScript/OpenAPI Code Mode exports
#[cfg(feature = "openapi-code-mode")]
pub use javascript::{
    ApiCall, HttpMethod, JavaScriptCodeInfo, JavaScriptValidator, OutputDeclaration,
    SafetyViolation, SafetyViolationType,
};

// JavaScript execution runtime exports
#[cfg(feature = "js-runtime")]
pub use executor::{
    filter_blocked_fields,
    find_blocked_fields_in_output,
    ApiCallLog,
    ArrayMethodCall,
    BinaryOperator,
    BuiltinFunction,
    CompileError,
    ExecutionConfig,
    ExecutionPlan,
    ExecutionResult,
    HttpExecutor,
    JsExecutor,
    MockExecutionMode,
    MockHttpExecutor,
    MockedCall,
    PathPart,
    PathTemplate,
    PlanCompiler,
    PlanExecutor,
    PlanMetadata,
    PlanStep,
    UnaryOperator,
    ValueExpr,
};

// MCP Code Mode executor
#[cfg(feature = "mcp-code-mode")]
pub use executor::McpExecutor;

pub use token::{
    canonicalize_code, compute_context_hash, hash_code, ApprovalToken, HmacTokenGenerator,
    TokenGenerator, TokenSecret,
};

pub use types::{
    CodeLocation, CodeType, Complexity, ExecutionError, PolicyViolation, RiskLevel,
    SecurityAnalysis, SecurityIssue, SecurityIssueType, UnifiedAction, ValidationError,
    ValidationMetadata, ValidationResult,
};

pub use validation::{ValidationContext, ValidationPipeline};

// Code Mode templates
pub use templates::TemplateContext;

// Code Mode handler trait and utilities
pub use handler::{
    format_error_response, format_execution_error, CodeModeHandler, CodeModeToolBuilder,
    ExecuteCodeInput, ValidateCodeInput, ValidationResponse,
};

// Policy types re-exports
pub use policy::{
    AuthorizationDecision, OperationEntity, PolicyEvaluationError, PolicyEvaluator,
    ServerConfigEntity, get_baseline_policies, get_code_mode_schema_json,
};

#[cfg(feature = "openapi-code-mode")]
pub use policy::{
    OpenAPIServerEntity, ScriptEntity, get_openapi_baseline_policies,
    get_openapi_code_mode_schema_json, normalize_operation_format, normalize_path_to_pattern,
};

// Cedar policy evaluator
#[cfg(feature = "cedar")]
pub use policy::cedar::CedarPolicyEvaluator;

// Schema Exposure Architecture types
pub use schema_exposure::{
    CodeModeExposurePolicy,
    DerivationMetadata,
    DerivationStats,
    DerivedSchema,
    ExposureMode,
    FilterReason,
    FilteredOperation,
    GlobalBlocklist,
    McpExposurePolicy,
    MethodExposurePolicy,
    Operation,
    OperationCategory,
    OperationDetails,
    OperationParameter,
    OperationRiskLevel,
    SchemaDeriver,
    SchemaFormat,
    SchemaMetadata,
    SchemaSource,
    ToolExposurePolicy,
    ToolOverride,
};
