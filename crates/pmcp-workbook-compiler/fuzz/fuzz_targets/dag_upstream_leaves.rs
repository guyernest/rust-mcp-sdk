//! Fuzz target over `upstream_input_leaves` — the per-tool input-derivation
//! reachability traversal (WBV2-03 §4.2, T-100-06 tampering, ALWAYS: FUZZ).
//!
//! `upstream_input_leaves(dag, output_cell, input_cells)` walks the `depends-on`
//! edge from an output cell, collecting the `Role::Input` leaves transitively
//! upstream. The compiler feeds it a DAG built from BA-authored formulas; this
//! target feeds ARBITRARY (including CYCLIC and self-referential) edge sets to
//! prove the `seen`-guard keeps the traversal TOTAL.
//!
//! # Invariant
//!
//! For ANY input the call:
//!   1. ALWAYS returns a `BTreeSet<String>` — NEVER panics, hangs, stack-overflows,
//!      or unbounded-allocates (the cycle-safety the `seen` guard provides), AND
//!   2. the returned set is a SUBSET of `input_cells` — it NEVER leaks a
//!      non-input (constant/formula) cell as a derived tool input (T-100-06: a
//!      computed/constant cell wrongly advertised as a caller input is a tampering
//!      risk). This is the SAME `⊆ inputs` invariant the runtime property test
//!      asserts, here proven over hostile DAGs.

#![no_main]

use std::collections::{BTreeSet, HashSet};

use libfuzzer_sys::fuzz_target;
use pmcp_workbook_runtime::dag::{upstream_input_leaves, Dag};

/// A small, bounded node namespace so the fuzzer densely explores edge topologies
/// (incl. cycles, self-loops, disconnected components) rather than unique strings.
const NODE_COUNT: usize = 16;

fn node(i: u8) -> String {
    format!("N{}", i as usize % NODE_COUNT)
}

fuzz_target!(|data: &[u8]| {
    // Need at least an output-cell selector + an input mask byte to do anything.
    if data.len() < 2 {
        return;
    }

    // Build a Dag from arbitrary byte-pairs as directed edges. Pairs may form
    // cycles, self-loops, or duplicate edges — exactly the hostile shapes the
    // `seen` guard must survive. Register every node so isolated nodes exist too.
    let mut dag = Dag::new();
    for i in 0..NODE_COUNT {
        dag.add_node(&node(i as u8));
    }
    // Reserve the first two bytes for the output selector + the input mask; the
    // remainder are consumed in (from, depends_on) pairs.
    let output = node(data[0]);
    let input_mask = data[1];

    let mut bytes = data[2..].chunks_exact(2);
    for pair in &mut bytes {
        // `from` DEPENDS ON `depends_on` — a cyclic/self edge is intentional.
        dag.add_edge(&node(pair[0]), &node(pair[1]));
    }

    // Derive an arbitrary input-cell subset from the mask byte (bit i → node i is
    // an input). Capped at 8 nodes by the byte width, which is plenty to exercise
    // "an input is a leaf" vs "a constant recurses" branches.
    let mut input_cells: HashSet<String> = HashSet::new();
    for bit in 0..8u8 {
        if input_mask & (1 << bit) != 0 {
            input_cells.insert(node(bit));
        }
    }

    // The call must be TOTAL: it returns, never panics/hangs over the cyclic graph.
    let leaves: BTreeSet<String> = upstream_input_leaves(&dag, &output, &input_cells);

    // Subset invariant (T-100-06): no non-input cell may leak as a derived input.
    for leaf in &leaves {
        assert!(
            input_cells.contains(leaf),
            "upstream_input_leaves leaked a non-input cell {leaf:?} (output={output:?})"
        );
    }
});
