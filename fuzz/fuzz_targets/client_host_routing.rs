#![no_main]

use libfuzzer_sys::fuzz_target;
use pmcp::client::host::classify_host_request;
use pmcp::shared::parse_request;
use pmcp::types::elicitation::ElicitRequestParams;
use pmcp::types::jsonrpc::JSONRPCRequest;
use pmcp::types::sampling::CreateMessageParams;
use serde_json::{from_slice, from_value, Value};

// Fuzz the REAL inbound host-request routing path a client walks for every
// server -> client request: raw bytes -> JSONRPCRequest<Value> -> parse_request
// -> classify_host_request. This is the actual dispatch classification (not
// standalone serde on two param types), so it also exercises the parse
// ambiguity where inbound `sampling/createMessage` arrives as the CLIENT
// variant. The property under test: classification never panics and is total
// over every request `parse_request` can yield.
//
// Corpus cases worth seeding:
//   - both sampling parse variants (client-alias + server) for
//     `sampling/createMessage`
//   - `elicitation/create` and `roots/list`
//   - requests with missing / null / wrong-typed `params`
//   - deeply nested params and large content arrays
//   - unknown / non-host typed requests (must classify as Unhandled, no panic)
fuzz_target!(|data: &[u8]| {
    // Stage 1: JSON-RPC envelope parse. Bail on non-JSON / non-envelope input.
    let Ok(envelope) = from_slice::<JSONRPCRequest<Value>>(data) else {
        return;
    };

    // Also drive the raw param serde boundary directly (defensive: params may be
    // present even when the envelope is not a host method).
    let params_value = envelope.params.clone();
    if let Some(pv) = params_value.clone() {
        let _ = from_value::<CreateMessageParams>(pv.clone());
        let _ = from_value::<ElicitRequestParams>(pv);
    }

    // Stage 2: the real request grammar parse (client grammar tried first).
    let Ok((_id, request)) = parse_request(envelope) else {
        return;
    };

    // Stage 3: the real dispatch classification — must be total, never panic.
    let _kind = classify_host_request(&request);
});
