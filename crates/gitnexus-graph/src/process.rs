//! Process extraction via BFS over call graph (Task 4)
//!
//! Finds execution flows starting from HTTP routes, CLI entry points,
//! and event handlers, then stores them as Process nodes in KuzuDB.

use std::collections::{HashMap, HashSet, VecDeque};
use serde::{Deserialize, Serialize};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessConfig {
    pub max_depth:      u32,
    pub min_confidence: f64,
    pub max_steps:      usize,
    pub min_steps:      usize,
}

impl Default for ProcessConfig {
    fn default() -> Self {
        Self { max_depth: 10, min_confidence: 0.5, max_steps: 50, min_steps: 2 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessStep {
    pub node_id:      String,
    pub node_name:    String,
    pub node_type:    String,
    pub depth:        u32,
    pub confidence:   f64,
    pub from_node_id: Option<String>,
    pub relation_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Process {
    pub id:              String,
    pub label:           String,
    pub heuristic_label: String,
    pub process_type:    String,
    pub entry_point_id:  String,
    pub terminal_ids:    Vec<String>,
    pub step_count:      u32,
    pub steps:           Vec<ProcessStep>,
}

// ============================================================================
// BFS over an in-memory adjacency map
// ============================================================================

/// Edges: source_id → list of (target_id, rel_type, confidence)
pub type CallGraph = HashMap<String, Vec<(String, String, f64)>>;

/// Node metadata: id → (name, node_type)
pub type NodeMeta = HashMap<String, (String, String)>;

pub struct ProcessExtractor {
    pub config: ProcessConfig,
}

impl ProcessExtractor {
    pub fn new(config: ProcessConfig) -> Self {
        Self { config }
    }

    /// Detect entry-point node IDs from the metadata (Routes, CLI Tools).
    pub fn find_entry_points(meta: &NodeMeta) -> Vec<String> {
        meta.iter()
            .filter(|(_, (name, node_type))| {
                node_type == "Route"
                    || node_type == "Tool"
                    || name.to_lowercase().contains("handler")
                    || name.to_lowercase().contains("controller")
                    || name.to_lowercase().contains("command")
                    || name.to_lowercase().contains("main")
            })
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Extract a single process starting from `entry_id`.
    pub fn extract_one(
        &self,
        entry_id: &str,
        graph: &CallGraph,
        meta: &NodeMeta,
    ) -> Option<Process> {
        let mut visited:  HashSet<String>  = HashSet::new();
        let mut queue:    VecDeque<(String, u32, Option<String>, String)> = VecDeque::new();
        let mut steps:    Vec<ProcessStep> = Vec::new();
        let mut terminals: Vec<String>     = Vec::new();

        queue.push_back((entry_id.to_owned(), 0, None, "EntryPoint".to_owned()));
        visited.insert(entry_id.to_owned());

        while let Some((node_id, depth, from_id, rel_type)) = queue.pop_front() {
            if depth > self.config.max_depth || steps.len() >= self.config.max_steps {
                break;
            }

            let (node_name, node_type) = meta
                .get(&node_id)
                .cloned()
                .unwrap_or_else(|| ("unknown".to_owned(), "CodeElement".to_owned()));

            steps.push(ProcessStep {
                node_id:      node_id.clone(),
                node_name,
                node_type,
                depth,
                confidence:   1.0 - depth as f64 * 0.05,
                from_node_id: from_id,
                relation_type: rel_type,
            });

            let outgoing = graph.get(&node_id);
            match outgoing {
                Some(out) if !out.is_empty() => {
                    for (target_id, rel_type, confidence) in out {
                        if *confidence >= self.config.min_confidence
                            && !visited.contains(target_id)
                        {
                            visited.insert(target_id.clone());
                            queue.push_back((
                                target_id.clone(),
                                depth + 1,
                                Some(node_id.clone()),
                                rel_type.clone(),
                            ));
                        }
                    }
                }
                _ => {
                    terminals.push(node_id.clone());
                }
            }
        }

        if steps.len() < self.config.min_steps {
            return None;
        }

        let (entry_name, entry_type) = meta
            .get(entry_id)
            .cloned()
            .unwrap_or_else(|| (entry_id.to_owned(), "Unknown".to_owned()));

        let process_type = infer_type(&entry_type, &entry_name);
        let label = entry_name.clone();
        let heuristic_label = label.clone();
        let step_count = steps.len() as u32;

        Some(Process {
            id: format!("Process:{}", entry_id.replace(':', "_")),
            label,
            heuristic_label,
            process_type,
            entry_point_id: entry_id.to_owned(),
            terminal_ids: terminals,
            step_count,
            steps,
        })
    }

    /// Run extraction for all detected entry points.
    pub fn extract_all(
        &self,
        graph: &CallGraph,
        meta:  &NodeMeta,
    ) -> Vec<Process> {
        let entries = Self::find_entry_points(meta);
        let mut processes = Vec::new();
        let mut seen_entries: HashSet<String> = HashSet::new();

        for entry in &entries {
            if seen_entries.insert(entry.clone()) {
                if let Some(proc) = self.extract_one(entry, graph, meta) {
                    processes.push(proc);
                }
            }
        }
        processes
    }
}

fn infer_type(node_type: &str, name: &str) -> String {
    let n = name.to_lowercase();
    if node_type == "Route" || n.contains("route") || n.contains("api") {
        "HTTP".to_owned()
    } else if node_type == "Tool" || n.contains("command") || n.contains("cmd") {
        "CLI".to_owned()
    } else if n.contains("event") || n.contains("listener") || n.contains("subscriber") {
        "Event".to_owned()
    } else if n.contains("cron") || n.contains("schedule") || n.contains("job") {
        "Cron".to_owned()
    } else {
        "Unknown".to_owned()
    }
}

// ============================================================================
// WASM bridge helpers (called from gitnexus-graph/src/lib.rs)
// ============================================================================

/// Convert a KuzuDB query result row slice to a CallGraph + NodeMeta pair.
/// Each row must have: sourceId, targetId, relType, confidence, sourceName,
/// sourceType, targetName, targetType
pub fn build_graph_from_rows(
    rows: &[HashMap<String, serde_json::Value>],
) -> (CallGraph, NodeMeta) {
    let mut call_graph: CallGraph = HashMap::new();
    let mut meta: NodeMeta = HashMap::new();

    for row in rows {
        let src  = row.get("sourceId").and_then(|v| v.as_str()).unwrap_or("").to_owned();
        let tgt  = row.get("targetId").and_then(|v| v.as_str()).unwrap_or("").to_owned();
        let rel  = row.get("relType").and_then(|v| v.as_str()).unwrap_or("CALLS").to_owned();
        let conf = row.get("confidence").and_then(|v| v.as_f64()).unwrap_or(0.5);

        let src_name = row.get("sourceName").and_then(|v| v.as_str()).unwrap_or("").to_owned();
        let src_type = row.get("sourceType").and_then(|v| v.as_str()).unwrap_or("CodeElement").to_owned();
        let tgt_name = row.get("targetName").and_then(|v| v.as_str()).unwrap_or("").to_owned();
        let tgt_type = row.get("targetType").and_then(|v| v.as_str()).unwrap_or("CodeElement").to_owned();

        if !src.is_empty() {
            meta.entry(src.clone()).or_insert((src_name, src_type));
        }
        if !tgt.is_empty() {
            meta.entry(tgt.clone()).or_insert((tgt_name, tgt_type));
        }
        if !src.is_empty() && !tgt.is_empty() {
            call_graph.entry(src).or_default().push((tgt, rel, conf));
        }
    }

    (call_graph, meta)
}
