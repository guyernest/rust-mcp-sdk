//! Authentic in-process Athena mock — filled by Plan 07 Task 2.
//!
//! REVIEWS H5: lives under `src/` so examples can reach it via the `dev_mock`
//! feature without `#[path = "../tests/..."]`. Task 2 lands `AthenaMock`.

#![cfg(any(test, feature = "dev_mock"))]
