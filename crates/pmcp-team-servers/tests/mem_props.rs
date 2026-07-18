//! Property-based invariants for the mem-mcp BM25 scorer and in-memory backend
//! (the ALWAYS property requirement for TEAM-04).
//!
//! These replace the invalid *global* length-normalization monotonicity claim
//! flagged by the 109-03 review with invariants that actually hold under BM25:
//!
//! - (a) NON-NEGATIVITY — every score is `>= 0.0`.
//! - (b) ZERO FOR NO OVERLAP — a document sharing no query term scores `0.0`.
//! - (c) DETERMINISM — identical `(corpus, query)` yields an identical ranking
//!   on every run (both at the scorer and the backend level).
//! - (d) FINITE — no score is `NaN`/`Inf` for any generated input.
//! - (e) STABLE TIE-BREAK — equal-score documents always order by creation
//!   ordinal ascending.
//! - (f) TF-MONOTONICITY AT FIXED DOC LENGTH — at a fixed document length,
//!   increasing a query term's frequency never lowers the score versus a
//!   same-length document lacking the term.

use std::collections::HashSet;
use std::sync::Arc;

use proptest::prelude::*;

use pmcp_team_servers::mem::backend::{InMemoryMemoryBackend, TeamMemoryBackend};
use pmcp_team_servers::mem::bm25::Bm25Index;

/// A single token drawn from a small alphabet so overlaps are common.
fn word() -> impl Strategy<Value = String> {
    "[a-e]{1,4}".prop_map(|s| s.to_lowercase())
}

/// A document as a sequence of 0..8 tokens.
fn doc() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec(word(), 0..8)
}

/// A corpus of 0..6 documents.
fn corpus() -> impl Strategy<Value = Vec<Vec<String>>> {
    prop::collection::vec(doc(), 0..6)
}

/// A query of 0..4 tokens.
fn query() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec(word(), 0..4)
}

fn build_index(corpus: &[Vec<String>]) -> Bm25Index {
    let mut index = Bm25Index::new();
    for d in corpus {
        index.add_doc(d);
    }
    index
}

/// Builds a single-threaded runtime to drive the async backend synchronously.
fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(fut)
}

proptest! {
    /// (a) non-negativity + (d) finiteness.
    #[test]
    fn scores_are_non_negative_and_finite(corpus in corpus(), query in query()) {
        let index = build_index(&corpus);
        for doc_id in 0..corpus.len() {
            let score = index.score(&query, doc_id);
            prop_assert!(score >= 0.0, "score must be >= 0, got {score}");
            prop_assert!(score.is_finite(), "score must be finite, got {score}");
        }
    }

    /// (b) zero for no overlap.
    #[test]
    fn no_overlap_scores_zero(corpus in corpus(), query in query()) {
        let index = build_index(&corpus);
        for (doc_id, d) in corpus.iter().enumerate() {
            let doc_terms: HashSet<&String> = d.iter().collect();
            let overlaps = query.iter().any(|t| doc_terms.contains(t));
            if !overlaps {
                let score = index.score(&query, doc_id);
                prop_assert_eq!(score, 0.0, "no-overlap doc must score exactly 0.0");
            }
        }
    }

    /// (c) scorer determinism — rebuilding and rescoring is identical.
    #[test]
    fn scorer_is_deterministic(corpus in corpus(), query in query()) {
        let a = build_index(&corpus);
        let b = build_index(&corpus);
        for doc_id in 0..corpus.len() {
            prop_assert_eq!(a.score(&query, doc_id), b.score(&query, doc_id));
        }
    }

    /// (f) tf-monotonicity at FIXED document length.
    ///
    /// Two documents of identical length `len`: doc 0 contains the query term
    /// `x` exactly `k` times (rest filler `y`), doc 1 contains it zero times.
    /// The higher-frequency doc must score at least as high.
    #[test]
    fn higher_tf_scores_higher_at_fixed_length(len in 1usize..8, k in 1usize..8) {
        let k = k.min(len);
        let mut doc0: Vec<String> = std::iter::repeat_n("x".to_string(), k).collect();
        doc0.extend(std::iter::repeat_n("y".to_string(), len - k));
        let doc1: Vec<String> = std::iter::repeat_n("y".to_string(), len).collect();

        let index = build_index(&[doc0, doc1]);
        let q = vec!["x".to_string()];
        let s0 = index.score(&q, 0);
        let s1 = index.score(&q, 1);
        prop_assert!(s0 >= s1, "higher-tf doc ({s0}) must score >= term-absent doc ({s1})");
        prop_assert!(s0 > 0.0, "term-present doc must score > 0");
    }

    /// (c) backend search determinism — identical corpus + query yields an
    /// identical id ordering on repeated runs.
    #[test]
    fn backend_search_is_deterministic(corpus in corpus(), query in query()) {
        let query_text = query.join(" ");
        let first = block_on(async {
            let backend = InMemoryMemoryBackend::deterministic();
            for d in &corpus {
                let text = d.join(" ");
                if !text.is_empty() {
                    backend.add(text, vec![]).await.unwrap();
                }
            }
            backend.search(&query_text, 100).await.unwrap()
        });
        let second = block_on(async {
            let backend = InMemoryMemoryBackend::deterministic();
            for d in &corpus {
                let text = d.join(" ");
                if !text.is_empty() {
                    backend.add(text, vec![]).await.unwrap();
                }
            }
            backend.search(&query_text, 100).await.unwrap()
        });
        let ids_a: Vec<&str> = first.iter().map(|m| m.id.as_str()).collect();
        let ids_b: Vec<&str> = second.iter().map(|m| m.id.as_str()).collect();
        prop_assert_eq!(ids_a, ids_b);
    }

    /// (e) stable tie-break — N identical documents (identical scores) always
    /// come back ordered by creation ordinal ascending (mem-001, mem-002, …).
    #[test]
    fn equal_scores_break_by_creation_ordinal(n in 1usize..6) {
        let ids = block_on(async {
            let backend: Arc<dyn TeamMemoryBackend> =
                Arc::new(InMemoryMemoryBackend::deterministic());
            for _ in 0..n {
                backend.add("same shared token".to_string(), vec![]).await.unwrap();
            }
            backend
                .search("shared", 100)
                .await
                .unwrap()
                .into_iter()
                .map(|m| m.id)
                .collect::<Vec<_>>()
        });
        let expected: Vec<String> = (1..=n).map(|i| format!("mem-{i:03}")).collect();
        prop_assert_eq!(ids, expected);
    }
}
