# GitNexus WASM — Implementation Plan for Google Antigravity

> **Version**: 1.0  
> **Date**: 2026-04-22  
> **Status**: Ready for AI agent execution  
> **Estimated Effort**: 6-8 weeks (full-time equivalent)  
> **Priority**: P0 (Critical path to production)

---

## Executive Summary

This plan details the remaining work to bring the GitNexus WASM rewrite from its current **architecturally-complete but functionally-simplified** state to **production-ready**. Seven critical gaps have been identified, each with specific deliverables, acceptance criteria, and dependencies.

### Current State
- ✅ Core WASM engine compiles and initializes
- ✅ Tree-sitter parsers load and parse files
- ✅ KuzuDB WASM creates schema and stores nodes
- ✅ React UI renders, imports files, shows progress
- ❌ Tokenization is fake (hash-based, not BPE/WordPiece)
- ❌ Parsing is single-threaded (no Web Workers)
- ❌ No community detection (Louvain/Leiden missing)
- ❌ No process extraction (BFS flow detection stubbed)
- ❌ No semantic search (vector index not created)
- ❌ No git auth (HTTPS clone/push impossible)
- ❌ No incremental updates (full re-analysis every time)

### Target State
All P0 items complete, system can analyze a 5,000-file TypeScript repo in <2 minutes with full graph, communities, processes, semantic search, and git integration.

---

## Skill Requirements

| Skill | Level | Used In |
|-------|-------|---------|
| **Rust** (async, WASM, FFI) | Expert | All crates |
| **WebAssembly** (wasm-bindgen, JS interop) | Expert | Core, parse, graph, embed |
| **Tree-sitter** (grammar writing, WASM build) | Advanced | Parse crate |
| **Graph Algorithms** (Louvain, BFS, PageRank) | Advanced | Graph crate |
| **ONNX Runtime** (Web, quantization, opsets) | Intermediate | Embed crate |
| **Tokenizers** (BPE, WordPiece, Rust/HuggingFace) | Expert | Embed crate |
| **Web Workers** (SharedArrayBuffer, Atomics) | Intermediate | Web UI |
| **React/TypeScript** (hooks, state management) | Intermediate | Web UI |
| **Git Internals** (plumbing, packfiles, auth) | Advanced | Git crate |
| **CI/CD** (GitHub Actions, cross-compilation) | Intermediate | DevOps |

---

## Tooling Stack

### Development
| Tool | Version | Purpose |
|------|---------|---------|
| Rust | 1.75+ | Core language |
| wasm-pack | 0.12+ | WASM build tool |
| Node.js | 20+ | Web UI runtime |
| tree-sitter CLI | 0.22+ | Parser compilation |
| Python | 3.11+ | Model quantization scripts |

### Libraries & Crates
| Crate/Library | Version | Used For |
|---------------|---------|----------|
| `tokenizers` (HuggingFace) | 0.15+ | Real BPE/WordPiece tokenization |
| `petgraph` | 0.6+ | Graph algorithms (Louvain, BFS) |
| `louvain-community` | 0.3+ | Community detection (or custom) |
| `rayon` | 1.8+ | Data parallelism (where WASM allows) |
| `wasm-bindgen-rayon` | 1.2+ | Web Worker thread pool |
| `kuzu-wasm` | 0.6+ | Graph database |
| `onnxruntime-web` | 1.17+ | ML inference |
| `web-tree-sitter-sg` | 2.0+ | Parser bindings |
| `isomorphic-git` | 1.25+ (JS) | Git operations |

### Testing & Quality
| Tool | Purpose |
|------|---------|
| `wasm-pack test` | Rust WASM unit tests |
| `jest` / `vitest` | React component tests |
| `cypress` / `playwright` | E2E browser tests |
| `clippy` | Rust linting |
| `rustfmt` | Rust formatting |

---

## Task Breakdown

### Task 1: Real Tokenizer (P0) — **2 weeks**
**Owner**: AI Agent — Embed + Parse crates  
**Depends On**: None (foundational)  
**Blocks**: Task 5 (Semantic Search)

#### 1.1 Research & Select Tokenizer
- **Action**: Evaluate `tokenizers` crate WASM compatibility
- **Options**:
  - A) Port `tokenizers` (Rust) to WASM — uses `onig` regex, may need `regex` replacement
  - B) Use `web-tokenizers` (JS) via wasm-bindgen — lighter but slower
  - C) Custom minimal BPE implementation in Rust — most control, least deps
- **Deliverable**: Decision doc with trade-offs, selected approach
- **Acceptance**: Can tokenize "Hello world" → [1, 2, 3] with real vocab

#### 1.2 Implement Tokenizer in Rust
- **Action**: Create `gitnexus-tokenize` crate or extend `gitnexus-embed`
- **Requirements**:
  - Load `tokenizer.json` from HuggingFace (BPE or WordPiece)
  - Handle special tokens: `[CLS]`, `[SEP]`, `[PAD]`, `[UNK]`
  - Truncation to 512 tokens
  - Padding to batch size
  - Return `input_ids`, `attention_mask`, `token_type_ids`
- **Code Structure**:
  ```rust
  pub struct CodeTokenizer {
      tokenizer: Tokenizer, // from tokenizers crate
      max_length: usize,
  }

  impl CodeTokenizer {
      pub fn from_json(json: &str) -> Result<Self, TokenizerError>;
      pub fn encode(&self, text: &str) -> Encoding;
      pub fn encode_batch(&self, texts: Vec<&str>) -> Vec<Encoding>;
  }
  ```
- **Deliverable**: Working tokenizer with unit tests
- **Acceptance**: 100% match with Python `transformers` tokenizer output on 100 test strings

#### 1.3 Integrate with Embedding Engine
- **Action**: Replace fake `tokens_to_input_ids()` with real tokenizer
- **Changes in `gitnexus-embed/src/lib.rs`**:
  - Remove `create_simple_tokenizer()`
  - Add `tokenizer: Option<CodeTokenizer>` field
  - In `init()`, load `tokenizer.json` from URL
  - In `embed()`, call `tokenizer.encode()` then create tensor
- **Deliverable**: Embedding engine produces correct vectors
- **Acceptance**: Cosine similarity between "function foo" and "def foo" > 0.7

#### 1.4 Build Tokenizer JSON for Code
- **Action**: Create code-optimized tokenizer or reuse existing
- **Options**:
  - Use `xenova/all-MiniLM-L6-v2` tokenizer (general, works for code)
  - Use `microsoft/codebert-base` tokenizer (code-optimized)
  - Train custom BPE on GitHub code corpus (expensive)
- **Deliverable**: `tokenizer.json` committed to `web/public/assets/`
- **Acceptance**: Tokenizer file <5MB, loads in <500ms

#### 1.5 WASM Bundle Optimization
- **Action**: Ensure tokenizer doesn't bloat WASM binary
- **Techniques**:
  - `wee_alloc` for smaller allocator
  - `wasm-opt -Oz` for size optimization
  - Lazy loading: only load tokenizer when embeddings requested
- **Deliverable**: Core WASM <2MB, tokenizer WASM <1MB
- **Acceptance**: Total initial download <3MB (excluding model)

---

### Task 2: Web Workers for Parallel Parsing (P0) — **1.5 weeks**
**Owner**: AI Agent — Parse crate + Web UI  
**Depends On**: Task 1 (tokenization not blocking, but good to have first)  
**Blocks**: Task 4 (Process extraction needs full graph)

#### 2.1 Research WASM in Web Workers
- **Action**: Verify `wasm-bindgen-rayon` or custom Worker pool approach
- **Key Questions**:
  - Does `SharedArrayBuffer` work with KuzuDB WASM?
  - Can multiple Workers each load a tree-sitter parser?
  - Is `postMessage` transfer of File objects supported?
- **Deliverable**: Proof-of-concept: 2 Workers parsing 2 files simultaneously
- **Acceptance**: Both Workers return correct ASTs, no memory leaks

#### 2.2 Implement Worker Pool in Web UI
- **Action**: Create `web/src/wasm/worker-pool.ts`
- **Architecture**:
  ```typescript
  class ParserWorkerPool {
      workers: Worker[];
      queue: ParseJob[];
      maxWorkers: number; // navigator.hardwareConcurrency || 4

      async init(): Promise<void>; // Load WASM in each worker
      async parseBatch(files: FileEntry[]): Promise<ParsedFile[]>;
      private dispatch(job: ParseJob): Promise<ParsedFile>;
  }
  ```
- **Worker Script** (`web/public/workers/parser-worker.js`):
  - Load `tree-sitter.js` runtime
  - Load language-specific `.wasm` parser
  - Receive `{filePath, content, language}` via postMessage
  - Parse and return serialized AST
  - Handle errors gracefully
- **Deliverable**: Worker pool with configurable concurrency
- **Acceptance**: Parse 100 files in <5s on 4-core machine (vs <20s single-threaded)

#### 2.3 Integrate with Analysis Pipeline
- **Action**: Modify `GitNexus.analyze()` to use Worker pool
- **Changes in `gitnexus-core/src/lib.rs`**:
  - Add `parse_batch()` method that accepts multiple files
  - Return results as they complete (streaming)
  - Update progress callback per-file
- **Changes in Web UI**:
  - `useGitNexusStore.analyzeRepo()` calls `parseBatch()` instead of loop
  - Progress bar shows "Parsing X/Y files (Z workers active)"
- **Deliverable**: Full pipeline uses parallel parsing
- **Acceptance**: 1000-file repo parses in <30s on 8-core machine

#### 2.4 Handle Worker Errors & Recovery
- **Action**: Add fault tolerance
- **Requirements**:
  - If Worker crashes, respawn and retry job
  - If parser WASM fails to load, skip that language
  - Memory limit per Worker: 512MB (configurable)
  - Timeout per file: 30s
- **Deliverable**: Error handling with user-visible messages
- **Acceptance**: 0% data loss on Worker crash, graceful degradation

---

### Task 3: Community Detection (P0) — **1.5 weeks**
**Owner**: AI Agent — Graph crate  
**Depends On**: Task 2 (needs complete graph)  
**Blocks**: Task 4 (Process extraction uses communities)

#### 3.1 Port Louvain Algorithm to Rust
- **Action**: Implement or port Louvain community detection
- **Algorithm**: Modularity optimization for undirected weighted graphs
- **Implementation Options**:
  - A) Use `petgraph` + custom Louvain (most control)
  - B) Port from `python-louvain` (reference implementation)
  - C) Use `louvain-community` crate (if WASM-compatible)
- **Key Data Structures**:
  ```rust
  pub struct LouvainConfig {
      resolution: f64,      // Default 1.0
      min_community_size: usize, // Default 3
      max_levels: usize,      // Default 10
      tolerance: f64,         // Default 1e-6
  }

  pub struct CommunityResult {
      pub node_to_community: HashMap<String, u32>,
      pub community_stats: Vec<CommunityStat>,
      pub modularity: f64,
  }
  ```
- **Deliverable**: `gitnexus-graph/src/community.rs` with Louvain implementation
- **Acceptance**: Modularity >0.3 on test graph, communities >3 nodes each

#### 3.2 Integrate with KuzuDB Graph
- **Action**: Export graph to `petgraph`, run Louvain, write back
- **Pipeline**:
  1. Query KuzuDB: `MATCH (n)-[r]->(m) RETURN n.id, m.id, r.confidence`
  2. Build `petgraph::Graph<String, f64>` (node=id, edge=confidence)
  3. Run Louvain
  4. Create `Community` nodes in KuzuDB
  5. Create `MemberOf` relationships
  6. Calculate cohesion per community
- **Deliverable**: `GraphDatabase.detect_communities()` method
- **Acceptance**: 1000-node graph processed in <2s

#### 3.3 Heuristic Labeling
- **Action**: Generate human-readable community labels
- **Algorithm**:
  1. TF-IDF on community member names
  2. Extract most distinctive terms
  3. Format: "AuthService" or "UserManagement"
- **Deliverable**: `heuristic_label` field on Community nodes
- **Acceptance**: Labels are meaningful to human reviewers

#### 3.4 Visualization Integration
- **Action**: Color graph nodes by community in React UI
- **Changes**: `GraphView.tsx` uses `community` property for color
- **Deliverable**: Distinct colors per community, legend in UI
- **Acceptance**: Users can visually identify clusters

---

### Task 4: Process Extraction (P0) — **1 week**
**Owner**: AI Agent — Graph crate + Core  
**Depends On**: Task 3 (needs communities)  
**Blocks**: None (leaf task)

#### 4.1 Implement BFS Flow Detection
- **Action**: Detect execution flows from entry points
- **Algorithm**:
  1. Find entry points: HTTP routes, CLI commands, event handlers
  2. BFS traversal following `CALLS` edges
  3. Track depth, branch points, terminal nodes
  4. Group into processes by community membership
- **Key Structures**:
  ```rust
  pub struct ProcessExtractor {
      max_depth: u32,
      min_confidence: f64,
  }

  impl ProcessExtractor {
      pub fn extract_processes(&self, graph: &GraphDatabase) -> Vec<Process>;
      pub fn extract_from_entry(&self, entry_id: &str, graph: &GraphDatabase) -> Process;
  }
  ```
- **Deliverable**: `gitnexus-graph/src/process.rs`
- **Acceptance**: Detects "LoginFlow" with 5+ steps, "PaymentFlow" with 3+ steps

#### 4.2 Create Process Nodes
- **Action**: Store processes in KuzuDB
- **Schema**:
  ```cypher
  CREATE (p:Process {
      id: "Process:LoginFlow",
      label: "LoginFlow",
      processType: "HTTP",
      stepCount: 5,
      entryPointId: "Route:/api/login",
      terminalId: "Function:generateToken"
  })
  CREATE (step1)-[:StepInProcess {step: 1}]->(p)
  CREATE (step2)-[:StepInProcess {step: 2}]->(p)
  ```
- **Deliverable**: `GraphDatabase.extract_and_store_processes()`
- **Acceptance**: Processes link to real code elements

#### 4.3 UI Integration
- **Action**: Show process flows in ContextPanel
- **Changes**: `ContextPanel.tsx` renders process steps as timeline
- **Deliverable**: Visual process flow with step numbers
- **Acceptance**: Users can trace "click login → validate → query DB → return token"

---

### Task 5: Semantic Search (P0) — **2 weeks**
**Owner**: AI Agent — Embed + Graph crates  
**Depends On**: Task 1 (real tokenizer), Task 2 (parallel parsing for speed)  
**Blocks**: None (leaf task)

#### 5.1 Generate Embeddings for All Nodes
- **Action**: Batch-embed all code elements after parsing
- **Pipeline**:
  1. Query all `CodeElement`, `Function`, `Class` nodes
  2. Format text: `{label} {name} {content}`
  3. Chunk if >512 tokens
  4. Batch embed (32 at a time)
  5. Store in `CodeEmbedding` nodes
- **Optimization**:
  - Skip if `content_hash` unchanged (incremental)
  - Use Web Workers for embedding (parallel batches)
  - Stream results to IndexedDB
- **Deliverable**: `EmbeddingPipeline.embed_all_nodes()`
- **Acceptance**: 1000 nodes embedded in <5 minutes

#### 5.2 Create Vector Index in KuzuDB
- **Action**: Use KuzuDB's `CREATE_VECTOR_INDEX` and `QUERY_VECTOR_INDEX`
- **Steps**:
  1. `CREATE_VECTOR_INDEX('CodeEmbedding', 'embedding_idx', 'embedding', 384, false)`
  2. Verify index creation
  3. Handle KuzuDB WASM limitations (may need custom HNSW)
- **Fallback**: If KuzuDB WASM lacks vector index, implement brute-force cosine similarity in Rust
- **Deliverable**: `GraphDatabase.create_vector_index()` + `vector_search()`
- **Acceptance**: Index creation <10s, query <100ms for top-10

#### 5.3 Hybrid Search (BM25 + Semantic)
- **Action**: Combine keyword and vector search
- **Algorithm**:
  1. Run BM25 (KuzuDB FTS) → get keyword results
  2. Run vector search → get semantic results
  3. Reciprocal Rank Fusion: `score = Σ 1/(k + rank)`
  4. Return top-k combined
- **Deliverable**: `GitNexus.search()` with `mode: "hybrid"`
- **Acceptance**: "auth middleware" finds both `AuthMiddleware` class and `authenticate()` function

#### 5.4 Search UI Enhancement
- **Action**: Add search mode toggle, filters, result ranking
- **Changes in `SearchPanel.tsx`**:
  - Toggle: Hybrid | Semantic | Keyword
  - Filters: Language, File type, Community
  - Result score visualization
  - "Did you mean?" suggestions
- **Deliverable**: Full-featured search panel
- **Acceptance**: Users can find symbols by concept, not just name

---

### Task 6: Git Authentication (P1) — **1 week**
**Owner**: AI Agent — Git crate + Web UI  
**Depends On**: None  
**Blocks**: None (enhancement)

#### 6.1 HTTPS Authentication
- **Action**: Configure isomorphic-git for HTTPS clone/push
- **Requirements**:
  - Personal Access Token (PAT) input in UI
  - Store in `sessionStorage` (never `localStorage` for security)
  - Support GitHub, GitLab, Bitbucket
  - CORS proxy for cross-origin requests (or use `cors.isomorphic-git.org`)
- **Implementation**:
  ```typescript
  // UI component for git auth
  function GitAuthModal() {
      const [token, setToken] = useState('');
      const [provider, setProvider] = useState('github');

      async function cloneRepo(url: string) {
          const git = await GitRepo.new('/tmp/repo');
          await git.clone(url, {
              corsProxy: 'https://cors.isomorphic-git.org',
              headers: {
                  Authorization: `Bearer ${token}`
              }
          });
      }
  }
  ```
- **Deliverable**: `GitAuthModal.tsx` + `gitnexus-git` HTTPS support
- **Acceptance**: Can clone private GitHub repo with PAT

#### 6.2 SSH Key Support (Future/P2)
- **Action**: Research WebCrypto API for SSH key generation
- **Note**: Browser SSH is extremely limited; may require WebAssembly OpenSSL
- **Decision**: Defer to P2, document limitation

#### 6.3 Git Status Integration
- **Action**: Show git branch, modified files, diff in UI
- **Changes**: Header shows branch name, FileTree shows modified indicators
- **Deliverable**: Real-time git status display
- **Acceptance**: Users see which files are uncommitted

---

### Task 7: Incremental Updates (P1) — **1.5 weeks**
**Owner**: AI Agent — Core + Graph crates  
**Depends On**: Task 2 (Worker pool for speed), Task 6 (git for change detection)  
**Blocks**: None (enhancement)

#### 7.1 Content Hashing for Incremental Detection
- **Action**: Hash file content, skip unchanged files
- **Algorithm**:
  1. Calculate SHA-256 of each file's content
  2. Store in KuzuDB: `File {id, contentHash}`
  3. On re-import, compare hashes
  4. Only parse files with changed hashes
- **Deliverable**: `GitNexus.import_from_handle()` with hash check
- **Acceptance**: Re-import of unchanged repo completes in <2s

#### 7.2 Git Diff-Based Updates
- **Action**: Use `git diff` to find changed files
- **Pipeline**:
  1. `git statusMatrix()` → get modified/added/deleted files
  2. For modified: re-parse, update nodes, update edges
  3. For added: parse as new
  4. For deleted: remove nodes and dangling edges
  5. Update embeddings only for changed nodes
- **Deliverable**: `GitNexus.incremental_update()`
- **Acceptance**: 1-file change updates in <5s (vs 60s full re-analysis)

#### 7.3 Embedding Staleness Detection
- **Action**: Track which embeddings need regeneration
- **Schema**:
  ```cypher
  CREATE (e:CodeEmbedding {
      nodeId: "Function:foo",
      contentHash: "abc123",
      generatedAt: "2026-04-22T10:00:00Z"
  })
  ```
- **Logic**: If `node.contentHash != embedding.contentHash`, regenerate
- **Deliverable**: `EmbeddingPipeline.update_stale_embeddings()`
- **Acceptance**: Only changed nodes re-embedded

#### 7.4 Persistent State Management
- **Action**: Store analysis state in IndexedDB/OPFS
- **Schema**:
  - `repos` store: repo name, import date, file hashes
  - `graphs` store: KuzuDB database files
  - `embeddings` store: embedding vectors
- **Deliverable**: `GitNexus.save_state()` + `GitNexus.load_state()`
- **Acceptance**: Close tab, reopen → graph loads instantly from cache

---

## Dependency Graph

```
Task 1 (Tokenizer)
    │
    ├─→ Task 5 (Semantic Search) ──→ DONE
    │
Task 2 (Web Workers)
    │
    ├─→ Task 3 (Community Detection)
    │       │
    │       └─→ Task 4 (Process Extraction) ──→ DONE
    │
    ├─→ Task 7 (Incremental Updates) ──→ DONE
    │
Task 6 (Git Auth) ──→ DONE (independent)
```

---

## Milestones

| Milestone | Date | Deliverables | Criteria |
|-----------|------|--------------|----------|
| **M1: Tokenization** | Week 2 | Real tokenizer, correct embeddings | 100% match with Python transformers |
| **M2: Parallelism** | Week 3.5 | Web Workers, 4x speedup | 1000 files in <30s |
| **M3: Intelligence** | Week 5 | Communities, processes, semantic search | Modularity >0.3, hybrid search works |
| **M4: Polish** | Week 6.5 | Git auth, incremental updates, persistence | Private repo clone, <5s incremental |
| **M5: Release** | Week 8 | E2E tests, docs, performance benchmarks | All tests pass, <2min for 5000-file repo |

---

## Risk Register

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| `tokenizers` crate incompatible with WASM | High | Critical | Fallback to custom BPE or JS tokenizer |
| KuzuDB WASM lacks vector index | Medium | High | Implement brute-force search in Rust |
| Web Worker WASM loading flaky | Medium | High | Retry logic, fallback to main thread |
| 4GB WASM memory limit on large repos | Medium | High | Streaming, chunked processing, graph pruning |
| Safari lacks File System Access API | Medium | Medium | Fallback to drag-and-drop only |
| ONNX Runtime Web performance poor | Low | Medium | Use smaller model, INT4 quantization |
| isomorphic-git CORS issues | Medium | Medium | Self-host CORS proxy, document limitations |

---

## Testing Strategy

### Unit Tests (Rust)
```bash
cargo test --workspace --target wasm32-unknown-unknown
```
- Tokenizer: 100 test strings
- Louvain: 5 benchmark graphs
- Process extraction: 3 sample repos
- Embedding: Cosine similarity tests

### Integration Tests (Browser)
```bash
cd web && npx playwright test
```
- Import 10-file repo → analyze → search → context
- Import 1000-file repo → verify completes
- Close tab → reopen → verify cache works

### Performance Benchmarks
| Metric | Target | Measurement |
|--------|--------|-------------|
| Parse 1000 TS files | <30s | `performance.now()` |
| Build graph (10k nodes) | <10s | KuzuDB query timing |
| Community detection | <2s | Rust `Instant` |
| Semantic search query | <100ms | End-to-end |
| Incremental update (1 file) | <5s | End-to-end |
| Initial load (WASM + UI) | <3s | `performance.now()` |

---

## Documentation Deliverables

1. **API Reference** (`docs/API.md`): All WASM exports, JS types
2. **Architecture Decision Records** (`docs/ADR/`): Tokenizer choice, Worker approach
3. **Deployment Guide** (`docs/DEPLOY.md`): GitHub Pages, custom domain
4. **Troubleshooting** (`docs/TROUBLESHOOTING.md`): Common WASM errors
5. **Contributing** (`CONTRIBUTING.md`): Dev setup, PR process

---

## Appendix: File Checklist

### New Files to Create
- [ ] `crates/gitnexus-tokenize/Cargo.toml`
- [ ] `crates/gitnexus-tokenize/src/lib.rs`
- [ ] `crates/gitnexus-tokenize/src/bpe.rs` (if custom)
- [ ] `crates/gitnexus-graph/src/community.rs`
- [ ] `crates/gitnexus-graph/src/process.rs`
- [ ] `crates/gitnexus-graph/src/louvain.rs`
- [ ] `web/src/wasm/worker-pool.ts`
- [ ] `web/public/workers/parser-worker.js`
- [ ] `web/src/components/GitAuthModal.tsx`
- [ ] `web/src/components/SearchFilters.tsx`
- [ ] `docs/API.md`
- [ ] `docs/ADR/001-tokenizer.md`
- [ ] `docs/ADR/002-web-workers.md`
- [ ] `docs/ADR/003-community-detection.md`

### Files to Modify
- [ ] `crates/gitnexus-embed/src/lib.rs` — Replace fake tokenizer
- [ ] `crates/gitnexus-core/src/lib.rs` — Add Worker integration, incremental updates
- [ ] `crates/gitnexus-graph/src/lib.rs` — Add community/process methods
- [ ] `crates/gitnexus-git/src/lib.rs` — Add HTTPS auth
- [ ] `web/src/hooks/useStore.ts` — Add git state, cache management
- [ ] `web/src/components/SearchPanel.tsx` — Add hybrid search, filters
- [ ] `web/src/components/ContextPanel.tsx` — Add process timeline
- [ ] `web/src/components/GraphView.tsx` — Add community colors
- [ ] `.github/workflows/deploy.yml` — Add tokenizer build step

---

*End of Implementation Plan*
