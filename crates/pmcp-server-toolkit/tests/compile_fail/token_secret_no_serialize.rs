//! Must fail to compile because `SecretValue` does NOT implement `Serialize`.
//! Phase 83 review R5 enforcement.

use pmcp_server_toolkit::secrets::SecretValue;

fn main() {
    let s = SecretValue::new(vec![1u8, 2, 3]);
    // This line MUST fail to compile — Serialize not implemented on SecretValue.
    let _t = serde_json::to_string(&s).unwrap();
}
