# GitNexus WASM 🚀

> **Secure, Browser-Native Code Intelligence.**
> 
> AI-powered knowledge graphs, semantic search, and impact analysis — running entirely in your browser via WebAssembly. No data ever leaves your machine.

[![Deploy to GitHub Pages](https://github.com/abt0y/gitnexus-wasm/actions/workflows/deploy.yml/badge.svg)](https://github.com/abt0y/GitNexus/actions/workflows/deploy.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)

GitNexus WASM is a complete rewrite of the original GitNexus engine, designed to run 100% client-side. It leverages the latest advancements in WebAssembly to provide high-performance code analysis without the need for a server.

---

## ✨ Features

- **🌐 100% Browser-Based**: No backend server required. Your privacy is guaranteed.
- **🧠 Parallel Analysis**: Utilizes Web Workers for multi-threaded file parsing in the background.
- **🔍 Hybrid Semantic Search**: Combines BM25 keyword matching with vector embeddings via ONNX Runtime Web.
- **⚡ Impact Analysis**: Graph-based dependency tracing to visualize the side effects of code changes.
- **📊 Interactive Visualization**: Explore your codebase through a dynamic force-directed relationship graph.
- **🔒 Incremental Updates**: Smart hashing only re-analyzes files that have actually changed since your last session.

---

## 🏗️ How it Works

GitNexus bridges several cutting-edge WASM technologies into a single cohesive engine:

- **Rust Core**: Orchestrates analysis using high-performance Rust crates.
- **KuzuDB WASM**: An embeddable graph database running inside your browser tab.
- **Tree-sitter**: Language-specific grammars compiled to WASM for precise AST extraction.
- **ONNX Runtime Web**: In-browser ML inference for semantic token embeddings.

---

## 🚀 Deployment Guide (GitHub Pages)

Deploying GitNexus to GitHub Pages is the recommended way to use the tool. It costs $0 and ensures you always have access to your private analysis engine.

### Step 1: Fork & Setup
1. **Fork** this repository to your account.
2. Navigate to your fork's **Settings** tab.
3. Select **Pages** from the left sidebar.
4. Under **Build and deployment > Source**, ensure **GitHub Actions** is selected.

### Step 2: Automatic Build
1. Once GitHub Actions is enabled, the pipeline in `.github/workflows/deploy.yml` will trigger automatically on your next push to `main`.
2. The CI will:
   - Build all Rust crates to WASM.
   - Compile Tree-sitter parsers.
   - Download and bundle the quantized MiniLM embedding model.
   - Deploy the static Vite build to the `gh-pages` internal branch.

### Step 3: Access
1. Your site will be live at `https://<your-username>.github.io/<repo-name>/`.
2. **IMPORTANT**: If you encounter issues with the graph database, ensure your hosting provider supports `Cross-Origin-Opener-Policy: same-origin`. GitHub Pages supports this by default for standard assets.

---

## 🛠️ Local Development

### Getting Started (Local Development)

### Prerequisites
   ```

3. **Start Dev Server**:
   ```bash
   npm run dev
   ```
   Access the app at `http://localhost:5173`.

---

## 🔄 Comparison: Node.js vs WASM

| Feature | Node.js (Original) | WASM (Current) |
| :--- | :--- | :--- |
| **Setup** | `npm install -g gitnexus` | Visit URL |
| **Privacy** | Local file access | Local browser access |
| **Concurrency** | OS Threads | Web Workers |
| **Persistence** | SQLite / Local Files | IndexedDB / OPFS |
| **Mobile Support** | No | Yes (Modern browsers) |
| **Hosting Cost** | $0 (Self-hosted) | $0 (GitHub Pages) |

---

## 🧪 Testing

We use dynamic testing across the Rust and JS boundaries:

- **Rust Tests**: `cargo test --workspace`
- **WASM Browser Tests**: `wasm-pack test --headless --firefox`
- **Frontend Tests**: `cd web && npm test`

---

## 📝 License

Distributed under the MIT License. See `LICENSE` for more information.

**Made with ❤️ by the GitNexus Team.**
_No servers were harmed (or even used) in the making of this tool._
