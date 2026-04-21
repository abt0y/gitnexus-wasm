//! KuzuDB WASM bindings for GitNexus
//!
//! Provides graph database operations in the browser using KuzuDB's WASM build.
//! Supports both in-memory and persistent (IndexedDB) storage.

pub mod louvain;
pub mod process;
pub mod vector;

use wasm_bindgen::prelude::*;
use js_sys::{Promise, Reflect, Array, Object};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use log::{info, warn};

use gitnexus_shared::*;
use louvain::{LouvainConfig, detect_communities};
use process::{ProcessExtractor, ProcessConfig, build_graph_from_rows};
use vector::{BruteForceIndex, VectorEntry, reciprocal_rank_fusion};

/// Wrapper around KuzuDB WASM instance
#[wasm_bindgen]
pub struct GraphDatabase {
    #[wasm_bindgen(skip)]
    pub db: JsValue,
    #[wasm_bindgen(skip)]
    pub conn: JsValue,
    #[wasm_bindgen(skip)]
    pub in_memory: bool,
    #[wasm_bindgen(skip)]
    pub schema_initialized: bool,
    /// In-memory vector index (Task 5 fallback)
    #[wasm_bindgen(skip)]
    pub vector_index: BruteForceIndex,
}

#[wasm_bindgen]
impl GraphDatabase {
    #[wasm_bindgen]
    pub async fn open(db_path: Option<String>) -> Result<GraphDatabase, JsValue> {
        console_error_panic_hook::set_once();

        let window = web_sys::window().ok_or("No window")?;
        let kuzu = Reflect::get(&window, &"kuzu".into())?;

        if kuzu.is_undefined() {
            return Err(JsValue::from_str(
                "KuzuDB WASM not loaded. Include kuzu-wasm.js before this module.",
            ));
        }

        let db_class: js_sys::Function = Reflect::get(&kuzu, &"Database".into())?.dyn_into()?;
        let conn_class: js_sys::Function = Reflect::get(&kuzu, &"Connection".into())?.dyn_into()?;

        let db_instance = if let Some(path) = db_path.clone() {
            info!("Opening persistent KuzuDB at: {}", path);
            let args = Array::new();
            args.push(&JsValue::from_str(&path));
            Reflect::construct(&db_class, &args)?
        } else {
            info!("Creating in-memory KuzuDB");
            Reflect::construct(&db_class, &Array::new())?
        };

        let conn_args = Array::new();
        conn_args.push(&db_instance);
        let conn_instance = Reflect::construct(&conn_class, &conn_args)?;

        let mut graph = GraphDatabase {
            db: db_instance,
            conn: conn_instance,
            in_memory: db_path.is_none(),
            schema_initialized: false,
            vector_index: BruteForceIndex::new(),
        };

        graph.init_schema().await?;
        Ok(graph)
    }

    pub async fn query(&self, cypher: &str) -> Result<JsValue, JsValue> {
        let rows = self.query_internal(cypher).await?;
        serde_wasm_bindgen::to_value(&rows).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub async fn detect_communities(&self, config_js: JsValue) -> Result<JsResult, JsValue> {
        let config: LouvainConfig = serde_wasm_bindgen::from_value(config_js)
            .unwrap_or_default();

        let q = "MATCH (a)-[r:CodeRelation]->(b) \
                 RETURN a.id AS sourceId, b.id AS targetId, \
                        COALESCE(r.confidence, 0.5) AS confidence";
        let rows = self.query_internal(q).await?;

        let edges: Vec<(String, String, f64)> = rows.iter().filter_map(|r| {
            let s = r.get("sourceId")?.as_str()?.to_owned();
            let t = r.get("targetId")?.as_str()?.to_owned();
            let w = r.get("confidence")?.as_f64().unwrap_or(0.5);
            Some((s, t, w))
        }).collect();

        if edges.is_empty() {
            return Ok(JsResult::err("No edges found — run analysis first".to_string()));
        }

        let result = detect_communities(&edges, &config);

        for stat in &result.community_stats {
            let props = serde_json::json!({
                "id":             format!("Community:{}", stat.id),
                "label":          format!("Community {}", stat.id),
                "heuristicLabel": stat.dominant_label,
                "cohesion":       stat.cohesion,
                "symbolCount":    stat.size as u32,
            });
            let _ = self.create_node("Community",
                serde_wasm_bindgen::to_value(&props).unwrap()).await;
        }

        for (node_id, comm_id) in &result.node_to_community {
            let rp = serde_json::json!({"confidence": 1.0});
            let _ = self.create_relationship(
                node_id,
                &format!("Community:{}", comm_id),
                "MemberOf",
                Some(serde_wasm_bindgen::to_value(&rp).unwrap()),
            ).await;
        }

        Ok(JsResult::ok(serde_json::json!({
            "modularity":  result.modularity,
            "communities": result.community_stats.len(),
            "levels":      result.levels,
        }).to_string()))
    }

    pub async fn extract_processes(&self, config_js: JsValue) -> Result<JsResult, JsValue> {
        let config: ProcessConfig = serde_wasm_bindgen::from_value(config_js)
            .unwrap_or_default();

        let q = "MATCH (a)-[r:CodeRelation]->(b) \
                 RETURN a.id AS sourceId, a.name AS sourceName, labels(a)[0] AS sourceType, \
                        b.id AS targetId, b.name AS targetName, labels(b)[0] AS targetType, \
                        COALESCE(r.type, 'CALLS') AS relType, \
                        COALESCE(r.confidence, 0.5) AS confidence";
        let rows = self.query_internal(q).await?;
        let (call_graph, meta) = build_graph_from_rows(&rows);

        let extractor = ProcessExtractor::new(config);
        let processes = extractor.extract_all(&call_graph, &meta);

        let count = processes.len();
        for proc in &processes {
            let pp = serde_json::json!({
                "id":             proc.id,
                "label":          proc.label,
                "heuristicLabel": proc.heuristic_label,
                "processType":    proc.process_type,
                "stepCount":      proc.step_count,
                "entryPointId":   proc.entry_point_id,
                "terminalId":     proc.terminal_ids.first().cloned().unwrap_or_default(),
            });
            let _ = self.create_node("Process",
                serde_wasm_bindgen::to_value(&pp).unwrap()).await;

            let ep = serde_json::json!({"confidence": 1.0});
            let _ = self.create_relationship(
                &proc.entry_point_id, &proc.id, "EntryPointOf",
                Some(serde_wasm_bindgen::to_value(&ep).unwrap()),
            ).await;

            for (i, step) in proc.steps.iter().enumerate() {
                let sp = serde_json::json!({
                    "step":       (i + 1) as u32,
                    "confidence": step.confidence,
                });
                let _ = self.create_relationship(
                    &step.node_id, &proc.id, "StepInProcess",
                    Some(serde_wasm_bindgen::to_value(&sp).unwrap()),
                ).await;
            }
        }

        Ok(JsResult::ok(serde_json::json!({ "processes": count }).to_string()))
    }

    pub fn index_embedding(&mut self, entry_js: JsValue) -> Result<(), JsValue> {
        let entry: VectorEntry = serde_wasm_bindgen::from_value(entry_js)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        self.vector_index.insert(entry);
        Ok(())
    }

    pub async fn hybrid_search(
        &self,
        keyword:      &str,
        embedding_js: JsValue,
        k:            u32,
    ) -> Result<JsResult, JsValue> {
        let safe = keyword.replace('\'', "''");
        let bm25_q = format!(
            "MATCH (n) WHERE n.name CONTAINS '{0}' OR n.content CONTAINS '{0}' \
             RETURN n.id AS id LIMIT {1}",
            safe, k * 2,
        );
        let bm25_rows = self.query_internal(&bm25_q).await?;
        let bm25_ids: Vec<String> = bm25_rows.iter()
            .filter_map(|r| r.get("id")?.as_str().map(str::to_owned))
            .collect();

        let embedding: Vec<f32> = serde_wasm_bindgen::from_value(embedding_js)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        let sem_results = self.vector_index.search(&embedding, (k * 2) as usize);
        let sem_ids: Vec<String> = sem_results.iter().map(|r| r.node_id.clone()).collect();

        let fused = reciprocal_rank_fusion(&bm25_ids, &sem_ids, k as usize);

        Ok(JsResult::ok(serde_json::to_string(&fused).unwrap()))
    }

    pub async fn get_full_graph(&self) -> Result<JsValue, JsValue> {
        let mut nodes: Vec<serde_json::Value> = Vec::new();
        let labels = [
            "File", "Function", "Class", "Interface", "Method",
            "Struct", "Enum", "Trait", "Module", "Namespace",
            "Community", "Process", "Route", "Tool", "CodeElement",
        ];
        for label in &labels {
            let q = format!("MATCH (n:{}) RETURN n.id AS id, n.name AS name, n.filePath AS filePath, n.startLine AS startLine, n.endLine AS endLine", label);
            if let Ok(rows) = self.query_internal(&q).await {
                for row in rows {
                    nodes.push(serde_json::json!({
                        "id":       row.get("id").cloned(),
                        "label":    label,
                        "properties": {
                            "name":      row.get("name").cloned(),
                            "filePath":  row.get("filePath").cloned(),
                            "startLine": row.get("startLine").cloned(),
                            "endLine":   row.get("endLine").cloned(),
                        }
                    }));
                }
            }
        }

        let rel_q = "MATCH (a)-[r:CodeRelation]->(b) \
                     RETURN a.id AS sourceId, b.id AS targetId, \
                            COALESCE(r.type, 'CALLS') AS type, \
                            COALESCE(r.confidence, 0.5) AS confidence";
        let mut rels: Vec<serde_json::Value> = Vec::new();
        if let Ok(rows) = self.query_internal(rel_q).await {
            for row in rows {
                let s = row.get("sourceId").and_then(|v| v.as_str()).unwrap_or("").to_owned();
                let t = row.get("targetId").and_then(|v| v.as_str()).unwrap_or("").to_owned();
                let ty = row.get("type").and_then(|v| v.as_str()).unwrap_or("CALLS").to_owned();
                rels.push(serde_json::json!({
                    "id":       format!("{}_{}", s, t),
                    "sourceId": s,
                    "targetId": t,
                    "type":     ty,
                    "confidence": row.get("confidence").cloned(),
                }));
            }
        }

        let graph = serde_json::json!({ "nodes": nodes, "relationships": rels });
        serde_wasm_bindgen::to_value(&graph)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub async fn export(&self) -> Result<String, JsValue> {
        let g = self.get_full_graph().await?;
        let v: serde_json::Value = serde_wasm_bindgen::from_value(g)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(serde_json::to_string_pretty(&v).unwrap_or_default())
    }

    pub async fn search(&self, query_js: JsValue) -> Result<JsResult, JsValue> {
        let q: SearchQuery = serde_wasm_bindgen::from_value(query_js)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        
        let safe = q.query.replace('\'', "''");
        let cypher = format!(
            "MATCH (n) WHERE n.name CONTAINS '{0}' OR n.filePath CONTAINS '{0}' \
             RETURN n.id AS nodeId, n.name AS name, labels(n)[0] AS type, n.filePath AS filePath \
             LIMIT {1}",
            safe, q.limit.unwrap_or(10)
        );
        let rows = self.query_internal(&cypher).await?;
        Ok(JsResult::ok(serde_json::to_string(&rows).unwrap()))
    }

    pub async fn get_context(&self, name: String, _uid: Option<String>) -> Result<JsResult, JsValue> {
        let safe = name.replace('\'', "''");
        let cypher = format!(
             "MATCH (n {{name: '{}'}}) RETURN n.id AS id, n.name AS name, labels(n)[0] AS kind, n.filePath AS filePath, n.startLine AS startLine, n.endLine AS endLine, n.content AS content",
             safe
        );
        let rows = self.query_internal(&cypher).await?;
        if rows.is_empty() {
            return Ok(JsResult::err("Symbol not found".to_string()));
        }
        Ok(JsResult::ok(serde_json::to_string(&rows[0]).unwrap()))
    }

    pub fn close(&self) -> Result<(), JsValue> {
        if let Ok(m) = Reflect::get(&self.conn, &"close".into())
            .and_then(|f| f.dyn_into::<js_sys::Function>()) {
            let _ = m.call0(&self.conn);
        }
        if let Ok(m) = Reflect::get(&self.db, &"close".into())
            .and_then(|f| f.dyn_into::<js_sys::Function>()) {
            let _ = m.call0(&self.db);
        }
        Ok(())
    }
}

// Internal methods for GraphDatabase
impl GraphDatabase {
    async fn init_schema(&mut self) -> Result<(), JsValue> {
        if self.schema_initialized { return Ok(()); }

        let stmts = vec![
            "CREATE NODE TABLE IF NOT EXISTS File(id STRING PRIMARY KEY, name STRING, filePath STRING, content STRING, contentHash STRING)",
            "CREATE NODE TABLE IF NOT EXISTS Folder(id STRING PRIMARY KEY, name STRING, filePath STRING)",
            "CREATE NODE TABLE IF NOT EXISTS Function(id STRING PRIMARY KEY, name STRING, filePath STRING, startLine INT32, endLine INT32, content STRING, isExported BOOLEAN, parameterCount INT32, returnType STRING)",
            "CREATE NODE TABLE IF NOT EXISTS Class(id STRING PRIMARY KEY, name STRING, filePath STRING, startLine INT32, endLine INT32, content STRING, isExported BOOLEAN)",
            "CREATE NODE TABLE IF NOT EXISTS Interface(id STRING PRIMARY KEY, name STRING, filePath STRING, startLine INT32, endLine INT32, content STRING, isExported BOOLEAN)",
            "CREATE NODE TABLE IF NOT EXISTS Method(id STRING PRIMARY KEY, name STRING, filePath STRING, startLine INT32, endLine INT32, content STRING, isExported BOOLEAN, parameterCount INT32, returnType STRING)",
            "CREATE NODE TABLE IF NOT EXISTS Struct(id STRING PRIMARY KEY, name STRING, filePath STRING, startLine INT32, endLine INT32, content STRING)",
            "CREATE NODE TABLE IF NOT EXISTS Enum(id STRING PRIMARY KEY, name STRING, filePath STRING, startLine INT32, endLine INT32, content STRING)",
            "CREATE NODE TABLE IF NOT EXISTS Trait(id STRING PRIMARY KEY, name STRING, filePath STRING, startLine INT32, endLine INT32, content STRING)",
            "CREATE NODE TABLE IF NOT EXISTS Module(id STRING PRIMARY KEY, name STRING, filePath STRING, startLine INT32, endLine INT32, content STRING)",
            "CREATE NODE TABLE IF NOT EXISTS Namespace(id STRING PRIMARY KEY, name STRING, filePath STRING, startLine INT32, endLine INT32, content STRING)",
            "CREATE NODE TABLE IF NOT EXISTS Community(id STRING PRIMARY KEY, label STRING, heuristicLabel STRING, cohesion DOUBLE, symbolCount INT32)",
            "CREATE NODE TABLE IF NOT EXISTS Process(id STRING PRIMARY KEY, label STRING, heuristicLabel STRING, processType STRING, stepCount INT32, entryPointId STRING, terminalId STRING)",
            "CREATE NODE TABLE IF NOT EXISTS Route(id STRING PRIMARY KEY, name STRING, filePath STRING)",
            "CREATE NODE TABLE IF NOT EXISTS Tool(id STRING PRIMARY KEY, name STRING, filePath STRING, description STRING)",
            "CREATE NODE TABLE IF NOT EXISTS CodeElement(id STRING PRIMARY KEY, name STRING, filePath STRING, startLine INT32, endLine INT32, content STRING)",
            "CREATE NODE TABLE IF NOT EXISTS CodeEmbedding(id STRING PRIMARY KEY, nodeId STRING, chunkIndex INT32, startLine INT32, endLine INT32, contentHash STRING)",
            "CREATE REL TABLE IF NOT EXISTS CodeRelation(FROM CodeElement TO CodeElement, MANY_MANY, type STRING, confidence DOUBLE)",
            "CREATE REL TABLE IF NOT EXISTS FileRelation(FROM File TO File, MANY_MANY, type STRING)",
            "CREATE REL TABLE IF NOT EXISTS MemberOf(FROM CodeElement TO Community, MANY_MANY, confidence DOUBLE)",
            "CREATE REL TABLE IF NOT EXISTS StepInProcess(FROM CodeElement TO Process, MANY_MANY, step INT32, confidence DOUBLE)",
            "CREATE REL TABLE IF NOT EXISTS EntryPointOf(FROM CodeElement TO Process, MANY_MANY, confidence DOUBLE)",
            "CREATE REL TABLE IF NOT EXISTS Defines(FROM File TO CodeElement, ONE_MANY, confidence DOUBLE)",
            "CREATE REL TABLE IF NOT EXISTS Contains(FROM Folder TO File, ONE_MANY)",
        ];

        for stmt in stmts {
            let _ = self.execute(stmt).await;
        }

        self.schema_initialized = true;
        Ok(())
    }

    async fn execute(&self, query: &str) -> Result<JsValue, JsValue> {
        let method: js_sys::Function = Reflect::get(&self.conn, &"query".into())?.dyn_into()?;
        let promise: Promise = method.call1(&self.conn, &JsValue::from_str(query))?.dyn_into()?;
        wasm_bindgen_futures::JsFuture::from(promise).await
    }

    async fn query_internal(
        &self, cypher: &str,
    ) -> Result<Vec<HashMap<String, serde_json::Value>>, JsValue> {
        let result = self.execute(cypher).await?;
        let rows_method: js_sys::Function =
            Reflect::get(&result, &"getAllRows".into())?.dyn_into()?;
        let rows_promise: Promise = rows_method.call0(&result)?.dyn_into()?;
        let rows = wasm_bindgen_futures::JsFuture::from(rows_promise).await?;
        let rows_array = js_sys::Array::from(&rows);
        let mut out = Vec::new();
        for i in 0..rows_array.length() {
            let row = rows_array.get(i);
            let row_obj = Object::from(row);
            let mut map = HashMap::new();
            for key in Object::keys(&row_obj).iter() {
                let key_str = key.as_string().unwrap_or_default();
                let val = Reflect::get(&row_obj, &key.into())?;
                
                let jv = if val.is_string() {
                    serde_json::Value::String(val.as_string().unwrap_or_default())
                } else if let Some(n) = val.as_f64() {
                    serde_json::Value::Number(
                        serde_json::Number::from_f64(n)
                            .unwrap_or(serde_json::Number::from(0)),
                    )
                } else if let Some(b) = val.as_bool() {
                    serde_json::Value::Bool(b)
                } else if val.is_null() || val.is_undefined() {
                    serde_json::Value::Null
                } else {
                    let s = js_sys::JSON::stringify(&val)
                        .map_err(|_| JsValue::from_str("stringify fail"))?
                        .as_string()
                        .unwrap_or_else(|| "null".to_string());
                    serde_json::from_str(&s).unwrap_or(serde_json::Value::Null)
                };
                map.insert(key_str, jv);
            }
            out.push(map);
        }
        Ok(out)
    }

    async fn create_node(&self, label: &str, properties: JsValue) -> Result<(), JsValue> {
        let props: HashMap<String, serde_json::Value> =
            serde_wasm_bindgen::from_value(properties)
                .map_err(|e| JsValue::from_str(&e.to_string()))?;

        let id = props.get("id").and_then(|v| v.as_str())
            .ok_or_else(|| JsValue::from_str("Node must have 'id'"))?;

        let sets: Vec<String> = props.iter().filter(|(k, _)| k.as_str() != "id").map(|(k, v)| {
            let fv = match v {
                serde_json::Value::String(s)  => format!("'{}'", s.replace('\'', "''")),
                serde_json::Value::Number(n)  => n.to_string(),
                serde_json::Value::Bool(b)    => b.to_string(),
                serde_json::Value::Null       => "NULL".to_string(),
                other                         => format!("'{}'", other.to_string().replace('\'', "''")),
            };
            format!("n.{} = {}", k, fv)
        }).collect();

        let query = if sets.is_empty() {
            format!("MERGE (n:{} {{id: '{}'}})", label, id.replace('\'', "''"))
        } else {
            format!(
                "MERGE (n:{} {{id: '{}'}}) ON CREATE SET {} ON MATCH SET {}",
                label,
                id.replace('\'', "''"),
                sets.join(", "),
                sets.join(", "),
            )
        };

        self.execute(&query).await?;
        Ok(())
    }

    async fn create_relationship(
        &self,
        from_id: &str,
        to_id:   &str,
        rel:     &str,
        props:   Option<JsValue>,
    ) -> Result<(), JsValue> {
        let mut q = format!(
            "MATCH (a), (b) WHERE a.id = '{}' AND b.id = '{}' MERGE (a)-[r:{}]->(b)",
            from_id.replace('\'', "''"),
            to_id.replace('\'', "''"),
            rel,
        );
        if let Some(p) = props {
            let pm: HashMap<String, serde_json::Value> =
                serde_wasm_bindgen::from_value(p)
                    .map_err(|e| JsValue::from_str(&e.to_string()))?;
            let sets: Vec<String> = pm.iter().map(|(k, v)| {
                let fv = match v {
                    serde_json::Value::String(s)  => format!("'{}'", s.replace('\'', "''")),
                    serde_json::Value::Number(n)  => n.to_string(),
                    serde_json::Value::Bool(b)    => b.to_string(),
                    _                             => "NULL".to_string(),
                };
                format!("r.{} = {}", k, fv)
            }).collect();
            if !sets.is_empty() {
                q.push_str(&format!(" ON CREATE SET {} ON MATCH SET {}", sets.join(", "), sets.join(", ")));
            }
        }
        self.execute(&q).await?;
        Ok(())
    }
}

#[wasm_bindgen]
pub struct GraphBuilder { 
    #[wasm_bindgen(skip)]
    pub db: GraphDatabase 
}

#[wasm_bindgen]
impl GraphBuilder {
    #[wasm_bindgen]
    pub async fn create() -> Result<GraphBuilder, JsValue> {
        Ok(Self { db: GraphDatabase::open(None).await? })
    }

    pub async fn build_from_parsed(&self, parsed_js: JsValue) -> Result<JsResult, JsValue> {
        let files: Vec<ParsedFile> = serde_wasm_bindgen::from_value(parsed_js)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        let mut total_nodes = 0u32;
        let mut total_rels  = 0u32;

        for file in &files {
            let file_id = format!("File:{}", file.file_path);
            let fp = serde_json::json!({
                "id":       file_id,
                "name":     file.file_path.split('/').last().unwrap_or(""),
                "filePath": file.file_path,
            });
            if self.db.create_node("File", serde_wasm_bindgen::to_value(&fp).unwrap()).await.is_ok() {
                total_nodes += 1;
            }

            for sym in &file.symbols {
                let label = sym.kind_label();
                let mut props = serde_json::json!({
                    "id":        sym.id,
                    "name":      sym.name,
                    "filePath":  sym.file_path,
                    "startLine": sym.start_line,
                    "endLine":   sym.end_line,
                });
                if let Some(c) = &sym.content      { props["content"]  = c.clone().into(); }
                if let Some(e) = sym.is_exported    { props["isExported"] = e.into(); }
                if let Some(p) = sym.parameter_count { props["parameterCount"] = p.into(); }
                if let Some(r) = &sym.return_type   { props["returnType"] = r.clone().into(); }

                if self.db.create_node(label, serde_wasm_bindgen::to_value(&props).unwrap()).await.is_ok() {
                    total_nodes += 1;
                }

                let rp = serde_json::json!({"confidence": 1.0});
                if self.db.create_relationship(&file_id, &sym.id, "Defines",
                    Some(serde_wasm_bindgen::to_value(&rp).unwrap())).await.is_ok() {
                    total_rels += 1;
                }
            }

            for imp in &file.imports {
                let mod_id = format!("Module:{}", imp.source.replace('\'', ""));
                let mp = serde_json::json!({
                    "id": mod_id, "name": imp.source, "filePath": "",
                    "startLine": imp.line, "endLine": imp.line,
                });
                let _ = self.db.create_node("Module",
                    serde_wasm_bindgen::to_value(&mp).unwrap()).await;
                let rp = serde_json::json!({"type": "IMPORTS", "confidence": 0.9});
                if self.db.create_relationship(&file_id, &mod_id, "CodeRelation",
                    Some(serde_wasm_bindgen::to_value(&rp).unwrap())).await.is_ok() {
                    total_rels += 1;
                }
            }
        }

        self.resolve_calls(&files).await?;

        Ok(JsResult::ok(serde_json::json!({
            "files": files.len() as u32,
            "nodes": total_nodes,
            "edges": total_rels
        }).to_string()))
    }
}

// Internal methods for GraphBuilder
impl GraphBuilder {
    async fn resolve_calls(&self, files: &[ParsedFile]) -> Result<(), JsValue> {
        let mut sym_index: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
        for f in files {
            for s in &f.symbols { sym_index.insert(&s.name, &s.id); }
        }
        for f in files {
            let caller = format!("File:{}", f.file_path);
            for call in &f.calls {
                if let Some(&target_id) = sym_index.get(call.target.as_str()) {
                    let rp = serde_json::json!({"type": "CALLS", "confidence": 0.8});
                    let _ = self.db.create_relationship(&caller, target_id, "CodeRelation",
                        Some(serde_wasm_bindgen::to_value(&rp).unwrap())).await;
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedFile {
    pub file_path: String,
    pub language:  String,
    pub symbols:   Vec<Symbol>,
    pub imports:   Vec<Import>,
    pub calls:     Vec<CallSite>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Symbol {
    pub id: String, pub name: String, pub kind: String,
    pub file_path: String, pub start_line: u32, pub end_line: u32,
    pub content: Option<String>, pub is_exported: Option<bool>,
    pub parameter_count: Option<u32>, pub return_type: Option<String>,
}

impl Symbol {
    fn kind_label(&self) -> &str {
        match self.kind.as_str() {
            "Function" | "FUNCTION" => "Function",
            "Class"    | "CLASS"    => "Class",
            "Interface"| "INTERFACE"=> "Interface",
            "Method"   | "METHOD"   => "Method",
            "Struct"   | "STRUCT"   => "Struct",
            "Enum"     | "ENUM"     => "Enum",
            "Trait"    | "TRAIT"    => "Trait",
            "Module"   | "MODULE"   => "Module",
            "Namespace"| "NAMESPACE"=> "Namespace",
            _                       => "CodeElement",
        }
    }
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Import { pub source: String, pub line: u32 }

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallSite { pub target: String, pub line: u32 }
