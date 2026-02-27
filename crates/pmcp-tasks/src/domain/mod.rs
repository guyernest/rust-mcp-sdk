//! Domain types for internal task representation.
//!
//! These types separate internal storage concerns from wire-format concerns.
//! [`TaskRecord`] is the store's internal representation with fields like
//! `owner_id`, `variables`, and `result`. [`TaskWithVariables`] is a
//! convenience type that injects shared variables into the wire [`Task`]'s
//! `_meta` field at the serialization boundary.

pub mod record;
pub mod variables;

pub use record::*;
pub use variables::*;
