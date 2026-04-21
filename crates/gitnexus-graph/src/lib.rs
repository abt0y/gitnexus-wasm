//! KuzuDB WASM bindings for GitNexus
//!
//! Provides graph database operations in the browser using KuzuDB's WASM build.
//! Supports both in-memory and persistent (IndexedDB) storage.

use wasm_bindgen::prelude::*;
use js_sys::{Function, Promise, Reflect, Array, Object};
use web_sys::console;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;
use log::{info, warn, error};

use gitnexus_shared::*;

// ============================================================================
// KuzuDB WASM Bridge
// ============================================================================

/// Wrapper around KuzuDB WASM instance
#[wasm_bindgen]
pub struct GraphDatabase {
    #[wasm_bindgen(skip)]
    db: JsValue,           // Kuzu Database instance
    #[wasm_bindgen(skip)]
    conn: JsValue,         // Kuzu Connection instance
    #[wasm_bindgen(skip)]
    in_memory: bool,
    #[wasm_bindgen(skip)]
    schema_initialized: bool,
}

#[wasm_bindgen]
impl GraphDatabase {
    #[wasm_bindgen(constructor)]
    pub async fn new(db_path: Option<String>) -> Result<GraphDatabase, JsValue> {
        console_error_panic_hook::set_once();

        let window = web_sys::window().ok_or("No window")?;
        let kuzu = Reflect::get(&window, &"kuzu".into())?;

        if kuzu.is_undefined() {
            return Err(JsValue::from_str("KuzuDB WASM not loaded. Include kuzu-wasm.js before this module."));
        }

        let db_class: js_sys::Function = Reflect::get(&kuzu, &"Database".into())?.dyn_into()?;
        let conn_class: js_sys::Function = Reflect::get(&kuzu, &"Connection".into())?.dyn_into()?;

        // Create database (in-memory if no path, or persistent via OPFS/IndexedDB)
        let db_instance = if let Some(path) = db_path {
            info!("Opening persistent KuzuDB at: {}", path);
            let path_js = JsValue::from_str(&path);
            db_class.new1(&path_js)?
        } else {
            info!("Creating in-memory KuzuDB");
            db_class.new0()?
        };

        let conn_instance = conn_class.new1(&db_instance)?;

        let mut graph = GraphDatabase {
            db: db_instance,
            conn: conn_instance,
            in_memory: db_path.is_none(),
            schema_initialized: false,
        };

        graph.init_schema().await?;

        Ok(graph)
    }

    /// Initialize the GitNexus schema
    async fn init_schema(&mut self) -> Result<(), JsValue> {
        if self.schema_initialized {
            return Ok(());
        }

        let schema_statements = vec![
            // Node tables
            "CREATE NODE TABLE File(id STRING PRIMARY KEY, name STRING, filePath STRING, content STRING)",
            "CREATE NODE TABLE Folder(id STRING PRIMARY KEY, name STRING, filePath STRING)",
            "CREATE NODE TABLE Function(id STRING PRIMARY KEY, name STRING, filePath STRING, startLine INT32, endLine INT32, content STRING, isExported BOOLEAN, parameterCount INT32, returnType STRING)",
            "CREATE NODE TABLE Class(id STRING PRIMARY KEY, name STRING, filePath STRING, startLine INT32, endLine INT32, content STRING, isExported BOOLEAN)",
            "CREATE NODE TABLE Interface(id STRING PRIMARY KEY, name STRING, filePath STRING, startLine INT32, endLine INT32, content STRING, isExported BOOLEAN)",
            "CREATE NODE TABLE Method(id STRING PRIMARY KEY, name STRING, filePath STRING, startLine INT32, endLine INT32, content STRING, isExported BOOLEAN, parameterCount INT32, returnType STRING)",
            "CREATE NODE TABLE Struct(id STRING PRIMARY KEY, name STRING, filePath STRING, startLine INT32, endLine INT32, content STRING)",
            "CREATE NODE TABLE Enum(id STRING PRIMARY KEY, name STRING, filePath STRING, startLine INT32, endLine INT32, content STRING)",
            "CREATE NODE TABLE Trait(id STRING PRIMARY KEY, name STRING, filePath STRING, startLine INT32, endLine INT32, content STRING)",
            "CREATE NODE TABLE Module(id STRING PRIMARY KEY, name STRING, filePath STRING, startLine INT32, endLine INT32, content STRING)",
            "CREATE NODE TABLE Namespace(id STRING PRIMARY KEY, name STRING, filePath STRING, startLine INT32, endLine INT32, content STRING)",
            "CREATE NODE TABLE Community(id STRING PRIMARY KEY, label STRING, heuristicLabel STRING, cohesion DOUBLE, symbolCount INT32)",
            "CREATE NODE TABLE Process(id STRING PRIMARY KEY, label STRING, heuristicLabel STRING, processType STRING, stepCount INT32, communities STRING[], entryPointId STRING, terminalId STRING)",
            "CREATE NODE TABLE Route(id STRING PRIMARY KEY, name STRING, filePath STRING, responseKeys STRING[], errorKeys STRING[], middleware STRING[])",
            "CREATE NODE TABLE Tool(id STRING PRIMARY KEY, name STRING, filePath STRING, description STRING)",
            "CREATE NODE TABLE CodeElement(id STRING PRIMARY KEY, name STRING, filePath STRING, startLine INT32, endLine INT32, content STRING)",
            // Embedding table
            "CREATE NODE TABLE CodeEmbedding(id STRING PRIMARY KEY, nodeId STRING, chunkIndex INT32, startLine INT32, endLine INT32, embedding DOUBLE[], contentHash STRING)",
            // Relationship table
            "CREATE REL TABLE CodeRelation(FROM CodeElement TO CodeElement, MANY_MANY)",
            "CREATE REL TABLE FileRelation(FROM File TO File, MANY_MANY)",
            "CREATE REL TABLE MemberOf(FROM CodeElement TO Community, MANY_MANY)",
            "CREATE REL TABLE StepInProcess(FROM CodeElement TO Process, MANY_MANY)",
            "CREATE REL TABLE EntryPointOf(FROM CodeElement TO Process, MANY_MANY)",
            "CREATE REL TABLE Defines(FROM File TO CodeElement, ONE_MANY)",
            "CREATE REL TABLE Contains(FROM Folder TO File, ONE_MANY)",
        ];

        for stmt in schema_statements {
            match self.execute(stmt).await {
                Ok(_) => {},
                Err(e) => {
                    let msg = e.as_string().unwrap_or_default();
                    // Ignore "already exists" errors
                    if !msg.contains("already exists") {
                        warn!("Schema init warning: {}", msg);
                    }
                }
            }
        }

        self.schema_initialized = true;
        info!("Schema initialized successfully");
        Ok(())
    }

    /// Execute a Cypher query
    pub async fn execute(&self, query: &str) -> Result<JsValue, JsValue> {
        let query_method: js_sys::Function = Reflect::get(&self.conn, &"query".into())?.dyn_into()?;
        let query_promise: Promise = query_method.call1(&self.conn, &JsValue::from_str(query))?.dyn_into()?;

        let result = wasm_bindgen_futures::JsFuture::from(query_promise).await?;
        Ok(result)
    }

    /// Execute query and return structured results
    pub async fn query(&self, cypher: &str) -> Result<Vec<HashMap<String, serde_json::Value>>, JsValue> {
        let result = self.execute(cypher).await?;

        // Convert Kuzu result to Vec<HashMap>
        let rows_method: js_sys::Function = Reflect::get(&result, &"getAllRows".into())?.dyn_into()?;
        let rows_promise: Promise = rows_method.call0(&result)?.dyn_into()?;
        let rows = wasm_bindgen_futures::JsFuture::from(rows_promise).await?;

        let rows_array = js_sys::Array::from(&rows);
        let mut results = Vec::new();

        for i in 0..rows_array.length() {
            let row = rows_array.get(i);
            let row_obj = Object::from(row);
            let mut map = HashMap::new();

            let keys = Object::keys(&row_obj);
            for j in 0..keys.length() {
                let key = keys.get(j).as_string().unwrap_or_default();
                let val = Reflect::get(&row_obj, &key.into())?;

                // Convert JS value to serde_json::Value
                let json_val = if val.is_string() {
                    serde_json::Value::String(val.as_string().unwrap_or_default())
                } else if val.is_number() {
                    serde_json::Value::Number(
                        serde_json::Number::from_f64(val.as_f64().unwrap_or(0.0))
                            .unwrap_or(serde_json::Number::from(0))
                    )
                } else if val.is_boolean() {
                    serde_json::Value::Bool(val.as_bool().unwrap_or(false))
                } else if val.is_null() || val.is_undefined() {
                    serde_json::Value::Null
                } else {
                    // Try to convert via JSON.stringify
                    let json_str = js_sys::JSON::stringify(&val)
                        .map_err(|_| JsValue::from_str("Failed to stringify"))?
                        .as_string()
                        .unwrap_or("null".to_string());
                    serde_json::from_str(&json_str).unwrap_or(serde_json::Value::Null)
                };

                map.insert(key, json_val);
            }
            results.push(map);
        }

        Ok(results)
    }

    /// Create a node in the graph
    pub async fn create_node(&self, label: &str, properties: JsValue) -> Result<(), JsValue> {
        let props: HashMap<String, serde_json::Value> = serde_wasm_bindgen::from_value(properties)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        let mut set_clauses = Vec::new();
        for (key, val) in &props {
            let formatted = match val {
                serde_json::Value::String(s) => format!("'{}'", s.replace("'", "''")),
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::Bool(b) => b.to_string(),
                serde_json::Value::Null => "NULL".to_string(),
                _ => format!("'{}'", val.to_string().replace("'", "''")),
            };
            set_clauses.push(format!("n.{} = {}", key, formatted));
        }

        let id = props.get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| JsValue::from_str("Node must have 'id' property"))?;

        let query = format!(
            "CREATE (n:{} {{id: '{}'}}) SET {}",
            label,
            id.replace("'", "''"),
            set_clauses.join(", ")
        );

        self.execute(&query).await?;
        Ok(())
    }

    /// Create a relationship between nodes
    pub async fn create_relationship(
        &self,
        from_id: &str,
        to_id: &str,
        rel_type: &str,
        properties: Option<JsValue>,
    ) -> Result<(), JsValue> {
        let mut query = format!(
            "MATCH (a), (b) WHERE a.id = '{}' AND b.id = '{}' CREATE (a)-[r:{}]->(b)",
            from_id.replace("'", "''"),
            to_id.replace("'", "''"),
            rel_type
        );

        if let Some(props) = properties {
            let props_map: HashMap<String, serde_json::Value> = serde_wasm_bindgen::from_value(props)
                .map_err(|e| JsValue::from_str(&e.to_string()))?;

            let mut set_clauses = Vec::new();
            for (key, val) in &props_map {
                let formatted = match val {
                    serde_json::Value::String(s) => format!("'{}'", s.replace("'", "''")),
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::Bool(b) => b.to_string(),
                    _ => format!("'{}'", val.to_string().replace("'", "''")),
                };
                set_clauses.push(format!("r.{} = {}", key, formatted));
            }

            if !set_clauses.is_empty() {
                query.push_str(&format!(" SET {}", set_clauses.join(", ")));
            }
        }

        self.execute(&query).await?;
        Ok(())
    }

    /// Batch insert nodes (optimized)
    pub async fn batch_create_nodes(&self, label: &str, nodes: JsValue) -> Result<u32, JsValue> {
        let nodes_vec: Vec<HashMap<String, serde_json::Value>> = serde_wasm_bindgen::from_value(nodes)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        let mut count = 0u32;
        // Process in batches of 100 to avoid memory issues
        for chunk in nodes_vec.chunks(100) {
            let mut creates = Vec::new();
            for node in chunk {
                let id = node.get("id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| JsValue::from_str("Node missing 'id'"))?;

                let mut props = Vec::new();
                for (key, val) in node {
                    let formatted = match val {
                        serde_json::Value::String(s) => format!("'{}'", s.replace("'", "''")),
                        serde_json::Value::Number(n) => n.to_string(),
                        serde_json::Value::Bool(b) => b.to_string(),
                        serde_json::Value::Null => continue,
                        _ => format!("'{}'", val.to_string().replace("'", "''")),
                    };
                    props.push(format!("{}: {}", key, formatted));
                }

                creates.push(format!("(:{} {{ {} }})", label, props.join(", ")));
            }

            let query = format!("CREATE {}", creates.join(", "));
            self.execute(&query).await?;
            count += chunk.len() as u32;
        }

        Ok(count)
    }

    /// Get all nodes of a specific label
    pub async fn get_nodes(&self, label: &str) -> Result<JsValue, JsValue> {
        let query = format!("MATCH (n:{}) RETURN n.id AS id, n.name AS name, n.filePath AS filePath, n.startLine AS startLine, n.endLine AS endLine", label);
        let results = self.query(&query).await?;
        serde_wasm_bindgen::to_value(&results)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get full graph (nodes + relationships)
    pub async fn get_full_graph(&self) -> Result<JsValue, JsValue> {
        let node_labels = vec!["File", "Folder", "Function", "Class", "Interface", "Method", 
                               "Struct", "Enum", "Trait", "Module", "Namespace", "Community", 
                               "Process", "Route", "Tool", "CodeElement"];

        let mut nodes = Vec::new();
        for label in &node_labels {
            let query = format!("MATCH (n:{}) RETURN n", label);
            match self.query(&query).await {
                Ok(rows) => {
                    for row in rows {
                        if let Some(node_data) = row.get("n") {
                            if let Ok(node) = serde_json::from_value::<GraphNode>(node_data.clone()) {
                                nodes.push(node);
                            }
                        }
                    }
                }
                Err(_) => continue, // Table might not exist yet
            }
        }

        let mut relationships = Vec::new();
        let rel_query = "MATCH (a)-[r:CodeRelation]->(b) RETURN a.id AS sourceId, b.id AS targetId, r.type AS type, r.confidence AS confidence";
        match self.query(rel_query).await {
            Ok(rows) => {
                for row in rows {
                    relationships.push(GraphRelationship {
                        id: format!("{}_{}_{}", 
                            row.get("sourceId").and_then(|v| v.as_str()).unwrap_or(""),
                            row.get("type").and_then(|v| v.as_str()).unwrap_or(""),
                            row.get("targetId").and_then(|v| v.as_str()).unwrap_or("")
                        ),
                        source_id: row.get("sourceId").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        target_id: row.get("targetId").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        rel_type: RelationType::Calls, // Simplified - would map from string
                        confidence: row.get("confidence").and_then(|v| v.as_f64()),
                        reason: None,
                        step: None,
                    });
                }
            }
            Err(_) => {}
        }

        let graph = serde_json::json!({
            "nodes": nodes,
            "relationships": relationships,
        });

        serde_wasm_bindgen::to_value(&graph)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Create vector index for semantic search
    pub async fn create_vector_index(&self) -> Result<(), JsValue> {
        let query = format!(
            "CALL CREATE_VECTOR_INDEX('CodeEmbedding', 'embedding_idx', 'embedding', {}, false)",
            384 // Dimension for all-MiniLM-L6-v2
        );
        match self.execute(&query).await {
            Ok(_) => Ok(()),
            Err(e) => {
                let msg = e.as_string().unwrap_or_default();
                if msg.contains("already exists") {
                    Ok(())
                } else {
                    Err(e)
                }
            }
        }
    }

    /// Semantic search using vector index
    pub async fn vector_search(&self, embedding: Vec<f32>, k: u32) -> Result<JsValue, JsValue> {
        let embedding_str = embedding.iter()
            .map(|f| f.to_string())
            .collect::<Vec<_>>()
            .join(", ");

        let query = format!(
            "CALL QUERY_VECTOR_INDEX('CodeEmbedding', 'embedding_idx', CAST([{}] AS FLOAT[{}]), {}) YIELD node AS emb, distance RETURN emb.nodeId AS nodeId, emb.chunkIndex AS chunkIndex, emb.startLine AS startLine, emb.endLine AS endLine, distance ORDER BY distance",
            embedding_str, embedding.len(), k
        );

        let results = self.query(&query).await?;
        serde_wasm_bindgen::to_value(&results)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Export database to JSON
    pub async fn export(&self) -> Result<String, JsValue> {
        let graph = self.get_full_graph().await?;
        let graph_obj: serde_json::Value = serde_wasm_bindgen::from_value(graph)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        Ok(serde_json::to_string_pretty(&graph_obj)
            .unwrap_or_default())
    }

    /// Close database connection
    pub fn close(&self) -> Result<(), JsValue> {
        let close_method: js_sys::Function = Reflect::get(&self.conn, &"close".into())?.dyn_into()?;
        close_method.call0(&self.conn)?;

        let close_db: js_sys::Function = Reflect::get(&self.db, &"close".into())?.dyn_into()?;
        close_db.call0(&self.db)?;

        info!("Database closed");
        Ok(())
    }
}

// ============================================================================
// Graph Builder - High-level API for constructing knowledge graphs
// ============================================================================

#[wasm_bindgen]
pub struct GraphBuilder {
    db: GraphDatabase,
}

#[wasm_bindgen]
impl GraphBuilder {
    #[wasm_bindgen(constructor)]
    pub async fn new() -> Result<GraphBuilder, JsValue> {
        let db = GraphDatabase::new(None).await?;
        Ok(Self { db })
    }

    /// Build graph from parsed files
    pub async fn build_from_parsed(&self, parsed_files: JsValue) -> Result<JsResult, JsValue> {
        let files: Vec<ParsedFile> = serde_wasm_bindgen::from_value(parsed_files)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        let mut total_nodes = 0u32;
        let mut total_rels = 0u32;

        for file in &files {
            // Create File node
            let file_id = format!("File:{}", file.file_path);
            let file_props = serde_json::json!({
                "id": file_id,
                "name": file.file_path.split('/').last().unwrap_or(""),
                "filePath": file.file_path,
            });

            match self.db.create_node("File", serde_wasm_bindgen::to_value(&file_props).unwrap()).await {
                Ok(_) => total_nodes += 1,
                Err(_) => {}, // May already exist
            }

            // Create symbol nodes
            for symbol in &file.symbols {
                let label = match symbol.kind {
                    SymbolKind::Function => "Function",
                    SymbolKind::Class => "Class",
                    SymbolKind::Interface => "Interface",
                    SymbolKind::Method => "Method",
                    SymbolKind::Struct => "Struct",
                    SymbolKind::Enum => "Enum",
                    SymbolKind::Trait => "Trait",
                    SymbolKind::Module => "Module",
                    SymbolKind::Namespace => "Namespace",
                    SymbolKind::Property => "CodeElement",
                    SymbolKind::Const => "CodeElement",
                    SymbolKind::Static => "CodeElement",
                };

                let mut props = serde_json::json!({
                    "id": symbol.id,
                    "name": symbol.name,
                    "filePath": symbol.file_path,
                    "startLine": symbol.start_line,
                    "endLine": symbol.end_line,
                });

                if let Some(content) = &symbol.content {
                    props["content"] = serde_json::Value::String(content.clone());
                }
                if let Some(is_exported) = symbol.is_exported {
                    props["isExported"] = serde_json::Value::Bool(is_exported);
                }
                if let Some(param_count) = symbol.parameter_count {
                    props["parameterCount"] = serde_json::Value::Number(param_count.into());
                }
                if let Some(return_type) = &symbol.return_type {
                    props["returnType"] = serde_json::Value::String(return_type.clone());
                }

                match self.db.create_node(label, serde_wasm_bindgen::to_value(&props).unwrap()).await {
                    Ok(_) => total_nodes += 1,
                    Err(_) => {},
                }

                // Create DEFINES relationship from File to symbol
                let rel_props = serde_json::json!({
                    "type": "DEFINES",
                    "confidence": 1.0,
                });
                match self.db.create_relationship(&file_id, &symbol.id, "Defines", 
                    Some(serde_wasm_bindgen::to_value(&rel_props).unwrap())).await {
                    Ok(_) => total_rels += 1,
                    Err(_) => {},
                }
            }

            // Create import relationships
            for import in &file.imports {
                // Simplified: create IMPORTS relationship to a module node
                let import_id = format!("Module:{}", import.source.replace("'", "").replace(""", ""));
                let import_props = serde_json::json!({
                    "id": import_id,
                    "name": import.source,
                    "filePath": "",
                    "startLine": import.line,
                    "endLine": import.line,
                });

                match self.db.create_node("Module", serde_wasm_bindgen::to_value(&import_props).unwrap()).await {
                    Ok(_) => total_nodes += 1,
                    Err(_) => {},
                }

                let rel_props = serde_json::json!({
                    "type": "IMPORTS",
                    "confidence": 0.9,
                });
                match self.db.create_relationship(&file_id, &import_id, "CodeRelation",
                    Some(serde_wasm_bindgen::to_value(&rel_props).unwrap())).await {
                    Ok(_) => total_rels += 1,
                    Err(_) => {},
                }
            }
        }

        // Build cross-file references (calls)
        self.resolve_calls(&files).await?;

        let stats = IndexStats {
            files: files.len() as u32,
            nodes: total_nodes,
            edges: total_rels,
            ..Default::default()
        };

        Ok(JsResult::ok(&stats))
    }

    async fn resolve_calls(&self, files: &[ParsedFile]) -> Result<(), JsValue> {
        // Build symbol index for quick lookup
        let mut symbol_index: HashMap<String, String> = HashMap::new();
        for file in files {
            for symbol in &file.symbols {
                symbol_index.insert(symbol.name.clone(), symbol.id.clone());
            }
        }

        // Resolve call sites
        for file in files {
            for call in &file.calls {
                if let Some(target_id) = symbol_index.get(&call.target) {
                    let caller_id = format!("File:{}", file.file_path);
                    let rel_props = serde_json::json!({
                        "type": "CALLS",
                        "confidence": 0.8,
                    });
                    let _ = self.db.create_relationship(&caller_id, target_id, "CodeRelation",
                        Some(serde_wasm_bindgen::to_value(&rel_props).unwrap())).await;
                }
            }
        }

        Ok(())
    }

    /// Get the underlying database
    pub fn db(&self) -> &GraphDatabase {
        &self.db
    }
}
