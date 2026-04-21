# Task 2: Web Workers for Parallel Parsing — Implementation Guide

**Priority**: P0 (Critical Path)  
**Estimated Effort**: 1.5 weeks  
**Skill Level**: Intermediate (Web Workers, WASM loading, message passing)  
**Dependencies**: None (can parallelize with current parser)  
**Blocks**: Task 3 (Community Detection needs full graph), Task 7 (Incremental Updates)

---

## Problem Statement

Current parsing is **single-threaded**:

```rust
// gitnexus-core/src/lib.rs (current)
for (idx, file) in repo.files.iter().enumerate() {
    match engine.parser.parse(&file.path, content) {
        Ok(parsed) => parsed_files.push(parsed),
        Err(e) => warn!("Failed to parse {}", file.path),
    }
    // Update progress...
}
```

For a 1000-file repo, this takes ~20s on a modern CPU. With 4 Web Workers, we can achieve **~4x speedup** (theoretical) by parsing files in parallel.

---

## Solution Architecture

### Worker Pool Pattern

```
Main Thread (React)
    ├── Worker 1: loads tree-sitter + ts_parser.wasm → parses .ts files
    ├── Worker 2: loads tree-sitter + py_parser.wasm → parses .py files
    ├── Worker 3: loads tree-sitter + go_parser.wasm → parses .go files
    └── Worker 4: loads tree-sitter + rust_parser.wasm → parses .rs files
```

Each Worker:
1. Loads `tree-sitter.js` runtime (once)
2. Loads language-specific `.wasm` parser (on demand)
3. Receives `{filePath, content, language}` messages
4. Parses and returns serialized `ParsedFile`
5. Can be reused for multiple files

### Why Not `wasm-bindgen-rayon`?

`wasm-bindgen-rayon` provides a thread pool for Rust WASM, but:
- Requires `SharedArrayBuffer` + COOP/COEP headers
- Tree-sitter parsers are JS objects, not Rust data
- Simpler to manage Workers explicitly for this use case

**Decision**: Use custom Worker pool with message passing.

---

## Step-by-Step Implementation

### Step 1: Create Worker Script (Day 1-2)

```javascript
// web/public/workers/parser-worker.js
importScripts('https://cdn.jsdelivr.net/npm/web-tree-sitter@0.22.0/tree-sitter.js');

let PARSERS = {}; // language -> Parser instance cache

async function loadParser(language) {
    if (PARSERS[language]) return PARSERS[language];

    const parser = new TreeSitter.Parser();
    const langUrl = `/parsers/${language}.wasm`;
    const lang = await TreeSitter.Language.load(langUrl);
    parser.setLanguage(lang);

    PARSERS[language] = parser;
    return parser;
}

self.onmessage = async function(e) {
    const { id, filePath, content, language } = e.data;

    try {
        const parser = await loadParser(language);
        const tree = parser.parse(content);

        // Extract symbols (simplified — match Rust logic)
        const symbols = extractSymbols(tree.rootNode, content, filePath);
        const imports = extractImports(tree.rootNode, content);
        const calls = extractCalls(tree.rootNode, content);

        self.postMessage({
            id,
            success: true,
            result: {
                file_path: filePath,
                language,
                symbols,
                imports,
                calls,
            }
        });
    } catch (err) {
        self.postMessage({
            id,
            success: false,
            error: err.message,
        });
    }
};

function extractSymbols(node, source, filePath) {
    const symbols = [];
    const cursor = node.walk();

    do {
        const node = cursor.currentNode;
        const type = node.type;

        if (type === 'function_declaration' || type === 'function_definition') {
            const nameNode = node.childForFieldName('name');
            symbols.push({
                id: `Function:${nameNode ? nameNode.text : 'unknown'}`,
                name: nameNode ? nameNode.text : 'unknown',
                kind: 'Function',
                file_path: filePath,
                start_line: node.startPosition.row + 1,
                end_line: node.endPosition.row + 1,
                content: source.substring(node.startIndex, node.endIndex),
            });
        }
        // ... handle classes, methods, etc.
    } while (cursor.gotoNextSibling());

    return symbols;
}

function extractImports(node, source) {
    // Implementation for each language
}

function extractCalls(node, source) {
    // Implementation for each language
}
```

### Step 2: Create Worker Pool Manager (Day 3-4)

```typescript
// web/src/wasm/worker-pool.ts
interface ParseJob {
    id: number;
    filePath: string;
    content: string;
    language: string;
}

interface ParseResult {
    id: number;
    success: boolean;
    result?: ParsedFile;
    error?: string;
}

export class ParserWorkerPool {
    private workers: Worker[] = [];
    private queue: ParseJob[] = [];
    private activeJobs: Map<number, { resolve: Function; reject: Function }> = new Map();
    private jobIdCounter = 0;
    private maxWorkers: number;
    private parsersPerWorker: Map<number, Set<string>> = new Map();

    constructor(maxWorkers: number = navigator.hardwareConcurrency || 4) {
        this.maxWorkers = maxWorkers;
    }

    async init(): Promise<void> {
        for (let i = 0; i < this.maxWorkers; i++) {
            const worker = new Worker('/workers/parser-worker.js');

            worker.onmessage = (e: MessageEvent<ParseResult>) => {
                const { id, success, result, error } = e.data;
                const job = this.activeJobs.get(id);

                if (job) {
                    this.activeJobs.delete(id);
                    if (success) {
                        job.resolve(result);
                    } else {
                        job.reject(new Error(error));
                    }

                    // Process next job from queue
                    this.processQueue();
                }
            };

            worker.onerror = (err) => {
                console.error('Worker error:', err);
                // Respawn worker
                this.respawnWorker(i);
            };

            this.workers.push(worker);
            this.parsersPerWorker.set(i, new Set());
        }
    }

    async parseBatch(files: Array<{ path: string; content: string }>): Promise<ParsedFile[]> {
        // Group files by language
        const byLanguage = new Map<string, Array<{ path: string; content: string }>>();

        for (const file of files) {
            const lang = detectLanguage(file.path);
            if (!lang) continue;

            if (!byLanguage.has(lang)) {
                byLanguage.set(lang, []);
            }
            byLanguage.get(lang)!.push(file);
        }

        // Distribute across workers
        const promises: Promise<ParsedFile>[] = [];
        let workerIndex = 0;

        for (const [lang, langFiles] of byLanguage) {
            for (const file of langFiles) {
                const jobId = ++this.jobIdCounter;

                const promise = new Promise<ParsedFile>((resolve, reject) => {
                    this.activeJobs.set(jobId, { resolve, reject });

                    // Assign to worker (round-robin)
                    const worker = this.workers[workerIndex % this.maxWorkers];
                    workerIndex++;

                    worker.postMessage({
                        id: jobId,
                        filePath: file.path,
                        content: file.content,
                        language: lang,
                    });
                });

                promises.push(promise);
            }
        }

        return Promise.all(promises);
    }

    private processQueue(): void {
        // If queue has items and workers are available, dispatch
        while (this.queue.length > 0 && this.activeJobs.size < this.maxWorkers * 2) {
            const job = this.queue.shift()!;
            // Find least-loaded worker
            const workerIndex = this.findLeastLoadedWorker();
            const worker = this.workers[workerIndex];

            this.activeJobs.set(job.id, {
                resolve: () => {}, // Will be set by parseBatch
                reject: () => {},
            });

            worker.postMessage(job);
        }
    }

    private findLeastLoadedWorker(): number {
        let minLoad = Infinity;
        let minIndex = 0;

        for (let i = 0; i < this.maxWorkers; i++) {
            const load = Array.from(this.activeJobs.values()).filter(
                (_, idx) => idx % this.maxWorkers === i
            ).length;

            if (load < minLoad) {
                minLoad = load;
                minIndex = i;
            }
        }

        return minIndex;
    }

    private respawnWorker(index: number): void {
        this.workers[index].terminate();

        const newWorker = new Worker('/workers/parser-worker.js');
        // ... reattach handlers
        this.workers[index] = newWorker;
        this.parsersPerWorker.set(index, new Set());
    }

    terminate(): void {
        for (const worker of this.workers) {
            worker.terminate();
        }
        this.workers = [];
        this.queue = [];
        this.activeJobs.clear();
    }
}

function detectLanguage(filePath: string): string | null {
    const ext = filePath.split('.').pop()?.toLowerCase();
    const map: Record<string, string> = {
        'ts': 'typescript', 'tsx': 'typescript', 'mts': 'typescript',
        'js': 'javascript', 'jsx': 'javascript', 'mjs': 'javascript',
        'py': 'python', 'pyi': 'python',
        'go': 'go',
        'rs': 'rust',
        'java': 'java',
        'c': 'c', 'cpp': 'cpp', 'h': 'c', 'hpp': 'cpp',
        'cs': 'csharp',
        'php': 'php',
        'swift': 'swift',
        'rb': 'ruby',
    };
    return map[ext || ''] || null;
}
```

### Step 3: Integrate with Rust Core (Day 5-6)

Modify `gitnexus-core/src/lib.rs` to accept pre-parsed results:

```rust
#[wasm_bindgen]
impl GitNexus {
    pub async fn analyze_with_parsed(
        &self,
        parsed_files: JsValue,  // From Worker pool
        progress_callback: JsValue,
    ) -> Result<JsResult, JsValue> {
        let files: Vec<ParsedFile> = serde_wasm_bindgen::from_value(parsed_files)?;

        // Skip parsing phase, go straight to graph building
        self.report_progress(&callback, "building_graph", 30, "Building knowledge graph...", None)?;

        // Build graph from pre-parsed files
        let builder = GraphBuilder::new().await?;
        let result = builder.build_from_parsed(
            serde_wasm_bindgen::to_value(&files).unwrap()
        ).await?;

        // ... rest of pipeline
    }
}
```

### Step 4: UI Integration (Day 7-8)

Modify `useGitNexusStore.ts`:

```typescript
async analyzeRepo(): Promise<void> {
    const { engine } = get();
    if (!engine) return;

    set({ isAnalyzing: true });

    try {
        // Step 1: Parse files in parallel using Workers
        const workerPool = new ParserWorkerPool();
        await workerPool.init();

        const files = get().currentRepo?.files.filter(f => !f.isDirectory) || [];
        const parsedFiles = await workerPool.parseBatch(files);

        workerPool.terminate();

        // Step 2: Build graph in Rust
        const result = await engine.analyze_with_parsed(
            parsedFiles,
            progressCallback
        );

        // ...
    } catch (err) {
        console.error('Analysis failed:', err);
    } finally {
        set({ isAnalyzing: false });
    }
}
```

### Step 5: Error Handling & Recovery (Day 9-10)

**Worker Crash Recovery**:
```typescript
private respawnWorker(index: number): void {
    console.warn(`Respawning Worker ${index}...`);

    // Terminate crashed worker
    this.workers[index].terminate();

    // Create new worker
    const newWorker = new Worker('/workers/parser-worker.js');
    // Reattach all message/error handlers
    // ...

    this.workers[index] = newWorker;
    this.parsersPerWorker.set(index, new Set());

    // Retry failed jobs
    for (const [id, job] of this.activeJobs) {
        if (job.workerIndex === index) {
            this.queue.unshift(job); // Re-queue at front
            this.activeJobs.delete(id);
        }
    }

    this.processQueue();
}
```

**Timeout Handling**:
```typescript
const TIMEOUT_MS = 30000; // 30 seconds per file

const promise = new Promise<ParsedFile>((resolve, reject) => {
    const timeout = setTimeout(() => {
        reject(new Error(`Parse timeout after ${TIMEOUT_MS}ms`));
    }, TIMEOUT_MS);

    this.activeJobs.set(jobId, {
        resolve: (result: ParsedFile) => {
            clearTimeout(timeout);
            resolve(result);
        },
        reject: (err: Error) => {
            clearTimeout(timeout);
            reject(err);
        },
    });
});
```

---

## Acceptance Criteria

- [ ] 4 Workers parse 1000 files in <30s (vs <120s single-threaded)
- [ ] Worker crash auto-recovers with retry
- [ ] Timeout after 30s per file, doesn't hang
- [ ] Memory per Worker <512MB
- [ ] All 12 language parsers load correctly in Workers
- [ ] Progress bar shows "Parsing X/Y files (Z workers active)"
- [ ] No memory leaks after parsing 10 batches
- [ ] Safari, Chrome, Firefox all support Worker WASM loading

---

## Deliverables

1. `web/public/workers/parser-worker.js` — Worker script
2. `web/src/wasm/worker-pool.ts` — Pool manager
3. `web/src/hooks/useStore.ts` — Modified (uses Worker pool)
4. `crates/gitnexus-core/src/lib.rs` — Modified (accepts pre-parsed)
5. `docs/ADR/002-web-workers.md` — Architecture decision record
