// Auto-generated contract assertions from YAML — DO NOT EDIT.
// Zero cost in release builds (debug_assert!).
// Regenerate: pv codegen contracts/ -o src/generated_contracts.rs
// Include:   #[macro_use] #[allow(unused_macros)] mod generated_contracts;

// Auto-generated from contracts/mcp-protocol-sdk-v1.yaml — DO NOT EDIT
// Contract: mcp-protocol-sdk-v1

/// Preconditions for equation `batch_request_ordering`.
/// Call at function entry: `contract_pre_batch_request_ordering!(input_expr)`
macro_rules! contract_pre_batch_request_ordering {
    () => {{}};
    ($input:expr) => {{
        let _contract_input = &$input;
    }};
}

/// Postconditions for equation `batch_request_ordering`.
/// Call before return: `contract_post_batch_request_ordering!(result_expr)`
macro_rules! contract_post_batch_request_ordering {
    ($result:expr) => {{
        let _contract_result = &$result;
    }};
}

/// Combined pre+post contract for equation `batch_request_ordering`.
macro_rules! contract_batch_request_ordering {
    ($input:expr, $body:expr) => {{
        contract_pre_batch_request_ordering!($input);
        let _contract_result = $body;
        contract_post_batch_request_ordering!(_contract_result);
        _contract_result
    }};
}

/// Preconditions for equation `cancellation_safety`.
/// Call at function entry: `contract_pre_cancellation_safety!(input_expr)`
macro_rules! contract_pre_cancellation_safety {
    () => {{}};
    ($input:expr) => {{
        let _contract_input = &$input;
    }};
}

/// Postconditions for equation `cancellation_safety`.
/// Call before return: `contract_post_cancellation_safety!(result_expr)`
macro_rules! contract_post_cancellation_safety {
    ($result:expr) => {{
        let _contract_result = &$result;
    }};
}

/// Combined pre+post contract for equation `cancellation_safety`.
macro_rules! contract_cancellation_safety {
    ($input:expr, $body:expr) => {{
        contract_pre_cancellation_safety!($input);
        let _contract_result = $body;
        contract_post_cancellation_safety!(_contract_result);
        _contract_result
    }};
}

/// Preconditions for equation `error_code_mapping`.
/// Call at function entry: `contract_pre_error_code_mapping!(input_expr)`
macro_rules! contract_pre_error_code_mapping {
    () => {{}};
    ($input:expr) => {{
        let _contract_input = &$input;
    }};
}

/// Postconditions for equation `error_code_mapping`.
/// Call before return: `contract_post_error_code_mapping!(result_expr)`
macro_rules! contract_post_error_code_mapping {
    ($result:expr) => {{
        let _contract_result = &$result;
    }};
}

/// Combined pre+post contract for equation `error_code_mapping`.
macro_rules! contract_error_code_mapping {
    ($input:expr, $body:expr) => {{
        contract_pre_error_code_mapping!($input);
        let _contract_result = $body;
        contract_post_error_code_mapping!(_contract_result);
        _contract_result
    }};
}

/// Preconditions for equation `jsonrpc_framing`.
/// Call at function entry: `contract_pre_jsonrpc_framing!(input_expr)`
macro_rules! contract_pre_jsonrpc_framing {
    () => {{}};
    ($input:expr) => {{
        let _contract_input = &$input;
    }};
}

/// Postconditions for equation `jsonrpc_framing`.
/// Call before return: `contract_post_jsonrpc_framing!(result_expr)`
macro_rules! contract_post_jsonrpc_framing {
    ($result:expr) => {{
        let _contract_result = &$result;
    }};
}

/// Combined pre+post contract for equation `jsonrpc_framing`.
macro_rules! contract_jsonrpc_framing {
    ($input:expr, $body:expr) => {{
        contract_pre_jsonrpc_framing!($input);
        let _contract_result = $body;
        contract_post_jsonrpc_framing!(_contract_result);
        _contract_result
    }};
}

/// Preconditions for equation `payload_limits`.
/// Call at function entry: `contract_pre_payload_limits!(input_expr)`
macro_rules! contract_pre_payload_limits {
    () => {{}};
    ($input:expr) => {{
        let _contract_input = &$input;
    }};
}

/// Postconditions for equation `payload_limits`.
/// Call before return: `contract_post_payload_limits!(result_expr)`
macro_rules! contract_post_payload_limits {
    ($result:expr) => {{
        let _contract_result = &$result;
    }};
}

/// Combined pre+post contract for equation `payload_limits`.
macro_rules! contract_payload_limits {
    ($input:expr, $body:expr) => {{
        contract_pre_payload_limits!($input);
        let _contract_result = $body;
        contract_post_payload_limits!(_contract_result);
        _contract_result
    }};
}

/// Preconditions for equation `protocol_version_negotiation`.
/// Call at function entry: `contract_pre_protocol_version_negotiation!(input_expr)`
macro_rules! contract_pre_protocol_version_negotiation {
    () => {{}};
    ($input:expr) => {{
        let _contract_input = &$input;
    }};
}

/// Postconditions for equation `protocol_version_negotiation`.
/// Call before return: `contract_post_protocol_version_negotiation!(result_expr)`
macro_rules! contract_post_protocol_version_negotiation {
    ($result:expr) => {{
        let _contract_result = &$result;
    }};
}

/// Combined pre+post contract for equation `protocol_version_negotiation`.
macro_rules! contract_protocol_version_negotiation {
    ($input:expr, $body:expr) => {{
        contract_pre_protocol_version_negotiation!($input);
        let _contract_result = $body;
        contract_post_protocol_version_negotiation!(_contract_result);
        _contract_result
    }};
}

/// Preconditions for equation `session_lifecycle`.
/// Call at function entry: `contract_pre_session_lifecycle!(input_expr)`
macro_rules! contract_pre_session_lifecycle {
    () => {{}};
    ($input:expr) => {{
        let _contract_input = &$input;
    }};
}

/// Postconditions for equation `session_lifecycle`.
/// Call before return: `contract_post_session_lifecycle!(result_expr)`
macro_rules! contract_post_session_lifecycle {
    ($result:expr) => {{
        let _contract_result = &$result;
    }};
}

/// Combined pre+post contract for equation `session_lifecycle`.
macro_rules! contract_session_lifecycle {
    ($input:expr, $body:expr) => {{
        contract_pre_session_lifecycle!($input);
        let _contract_result = $body;
        contract_post_session_lifecycle!(_contract_result);
        _contract_result
    }};
}

/// Preconditions for equation `tool_dispatch_integrity`.
/// Call at function entry: `contract_pre_tool_dispatch_integrity!(input_expr)`
macro_rules! contract_pre_tool_dispatch_integrity {
    () => {{}};
    ($input:expr) => {{
        let _contract_input = &$input;
    }};
}

/// Postconditions for equation `tool_dispatch_integrity`.
/// Call before return: `contract_post_tool_dispatch_integrity!(result_expr)`
macro_rules! contract_post_tool_dispatch_integrity {
    ($result:expr) => {{
        let _contract_result = &$result;
    }};
}

/// Combined pre+post contract for equation `tool_dispatch_integrity`.
macro_rules! contract_tool_dispatch_integrity {
    ($input:expr, $body:expr) => {{
        contract_pre_tool_dispatch_integrity!($input);
        let _contract_result = $body;
        contract_post_tool_dispatch_integrity!(_contract_result);
        _contract_result
    }};
}

/// Preconditions for equation `transport_abstraction`.
/// Call at function entry: `contract_pre_transport_abstraction!(input_expr)`
macro_rules! contract_pre_transport_abstraction {
    () => {{}};
    ($input:expr) => {{
        let _contract_input = &$input;
    }};
}

/// Postconditions for equation `transport_abstraction`.
/// Call before return: `contract_post_transport_abstraction!(result_expr)`
macro_rules! contract_post_transport_abstraction {
    ($result:expr) => {{
        let _contract_result = &$result;
    }};
}

/// Combined pre+post contract for equation `transport_abstraction`.
macro_rules! contract_transport_abstraction {
    ($input:expr, $body:expr) => {{
        contract_pre_transport_abstraction!($input);
        let _contract_result = $body;
        contract_post_transport_abstraction!(_contract_result);
        _contract_result
    }};
}

// Total: 0 preconditions, 0 postconditions from 1 contracts
