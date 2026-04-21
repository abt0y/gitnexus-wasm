# Task 5: Semantic Search — Implementation Guide

**Priority**: P0 (Critical Path)  
**Estimated Effort**: 2 weeks  
**Skill Level**: Expert (ONNX, vector search, embeddings)  
**Dependencies**: Task 1 (Real Tokenizer), Task 2 (Web Workers for batch speed)  
**Blocks**: None (leaf task)

---

## Problem Statement

Current search is **keyword-only** (BM25 via KuzuDB CONTAINS). We need:
1. **Vector embeddings** for all code elements
2. **Vector index** for fast similarity search
3. **Hybrid search** combining BM25 + semantic with reciprocal rank fusion

---

## Implementation

### Step 1: Batch Embedding Generation (Day 1-5)

```rust
// crates/gitnexus-embed/src/pipeline.rs
use rayon::prelude::*;

pub struct EmbeddingPipeline {
    engine: EmbeddingEngine,
    chunker: TextChunker,
    batch_size: usize,
}

impl EmbeddingPipeline {
    pub async fn embed_all_nodes(
        &self,
        graph: &GraphDatabase,
        progress_callback: JsValue,
    ) -> Result<JsResult, JsValue> {
        let callback: js_sys::Function = progress_callback.dyn_into()?;

        // Query all code elements
        let query = "MATCH (n) WHERE labels(n)[0] IN ['Function', 'Class', 'Method', 'Interface'] RETURN n.id AS id, n.name AS name, n.content AS content, n.filePath AS filePath";
        let nodes = graph.query(query).await?;

        let total = nodes.len();
        let mut processed = 0;

        // Process in batches
        for batch in nodes.chunks(self.batch_size) {
            let mut embeddings = Vec::new();

            for node in batch {
                let id = node.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let name = node.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let content = node.get("content").and_then(|v| v.as_str()).unwrap_or("");
                let file_path = node.get("filePath").and_then(|v| v.as_str()).unwrap_or("");

                // Format embedding text
                let embed_text = format!("{} {} {}", name, file_path, content);

                // Chunk if too long
                let chunks = self.chunker.chunk(&embed_text);

                for (chunk_idx, chunk) in chunks.iter().enumerate() {
                    let embedding = self.engine.embed(&chunk.text).await?;

                    embeddings.push((id, chunk_idx, chunk.start_word, chunk.end_word, embedding));
                }
            }

            // Store embeddings in KuzuDB
            for (node_id, chunk_idx, start_word, end_word, embedding) in &embeddings {
                let content_hash = format!("{:x}", md5::compute(
                    nodes[processed].get("content").and_then(|v| v.as_str()).unwrap_or("")
                ));

                let props = serde_json::json!({
                    "id": format!("Embedding:{}:{}", node_id, chunk_idx),
                    "nodeId": node_id,
                    "chunkIndex": *chunk_idx as u32,
                    "startLine": *start_word as u32,
                    "endLine": *end_word as u32,
                    "embedding": embedding,
                    "contentHash": content_hash,
                });

                graph.create_node("CodeEmbedding", serde_wasm_bindgen::to_value(&props).unwrap()).await?;
            }

            processed += batch.len();

            // Report progress
            let progress = EmbeddingProgress {
                phase: "embedding".to_string(),
                percent: ((processed as f32 / total as f32) * 100.0) as u8,
                nodes_processed: Some(processed as u32),
                total_nodes: Some(total as u32),
                error: None,
            };

            let js_progress = serde_wasm_bindgen::to_value(&progress).unwrap_or(JsValue::NULL);
            let _ = callback.call1(&JsValue::NULL, &js_progress);
        }

        Ok(JsResult::ok(&serde_json::json!({
            "processed": processed,
            "total": total,
        })))
    }
}
```

### Step 2: Vector Index in KuzuDB (Day 6-8)

KuzuDB supports vector indexes via `CREATE_VECTOR_INDEX`:

```rust
impl GraphDatabase {
    pub async fn create_vector_index(&self) -> Result<(), JsValue> {
        // KuzuDB WASM may have limited vector index support
        // Fallback: custom HNSW or brute-force

        let query = format!(
            "CALL CREATE_VECTOR_INDEX('CodeEmbedding', 'embedding_idx', 'embedding', {}, false)",
            384
        );

        match self.execute(&query).await {
            Ok(_) => Ok(()),
            Err(e) => {
                let msg = e.as_string().unwrap_or_default();
                if msg.contains("not supported") || msg.contains("unimplemented") {
                    // Fallback: store embeddings and do brute-force search
                    self.store_embeddings_for_brute_force().await?;
                    Ok(())
                } else {
                    Err(e)
                }
            }
        }
    }

    async fn store_embeddings_for_brute_force(&self) -> Result<(), JsValue> {
        // Store all embeddings in memory for fast access
        // Or use IndexedDB for persistence
        Ok(())
    }
}
```

### Step 3: Brute-Force Cosine Similarity (Fallback) (Day 9-10)

If KuzuDB WASM lacks vector index:

```rust
pub struct BruteForceIndex {
    embeddings: Vec<(String, Vec<f32>)>, // node_id -> embedding
}

impl BruteForceIndex {
    pub fn from_graph(graph: &GraphDatabase) -> Self {
        // Load all embeddings from KuzuDB
        // Store in memory
        Self { embeddings: vec![] }
    }

    pub fn search(&self, query_embedding: &[f32], k: usize) -> Vec<(String, f64)> {
        let mut results: Vec<(String, f64)> = self.embeddings.iter()
            .map(|(id, emb)| {
                let similarity = cosine_similarity(query_embedding, emb);
                (id.clone(), similarity)
            })
            .collect();

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        results.into_iter().take(k).collect()
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        (dot / (norm_a * norm_b)) as f64
    }
}
```

### Step 4: Hybrid Search (Day 11-12)

```rust
pub async fn hybrid_search(
    &self,
    query: &str,
    query_embedding: Vec<f32>,
    k: usize,
) -> Result<Vec<SearchResult>, JsValue> {
    // 1. BM25 search
    let bm25_results = self.bm25_search(query, k * 2).await?;

    // 2. Semantic search
    let semantic_results = self.semantic_search(&query_embedding, k * 2).await?;

    // 3. Reciprocal Rank Fusion
    let mut rrf_scores: HashMap<String, f64> = HashMap::new();
    const K: f64 = 60.0;

    for (rank, result) in bm25_results.iter().enumerate() {
        let score = 1.0 / (K + (rank + 1) as f64);
        *rrf_scores.entry(result.node_id.clone()).or_insert(0.0) += score;
    }

    for (rank, result) in semantic_results.iter().enumerate() {
        let score = 1.0 / (K + (rank + 1) as f64);
        *rrf_scores.entry(result.node_id.clone()).or_insert(0.0) += score;
    }

    // 4. Combine and sort
    let mut combined: Vec<SearchResult> = Vec::new();
    let mut seen = HashSet::new();

    for result in bm25_results.iter().chain(semantic_results.iter()) {
        if seen.insert(result.node_id.clone()) {
            let mut merged = result.clone();
            merged.score = *rrf_scores.get(&result.node_id).unwrap_or(&0.0);
            merged.sources = Some(vec![
                if bm25_results.iter().any(|r| r.node_id == result.node_id) { "bm25" } else { "" }.to_string(),
                if semantic_results.iter().any(|r| r.node_id == result.node_id) { "semantic" } else { "" }.to_string(),
            ]);
            combined.push(merged);
        }
    }

    combined.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
    combined.truncate(k);

    Ok(combined)
}
```

### Step 5: UI Integration (Day 13-14)

```typescript
// web/src/components/SearchPanel.tsx
function SearchModeToggle({ mode, onChange }: { mode: SearchMode; onChange: (m: SearchMode) => void }) {
    return (
        <div className="flex gap-1 p-1 bg-bg-tertiary rounded-lg">
            {(['hybrid', 'semantic', 'keyword'] as SearchMode[]).map((m) => (
                <button
                    key={m}
                    onClick={() => onChange(m)}
                    className={`px-3 py-1 rounded-md text-sm capitalize transition-colors ${
                        mode === m ? 'bg-accent text-white' : 'text-text-secondary hover:text-text-primary'
                    }`}
                >
                    {m}
                </button>
            ))}
        </div>
    );
}
```

---

## Acceptance Criteria

- [ ] 1000 nodes embedded in <5 minutes
- [ ] Vector search query <100ms for top-10
- [ ] "auth middleware" finds `AuthMiddleware` class AND `authenticate()` function
- [ ] Hybrid search outperforms keyword-only on concept queries
- [ ] Embeddings stored with content hash for staleness detection
- [ ] UI shows search mode toggle and result scores

---

## Deliverables

1. `crates/gitnexus-embed/src/pipeline.rs` — Batch embedding pipeline
2. `crates/gitnexus-graph/src/vector.rs` — Vector index + brute-force fallback
3. `web/src/components/SearchPanel.tsx` — Modified (hybrid search UI)
