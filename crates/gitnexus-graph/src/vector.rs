//! Vector search — brute-force cosine similarity fallback (Task 5)
//!
//! Used when KuzuDB WASM does not support CREATE_VECTOR_INDEX.
//! Holds embeddings in WASM memory; fast enough for <5 000 nodes.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VectorEntry {
    pub node_id:      String,
    pub chunk_index:  u32,
    pub start_line:   u32,
    pub end_line:     u32,
    pub content_hash: String,
    pub embedding:    Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VectorSearchResult {
    pub node_id:     String,
    pub chunk_index: u32,
    pub start_line:  u32,
    pub end_line:    u32,
    pub distance:    f64,
}

// ============================================================================
// In-memory index
// ============================================================================

pub struct BruteForceIndex {
    entries: Vec<VectorEntry>,
    /// node_id → index positions into `entries`
    by_node: HashMap<String, Vec<usize>>,
}

impl BruteForceIndex {
    pub fn new() -> Self {
        Self { entries: Vec::new(), by_node: HashMap::new() }
    }

    pub fn insert(&mut self, entry: VectorEntry) {
        let idx = self.entries.len();
        self.by_node
            .entry(entry.node_id.clone())
            .or_default()
            .push(idx);
        self.entries.push(entry);
    }

    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    /// Remove all entries for a node (used during incremental update).
    pub fn remove_node(&mut self, node_id: &str) {
        if let Some(indices) = self.by_node.remove(node_id) {
            // Mark entries as tombstoned (set empty embedding)
            for i in indices {
                self.entries[i].embedding.clear();
            }
        }
    }

    /// Top-k nearest neighbours by cosine similarity.
    /// Returns results sorted by ascending distance (1 - cosine).
    pub fn search(&self, query: &[f32], k: usize) -> Vec<VectorSearchResult> {
        let q_norm = l2_norm(query);
        if q_norm == 0.0 || self.entries.is_empty() {
            return vec![];
        }

        let mut scored: Vec<(f64, usize)> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| !e.embedding.is_empty())
            .map(|(i, e)| {
                let sim = cosine_sim_normed(query, &e.embedding, q_norm);
                let dist = 1.0 - sim; // 0 = identical, 2 = opposite
                (dist, i)
            })
            .collect();

        scored.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(k);

        scored
            .into_iter()
            .map(|(dist, i)| {
                let e = &self.entries[i];
                VectorSearchResult {
                    node_id:     e.node_id.clone(),
                    chunk_index: e.chunk_index,
                    start_line:  e.start_line,
                    end_line:    e.end_line,
                    distance:    dist,
                }
            })
            .collect()
    }
}

// ============================================================================
// Hybrid search — reciprocal rank fusion (BM25 + vector)
// ============================================================================

/// Generic result used for fusion; `id` is node_id.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FusedResult {
    pub node_id: String,
    pub score:   f64,
    /// "bm25", "semantic", or "bm25,semantic"
    pub sources: String,
}

const RRF_K: f64 = 60.0;

/// Reciprocal Rank Fusion over two ranked lists of node IDs.
pub fn reciprocal_rank_fusion(
    bm25_ids:     &[String],
    semantic_ids: &[String],
    top_k:        usize,
) -> Vec<FusedResult> {
    let mut scores: HashMap<String, (f64, bool, bool)> = HashMap::new();

    for (rank, id) in bm25_ids.iter().enumerate() {
        let e = scores.entry(id.clone()).or_insert((0.0, false, false));
        e.0 += 1.0 / (RRF_K + (rank + 1) as f64);
        e.1 = true;
    }
    for (rank, id) in semantic_ids.iter().enumerate() {
        let e = scores.entry(id.clone()).or_insert((0.0, false, false));
        e.0 += 1.0 / (RRF_K + (rank + 1) as f64);
        e.2 = true;
    }

    let mut results: Vec<FusedResult> = scores
        .into_iter()
        .map(|(node_id, (score, is_bm25, is_sem))| {
            let sources = match (is_bm25, is_sem) {
                (true,  true)  => "bm25,semantic".to_owned(),
                (true,  false) => "bm25".to_owned(),
                (false, true)  => "semantic".to_owned(),
                _              => String::new(),
            };
            FusedResult { node_id, score, sources }
        })
        .collect();

    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(top_k);
    results
}

// ============================================================================
// Math helpers
// ============================================================================

fn l2_norm(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum::<f32>().sqrt()
}

/// Cosine similarity when the query norm is already computed.
fn cosine_sim_normed(query: &[f32], doc: &[f32], q_norm: f32) -> f64 {
    let len = query.len().min(doc.len());
    let dot: f32 = query[..len].iter().zip(&doc[..len]).map(|(a, b)| a * b).sum();
    let d_norm = l2_norm(&doc[..len]);
    if d_norm == 0.0 { return 0.0; }
    (dot / (q_norm * d_norm)) as f64
}

/// L2-normalise a vector in place (for storage; cosine → dot product).
pub fn l2_normalize(v: &mut Vec<f32>) {
    let norm = l2_norm(v);
    if norm > 0.0 { v.iter_mut().for_each(|x| *x /= norm); }
}

impl Default for BruteForceIndex {
    fn default() -> Self { Self::new() }
}
