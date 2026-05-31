//! Must fail to compile because `SecretValue` does NOT implement `Clone`.
//! Phase 83 review R5 enforcement.

use pmcp_server_toolkit::secrets::SecretValue;

fn main() {
    let s = SecretValue::new(vec![1u8, 2, 3]);
    // This line MUST fail to compile — Clone not implemented on SecretValue.
    let _t = s.clone();
}
