//! Shared helpers for the toolkit integration tests.
#![allow(dead_code)] // not every integration binary uses every helper

use std::sync::{Mutex, MutexGuard};

/// Per-test-binary lock serializing tests that read or mutate the shared
/// process environment via `std::env::{set_var, remove_var}`.
///
/// Those calls are process-global and not thread-safe, so under the default
/// multi-threaded test runner concurrent env-touching tests within one binary
/// corrupt each other's variables (e.g. a pipeline build fails to read the
/// secret a sibling test just set). Acquire this at the top of any env-touching
/// test and hold it for the test body — but NEVER across an `.await` (the `std`
/// `MutexGuard` is `!Send`; keep the locked section synchronous).
///
/// Each integration test binary links its own copy of this static, which is
/// exactly right: separate binaries run as separate processes and need no
/// cross-binary coordination.
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Lock the process-env mutex, recovering from poisoning so a panicking test
/// does not cascade-fail its siblings.
pub fn env_lock() -> MutexGuard<'static, ()> {
    ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
