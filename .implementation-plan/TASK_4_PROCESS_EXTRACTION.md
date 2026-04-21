# Task 4: Process Extraction — Implementation Guide

**Priority**: P0 (Critical Path)  
**Estimated Effort**: 1 week  
**Skill Level**: Advanced (BFS/DFS, graph traversal)  
**Dependencies**: Task 3 (Community Detection)  
**Blocks**: None (leaf task)

---

## Problem Statement

Process extraction identifies **execution flows** through code — e.g., "HTTP request → auth middleware → route handler → database query → response". Currently stubbed; needs full BFS-based implementation.

---

## Algorithm: BFS Flow Detection

1. **Find entry points**: Routes, CLI commands, event handlers, main functions
2. **BFS traversal**: Follow `CALLS` edges from entry points
3. **Track depth**: Limit to avoid infinite recursion (cycles)
4. **Group by community**: Each process belongs to one community
5. **Identify terminals**: Functions that don't call others (leaves)

---

## Implementation

```rust
// crates/gitnexus-graph/src/process.rs
use std::collections::{HashMap, HashSet, VecDeque};

pub struct ProcessExtractor {
    max_depth: u32,
    min_confidence: f64,
    max_steps: usize,
}

impl Default for ProcessExtractor {
    fn default() -> Self {
        Self {
            max_depth: 10,
            min_confidence: 0.5,
            max_steps: 50,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Process {
    pub id: String,
    pub label: String,
    pub process_type: ProcessType,
    pub entry_point_id: String,
    pub terminal_ids: Vec<String>,
    pub steps: Vec<ProcessStep>,
    pub communities: Vec<u32>,
    pub step_count: u32,
}

#[derive(Debug, Clone)]
pub enum ProcessType {
    Http,
    Cli,
    Event,
    Background,
    Cron,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct ProcessStep {
    pub node_id: String,
    pub node_name: String,
    pub node_type: String,
    pub depth: u32,
    pub confidence: f64,
    pub from_node_id: Option<String>,
    pub relation_type: String,
}

impl ProcessExtractor {
    pub fn extract_from_graph(
        &self,
        graph: &GraphDatabase,
        entry_points: Vec<String>,
    ) -> Vec<Process> {
        let mut processes = Vec::new();

        for entry_id in entry_points {
            if let Some(process) = self.extract_single_process(graph, &entry_id) {
                processes.push(process);
            }
        }

        processes
    }

    fn extract_single_process(
        &self,
        graph: &GraphDatabase,
        entry_id: &str,
    ) -> Option<Process> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let mut steps = Vec::new();
        let mut terminals = Vec::new();

        // Get entry point info
        let entry_query = format!(
            "MATCH (n) WHERE n.id = '{}' RETURN n.name AS name, labels(n)[0] AS type",
            entry_id.replace("'", "''")
        );

        let entry_info = graph.query(&entry_query).await.ok()?;
        let entry_name = entry_info.get(0)?.get("name")?.as_str()?;
        let entry_type = entry_info.get(0)?.get("type")?.as_str()?;

        // Determine process type from entry point
        let process_type = self.infer_process_type(entry_type, entry_name);

        // BFS
        queue.push_back((entry_id.to_string(), 0u32, None::<String>, "EntryPoint".to_string()));
        visited.insert(entry_id.to_string());

        while let Some((node_id, depth, from_id, rel_type)) = queue.pop_front() {
            if depth > self.max_depth || steps.len() >= self.max_steps {
                break;
            }

            // Get node info
            let node_query = format!(
                "MATCH (n) WHERE n.id = '{}' RETURN n.name AS name, labels(n)[0] AS type",
                node_id.replace("'", "''")
            );

            let node_info = match graph.query(&node_query).await {
                Ok(rows) if !rows.is_empty() => rows.into_iter().next().unwrap(),
                _ => continue,
            };

            let node_name = node_info.get("name")?.as_str()?.to_string();
            let node_type = node_info.get("type")?.as_str()?.to_string();

            steps.push(ProcessStep {
                node_id: node_id.clone(),
                node_name,
                node_type,
                depth,
                confidence: 1.0 - (depth as f64 * 0.05), // Decrease confidence with depth
                from_node_id: from_id,
                relation_type: rel_type,
            });

            // Find outgoing calls
            let outgoing_query = format!(
                "MATCH (n)-[r:CodeRelation]->(target) WHERE n.id = '{}' AND r.confidence >= {} RETURN target.id AS targetId, target.name AS name, r.type AS relType, r.confidence AS confidence",
                node_id.replace("'", "''"),
                self.min_confidence
            );

            let outgoing = match graph.query(&outgoing_query).await {
                Ok(rows) => rows,
                Err(_) => {
                    terminals.push(node_id);
                    continue;
                }
            };

            if outgoing.is_empty() {
                terminals.push(node_id);
            }

            for row in outgoing {
                let target_id = row.get("targetId")?.as_str()?.to_string();
                let confidence = row.get("confidence")?.as_f64()?;
                let rel_type = row.get("relType")?.as_str()?.to_string();

                if !visited.contains(&target_id) && confidence >= self.min_confidence {
                    visited.insert(target_id.clone());
                    queue.push_back((target_id, depth + 1, Some(node_id.clone()), rel_type));
                }
            }
        }

        if steps.len() < 2 {
            return None; // Too short to be a meaningful process
        }

        Some(Process {
            id: format!("Process:{}", entry_id.replace(":", "_")),
            label: format!("{}", entry_name),
            process_type,
            entry_point_id: entry_id.to_string(),
            terminal_ids: terminals,
            step_count: steps.len() as u32,
            steps,
            communities: vec![], // Would be filled from community detection
        })
    }

    fn infer_process_type(entry_type: &str, entry_name: &str) -> ProcessType {
        match entry_type {
            "Route" => ProcessType::Http,
            "Tool" => ProcessType::Cli,
            _ => {
                if entry_name.contains("handler") || entry_name.contains("controller") {
                    ProcessType::Http
                } else if entry_name.contains("command") || entry_name.contains("cmd") {
                    ProcessType::Cli
                } else if entry_name.contains("event") || entry_name.contains("listener") {
                    ProcessType::Event
                } else if entry_name.contains("cron") || entry_name.contains("schedule") {
                    ProcessType::Cron
                } else {
                    ProcessType::Unknown
                }
            }
        }
    }
}
```

### Storage in KuzuDB

```rust
pub async fn store_process(&self, process: &Process) -> Result<(), JsValue> {
    // Create Process node
    let props = serde_json::json!({
        "id": process.id,
        "label": process.label,
        "heuristicLabel": process.label,
        "processType": format!("{:?}", process.process_type),
        "stepCount": process.step_count,
        "entryPointId": process.entry_point_id,
        "terminalId": process.terminal_ids.first().unwrap_or(&"".to_string()),
    });

    self.create_node("Process", serde_wasm_bindgen::to_value(&props).unwrap()).await?;

    // Create StepInProcess relationships
    for (i, step) in process.steps.iter().enumerate() {
        let rel_props = serde_json::json!({
            "step": (i + 1) as u32,
            "confidence": step.confidence,
        });

        self.create_relationship(
            &step.node_id,
            &process.id,
            "StepInProcess",
            Some(serde_wasm_bindgen::to_value(&rel_props).unwrap()),
        ).await?;
    }

    // EntryPointOf relationship
    let entry_props = serde_json::json!({
        "confidence": 1.0,
    });

    self.create_relationship(
        &process.entry_point_id,
        &process.id,
        "EntryPointOf",
        Some(serde_wasm_bindgen::to_value(&entry_props).unwrap()),
    ).await?;

    Ok(())
}
```

---

## Acceptance Criteria

- [ ] Detects "LoginFlow" with 5+ steps from `/api/login` route
- [ ] Detects "PaymentFlow" with 3+ steps from `/api/pay` route
- [ ] Handles cycles (A→B→C→A) without infinite loops
- [ ] Process nodes stored in KuzuDB with `StepInProcess` edges
- [ ] UI shows process as timeline with step numbers
- [ ] Entry points auto-detected (Routes, CLI commands)

---

## Deliverables

1. `crates/gitnexus-graph/src/process.rs` — Process extractor
2. `crates/gitnexus-graph/src/lib.rs` — Modified (process storage)
3. `web/src/components/ContextPanel.tsx` — Modified (process timeline)
