//! `executable.ir.json` emission (ART-01/D-03).
//!
//! Serializes the typed IR (`HashMap<String, Cell>`) to HUMAN-READABLE pretty
//! JSON — NOT a binary encoding (D-03 locked). The runtime IR types derive
//! `Deserialize`, so the emitted file re-parses to an EQUAL IR (the ART-01
//! round-trip contract) — the served loader deserializes the embedded bundle from
//! these exact bytes.
//!
//! Keys are serialized in CANONICAL sorted order (determinism): serializing the
//! `HashMap` directly emits keys in per-process random iteration order, which
//! breaks the idempotent re-emit gate (the `BUNDLE.lock` hashes these exact
//! bytes). The write routes through the deterministic
//! [`crate::artifact::serialize::to_bundle_json_sorted_map`] choke point — the
//! single place the sorted-key + pretty + no-trailing-newline policy lives. The IR
//! itself stays a `HashMap` (the deserialized runtime shape is unchanged).

use std::collections::HashMap;
use std::path::Path;

use pmcp_workbook_runtime::sheet_ir::Cell;

use super::serialize::to_bundle_json_sorted_map;
use super::{write_file, EmitError};

/// Serialize `ir` to deterministic pretty JSON (sorted keys, no trailing
/// newline), write it to `dir/executable.ir.json`, and return the JSON string
/// (the `BUNDLE.lock` hashes these exact bytes).
///
/// # Errors
/// Returns [`EmitError::Serde`] on a serialization failure or [`EmitError::Io`]
/// on a write failure.
pub fn emit_executable(ir: &HashMap<String, Cell>, dir: &Path) -> Result<String, EmitError> {
    let ir_json = to_bundle_json_sorted_map(ir, "executable.ir.json")?;
    write_file(&dir.join("executable.ir.json"), &ir_json)?;
    Ok(ir_json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pmcp_workbook_runtime::sheet_ir::CellExpr;
    use pmcp_workbook_runtime::CellValue;

    fn sample_ir() -> HashMap<String, Cell> {
        let mut ir = HashMap::new();
        ir.insert(
            "2_Constants!B2".to_string(),
            Cell {
                key: "2_Constants!B2".to_string(),
                expr: CellExpr::Literal(CellValue::Number(1.05)),
            },
        );
        ir.insert(
            "3_Outputs!B2".to_string(),
            Cell {
                key: "3_Outputs!B2".to_string(),
                expr: CellExpr::Formula(pmcp_workbook_runtime::formula::Expr::Number(700.0)),
            },
        );
        ir
    }

    #[test]
    fn executable_emits_pretty_and_reparses() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let ir = sample_ir();
        let json = emit_executable(&ir, dir.path()).expect("emit executable");

        // The file exists and its bytes are PRETTY (contain newlines).
        let path = dir.path().join("executable.ir.json");
        assert!(path.exists(), "executable.ir.json must be written");
        let bytes = std::fs::read_to_string(&path).expect("read back");
        assert!(
            bytes.contains('\n'),
            "pretty JSON must contain newlines (D-03 human-readable)"
        );
        assert_eq!(bytes, json, "returned string equals the on-disk bytes");
        assert!(
            !bytes.ends_with('\n'),
            "no trailing newline (golden policy)"
        );

        // Re-parses to an EQUAL IR (ART-01 round-trip).
        let back: HashMap<String, Cell> =
            serde_json::from_str(&bytes).expect("re-parse executable.ir.json");
        assert_eq!(back, ir, "the emitted IR re-parses to an equal IR");
    }

    #[test]
    fn executable_emit_is_deterministic_across_runs() {
        let dir1 = tempfile::TempDir::new().expect("tempdir 1");
        let dir2 = tempfile::TempDir::new().expect("tempdir 2");
        let ir = sample_ir();
        let a = emit_executable(&ir, dir1.path()).expect("emit 1");
        let b = emit_executable(&ir, dir2.path()).expect("emit 2");
        assert_eq!(a, b, "two emits of the same IR are byte-identical");
    }
}
