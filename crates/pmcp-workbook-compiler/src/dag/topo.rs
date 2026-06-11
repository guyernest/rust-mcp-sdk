//! Kahn topo-sort + cycle detection + the [`build_dag`] entry point (WBCO-03,
//! D-06).
//!
//! The dependency DAG is reconstructed SOLELY from the parsed [`Expr`]
//! references (never from `calcChain.xml`). [`toposort`] (re-exported from
//! `pmcp-workbook-runtime`) runs Kahn's algorithm: it drains nodes whose
//! DEPENDENCY-count is zero (the leaf inputs) and walks the reverse map to
//! decrement each dependent's remaining count — so a formula cell `C = A + B`
//! topo-sorts AFTER the cells it reads (A and B precede C). The residual set of
//! never-emitted nodes IS the cycle, surfaced as a typed
//! [`DagBuildError::Cycle`] enumerating the cells (NOT a `LintFinding` — the
//! parser/DAG transform boundary stays crisp vs the linter's reporting
//! boundary).
//!
//! No petgraph: the toposort is the runtime's hand-rolled Kahn implementation.
//! The build operates on a SYNTHETIC [`ParsedCell`] slice + a synthetic
//! defined-name table — never 93-02's owned cell model — so this plan stays
//! parallel with 93-02.

use pmcp_workbook_runtime::range_ref::cell_key;
use pmcp_workbook_runtime::Expr;

// The pure `Dag` container + Kahn's `toposort` live in `pmcp-workbook-runtime`
// (the served binary re-runs an already-built `Dag`); the DAG-BUILD path stays
// here and re-exports them so `crate::dag::topo::{toposort, Dag}` resolves.
pub use pmcp_workbook_runtime::{toposort, Dag};

use crate::dag::resolve::{self, DagBuildError};
use crate::dialect::DefinedName;

/// One parsed formula cell handed to [`build_dag`]: the sheet + A1 address it
/// lives on and its parsed [`Expr`]. The DAG is reconstructed from these — never
/// from `calcChain.xml` (D-08).
#[derive(Debug, Clone)]
pub struct ParsedCell {
    /// The sheet the formula cell lives on (e.g. `"5_Quantities"`).
    pub sheet: String,
    /// The A1 address within `sheet` (e.g. `"C6"`).
    pub addr: String,
    /// The parsed formula expression.
    pub expr: Expr,
}

/// Reconstruct the dependency [`Dag`] from the parsed `Expr` references (WBCO-03)
/// and topo-sort it. For every parsed formula cell this adds a node and one
/// `add_edge(cell, dep)` per resolved dependency key (D-06, via
/// [`resolve::collect_refs`]); a cycle is a typed [`DagBuildError::Cycle`]
/// ENUMERATING the residual cells. The DAG is built SOLELY from `parsed` —
/// `calcChain.xml` is NEVER consulted on the build path (D-08).
///
/// Returns the [`Dag`] and the topo order.
///
/// # Errors
/// Returns [`DagBuildError`] on a reference-resolution failure (too-large /
/// malformed range, unknown defined name) or a dependency cycle.
pub fn build_dag(
    parsed: &[ParsedCell],
    names: &[DefinedName],
) -> Result<(Dag, Vec<String>), DagBuildError> {
    let mut dag = Dag::new();

    for cell in parsed {
        let node = cell_key(&cell.sheet, &cell.addr);
        dag.add_node(&node);
        let deps = resolve::collect_refs(&cell.expr, &cell.sheet, names)?;
        for dep in deps {
            dag.add_edge(&node, &dep);
        }
    }

    match toposort(&dag) {
        Ok(order) => Ok((dag, order)),
        Err(residual) => Err(DagBuildError::Cycle(residual)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pmcp_workbook_runtime::BinOp;

    fn cell(sheet: &str, addr: &str, expr: Expr) -> ParsedCell {
        ParsedCell {
            sheet: sheet.to_string(),
            addr: addr.to_string(),
            expr,
        }
    }

    /// kahn_toposort_orders_deps: a 3-cell dependency chain (built from a
    /// synthetic cell source) topo-sorts in dependency order.
    #[test]
    fn kahn_toposort_orders_deps() {
        // D = C, C = A + B → A,B before C before D.
        let names: Vec<DefinedName> = Vec::new();
        let parsed = vec![
            cell(
                "S",
                "C1",
                Expr::BinaryOp {
                    left: Box::new(Expr::Ref("A1".to_string())),
                    op: BinOp::Add,
                    right: Box::new(Expr::Ref("B1".to_string())),
                },
            ),
            cell("S", "D1", Expr::Ref("C1".to_string())),
        ];
        let (_dag, order) = build_dag(&parsed, &names).expect("acyclic");
        let idx = |k: &str| order.iter().position(|n| n == k).expect("node present");
        assert!(idx("S!A1") < idx("S!C1"));
        assert!(idx("S!B1") < idx("S!C1"));
        assert!(idx("S!C1") < idx("S!D1"));
    }

    /// cycle_detected: a circular reference is detected and surfaced as a typed
    /// error (not an infinite loop), enumerating the cells in the cycle.
    #[test]
    fn cycle_detected() {
        let names: Vec<DefinedName> = Vec::new();
        let parsed = vec![
            cell("S", "A1", Expr::Ref("B1".to_string())),
            cell("S", "B1", Expr::Ref("A1".to_string())),
        ];
        let err = build_dag(&parsed, &names).expect_err("a cycle must be a typed error");
        match err {
            DagBuildError::Cycle(residual) => {
                assert!(
                    residual.contains(&"S!A1".to_string())
                        && residual.contains(&"S!B1".to_string()),
                    "the cycle must enumerate both cells, got {residual:?}"
                );
            },
            other => panic!("expected a Cycle error, got {other:?}"),
        }
    }

    #[test]
    fn single_cell_chain_orders_dependency_first() {
        // C = A + B → A and B precede C.
        let names: Vec<DefinedName> = Vec::new();
        let parsed = vec![cell(
            "S",
            "C1",
            Expr::BinaryOp {
                left: Box::new(Expr::Ref("A1".to_string())),
                op: BinOp::Add,
                right: Box::new(Expr::Ref("B1".to_string())),
            },
        )];
        let (_dag, order) = build_dag(&parsed, &names).expect("acyclic");
        let idx = |k: &str| order.iter().position(|n| n == k).expect("present");
        assert!(idx("S!A1") < idx("S!C1"));
        assert!(idx("S!B1") < idx("S!C1"));
    }

    #[test]
    fn unknown_name_in_a_cell_propagates_the_typed_error() {
        let names: Vec<DefinedName> = Vec::new();
        let parsed = vec![cell("S", "C1", Expr::Name("Missing".to_string()))];
        let err = build_dag(&parsed, &names).expect_err("unknown name must error");
        assert_eq!(err, DagBuildError::UnknownName("Missing".to_string()));
    }
}
