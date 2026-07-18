//! Pure decision functions + the async iteration engine (plan 108-03).
//!
//! The iteration loop runs PURE between the three effect seams: every
//! between-await computation lives in [`decide`] as a synchronous,
//! side-effect-free function on data types (no wall-clock, no RNG — counters
//! only), and every value the loop produces is a `Serialize + Deserialize` data
//! type in [`result`]. That discipline is what makes the loop replay-safe
//! (AGNT-03) and its retry classification returnable as data (AGNT-02).

pub mod decide;
pub mod result;

pub use decide::{
    assistant_turn, check_limits, classify_retry, digest_tool_results, evaluate_submit_result,
    extract_token_usage, extract_tool_calls, is_end_turn, parse_completion, parse_tool_result,
    ErrorSignal,
};
pub use result::{IterationResult, LimitDecision, RunOutcome, TurnMessage};
