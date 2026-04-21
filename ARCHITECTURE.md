# GitNexus WASM Architecture

## Overview

GitNexus WASM is a complete rewrite of the original Node.js-based GitNexus tool to run entirely within the browser using WebAssembly. This document describes the architecture, design decisions, and implementation details.

## Design Goals

1. **Zero-Server**: No backend infrastructure required
2. **Zero-Install**: Users visit a URL, no package managers needed
3. **Privacy-First**: All data stays in the browser
4. **Free Hosting**: Deployable to GitHub Pages
5. **Feature Parity**: Match original tool's core capabilities

## System Architecture

### 1. WASM Runtime Layer

The core engine is written in Rust and compiled to WebAssembly:

```
Rust Code → wasm-bindgen → WASM Module → JS Glue → Browser
```

**Key crates:**
- `gitnexus-core`: Main orchestrator, WASM exports
- `gitnexus-parse`: Tree-sitter integration
- `gitnexus-graph`: KuzuDB graph operations
- `gitnexus-embed`: ONNX embedding inference
- `gitnexus-git`: Git operations wrapper
- `gitnexus-shared`: Cross-crate types

### 2. Browser APIs Used

| API | Purpose |
|-----|---------|
| **File System Access API** | Read local directories |
| **Drag & Drop API** | Import files/folders |
| **IndexedDB** | Persistent graph storage |
| **Origin Private File System (OPFS)** | Large file storage |
| **Web Workers** | Background parsing |
| **SharedArrayBuffer** | Worker communication |
| **WebAssembly** | Core engine execution |

### 3. External JS Libraries

Loaded dynamically via `<script>` tags or dynamic imports:

- **KuzuDB WASM** (`kuzu-wasm`): Graph database engine
- **ONNX Runtime Web** (`onnxruntime-web`): ML inference
- **Tree-sitter** (`web-tree-sitter`): Parser runtime
- **isomorphic-git** + **LightningFS**: Git operations

### 4. Data Flow

```
User drops folder
    ↓
File System Access API reads files
    ↓
Rust WASM parses each file (tree-sitter WASM)
    ↓
AST → Graph nodes/edges (KuzuDB WASM)
    ↓
Community detection (Louvain in Rust)
    ↓
Process extraction (BFS/DFS in Rust)
    ↓
Optional: Generate embeddings (ONNX WASM)
    ↓
Store in IndexedDB (via KuzuDB persistence)
    ↓
Visualize in React (react-force-graph)
```

## Memory Management

WASM has a hard ~4GB memory limit (32-bit). Strategies to handle large repos:

1. **Chunked Processing**: Process files in batches of 100
2. **Streaming Parsing**: Don't hold full AST in memory
3. **Lazy Loading**: Load parsers on demand
4. **Graph Pruning**: Remove low-confidence edges
5. **Incremental Updates**: Only re-parse changed files

## Performance Considerations

| Operation | Node.js (ms) | WASM (ms) | Notes |
|-----------|-------------|-----------|-------|
| Parse 1000 TS files | ~500 | ~1500 | WASM tree-sitter slower |
| Build graph (10k nodes) | ~200 | ~600 | KuzuDB WASM overhead |
| Embed 1000 chunks | ~300 | ~2000 | ONNX Web slower |
| Search (BM25) | ~50 | ~150 | Acceptable |
| Impact analysis (BFS) | ~100 | ~300 | Graph traversal |

**Optimization strategies:**
- Web Workers for parallel parsing
- SIMD for embedding computation
- IndexedDB caching for repeated queries
- Lazy model loading

## Security Model

1. **CSP**: Strict Content Security Policy
2. **No eval()**: All code is compiled WASM
3. **Sandboxed**: Browser security model applies
4. **No network**: Except for CDN-loaded libraries
5. **OPFS isolation**: Each origin has isolated storage

## Trade-offs vs Original

### What We Gained
- Zero install for users
- Free hosting (GitHub Pages)
- No server maintenance
- True zero-server architecture

### What We Lost
- MCP server (stdio not available in browser)
- Native git binary (isomorphic-git is slower)
- Multi-repo global registry (per-origin storage)
- File watching (no chokidar equivalent)
- Raw performance (~2-5x slower)

### What's Different
- No `npx gitnexus serve` — just open URL
- No `gitnexus analyze` CLI — browser UI only
- Embeddings are optional (lazy load)
- Git operations are limited (no push/pull auth)

## Deployment

### GitHub Pages

```yaml
# .github/workflows/deploy.yml
# Builds on every push to main
# - Compiles Rust to WASM
# - Builds tree-sitter parsers
# - Bundles React app
# - Deploys to gh-pages branch
```

### CDN Assets

Large assets (models, parsers) are:
1. Built in CI
2. Committed to `gh-pages` branch
3. Served from same origin (no CORS issues)

### Caching Strategy

- **WASM module**: Cache for 1 year (immutable)
- **Parsers**: Cache for 1 year (versioned)
- **Model**: Cache for 1 week (may update)
- **App shell**: Cache for 1 hour (frequent updates)

## Future Work

1. **WebGPU acceleration** for embeddings (when ONNX supports)
2. **Service Worker** for offline capability
3. **WebRTC** for peer-to-peer collaboration
4. **WebAssembly GC** when stable (reduce JS glue)
5. **Streaming parsing** for >10k file repos
6. **Incremental analysis** with git diff

## References

- [WASM Memory Limits](https://v8.dev/blog/4gb-wasm-memory)
- [KuzuDB WASM](https://github.com/kuzudb/kuzu-wasm)
- [Tree-sitter WASM](https://github.com/tree-sitter/tree-sitter/blob/master/lib/binding_web/README.md)
- [ONNX Runtime Web](https://onnxruntime.ai/docs/tutorials/web/)
- [File System Access API](https://developer.mozilla.org/en-US/docs/Web/API/File_System_Access_API)
