//! GitNexus Core - Main WASM entry point
//!
//! This is the primary WASM module that orchestrates:
//! - File system access (File System Access API / drag-drop)
//! - Code parsing via tree-sitter WASM
//! - Knowledge graph construction via KuzuDB WASM
//! - Semantic embeddings via ONNX Runtime Web
//! - Git operations via isomorphic-git
//! - Search, impact analysis, and context queries

use wasm_bindgen::prelude::*;
use js_sys::{Promise, Reflect, Object};
use web_sys::{File, FileReader};
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;
use log::info;

use gitnexus_shared::*;
use gitnexus_parse::WasmParser;
use gitnexus_graph::{GraphDatabase, GraphBuilder};

// ============================================================================
// Main engine state
// ============================================================================

pub struct GitNexusEngine {
    parser: WasmParser,
    graph: Option<GraphDatabase>,
    embedder: Option<serde_json::Value>, // Placeholder for embedding engine logic
    current_repo: Option<RepoState>,
}

pub struct RepoState {
    name: String,
    files: Vec<FileEntry>,
    hashes: HashMap<String, String>,
}

// ============================================================================
// WASM API - Main Entry Point
// ============================================================================

#[wasm_bindgen]
pub struct GitNexus {
    engine: Arc<RwLock<GitNexusEngine>>,
}

#[wasm_bindgen]
impl GitNexus {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<GitNexus, JsValue> {
        console_error_panic_hook::set_once();
        wasm_logger::init(wasm_logger::Config::new(log::Level::Info));

        info!("GitNexus WASM initializing");

        let engine = GitNexusEngine {
            parser: WasmParser::new(),
            graph: None,
            embedder: None,
            current_repo: None,
        };

        Ok(GitNexus {
            engine: Arc::new(RwLock::new(engine)),
        })
    }

    pub async fn init(&self) -> Result<JsResult, JsValue> {
        // Refactor: parser.init may stay async, but constructors are now static factories
        self.engine.write().parser.init().await?;
        
        let graph = GraphDatabase::open(None).await?;
        self.engine.write().graph = Some(graph);
        
        Ok(JsResult::ok(serde_json::json!({
            "status": "ready",
            "version": env!("CARGO_PKG_VERSION")
        }).to_string()))
    }

    pub async fn import_from_handle(&self, handle: JsValue) -> Result<JsResult, JsValue> {
        let dir_handle = Object::from(handle);
        let mut files = Vec::new();
        self.read_directory_recursive(&dir_handle, "", &mut files).await?;

        let repo_name = Reflect::get(&dir_handle, &"name".into())?
            .as_string().unwrap_or_default();

        let mut hashes = HashMap::new();
        for file in &files {
            if let Some(content) = &file.content {
                hashes.insert(file.path.clone(), gitnexus_shared::hash::hash_file(&file.path, content));
            }
        }

        let repo = RepoState {
            name: repo_name.clone(),
            files,
            hashes,
        };

        self.engine.write().current_repo = Some(repo);

        Ok(JsResult::ok(serde_json::json!({
            "name": repo_name,
            "fileCount": self.engine.read().current_repo.as_ref().map(|r| r.files.len()).unwrap_or(0),
        }).to_string()))
    }

    async fn read_directory_recursive(
        &self,
        dir_handle: &Object,
        path_prefix: &str,
        files: &mut Vec<FileEntry>,
    ) -> Result<(), JsValue> {
        let entries_method: js_sys::Function = Reflect::get(dir_handle, &"entries".into())?.dyn_into()?;
        let entries_async_iter = entries_method.call0(dir_handle)?;

        loop {
            let next_method: js_sys::Function = Reflect::get(&entries_async_iter, &"next".into())?.dyn_into()?;
            let next_promise: Promise = next_method.call0(&entries_async_iter)?.dyn_into()?;
            let next_result = wasm_bindgen_futures::JsFuture::from(next_promise).await?;

            if Reflect::get(&next_result, &"done".into())?.as_bool().unwrap_or(true) {
                break;
            }

            let entry = Reflect::get(&next_result, &"value".into())?;
            let entry_array = js_sys::Array::from(&entry);
            let name = entry_array.get(0).as_string().unwrap_or_default();
            let handle = entry_array.get(1);
            let kind = Reflect::get(&handle, &"kind".into())?.as_string().unwrap_or_default();

            if name == ".git" || name == "node_modules" || name == "dist" || name == "target" {
                continue;
            }

            let full_path = if path_prefix.is_empty() { name } else { format!("{}/{}", path_prefix, name) };

            if kind == "directory" {
                self.read_directory_recursive(&Object::from(handle), &full_path, files).await?;
            } else {
                let get_file_method: js_sys::Function = Reflect::get(&handle, &"getFile".into())?.dyn_into()?;
                let file_promise: Promise = get_file_method.call0(&handle)?.dyn_into()?;
                let file: File = wasm_bindgen_futures::JsFuture::from(file_promise).await?.dyn_into()?;
                let content = self.read_file_text(&file).await?;

                files.push(FileEntry {
                    path: full_path,
                    name: file.name(),
                    is_directory: false,
                    content: Some(content),
                    size: Some(file.size() as u64),
                });
            }
        }
        Ok(())
    }

    async fn read_file_text(&self, file: &File) -> Result<String, JsValue> {
        let promise = Promise::new(&mut |resolve, _reject| {
            let reader = FileReader::new().unwrap();
            let onload = Closure::once_into_js(move |event: web_sys::ProgressEvent| {
                let target = event.target().unwrap();
                let reader: FileReader = target.dyn_into().unwrap();
                let result = reader.result().unwrap_or(JsValue::NULL);
                let _ = resolve.call1(&JsValue::NULL, &result);
            });
            reader.set_onload(Some(onload.as_ref().dyn_ref().unwrap()));
            reader.read_as_text(file).unwrap();
        });
        let result = wasm_bindgen_futures::JsFuture::from(promise).await?;
        Ok(result.as_string().unwrap_or_default())
    }

    pub fn get_files_for_parsing(&self) -> Result<JsValue, JsValue> {
        let engine = self.engine.read();
        let repo = engine.current_repo.as_ref().ok_or_else(|| JsValue::from_str("No repo"))?;
        let files: Vec<_> = repo.files.iter()
            .filter(|f| !f.is_directory && f.content.is_some())
            .map(|f| serde_json::json!({ "path": f.path, "content": f.content }))
            .collect();
        serde_wasm_bindgen::to_value(&files).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub async fn ingest_parsed_results(&self, results_js: JsValue) -> Result<JsResult, JsValue> {
        // Use static factory
        let builder = GraphBuilder::create().await?;
        let result = builder.build_from_parsed(results_js).await?;
        Ok(result)
    }

    pub async fn run_community_detection(&self, config: JsValue) -> Result<JsResult, JsValue> {
        let engine = self.engine.read();
        let graph = engine.graph.as_ref().ok_or_else(|| JsValue::from_str("No graph"))?;
        graph.detect_communities(config).await
    }

    pub async fn run_process_extraction(&self, config: JsValue) -> Result<JsResult, JsValue> {
        let engine = self.engine.read();
        let graph = engine.graph.as_ref().ok_or_else(|| JsValue::from_str("No graph"))?;
        graph.extract_processes(config).await
    }

    pub async fn search(&self, query: JsValue) -> Result<JsResult, JsValue> {
        let q: SearchQuery = serde_wasm_bindgen::from_value(query).unwrap();
        let engine = self.engine.read();
        let graph = engine.graph.as_ref().ok_or_else(|| JsValue::from_str("No graph"))?;
        
        if q.semantic.unwrap_or(false) && q.embedding.is_some() {
             let results = graph.hybrid_search(
                 &q.query, 
                 serde_wasm_bindgen::to_value(&q.embedding).unwrap(),
                 q.limit.unwrap_or(10)
             ).await?;
             Ok(results)
        } else {
             graph.search(serde_wasm_bindgen::to_value(&q).unwrap()).await
        }
    }

    pub async fn export_graph(&self) -> Result<String, JsValue> {
        let engine = self.engine.read();
        let graph = engine.graph.as_ref().ok_or_else(|| JsValue::from_str("No graph"))?;
        graph.export().await
    }

    pub async fn context(&self, name: String, uid: Option<String>) -> Result<JsResult, JsValue> {
        let engine = self.engine.read();
        let graph = engine.graph.as_ref().ok_or_else(|| JsValue::from_str("No graph"))?;
        graph.get_context(name, uid).await
    }
}
