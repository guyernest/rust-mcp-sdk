#![no_main]

use libfuzzer_sys::fuzz_target;
use pmcp::server::roots::ListRootsResult;
use pmcp::types::sampling::{CreateMessageParams, CreateMessageResult};
use serde_json::{from_slice, from_value, Value};

// Fuzz the serde boundary that `DispatchPeerHandle::sample` and
// `DispatchPeerHandle::list_roots` rely on when deserializing client
// responses via `serde_json::from_value`. The dispatcher returns an
// arbitrary `Value`; the peer impl must never panic on adversarial JSON —
// valid inputs round-trip, invalid inputs produce `Err`.
//
// Target surfaces:
// - `CreateMessageParams`  — outbound request (client may echo shape back)
// - `CreateMessageResult`  — sampling/createMessage response
// - `ListRootsResult`      — roots/list response
fuzz_target!(|data: &[u8]| {
    let Ok(json) = from_slice::<Value>(data) else {
        return;
    };
    let _ = from_value::<CreateMessageParams>(json.clone());
    let _ = from_value::<CreateMessageResult>(json.clone());
    let _ = from_value::<ListRootsResult>(json);
});
