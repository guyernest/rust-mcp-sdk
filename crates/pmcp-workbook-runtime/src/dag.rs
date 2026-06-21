//! The PURE owned per-cell dependency-graph container [`Dag`] (CMP-02, D-06) +
//! Kahn's [`toposort`] over it.
//!
//! RELOCATED into `workbook-runtime` (Phase 11, Plan 05): the runtime executor's
//! `run()` re-runs an ALREADY-built+expanded `Dag` (the server deserializes a
//! pre-built IR + DAG), and it only needs the owned container + the toposort.
//! The DAG-BUILD path (`build_dag`/`collect_refs`/`cross_check_calcchain`) STAYS
//! in `workbook-compiler` (it links `crate::dialect`+`crate::ingest`); it imports
//! [`Dag`] + [`toposort`] from here.
//!
//! The forward map is `node → the keys it DEPENDS ON`. Kahn's topo-sort drains
//! zero-dependency nodes and walks dependency → dependents to decrement, so the
//! container also exposes a derived [`Dag::dependents`] reverse-map accessor.
//!
//! Owned, serde/schemars-clean (the umya-quarantine invariant): keys are plain
//! `String`s; no foreign type appears in any public signature.

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

use serde::Serialize;

/// A pure owned per-cell dependency graph.
///
/// `dependencies[node]` is the list of node keys `node` depends on. Build the
/// canonical `sheet!addr` keys with the shared `cell_key` helper in the CALLER
/// (workbook-compiler), then pass the finished strings here via
/// [`Dag::add_node`]/[`Dag::add_edge`].
#[derive(Debug, Clone, Default, Serialize, schemars::JsonSchema)]
pub struct Dag {
    /// node key → the node keys it DEPENDS ON.
    dependencies: HashMap<String, Vec<String>>,
}

impl Dag {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a node (no-op if it already exists). A node with no edges is a
    /// zero-dependency node Kahn drains first.
    pub fn add_node(&mut self, key: &str) {
        self.dependencies.entry(key.to_string()).or_default();
    }

    /// Record that `from` DEPENDS ON `depends_on`. Both endpoints are registered
    /// as nodes. Duplicate edges are not de-duplicated (the caller controls
    /// edge multiplicity; the lighthouse never produces duplicate-edge cycles).
    pub fn add_edge(&mut self, from: &str, depends_on: &str) {
        self.dependencies
            .entry(from.to_string())
            .or_default()
            .push(depends_on.to_string());
        // Ensure the dependency endpoint is also a known node.
        self.dependencies.entry(depends_on.to_string()).or_default();
    }

    /// Iterate over every node key.
    pub fn nodes(&self) -> impl Iterator<Item = &String> {
        self.dependencies.keys()
    }

    /// The cell keys `key` depends on (empty slice if `key` is unknown or has no
    /// dependencies).
    pub fn dependencies_of(&self, key: &str) -> &[String] {
        self.dependencies.get(key).map_or(&[], |v| v.as_slice())
    }

    /// Derive the REVERSE adjacency (dependency → the nodes that depend on it).
    ///
    /// This is the direction Kahn's algorithm walks to decrement in-degree as it
    /// drains zero-dependency nodes (finding #2). Every node appears as a key
    /// (with an empty `Vec` when nothing depends on it).
    pub fn dependents(&self) -> HashMap<String, Vec<String>> {
        let mut rev: HashMap<String, Vec<String>> = HashMap::new();
        for (node, deps) in &self.dependencies {
            rev.entry(node.clone()).or_default();
            for dep in deps {
                rev.entry(dep.clone()).or_default().push(node.clone());
            }
        }
        rev
    }
}

/// Topo-sort a [`Dag`] with Kahn's algorithm over the explicit
/// dependency-count/dependents contract (finding #2). On success returns the
/// nodes in dependency order (every node AFTER all its dependencies). On a cycle
/// returns `Err` carrying the residual set — the exact cell keys IN the cycle
/// (finding #7), sorted for a deterministic message.
pub fn toposort(dag: &Dag) -> Result<Vec<String>, Vec<String>> {
    // Remaining dependency count per node (in-degree on the dependency edge).
    let mut remaining: HashMap<String, usize> = HashMap::new();
    for node in dag.nodes() {
        remaining.insert(node.clone(), dag.dependencies_of(node).len());
    }

    let dependents = dag.dependents();

    // Seed the ready-queue with the zero-dependency (leaf input) nodes. Sort for
    // a deterministic topo order across HashMap iteration nondeterminism.
    let mut ready: Vec<String> = remaining
        .iter()
        .filter(|(_, &count)| count == 0)
        .map(|(k, _)| k.clone())
        .collect();
    ready.sort();
    let mut queue: VecDeque<String> = ready.into_iter().collect();

    let mut order: Vec<String> = Vec::with_capacity(remaining.len());
    while let Some(node) = queue.pop_front() {
        order.push(node.clone());
        // Walk the dependents (nodes that depend on `node`) and decrement.
        if let Some(deps) = dependents.get(&node) {
            // Sort for deterministic ordering of newly-ready nodes.
            let mut newly_ready: Vec<String> = Vec::new();
            for dependent in deps {
                if let Some(count) = remaining.get_mut(dependent) {
                    *count = count.saturating_sub(1);
                    if *count == 0 {
                        newly_ready.push(dependent.clone());
                    }
                }
            }
            newly_ready.sort();
            for n in newly_ready {
                queue.push_back(n);
            }
        }
    }

    // `remaining` holds exactly one entry per node (counts are decremented, not
    // removed), so its length is the node count without re-walking the Dag.
    if order.len() == remaining.len() {
        Ok(order)
    } else {
        // The residual — nodes never drained to zero — IS the cycle (finding #7).
        let mut residual: Vec<String> = remaining
            .iter()
            .filter(|(_, &count)| count > 0)
            .map(|(k, _)| k.clone())
            .collect();
        residual.sort();
        Err(residual)
    }
}

/// Collect the `Role::Input` LEAF cells transitively reachable UPSTREAM of
/// `output_cell` — each tool's minimal, DAG-derived input set (WBV2-03 §4.2).
///
/// Walks the "depends on" edge ([`Dag::dependencies_of`]) from `output_cell`
/// inward. A cell present in `input_cells` is a LEAF: it is collected and the
/// traversal stops there (an input never recurses into its own dependencies). A
/// cell NOT in `input_cells` (a constant or an intermediate formula) is NOT
/// collected — the traversal recurses through it — so a constant-only upstream
/// path is naturally EXCLUDED (constants are never in `input_cells`). When several
/// outputs share an intermediate, each output's call returns the union of ITS OWN
/// upstream leaves.
///
/// Determinism: results land in a [`BTreeSet`] (sorted by construction — the same
/// determinism discipline [`toposort`]'s `ready.sort()`/`newly_ready.sort()` use).
///
/// Total + cycle-safe: a `seen` guard prevents revisiting a node, so an arbitrary
/// (even CYCLIC) edge set TERMINATES — never recursing infinitely or overflowing
/// the stack. This is the totality Task 4's fuzz target proves over hostile DAGs.
pub fn upstream_input_leaves(
    dag: &Dag,
    output_cell: &str,
    input_cells: &HashSet<String>,
) -> BTreeSet<String> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut leaves: BTreeSet<String> = BTreeSet::new();
    let mut stack: Vec<String> = vec![output_cell.to_string()];
    while let Some(cell) = stack.pop() {
        // The `seen` guard makes the traversal terminate on a cyclic edge set.
        if !seen.insert(cell.clone()) {
            continue;
        }
        if input_cells.contains(&cell) {
            // A Role::Input cell is a LEAF — collect it and STOP (do not recurse
            // into its own dependencies).
            leaves.insert(cell);
            continue;
        }
        // A constant / intermediate: recurse through it (not collected).
        for dep in dag.dependencies_of(&cell) {
            stack.push(dep.clone());
        }
    }
    leaves
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_serde() {
        let mut dag = Dag::new();
        dag.add_edge("S!C1", "S!A1");
        let v = serde_json::to_value(&dag).expect("serialize Dag");
        assert_eq!(v["dependencies"]["S!C1"][0], "S!A1");
    }

    #[test]
    fn dependencies_of_returns_stored_keys() {
        let mut dag = Dag::new();
        dag.add_edge("S!C1", "S!A1");
        dag.add_edge("S!C1", "S!B1");
        assert_eq!(
            dag.dependencies_of("S!C1"),
            &["S!A1".to_string(), "S!B1".to_string()]
        );
        assert!(dag.dependencies_of("S!Z9").is_empty());
    }

    #[test]
    fn dependents_yields_the_reverse_map_kahn_needs() {
        let mut dag = Dag::new();
        dag.add_edge("S!C", "S!A");
        dag.add_edge("S!C", "S!B");

        let dependents = dag.dependents();
        assert_eq!(dependents.get("S!A"), Some(&vec!["S!C".to_string()]));
        assert_eq!(dependents.get("S!B"), Some(&vec!["S!C".to_string()]));
        assert_eq!(dependents.get("S!C"), Some(&Vec::<String>::new()));
    }

    #[test]
    fn add_node_registers_a_zero_dependency_node() {
        let mut dag = Dag::new();
        dag.add_node("S!A1");
        assert_eq!(dag.nodes().count(), 1);
        assert!(dag.dependencies_of("S!A1").is_empty());
    }

    #[test]
    fn toposort_orders_dependencies_before_dependents() {
        // C depends on A and B → A, B precede C.
        let mut dag = Dag::new();
        dag.add_node("S!A");
        dag.add_node("S!B");
        dag.add_edge("S!C", "S!A");
        dag.add_edge("S!C", "S!B");
        let order = toposort(&dag).expect("acyclic");
        let pos = |k: &str| order.iter().position(|n| n == k).unwrap();
        assert!(pos("S!A") < pos("S!C"));
        assert!(pos("S!B") < pos("S!C"));
    }

    #[test]
    fn toposort_returns_residual_on_a_cycle() {
        let mut dag = Dag::new();
        dag.add_edge("S!A", "S!B");
        dag.add_edge("S!B", "S!A");
        let residual = toposort(&dag).expect_err("a cycle must be Err");
        assert_eq!(residual, vec!["S!A".to_string(), "S!B".to_string()]);
    }

    // ---- upstream_input_leaves (WBV2-03 §4.2) ------------------------------

    fn inputs(keys: &[&str]) -> HashSet<String> {
        keys.iter().map(|k| (*k).to_string()).collect()
    }

    fn leaves(set: &BTreeSet<String>) -> Vec<String> {
        set.iter().cloned().collect()
    }

    #[test]
    fn upstream_input_leaves_returns_exactly_reachable_inputs() {
        // out depends on f1; f1 depends on income + filing (both inputs).
        let mut dag = Dag::new();
        dag.add_edge("Calc!out", "Calc!f1");
        dag.add_edge("Calc!f1", "In!income");
        dag.add_edge("Calc!f1", "In!filing");
        let input_cells = inputs(&["In!income", "In!filing", "In!withheld"]);
        let got = upstream_input_leaves(&dag, "Calc!out", &input_cells);
        // withheld is an input but NOT upstream of out → excluded (minimal).
        assert_eq!(leaves(&got), vec!["In!filing", "In!income"]);
    }

    #[test]
    fn upstream_input_leaves_excludes_constant_only_path() {
        // out depends on a constant (const_rate) that depends on nothing in
        // input_cells → the constant path contributes NO leaf.
        let mut dag = Dag::new();
        dag.add_edge("Calc!out", "In!income");
        dag.add_edge("Calc!out", "Const!rate"); // a constant cell (not an input)
        dag.add_edge("Const!rate", "Const!base"); // constant-only upstream
        let input_cells = inputs(&["In!income"]);
        let got = upstream_input_leaves(&dag, "Calc!out", &input_cells);
        assert_eq!(
            leaves(&got),
            vec!["In!income"],
            "a constant-only upstream path yields no input leaf"
        );
    }

    #[test]
    fn upstream_input_leaves_input_is_a_leaf_traversal_stops() {
        // An input cell that itself has upstream edges (pathological) must still be
        // a LEAF — the traversal stops at it and does NOT collect its dependencies.
        let mut dag = Dag::new();
        dag.add_edge("Calc!out", "In!income");
        dag.add_edge("In!income", "Const!hidden"); // never followed past the input
        let input_cells = inputs(&["In!income"]);
        let got = upstream_input_leaves(&dag, "Calc!out", &input_cells);
        assert_eq!(leaves(&got), vec!["In!income"]);
    }

    #[test]
    fn upstream_input_leaves_shared_intermediate_unions_per_output() {
        // A shared intermediate `shared` feeds two outputs; each output gets the
        // union of ITS OWN upstream input leaves.
        //   tax  = shared + filing   (shared <- income)
        //   refund = shared + withheld
        let mut dag = Dag::new();
        dag.add_edge("Calc!tax", "Calc!shared");
        dag.add_edge("Calc!tax", "In!filing");
        dag.add_edge("Calc!refund", "Calc!shared");
        dag.add_edge("Calc!refund", "In!withheld");
        dag.add_edge("Calc!shared", "In!income");
        let input_cells = inputs(&["In!income", "In!filing", "In!withheld"]);

        let tax = upstream_input_leaves(&dag, "Calc!tax", &input_cells);
        assert_eq!(
            leaves(&tax),
            vec!["In!filing", "In!income"],
            "tax = its own upstream leaves (income via shared + filing)"
        );
        let refund = upstream_input_leaves(&dag, "Calc!refund", &input_cells);
        assert_eq!(
            leaves(&refund),
            vec!["In!income", "In!withheld"],
            "refund = its own upstream leaves (income via shared + withheld)"
        );
    }

    #[test]
    fn upstream_input_leaves_terminates_on_a_cycle() {
        // A cyclic edge set must terminate (the seen-guard) — Task 4's fuzz relies
        // on this. a <-> b cycle upstream of out, with one real input leaf.
        let mut dag = Dag::new();
        dag.add_edge("Calc!out", "Calc!a");
        dag.add_edge("Calc!a", "Calc!b");
        dag.add_edge("Calc!b", "Calc!a"); // cycle
        dag.add_edge("Calc!a", "In!income");
        let input_cells = inputs(&["In!income"]);
        let got = upstream_input_leaves(&dag, "Calc!out", &input_cells);
        assert_eq!(leaves(&got), vec!["In!income"]);
    }

    // PROPERTY (SC2 Wave-0 gap): over a RANDOM acyclic DAG + a random input-cell
    // subset, the derived set is a SUBSET of input_cells AND every member has a
    // directed dependency path to the output (⊆ inputs ∧ reachable ∧ minimal).
    proptest::proptest! {
        #[test]
        fn prop_upstream_leaves_subset_and_reachable(
            // A layered acyclic DAG: node i may depend only on lower-indexed nodes
            // (so the generated graph is ALWAYS acyclic), plus a random input mask.
            edges in proptest::collection::vec(
                (0usize..12, 0usize..12),
                0..40,
            ),
            input_mask in proptest::collection::vec(proptest::bool::ANY, 12),
        ) {
            let node = |i: usize| format!("N{i}");
            let mut dag = Dag::new();
            for i in 0..12 {
                dag.add_node(&node(i));
            }
            // Keep only lower-index dependencies → guaranteed acyclic.
            for (from, dep) in &edges {
                if dep < from {
                    dag.add_edge(&node(*from), &node(*dep));
                }
            }
            let input_cells: HashSet<String> = (0..12)
                .filter(|i| input_mask[*i])
                .map(node)
                .collect();
            let output = node(11);
            let got = upstream_input_leaves(&dag, &output, &input_cells);

            // ⊆ inputs: every derived leaf is a declared input cell.
            for leaf in &got {
                proptest::prop_assert!(
                    input_cells.contains(leaf),
                    "derived leaf {leaf} must be an input cell"
                );
            }

            // reachable: every derived leaf has a directed depends-on path to the
            // output (re-walk dependencies_of independently of the impl under test).
            for leaf in &got {
                let mut seen: HashSet<String> = HashSet::new();
                let mut stack = vec![output.clone()];
                let mut reached = false;
                while let Some(c) = stack.pop() {
                    if c == *leaf {
                        reached = true;
                        break;
                    }
                    if !seen.insert(c.clone()) {
                        continue;
                    }
                    for d in dag.dependencies_of(&c) {
                        stack.push(d.clone());
                    }
                }
                proptest::prop_assert!(
                    reached,
                    "derived leaf {leaf} must be reachable upstream of {output}"
                );
            }
        }
    }
}
