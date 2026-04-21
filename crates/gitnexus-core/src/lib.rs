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
use js_sys::{Function, Promise, Reflect, Array, Object};
use web_sys::{console, File, FileList, FileReader, HtmlInputElement, DragEvent, DataTransfer};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;
use once_cell::sync::OnceCell;
use log::{info, warn, error};

use gitnexus_shared::*;
use gitnexus_parse::{WasmParser, ParserRegistry, ParsedFile};
use gitnexus_graph::{GraphDatabase, GraphBuilder};
use gitnexus_embed::{EmbeddingEngine, EmbeddingPipeline, TextChunker};
use gitnexus_git::GitRepo;

// ============================================================================
// Global State
// ============================================================================

static GLOBAL_ENGINE: OnceCell<RwLock<GitNexusEngine>> = OnceCell::new();

/// Main engine state
pub struct GitNexusEngine {
    parser: WasmParser,
    graph: Option<GraphDatabase>,
    embedder: Option<EmbeddingEngine>,
    current_repo: Option<RepoState>,
}

pub struct RepoState {
    name: String,
    path: String,
    files: Vec<FileEntry>,
    is_git_repo: bool,
    git_branch: Option<String>,
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
    /// Initialize the GitNexus engine
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<GitNexus, JsValue> {
        console_error_panic_hook::set_once();
        wasm_logger::init(wasm_logger::Config::default());

        info!("GitNexus WASM v{} initializing", env!("CARGO_PKG_VERSION"));

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

    /// Initialize all subsystems
    pub async fn init(&self) -> Result<JsResult, JsValue> {
        info!("Initializing GitNexus subsystems...");

        let mut engine = self.engine.write();

        // Initialize parser
        engine.parser.init().await.map_err(|e| {
            error!("Parser init failed: {:?}", e);
            e
        })?;
        info!("Parser initialized");

        // Initialize graph database (in-memory)
        let graph = GraphDatabase::new(None).await.map_err(|e| {
            error!("Graph DB init failed: {:?}", e);
            e
        })?;
        engine.graph = Some(graph);
        info!("Graph database initialized");

        // Initialize embedder (lazy - will load model on first use)
        let mut embedder = EmbeddingEngine::new();
        // Don't load model yet - do it on demand to save startup time
        engine.embedder = Some(embedder);
        info!("Embedder ready (model not loaded yet)");

        Ok(JsResult::ok(&serde_json::json!({
            "status": "ready",
            "version": env!("CARGO_PKG_VERSION"),
            "features": ["parse", "graph", "search", "impact", "context"]
        })))
    }

    // ========================================================================
    // File System / Repo Import
    // ========================================================================

    /// Import repository from File System Access API handle
    pub async fn import_from_handle(&self, handle: JsValue) -> Result<JsResult, JsValue> {
        info!("Importing repository from directory handle");

        // handle is a FileSystemDirectoryHandle
        let dir_handle = Object::from(handle);

        let mut files = Vec::new();
        self.read_directory_recursive(&dir_handle, "", &mut files).await?;

        let repo_name = Reflect::get(&dir_handle, &"name".into())?
            .as_string()
            .unwrap_or("unknown".to_string());

        let is_git = files.iter().any(|f| f.path.ends_with("/.git"));

        let repo = RepoState {
            name: repo_name.clone(),
            path: "/".to_string(),
            files,
            is_git_repo: is_git,
            git_branch: None,
        };

        self.engine.write().current_repo = Some(repo);

        Ok(JsResult::ok(&serde_json::json!({
            "name": repo_name,
            "fileCount": self.engine.read().current_repo.as_ref().map(|r| r.files.len()).unwrap_or(0),
            "isGitRepo": is_git,
        })))
    }

    /// Import from drag-and-drop FileList
    pub async fn import_from_files(&self, file_list: FileList) -> Result<JsResult, JsValue> {
        info!("Importing {} files from drag-and-drop", file_list.length());

        let mut files = Vec::new();
        for i in 0..file_list.length() {
            let file = file_list.get(i).ok_or("Invalid file")?;
            let content = self.read_file_text(&file).await?;

            files.push(FileEntry {
                path: file.name(),
                name: file.name(),
                is_directory: false,
                content: Some(content),
                size: Some(file.size() as u64),
            });
        }

        let repo = RepoState {
            name: "dropped-files".to_string(),
            path: "/".to_string(),
            files,
            is_git_repo: false,
            git_branch: None,
        };

        self.engine.write().current_repo = Some(repo);

        Ok(JsResult::ok(&serde_json::json!({
            "fileCount": file_list.length(),
        })))
    }

    async fn read_directory_recursive(
        &self,
        dir_handle: &Object,
        path_prefix: &str,
        files: &mut Vec<FileEntry>,
    ) -> Result<(), JsValue> {
        let entries_method: js_sys::Function = Reflect::get(dir_handle, &"entries".into())?.dyn_into()?;
        let entries_async_iter = entries_method.call0(dir_handle)?;

        // Iterate directory entries
        loop {
            let next_method: js_sys::Function = Reflect::get(&entries_async_iter, &"next".into())?.dyn_into()?;
            let next_promise: Promise = next_method.call0(&entries_async_iter)?.dyn_into()?;
            let next_result = wasm_bindgen_futures::JsFuture::from(next_promise).await?;

            if Reflect::get(&next_result, &"done".into())?.as_bool().unwrap_or(true) {
                break;
            }

            let entry = Reflect::get(&next_result, &"value".into())?;
            let entry_array = js_sys::Array::from(&entry);
            if entry_array.length() < 2 {
                continue;
            }

            let name = entry_array.get(0).as_string().unwrap_or_default();
            let handle = entry_array.get(1);
            let kind = Reflect::get(&handle, &"kind".into())?.as_string().unwrap_or_default();

            let full_path = if path_prefix.is_empty() {
                name.clone()
            } else {
                format!("{}/{}", path_prefix, name)
            };

            if kind == "directory" {
                files.push(FileEntry {
                    path: full_path.clone(),
                    name: name.clone(),
                    is_directory: true,
                    content: None,
                    size: None,
                });

                let sub_dir = Object::from(handle);
                self.read_directory_recursive(&sub_dir, &full_path, files).await?;
            } else {
                // Read file content
                let get_file_method: js_sys::Function = Reflect::get(&handle, &"getFile".into())?.dyn_into()?;
                let file_promise: Promise = get_file_method.call0(&handle)?.dyn_into()?;
                let file = wasm_bindgen_futures::JsFuture::from(file_promise).await?;
                let file_obj: File = file.dyn_into()?;

                let content = self.read_file_text(&file_obj).await?;

                files.push(FileEntry {
                    path: full_path,
                    name,
                    is_directory: false,
                    content: Some(content),
                    size: Some(file_obj.size() as u64),
                });
            }
        }

        Ok(())
    }

    async fn read_file_text(&self, file: &File) -> Result<String, JsValue> {
        let promise = Promise::new(&mut |resolve, reject| {
            let reader = FileReader::new().unwrap();
            let reader_clone = reader.clone();

            let onload = Closure::once_into_js(move |event: web_sys::ProgressEvent| {
                let target = event.target().unwrap();
                let reader: FileReader = target.dyn_into().unwrap();
                let result = reader.result().unwrap_or(JsValue::NULL);
                resolve.call1(&JsValue::NULL, &result).unwrap_or(JsValue::NULL);
            });

            let onerror = Closure::once_into_js(move |_event: web_sys::ProgressEvent| {
                reject.call0(&JsValue::NULL).unwrap_or(JsValue::NULL);
            });

            reader.set_onload(Some(onload.as_ref().dyn_ref().unwrap()));
            reader.set_onerror(Some(onerror.as_ref().dyn_ref().unwrap()));
            reader.read_as_text(file).unwrap();
        });

        let result = wasm_bindgen_futures::JsFuture::from(promise).await?;
        Ok(result.as_string().unwrap_or_default())
    }

    // ========================================================================
    // Analysis Pipeline
    // ========================================================================

    /// Run full analysis on imported repository
    pub async fn analyze(&self, progress_callback: JsValue) -> Result<JsResult, JsValue> {
        let callback: js_sys::Function = progress_callback.dyn_into()?;

        let engine = self.engine.read();
        let repo = engine.current_repo.as_ref()
            .ok_or_else(|| JsValue::from_str("No repository imported"))?;

        let total_files = repo.files.len();
        info!("Analyzing {} files...", total_files);

        // Phase 1: Parse files
        self.report_progress(&callback, "parsing", 0, "Parsing source files...", None)?;

        let mut parsed_files = Vec::new();
        for (idx, file) in repo.files.iter().enumerate() {
            if file.is_directory {
                continue;
            }

            if let Some(content) = &file.content {
                match engine.parser.parse(&file.path, content) {
                    Ok(parsed) => {
                        let parsed: ParsedFile = serde_wasm_bindgen::from_value(parsed)
                            .map_err(|e| JsValue::from_str(&e.to_string()))?;
                        parsed_files.push(parsed);
                    }
                    Err(e) => {
                        warn!("Failed to parse {}: {:?}", file.path, e);
                    }
                }
            }

            let percent = ((idx + 1) as f32 / total_files as f32 * 30.0) as u8;
            self.report_progress(&callback, "parsing", percent, 
                &format!("Parsed {}/{}", idx + 1, total_files), 
                Some(PipelineStats {
                    files_processed: Some((idx + 1) as u32),
                    total_files: Some(total_files as u32),
                    nodes_created: None,
                    edges_created: None,
                }))?;
        }

        // Phase 2: Build graph
        self.report_progress(&callback, "building_graph", 30, "Building knowledge graph...", None)?;

        drop(engine); // Release read lock

        {
            let mut engine = self.engine.write();
            let graph = engine.graph.as_mut()
                .ok_or_else(|| JsValue::from_str("Graph not initialized"))?;

            let builder = GraphBuilder::new().await?;
            let result = builder.build_from_parsed(
                serde_wasm_bindgen::to_value(&parsed_files).unwrap()
            ).await?;

            let stats: IndexStats = serde_json::from_str(
                &result.data.unwrap_or_default()
            ).map_err(|e| JsValue::from_str(&e.to_string()))?;

            self.report_progress(&callback, "building_graph", 60, 
                &format!("Graph built: {} nodes, {} edges", stats.nodes, stats.edges),
                Some(PipelineStats {
                    files_processed: Some(total_files as u32),
                    total_files: Some(total_files as u32),
                    nodes_created: Some(stats.nodes),
                    edges_created: Some(stats.edges),
                }))?;
        }

        // Phase 3: Detect communities and processes (simplified)
        self.report_progress(&callback, "communities", 60, "Detecting communities...", None)?;
        // In full implementation, run Louvain/Leiden community detection

        self.report_progress(&callback, "processes", 80, "Extracting execution flows...", None)?;
        // In full implementation, run process extraction

        // Phase 4: Generate embeddings (optional, async)
        self.report_progress(&callback, "embeddings", 90, "Optional: Generate embeddings...", None)?;
        // Lazy - skip unless explicitly requested

        self.report_progress(&callback, "complete", 100, "Analysis complete!", Some(PipelineStats {
            files_processed: Some(total_files as u32),
            total_files: Some(total_files as u32),
            nodes_created: None,
            edges_created: None,
        }))?;

        Ok(JsResult::ok(&serde_json::json!({
            "status": "complete",
            "filesProcessed": total_files,
        })))
    }

    fn report_progress(
        &self,
        callback: &js_sys::Function,
        phase: &str,
        percent: u8,
        message: &str,
        stats: Option<PipelineStats>,
    ) -> Result<(), JsValue> {
        let progress = PipelineProgress {
            phase: phase.to_string(),
            percent,
            message: message.to_string(),
            stats,
        };

        let js_value = serde_wasm_bindgen::to_value(&progress)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        callback.call1(&JsValue::NULL, &js_value)?;
        Ok(())
    }

    // ========================================================================
    // Query Tools
    // ========================================================================

    /// Search the knowledge graph
    pub async fn search(&self, query: JsValue) -> Result<JsResult, JsValue> {
        let search_query: SearchQuery = serde_wasm_bindgen::from_value(query)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        let engine = self.engine.read();
        let graph = engine.graph.as_ref()
            .ok_or_else(|| JsValue::from_str("Graph not initialized"))?;

        // Simple keyword search via BM25 (simplified - full impl would use FTS)
        let cypher = format!(
            "MATCH (n) WHERE n.name CONTAINS '{}' OR n.content CONTAINS '{}' RETURN n.id AS nodeId, n.name AS name, labels(n)[0] AS type, n.filePath AS filePath, n.startLine AS startLine, n.endLine AS endLine LIMIT {}",
            search_query.query.replace("'", "''"),
            search_query.query.replace("'", "''"),
            search_query.limit.unwrap_or(10)
        );

        let results = graph.query(&cypher).await?;

        let search_results: Vec<SearchResult> = results.into_iter()
            .map(|row| SearchResult {
                node_id: row.get("nodeId").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                name: row.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                node_type: row.get("type").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                file_path: row.get("filePath").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                start_line: row.get("startLine").and_then(|v| v.as_u64()).map(|v| v as u32),
                end_line: row.get("endLine").and_then(|v| v.as_u64()).map(|v| v as u32),
                score: 1.0,
                distance: None,
                sources: Some(vec!["keyword".to_string()]),
                connections: None,
                cluster: None,
                processes: None,
            })
            .collect();

        Ok(JsResult::ok(&search_results))
    }

    /// Get context for a symbol
    pub async fn context(&self, name: String, uid: Option<String>) -> Result<JsResult, JsValue> {
        let engine = self.engine.read();
        let graph = engine.graph.as_ref()
            .ok_or_else(|| JsValue::from_str("Graph not initialized"))?;

        let id_filter = uid.unwrap_or_else(|| format!("%{}%", name));

        let cypher = format!(
            "MATCH (n) WHERE n.id = '{}' OR n.name CONTAINS '{}' RETURN n",
            id_filter.replace("'", "''"),
            name.replace("'", "''")
        );

        let results = graph.query(&cypher).await?;

        if results.is_empty() {
            return Ok(JsResult::err("Symbol not found"));
        }

        // Get incoming/outgoing relationships
        let sym_id = results[0].get("n")
            .and_then(|v| v.get("id"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let incoming_cypher = format!(
            "MATCH (caller)-[r:CodeRelation]->(n) WHERE n.id = '{}' RETURN caller.name AS name, r.type AS type, caller.filePath AS filePath LIMIT 30",
            sym_id.replace("'", "''")
        );
        let incoming = graph.query(&incoming_cypher).await?;

        let outgoing_cypher = format!(
            "MATCH (n)-[r:CodeRelation]->(target) WHERE n.id = '{}' RETURN target.name AS name, r.type AS type, target.filePath AS filePath LIMIT 30",
            sym_id.replace("'", "''")
        );
        let outgoing = graph.query(&outgoing_cypher).await?;

        let context = serde_json::json!({
            "symbol": results[0].get("n"),
            "incoming": incoming,
            "outgoing": outgoing,
        });

        Ok(JsResult::ok(&context))
    }

    /// Impact analysis
    pub async fn impact(&self, target: String, direction: String, max_depth: Option<u32>) -> Result<JsResult, JsValue> {
        let engine = self.engine.read();
        let graph = engine.graph.as_ref()
            .ok_or_else(|| JsValue::from_str("Graph not initialized"))?;

        let dir = match direction.as_str() {
            "upstream" => "upstream",
            "downstream" => "downstream",
            _ => "upstream",
        };

        let depth = max_depth.unwrap_or(3);

        // BFS traversal
        let mut impacted = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut frontier = vec![target.clone()];

        for d in 1..=depth {
            let mut next_frontier = Vec::new();

            for node_id in &frontier {
                let query = if dir == "upstream" {
                    format!(
                        "MATCH (caller)-[r:CodeRelation]->(n) WHERE n.id = '{}' RETURN caller.id AS id, caller.name AS name, labels(caller)[0] AS type, caller.filePath AS filePath, r.type AS relType, r.confidence AS confidence",
                        node_id.replace("'", "''")
                    )
                } else {
                    format!(
                        "MATCH (n)-[r:CodeRelation]->(callee) WHERE n.id = '{}' RETURN callee.id AS id, callee.name AS name, labels(callee)[0] AS type, callee.filePath AS filePath, r.type AS relType, r.confidence AS confidence",
                        node_id.replace("'", "''")
                    )
                };

                let rows = graph.query(&query).await?;
                for row in rows {
                    let id = row.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    if !visited.contains(&id) {
                        visited.insert(id.clone());
                        next_frontier.push(id.clone());
                        impacted.push(serde_json::json!({
                            "depth": d,
                            "id": id,
                            "name": row.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                            "type": row.get("type").and_then(|v| v.as_str()).unwrap_or(""),
                            "filePath": row.get("filePath").and_then(|v| v.as_str()).unwrap_or(""),
                            "relationType": row.get("relType").and_then(|v| v.as_str()).unwrap_or(""),
                            "confidence": row.get("confidence").and_then(|v| v.as_f64()).unwrap_or(0.5),
                        }));
                    }
                }
            }

            frontier = next_frontier;
            if frontier.is_empty() {
                break;
            }
        }

        let result = serde_json::json!({
            "target": target,
            "direction": dir,
            "impactedCount": impacted.len(),
            "risk": if impacted.len() > 100 { "CRITICAL" } else if impacted.len() > 30 { "HIGH" } else if impacted.len() > 5 { "MEDIUM" } else { "LOW" },
            "impacted": impacted,
        });

        Ok(JsResult::ok(&result))
    }

    // ========================================================================
    // Graph Export / Import
    // ========================================================================

    /// Export graph as JSON
    pub async fn export_graph(&self) -> Result<String, JsValue> {
        let engine = self.engine.read();
        let graph = engine.graph.as_ref()
            .ok_or_else(|| JsValue::from_str("Graph not initialized"))?;

        graph.export().await
    }

    /// Get graph statistics
    pub async fn graph_stats(&self) -> Result<JsResult, JsValue> {
        let engine = self.engine.read();
        let graph = engine.graph.as_ref()
            .ok_or_else(|| JsValue::from_str("Graph not initialized"))?;

        let node_count_cypher = "MATCH (n) RETURN count(n) AS count";
        let rel_count_cypher = "MATCH ()-[r]->() RETURN count(r) AS count";

        let nodes = graph.query(node_count_cypher).await?;
        let rels = graph.query(rel_count_cypher).await?;

        let node_count = nodes.get(0)
            .and_then(|r| r.get("count"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;

        let rel_count = rels.get(0)
            .and_then(|r| r.get("count"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;

        Ok(JsResult::ok(&serde_json::json!({
            "nodes": node_count,
            "relationships": rel_count,
        })))
    }

    // ========================================================================
    // Embeddings
    // ========================================================================

    /// Initialize embedding model
    pub async fn init_embeddings(&self, model_url: Option<String>) -> Result<JsResult, JsValue> {
        let mut engine = self.engine.write();

        if let Some(embedder) = engine.embedder.as_mut() {
            embedder.init(model_url).await?;
            Ok(JsResult::ok(&serde_json::json!({"status": "ready"})))
        } else {
            Ok(JsResult::err("Embedder not available"))
        }
    }

    /// Check if embeddings are ready
    pub fn embeddings_ready(&self) -> bool {
        let engine = self.engine.read();
        engine.embedder.as_ref().map(|e| e.is_ready()).unwrap_or(false)
    }

    // ========================================================================
    // Git Operations
    // ========================================================================

    /// Detect changes in git repository
    pub async fn detect_changes(&self, scope: Option<String>) -> Result<JsResult, JsValue> {
        let engine = self.engine.read();
        let repo = engine.current_repo.as_ref()
            .ok_or_else(|| JsValue::from_str("No repository imported"))?;

        if !repo.is_git_repo {
            return Ok(JsResult::err("Not a git repository"));
        }

        // Simplified - in full implementation, use isomorphic-git
        Ok(JsResult::ok(&serde_json::json!({
            "changedFiles": [],
            "message": "Git operations require isomorphic-git initialization",
        })))
    }
}

// ============================================================================
// Utility Functions
// ============================================================================

#[wasm_bindgen]
pub fn supported_languages() -> JsValue {
    let langs = ParserRegistry::supported_languages();
    serde_wasm_bindgen::to_value(&langs).unwrap_or(JsValue::NULL)
}

#[wasm_bindgen]
pub fn detect_language(file_path: &str) -> Option<String> {
    ParserRegistry::detect_language(file_path)
}
