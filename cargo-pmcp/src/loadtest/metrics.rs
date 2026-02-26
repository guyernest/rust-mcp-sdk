//! HdrHistogram-based metrics pipeline.
//! Full implementation in Plan 01-03.

/// Type of MCP operation being measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperationType {
    /// MCP initialize handshake.
    Initialize,
    /// tools/call request.
    ToolsCall,
    /// resources/read request.
    ResourcesRead,
    /// prompts/get request.
    PromptsGet,
    /// tools/list discovery request.
    ToolsList,
    /// resources/list discovery request.
    ResourcesList,
    /// prompts/list discovery request.
    PromptsList,
}

/// A single request measurement sample.
pub struct RequestSample {
    _private: (),
}

/// Point-in-time snapshot of metrics.
pub struct MetricsSnapshot {
    _private: (),
}

/// Metrics recorder with HdrHistogram.
pub struct MetricsRecorder {
    _private: (),
}
