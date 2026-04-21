# GitNexus WASM 🚀

> **Zero-server, browser-native code intelligence.**
> 
> AI-powered knowledge graphs, semantic search, and impact analysis — running entirely in your browser via WebAssembly.

[![Deploy to GitHub Pages](https://github.com/abt0y/GitNexus/actions/workflows/deploy.yml/badge.svg)](https://github.com/abt0y/GitNexus/actions/workflows/deploy.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## ✨ Features

- **🌐 100% Browser-Based** — No backend server, no data leaves your machine
- **🧠 AI-Powered Analysis** — Knowledge graphs, community detection, process extraction
- **🔍 Semantic Search** — Hybrid BM25 + vector search with ONNX Runtime Web
- **⚡ Impact Analysis** — Upstream/downstream dependency tracing
- **📊 Interactive Visualization** — Force-directed graph with 2D/3D views
- **🔒 Privacy-First** — All code analysis happens locally in WASM
- **📁 File System Access** — Direct directory access via modern browser APIs
- **🆓 Free Hosting** — Deploy to GitHub Pages at zero cost

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    GitHub Pages (Static)                      │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐    │
│  │  index.html │  │  app.wasm   │  │  tree-sitter/*.wasm│    │
│  │  (shell)    │  │  (Rust core)│  │  (language parsers)│    │
│  └─────────────┘  └─────────────┘  └─────────────────────┘    │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐    │
│  │ onnx/*.wasm │  │ kuzu.wasm   │  │  embedding model    │    │
│  │ (inference) │  │ (graph DB)  │  │  (~20MB quantized)  │    │
│  └─────────────┘  └─────────────┘  └─────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                      Browser Runtime                          │
│  - WASM execution (single-threaded + SIMD)                  │
│  - IndexedDB / OPFS for persistent storage                    │
│  - File System Access API for local repo import               │
│  - Web Workers for background parsing                         │
└─────────────────────────────────────────────────────────────┘
```

## 🚀 Quick Start

### Visit the Hosted Version

Simply open **[gitnexus.vercel.app](https://gitnexus.vercel.app)** (or your GitHub Pages URL) in a modern browser:

1. Click **"Open Directory"** or **drag & drop** your code folder
2. Click **"Analyze"** to build the knowledge graph
3. Explore, search, and analyze your codebase!

### Local Development

```bash
# Clone the repository
git clone https://github.com/abt0y/GitNexus.git
cd GitNexus

# Install dependencies and build WASM
cargo build --target wasm32-unknown-unknown

# Build tree-sitter parsers
./scripts/build-parsers.sh

# Install web dependencies
cd web && npm install

# Start development server
npm run dev
```

## 📦 Tech Stack

| Layer | Technology |
|-------|-----------|
| **Core Engine** | Rust → WASM (wasm-bindgen) |
| **Parsing** | Tree-sitter grammars compiled to WASM |
| **Graph DB** | KuzuDB WASM (in-memory + IndexedDB persistence) |
| **Embeddings** | ONNX Runtime Web + all-MiniLM-L6-v2 (INT8) |
| **Git** | isomorphic-git (pure JS, no native binary) |
| **UI** | React 18 + Vite + Tailwind CSS v4 |
| **Visualization** | react-force-graph-2d |
| **Hosting** | GitHub Pages + GitHub Actions CI/CD |

## 🔧 Supported Languages

- **TypeScript / JavaScript** — Full AST extraction, imports, calls
- **Python** — Functions, classes, imports, decorators
- **Go** — Functions, methods, interfaces, structs
- **Rust** — Functions, traits, impls, modules
- **Java** — Classes, interfaces, methods, packages
- **C / C++** — Functions, classes, namespaces
- **C#** — Classes, interfaces, methods
- **PHP** — Functions, classes, namespaces
- **Swift** — Classes, functions, protocols
- **Ruby** — Methods, classes, modules

## 🧩 Crates

```
crates/
├── gitnexus-shared/     # Shared types (nodes, relationships, search results)
├── gitnexus-parse/      # Tree-sitter WASM parser integration
├── gitnexus-graph/      # KuzuDB WASM graph database bindings
├── gitnexus-embed/      # ONNX Runtime Web embedding engine
├── gitnexus-git/        # isomorphic-git wrapper for browser
└── gitnexus-core/       # Main orchestration engine (WASM entry point)
```

## 🔄 Comparison: Node.js vs WASM

| Feature | Original (Node.js) | WASM Rewrite |
|---------|-------------------|--------------|
| **Install** | `npm install -g gitnexus` | Visit URL |
| **Server** | Local Express on :4747 | None (browser-only) |
| **Data Privacy** | Local processing | Local processing |
| **Parsing Speed** | Fast (native) | ~2-5x slower |
| **Memory Limit** | 8GB+ configurable | ~4GB (WASM 32-bit) |
| **Embedding Speed** | Fast (ONNX native) | Slower (WASM ONNX) |
| **Git Operations** | Native `child_process` | isomorphic-git (pure JS) |
| **MCP Server** | stdio process | Not available (replace with in-browser chat) |
| **Hosting Cost** | $0 (self-hosted) | $0 (GitHub Pages) |

## 🛠️ Building from Source

### Prerequisites

- Rust 1.75+ with `wasm32-unknown-unknown` target
- wasm-pack
- Node.js 20+
- tree-sitter CLI

### Build Steps

```bash
# 1. Install Rust WASM target
rustup target add wasm32-unknown-unknown

# 2. Install wasm-pack
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

# 3. Build tree-sitter parsers
./scripts/build-parsers.sh

# 4. Quantize embedding model (optional, uses CDN fallback)
python scripts/quantize-model.py

# 5. Build Rust WASM core
cd crates/gitnexus-core
wasm-pack build --target web --out-dir ../../web/pkg

# 6. Build and serve web UI
cd ../../web
npm install
npm run dev
```

### Deploy to GitHub Pages

1. Fork this repository
2. Go to **Settings → Pages**
3. Set source to **GitHub Actions**
4. Push to `main` branch — CI will build and deploy automatically

## 🧪 Testing

```bash
# Run Rust unit tests
cargo test --workspace

# Run WASM tests in browser
wasm-pack test --headless --firefox

# Run web UI tests
cd web && npm test
```

## 📝 License

MIT License — see [LICENSE](LICENSE) for details.

## 🙏 Acknowledgments

- [KuzuDB](https://kuzudb.com/) — Graph database with WASM support
- [Tree-sitter](https://tree-sitter.github.io/) — Parser generator framework
- [ONNX Runtime Web](https://onnxruntime.ai/docs/tutorials/web/) — ML inference in browsers
- [isomorphic-git](https://isomorphic-git.org/) — Pure JavaScript git implementation
- [react-force-graph](https://github.com/vasturiano/react-force-graph) — Graph visualization

---

**Made with ❤️ and Rust.** No servers were harmed in the making of this tool.
