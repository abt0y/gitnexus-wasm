# Task 7: Incremental Updates — Implementation Guide

**Priority**: P1 (Enhancement)  
**Estimated Effort**: 1.5 weeks  
**Skill Level**: Advanced (content hashing, diff algorithms, state management)  
**Dependencies**: Task 2 (Web Workers for speed), Task 6 (Git for change detection)  
**Blocks**: None

---

## Problem Statement

Currently, every import triggers **full re-analysis**. For a 1000-file repo, this takes 60s+. We need:
1. **Content hashing** to skip unchanged files
2. **Git diff** to find changed files
3. **Incremental graph updates** (add/modify/remove nodes)
4. **Embedding staleness detection**
5. **Persistent state** across sessions

---

## Implementation

### Step 1: Content Hashing (Day 1-3)

```rust
// crates/gitnexus-core/src/hash.rs
use sha2::{Sha256, Digest};

pub fn compute_content_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn compute_file_hash(file_path: &str, content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(file_path.as_bytes());
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}
```

### Step 2: Store Hashes in KuzuDB (Day 4)

```rust
// In GraphDatabase::create_node for File nodes
let file_hash = compute_file_hash(&file_path, &content.unwrap_or(""));

let props = serde_json::json!({
    "id": file_id,
    "name": file_name,
    "filePath": file_path,
    "contentHash": file_hash,
    "lastAnalyzed": chrono::Utc::now().to_rfc3339(),
});
```

### Step 3: Incremental Import (Day 5-7)

```rust
#[wasm_bindgen]
impl GitNexus {
    pub async fn incremental_import(
        &self,
        files: JsValue,
        progress_callback: JsValue,
    ) -> Result<JsResult, JsValue> {
        let new_files: Vec<FileEntry> = serde_wasm_bindgen::from_value(files)?;
        let engine = self.engine.read();
        let graph = engine.graph.as_ref().ok_or("No graph")?;

        let mut changed_files = Vec::new();
        let mut unchanged_count = 0;
        let mut new_count = 0;
        let mut deleted_files = Vec::new();

        // Get existing file hashes from graph
        let existing_query = "MATCH (f:File) RETURN f.id AS id, f.filePath AS path, f.contentHash AS hash";
        let existing = graph.query(existing_query).await?;

        let mut existing_map: HashMap<String, String> = HashMap::new();
        for row in existing {
            let path = row.get("path").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let hash = row.get("hash").and_then(|v| v.as_str()).unwrap_or("").to_string();
            existing_map.insert(path, hash);
        }

        // Check each new file
        for file in &new_files {
            if file.is_directory {
                continue;
            }

            let new_hash = compute_file_hash(&file.path, &file.content.as_ref().unwrap_or(&"".to_string()));

            match existing_map.get(&file.path) {
                Some(old_hash) if old_hash == &new_hash => {
                    unchanged_count += 1;
                }
                Some(_) => {
                    // Changed
                    changed_files.push(file.clone());
                }
                None => {
                    // New file
                    new_count += 1;
                    changed_files.push(file.clone());
                }
            }
        }

        // Find deleted files
        let new_paths: HashSet<String> = new_files.iter().map(|f| f.path.clone()).collect();
        for (path, _) in &existing_map {
            if !new_paths.contains(path) {
                deleted_files.push(path.clone());
            }
        }

        // Process changes
        if !changed_files.is_empty() {
            // Parse changed files
            let parsed = self.parse_files_parallel(&changed_files).await?;

            // Update graph: remove old nodes, add new ones
            self.update_graph_nodes(&parsed, &deleted_files).await?;
        }

        // Remove deleted files from graph
        for path in &deleted_files {
            self.remove_file_from_graph(path).await?;
        }

        Ok(JsResult::ok(&serde_json::json!({
            "unchanged": unchanged_count,
            "changed": changed_files.len(),
            "new": new_count,
            "deleted": deleted_files.len(),
            "timeSaved": if changed_files.is_empty() { "100%" } else { "~80%" },
        })))
    }

    async fn remove_file_from_graph(&self, file_path: &str) -> Result<(), JsValue> {
        let engine = self.engine.read();
        let graph = engine.graph.as_ref().ok_or("No graph")?;

        // Delete File node (cascades to DEFINES relationships)
        let query = format!(
            "MATCH (f:File {{filePath: '{}'}}) DETACH DELETE f",
            file_path.replace("'", "''")
        );
        graph.execute(&query).await?;

        // Delete orphaned CodeElement nodes (no DEFINES relationship)
        let orphan_query = "MATCH (n:CodeElement) WHERE NOT (n)<-[:DEFINES]-() DELETE n";
        graph.execute(orphan_query).await?;

        Ok(())
    }

    async fn update_graph_nodes(
        &self,
        parsed_files: &[ParsedFile],
        deleted_files: &[String],
    ) -> Result<(), JsValue> {
        let mut engine = self.engine.write();
        let graph = engine.graph.as_mut().ok_or("No graph")?;

        for file in parsed_files {
            let file_id = format!("File:{}", file.file_path);

            // 1. Delete old symbols for this file
            let delete_query = format!(
                "MATCH (f:File {{id: '{}'}})-[:DEFINES]->(n) DETACH DELETE n",
                file_id.replace("'", "''")
            );
            graph.execute(&delete_query).await?;

            // 2. Update File node hash
            let new_hash = compute_file_hash(&file.file_path, &file.content.as_ref().unwrap_or(&"".to_string()));
            let update_query = format!(
                "MATCH (f:File {{id: '{}'}}) SET f.contentHash = '{}'",
                file_id.replace("'", "''"),
                new_hash
            );
            graph.execute(&update_query).await?;

            // 3. Create new symbols (same as initial analysis)
            for symbol in &file.symbols {
                let label = match symbol.kind {
                    SymbolKind::Function => "Function",
                    SymbolKind::Class => "Class",
                    // ... etc
                };

                let props = serde_json::json!({
                    "id": symbol.id,
                    "name": symbol.name,
                    "filePath": symbol.file_path,
                    "startLine": symbol.start_line,
                    "endLine": symbol.end_line,
                });

                graph.create_node(label, serde_wasm_bindgen::to_value(&props).unwrap()).await?;

                // DEFINES relationship
                let rel_props = serde_json::json!({"confidence": 1.0});
                graph.create_relationship(&file_id, &symbol.id, "Defines", 
                    Some(serde_wasm_bindgen::to_value(&rel_props).unwrap())).await?;
            }
        }

        // 4. Re-resolve cross-file references
        self.resolve_calls(parsed_files).await?;

        Ok(())
    }
}
```

### Step 4: Git Diff-Based Updates (Day 8-10)

```rust
pub async fn git_diff_update(&self) -> Result<JsResult, JsValue> {
    let engine = self.engine.read();
    let repo = engine.current_repo.as_ref().ok_or("No repo")?;

    if !repo.is_git_repo {
        return Ok(JsResult::err("Not a git repository"));
    }

    let git = GitRepo::new(repo.path.clone()).await?;
    let status = git.status().await?;

    let mut changed_files = Vec::new();
    let mut deleted_files = Vec::new();

    for file_status in status {
        match file_status.status.as_str() {
            "modified" | "added" => {
                // Read file content
                let content = self.read_file(&file_status.path).await?;
                changed_files.push(FileEntry {
                    path: file_status.path,
                    name: file_status.path.split('/').last().unwrap_or("").to_string(),
                    is_directory: false,
                    content: Some(content),
                    size: None,
                });
            }
            "deleted" => {
                deleted_files.push(file_status.path);
            }
            _ => {}
        }
    }

    // Run incremental update
    self.incremental_import(
        serde_wasm_bindgen::to_value(&changed_files).unwrap(),
        JsValue::NULL,
    ).await?;

    // Mark embeddings as stale
    self.mark_embeddings_stale(&changed_files).await?;

    Ok(JsResult::ok(&serde_json::json!({
        "changed": changed_files.len(),
        "deleted": deleted_files.len(),
    })))
}
```

### Step 5: Embedding Staleness (Day 11-12)

```rust
pub async fn mark_embeddings_stale(&self, changed_files: &[FileEntry]) -> Result<(), JsValue> {
    let engine = self.engine.read();
    let graph = engine.graph.as_ref().ok_or("No graph")?;

    for file in changed_files {
        // Find all embeddings for nodes in this file
        let query = format!(
            "MATCH (e:CodeEmbedding) WHERE e.nodeId STARTS WITH '{}' SET e.stale = true",
            file.path.replace("'", "''")
        );
        graph.execute(&query).await?;
    }

    Ok(())
}

pub async fn update_stale_embeddings(&self) -> Result<JsResult, JsValue> {
    let engine = self.engine.read();
    let graph = engine.graph.as_ref().ok_or("No graph")?;

    // Find stale embeddings
    let stale_query = "MATCH (e:CodeEmbedding) WHERE e.stale = true RETURN e.nodeId AS nodeId, e.chunkIndex AS chunkIndex";
    let stale = graph.query(stale_query).await?;

    let mut updated = 0;

    for row in stale {
        let node_id = row.get("nodeId").and_then(|v| v.as_str()).unwrap_or("");
        let chunk_idx = row.get("chunkIndex").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

        // Get current node content
        let node_query = format!("MATCH (n) WHERE n.id = '{}' RETURN n.content AS content", node_id);
        let node = graph.query(&node_query).await?;

        if let Some(content) = node.get(0).and_then(|r| r.get("content")).and_then(|v| v.as_str()) {
            // Re-embed
            let embedding = self.embed(content).await?;
            let new_hash = compute_content_hash(content);

            // Update embedding
            let update_query = format!(
                "MATCH (e:CodeEmbedding {{nodeId: '{}', chunkIndex: {}}}) SET e.embedding = {:?}, e.contentHash = '{}', e.stale = false",
                node_id, chunk_idx, embedding, new_hash
            );
            graph.execute(&update_query).await?;
            updated += 1;
        }
    }

    Ok(JsResult::ok(&serde_json::json!({"updated": updated})))
}
```

### Step 6: Persistent State (Day 13-15)

```typescript
// web/src/wasm/persistence.ts
const DB_NAME = 'gitnexus-state';
const DB_VERSION = 1;

interface RepoState {
    name: string;
    importDate: string;
    fileHashes: Record<string, string>;
    graphExported: string; // JSON export of graph
    embeddingsExported: string;
}

export class StatePersistence {
    private db: IDBDatabase | null = null;

    async init(): Promise<void> {
        return new Promise((resolve, reject) => {
            const request = indexedDB.open(DB_NAME, DB_VERSION);

            request.onerror = () => reject(request.error);
            request.onsuccess = () => {
                this.db = request.result;
                resolve();
            };

            request.onupgradeneeded = (event) => {
                const db = (event.target as IDBOpenDBRequest).result;
                db.createObjectStore('repos', { keyPath: 'name' });
                db.createObjectStore('graphs', { keyPath: 'repoName' });
                db.createObjectStore('embeddings', { keyPath: 'repoName' });
            };
        });
    }

    async saveRepoState(repo: RepoState): Promise<void> {
        if (!this.db) throw new Error('DB not initialized');

        const tx = this.db.transaction(['repos', 'graphs', 'embeddings'], 'readwrite');

        await Promise.all([
            this.put(tx.objectStore('repos'), repo),
            this.put(tx.objectStore('graphs'), { repoName: repo.name, graph: repo.graphExported }),
            this.put(tx.objectStore('embeddings'), { repoName: repo.name, embeddings: repo.embeddingsExported }),
        ]);
    }

    async loadRepoState(repoName: string): Promise<RepoState | null> {
        if (!this.db) return null;

        const tx = this.db.transaction(['repos'], 'readonly');
        const store = tx.objectStore('repos');

        return new Promise((resolve, reject) => {
            const request = store.get(repoName);
            request.onsuccess = () => resolve(request.result || null);
            request.onerror = () => reject(request.error);
        });
    }

    private put(store: IDBObjectStore, data: any): Promise<void> {
        return new Promise((resolve, reject) => {
            const request = store.put(data);
            request.onsuccess = () => resolve();
            request.onerror = () => reject(request.error);
        });
    }
}
```

---

## Acceptance Criteria

- [ ] Re-import of unchanged repo completes in <2s
- [ ] 1-file change updates in <5s (vs 60s full re-analysis)
- [ ] Deleted files removed from graph (no orphans)
- [ ] Embeddings marked stale on content change, updated on demand
- [ ] Graph state persists across tab closes (IndexedDB)
- [ ] Git diff correctly identifies modified/added/deleted files
- [ ] No data loss on incremental update

---

## Deliverables

1. `crates/gitnexus-core/src/hash.rs` — Content hashing
2. `crates/gitnexus-core/src/lib.rs` — Modified (incremental methods)
3. `crates/gitnexus-embed/src/lib.rs` — Modified (staleness tracking)
4. `web/src/wasm/persistence.ts` — IndexedDB state management
5. `web/src/hooks/useStore.ts` — Modified (cache management)
