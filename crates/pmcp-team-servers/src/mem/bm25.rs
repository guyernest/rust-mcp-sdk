//! A small dependency-free BM25 ranker backing `mem__search` (the `bm25`
//! third-party crate is deliberately NOT adopted — it bundles an embedder, and
//! TEAM-04 + the milestone put vector search out of scope; dev-grade only).
//!
//! # Formula
//!
//! For a query `Q` scored against document `D` in a corpus of `N` documents:
//!
//! ```text
//! score(D, Q) = Σ_{t ∈ Q}  IDF(t) · ( f(t,D) · (k1 + 1) )
//!                           ───────────────────────────────────────────
//!                            f(t,D) + k1 · (1 − b + b · |D| / L_avg)
//! ```
//!
//! where `f(t,D)` is the term frequency of `t` in `D`, `|D|` is the document
//! length (token count), `L_avg` is the corpus average document length, and
//! `IDF(t)` is the inverse document frequency of `t`.
//!
//! # Constants
//!
//! `k1 = 1.2` controls term-frequency saturation and `b = 0.75` controls
//! length normalization — the canonical BM25 defaults, adequate for a dev-grade
//! keyword ranker.
//!
//! # Numeric safety (109-03 review)
//!
//! - **Empty corpus / `L_avg == 0`** → [`Bm25Index::score`] returns `0.0`
//!   immediately, so the `|D| / L_avg` term never divides by zero.
//! - **Smoothed IDF** — `IDF(t) = ln(1 + (N − n + 0.5)/(n + 0.5))`. Because the
//!   argument is always `> 1`, the logarithm is always `> 0`; a term appearing
//!   in more than half the corpus therefore still contributes a *non-negative*
//!   score (the raw Robertson/Spärck-Jones IDF can go negative — the smoothed
//!   form used here never does). This is preferred over the floored variant
//!   `raw_idf.max(0.0)` because it keeps a small positive signal for common
//!   terms instead of collapsing them to zero.
//!
//! These guarantees are proven by the in-file unit tests and the
//! `tests/mem_props.rs` property suite (non-negative, finite, deterministic).

use std::collections::HashMap;

/// BM25 term-frequency saturation parameter (canonical default).
pub const K1: f64 = 1.2;

/// BM25 length-normalization parameter (canonical default).
pub const B: f64 = 0.75;

/// Splits `text` into lowercase alphanumeric terms.
///
/// Tokenization is deterministic: it lowercases and splits on any
/// non-alphanumeric character, discarding empty fragments. Unicode alphanumeric
/// characters are preserved (via [`char::is_alphanumeric`]).
///
/// # Examples
///
/// ```
/// use pmcp_team_servers::mem::bm25::tokenize;
///
/// assert_eq!(tokenize("Hello, World!"), vec!["hello", "world"]);
/// assert_eq!(tokenize("  "), Vec::<String>::new());
/// ```
#[must_use]
pub fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// An in-memory BM25 index over a corpus of documents.
///
/// Documents are identified by their insertion index (`doc_id`), a `usize` that
/// matches the caller's own document ordering. The index stores per-document
/// term frequencies and lengths plus a corpus-wide document-frequency map — it
/// holds no reference to the original text.
///
/// For the dev-grade `mem-mcp` backend the index is rebuilt from the live
/// corpus on every search. That is `O(total_tokens)` per query, which is
/// acceptable at the backend's bounded dev scale (see `MemLimits`); a scaled
/// backend would keep a persistent inverted index instead.
#[derive(Debug, Default, Clone)]
pub struct Bm25Index {
    /// Per-document term-frequency maps (index == `doc_id`).
    doc_terms: Vec<HashMap<String, u32>>,
    /// Per-document token counts (index == `doc_id`).
    doc_lengths: Vec<u32>,
    /// Corpus document frequency: term → number of documents containing it.
    doc_freq: HashMap<String, u32>,
}

impl Bm25Index {
    /// Creates an empty index.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds an index from raw document texts, tokenizing each in turn.
    ///
    /// The resulting `doc_id`s match the order of `docs`.
    #[must_use]
    pub fn build(docs: &[String]) -> Self {
        let mut index = Self::new();
        for doc in docs {
            index.push_text(doc);
        }
        index
    }

    /// Tokenizes `text` and appends it as the next document.
    pub fn push_text(&mut self, text: &str) {
        self.add_doc(&tokenize(text));
    }

    /// Appends a pre-tokenized document to the index.
    pub fn add_doc(&mut self, terms: &[String]) {
        let mut tf: HashMap<String, u32> = HashMap::new();
        for term in terms {
            *tf.entry(term.clone()).or_insert(0) += 1;
        }
        for term in tf.keys() {
            *self.doc_freq.entry(term.clone()).or_insert(0) += 1;
        }
        let len = u32::try_from(terms.len()).unwrap_or(u32::MAX);
        self.doc_lengths.push(len);
        self.doc_terms.push(tf);
    }

    /// Number of documents in the index.
    #[must_use]
    pub fn len(&self) -> usize {
        self.doc_terms.len()
    }

    /// Whether the index holds no documents.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.doc_terms.is_empty()
    }

    /// Average document length (`L_avg`), or `0.0` for an empty corpus.
    fn avgdl(&self) -> f64 {
        let n = self.doc_lengths.len();
        if n == 0 {
            return 0.0;
        }
        let total: f64 = self.doc_lengths.iter().map(|&l| f64::from(l)).sum();
        total / n as f64
    }

    /// Smoothed inverse document frequency of `term` — always `> 0` (see the
    /// module docs), so common terms never contribute a negative penalty.
    fn idf(&self, term: &str) -> f64 {
        let n = self.doc_terms.len() as f64;
        let df = f64::from(*self.doc_freq.get(term).unwrap_or(&0));
        (1.0 + (n - df + 0.5) / (df + 0.5)).ln()
    }

    /// Scores document `doc_id` against `query_terms` using BM25.
    ///
    /// The score is a total, finite, non-negative function of the corpus:
    ///
    /// - Returns `0.0` when the corpus is empty, when `L_avg == 0` (every
    ///   document is empty — no division by zero), when `doc_id` is out of
    ///   range, or when no query term appears in the document.
    /// - Returns a strictly positive value when at least one query term appears
    ///   (smoothed IDF is always positive).
    ///
    /// `query_terms` must already be tokenized (see [`tokenize`]).
    #[must_use]
    pub fn score(&self, query_terms: &[String], doc_id: usize) -> f64 {
        // Guard: empty corpus — nothing to score against.
        if self.doc_terms.is_empty() {
            return 0.0;
        }
        let avgdl = self.avgdl();
        // Guard: average document length 0 → the `|D| / L_avg` term would
        // divide by zero. Short-circuit instead.
        if avgdl <= 0.0 {
            return 0.0;
        }
        let Some(tf_map) = self.doc_terms.get(doc_id) else {
            return 0.0;
        };
        let dl = f64::from(self.doc_lengths[doc_id]);
        let mut score = 0.0;
        for term in query_terms {
            let tf = *tf_map.get(term).unwrap_or(&0);
            if tf == 0 {
                continue;
            }
            let f = f64::from(tf);
            let denom = f + K1 * (1.0 - B + B * dl / avgdl);
            score += self.idf(term) * (f * (K1 + 1.0)) / denom;
        }
        score
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terms(text: &str) -> Vec<String> {
        tokenize(text)
    }

    #[test]
    fn tokenize_lowercases_and_splits_on_non_alphanumeric() {
        assert_eq!(tokenize("Hello, World!"), vec!["hello", "world"]);
        assert_eq!(tokenize("a-b_c.d"), vec!["a", "b", "c", "d"]);
        assert_eq!(tokenize(""), Vec::<String>::new());
        assert_eq!(tokenize("   \t\n"), Vec::<String>::new());
    }

    #[test]
    fn empty_corpus_scores_zero() {
        let index = Bm25Index::new();
        assert_eq!(index.score(&terms("anything"), 0), 0.0);
        assert!(index.is_empty());
    }

    #[test]
    fn empty_query_scores_zero() {
        let index = Bm25Index::build(&["the quick brown fox".to_string()]);
        assert_eq!(index.score(&[], 0), 0.0);
    }

    #[test]
    fn all_empty_docs_avgdl_zero_no_panic() {
        // Every document is empty → L_avg == 0. Must short-circuit, not divide
        // by zero.
        let index = Bm25Index::build(&[String::new(), String::new()]);
        assert_eq!(index.avgdl(), 0.0);
        assert_eq!(index.score(&terms("anything"), 0), 0.0);
        assert_eq!(index.score(&terms("anything"), 1), 0.0);
    }

    #[test]
    fn out_of_range_doc_id_scores_zero() {
        let index = Bm25Index::build(&["hello world".to_string()]);
        assert_eq!(index.score(&terms("hello"), 99), 0.0);
    }

    #[test]
    fn term_present_outscores_term_absent() {
        let index = Bm25Index::build(&[
            "the quick brown fox".to_string(),
            "lorem ipsum dolor sit".to_string(),
        ]);
        let q = terms("quick");
        let present = index.score(&q, 0);
        let absent = index.score(&q, 1);
        assert!(present > 0.0, "term-present doc must score > 0");
        assert_eq!(absent, 0.0, "term-absent doc must score 0");
        assert!(present > absent);
    }

    #[test]
    fn common_term_idf_is_non_negative() {
        // "shared" appears in every document (> half the corpus). With the raw
        // Robertson IDF this would be negative; the smoothed form stays > 0.
        let index = Bm25Index::build(&[
            "shared token".to_string(),
            "shared other".to_string(),
            "shared word".to_string(),
        ]);
        assert!(
            index.idf("shared") > 0.0,
            "smoothed IDF for a corpus-wide term must be positive, got {}",
            index.idf("shared")
        );
        for doc_id in 0..index.len() {
            assert!(index.score(&terms("shared"), doc_id) >= 0.0);
        }
    }

    #[test]
    fn higher_term_frequency_scores_at_least_as_high_at_fixed_length() {
        // Two documents of the SAME length (4 tokens); doc 0 mentions the query
        // term twice, doc 1 not at all.
        let index = Bm25Index::build(&[
            "alpha alpha beta gamma".to_string(),
            "delta epsilon zeta eta".to_string(),
        ]);
        let q = terms("alpha");
        assert!(index.score(&q, 0) >= index.score(&q, 1));
        assert!(index.score(&q, 0) > 0.0);
    }

    #[test]
    fn scores_are_finite() {
        let index = Bm25Index::build(&[
            "one two three".to_string(),
            "two three four four four".to_string(),
        ]);
        for doc_id in 0..index.len() {
            let s = index.score(&terms("two four"), doc_id);
            assert!(s.is_finite(), "score must be finite, got {s}");
        }
    }
}
