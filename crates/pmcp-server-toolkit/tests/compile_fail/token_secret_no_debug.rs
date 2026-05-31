//! Must fail to compile because `SecretValue` does NOT implement `Debug`.
//! Phase 83 review R5 enforcement.

use pmcp_server_toolkit::secrets::SecretValue;

fn main() {
    let s = SecretValue::new(vec![1u8, 2, 3]);
    // This line MUST fail to compile — Debug not implemented on SecretValue.
    println!("{:?}", s);
}
