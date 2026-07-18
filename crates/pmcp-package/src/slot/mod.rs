//! Config-slot type system (,): typed slot declarations that structurally
//! cannot carry a secret/identity value, classification into identity-bearing vs
//! behavior-relevant, aggregation across a component graph, and deviation detection for
//! behavior-relevant slots.
//!
//! See `types`/`classification` module docs for the structural "secrets never travel"
//! guarantee this module tree enforces, and `aggregate`/`deviation` for the pure functions
//! the pre-flight will call.

pub mod aggregate;
pub mod classification;
pub mod deviation;
pub mod types;

pub use aggregate::aggregate;
pub use classification::{classify, SlotClass};
pub use deviation::{detect_deviation, Deviation};
pub use types::{ConfigSlot, SlotType};
