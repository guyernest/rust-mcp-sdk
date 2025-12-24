//! Async operation types for long-running deployment operations.
//!
//! This module provides types for handling async operations that may take
//! longer than typical API timeout limits (e.g., CloudFormation stack deletion).

use serde::{Deserialize, Serialize};

/// Represents an async operation that may be polled for completion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsyncOperation {
    /// Unique identifier for the operation
    pub operation_id: String,
    /// Type of operation (deploy, destroy, update)
    pub operation_type: OperationType,
    /// Current status of the operation
    pub status: OperationStatus,
    /// Human-readable message about the operation
    pub message: String,
    /// Target platform (e.g., "pmcp-run", "aws-lambda")
    pub target: String,
    /// Additional metadata specific to the operation
    pub metadata: Option<serde_json::Value>,
}

/// Types of async operations
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationType {
    /// Deployment operation
    Deploy,
    /// Destroy/deletion operation
    Destroy,
    /// Update operation
    Update,
}

impl std::fmt::Display for OperationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OperationType::Deploy => write!(f, "deploy"),
            OperationType::Destroy => write!(f, "destroy"),
            OperationType::Update => write!(f, "update"),
        }
    }
}

/// Status of an async operation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationStatus {
    /// Operation has been initiated but not yet started
    Initiated,
    /// Operation is currently running
    Running,
    /// Operation completed successfully
    Completed,
    /// Operation failed
    Failed,
}

impl std::fmt::Display for OperationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OperationStatus::Initiated => write!(f, "initiated"),
            OperationStatus::Running => write!(f, "running"),
            OperationStatus::Completed => write!(f, "completed"),
            OperationStatus::Failed => write!(f, "failed"),
        }
    }
}

/// Result type for destroy operations that can be either sync or async
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DestroyResult {
    /// Whether the operation was successful (for sync) or initiated (for async)
    pub success: bool,
    /// Human-readable message
    pub message: String,
    /// If async, contains the operation details for polling
    pub async_operation: Option<AsyncOperation>,
}

impl DestroyResult {
    /// Create a synchronous success result
    pub fn sync_success(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
            async_operation: None,
        }
    }

    /// Create a synchronous failure result
    #[allow(dead_code)]
    pub fn sync_failure(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
            async_operation: None,
        }
    }

    /// Create an async operation result
    pub fn async_operation(operation: AsyncOperation) -> Self {
        Self {
            success: true, // Initiated successfully
            message: operation.message.clone(),
            async_operation: Some(operation),
        }
    }

    /// Check if this is an async operation that needs polling
    #[allow(dead_code)]
    pub fn is_async(&self) -> bool {
        self.async_operation.is_some()
    }
}
