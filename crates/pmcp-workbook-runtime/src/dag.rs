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

use std::collections::{HashMap, VecDeque};

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
}
