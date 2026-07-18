//! Dispatch guards: strict depth parsing, self-call rejection, and
//! ancestor-cycle rejection — all comparing [`super::identity::MemberId`]s,
//! never display names — over guard state carried as namespaced `_meta`.
//!
//! Empty documented seam — implemented in 109-05 (fuzzed by `fuzz/team_guards`).
