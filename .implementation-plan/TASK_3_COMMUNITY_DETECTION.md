# Task 3: Community Detection — Implementation Guide

**Priority**: P0 (Critical Path)  
**Estimated Effort**: 1.5 weeks  
**Skill Level**: Advanced (graph algorithms, Rust performance)  
**Dependencies**: Task 2 (needs complete graph)  
**Blocks**: Task 4 (Process Extraction uses communities)

---

## Problem Statement

The current graph has no community structure. We need **Louvain community detection** to:
1. Group related code elements (e.g., all "Auth" functions together)
2. Enable process extraction (communities = business domains)
3. Improve visualization (color by community)
4. Enable community-scoped search

---

## Algorithm: Louvain Method

The Louvain algorithm maximizes **modularity** (Q) through iterative optimization:

```
Q = (1/2m) * Σ_ij [A_ij - (k_i * k_j / 2m)] * δ(c_i, c_j)

Where:
- A_ij = weight of edge between i and j
- k_i = sum of weights of edges attached to i
- m = sum of all edge weights
- c_i = community of node i
- δ = 1 if same community, 0 otherwise
```

**Phases**:
1. **Local optimization**: Move each node to neighbor's community if modularity increases
2. **Aggregation**: Build new graph where nodes = communities
3. Repeat until modularity stops increasing

---

## Implementation

### Step 1: Add `petgraph` + Custom Louvain (Day 1-4)

```toml
# crates/gitnexus-graph/Cargo.toml
[dependencies]
petgraph = { version = "0.6", default-features = false }
# louvain-community = "0.3" # Alternative if available
```

```rust
// crates/gitnexus-graph/src/louvain.rs
use petgraph::graph::{Graph, NodeIndex};
use petgraph::Directed;
use std::collections::HashMap;

pub struct LouvainConfig {
    pub resolution: f64,           // γ parameter, default 1.0
    pub min_community_size: usize, // Default 3
    pub max_levels: usize,         // Default 10
    pub tolerance: f64,            // Modularity improvement threshold, default 1e-6
}

impl Default for LouvainConfig {
    fn default() -> Self {
        Self {
            resolution: 1.0,
            min_community_size: 3,
            max_levels: 10,
            tolerance: 1e-6,
        }
    }
}

pub struct LouvainResult {
    pub node_to_community: HashMap<String, u32>,
    pub community_stats: Vec<CommunityStat>,
    pub modularity: f64,
    pub levels: usize,
}

pub struct CommunityStat {
    pub id: u32,
    pub size: usize,
    pub internal_edges: u32,
    pub external_edges: u32,
    pub cohesion: f64, // internal / (internal + external)
    pub dominant_label: String,
}

pub fn detect_communities(
    graph: &Graph<String, f64, Directed>,
    config: &LouvainConfig,
) -> LouvainResult {
    let mut current_graph = graph.clone();
    let mut node_to_community: HashMap<NodeIndex, u32> = HashMap::new();
    let mut level_results: Vec<HashMap<NodeIndex, u32>> = Vec::new();

    // Phase 1: Initial assignment (each node in own community)
    for node in current_graph.node_indices() {
        node_to_community.insert(node, node.index() as u32);
    }

    for level in 0..config.max_levels {
        let mut improved = true;
        let mut passes = 0;

        while improved {
            improved = false;
            passes += 1;

            for node in current_graph.node_indices() {
                let current_comm = *node_to_community.get(&node).unwrap();
                let mut best_comm = current_comm;
                let mut best_gain = 0.0;

                // Calculate modularity gain for each neighbor's community
                let neighbor_comms = get_neighbor_communities(&current_graph, &node_to_community, node);

                for (comm, edge_weight) in neighbor_comms {
                    let gain = calculate_modularity_gain(
                        &current_graph, &node_to_community, node, current_comm, comm, edge_weight, config.resolution
                    );

                    if gain > best_gain {
                        best_gain = gain;
                        best_comm = comm;
                    }
                }

                if best_comm != current_comm && best_gain > config.tolerance {
                    node_to_community.insert(node, best_comm);
                    improved = true;
                }
            }
        }

        level_results.push(node_to_community.clone());

        // Phase 2: Aggregate communities into new graph
        let (new_graph, new_mapping) = aggregate_communities(&current_graph, &node_to_community);

        if new_graph.node_count() == current_graph.node_count() {
            break; // No further aggregation possible
        }

        current_graph = new_graph;

        // Remap node_to_community for new graph
        let mut remapped = HashMap::new();
        for (old_node, comm) in node_to_community {
            if let Some(&new_node) = new_mapping.get(&comm) {
                remapped.insert(new_node, comm);
            }
        }
        node_to_community = remapped;
    }

    // Calculate final modularity
    let modularity = calculate_modularity(&graph, &level_results.last().unwrap(), config.resolution);

    // Build final result
    let mut string_to_comm = HashMap::new();
    for (node_idx, comm) in level_results.last().unwrap() {
        let node_id = graph.node_weight(*node_idx).unwrap().clone();
        string_to_comm.insert(node_id, *comm);
    }

    let stats = calculate_community_stats(graph, &string_to_comm);

    LouvainResult {
        node_to_community: string_to_comm,
        community_stats: stats,
        modularity,
        levels: level_results.len(),
    }
}

fn get_neighbor_communities(
    graph: &Graph<String, f64, Directed>,
    communities: &HashMap<NodeIndex, u32>,
    node: NodeIndex,
) -> HashMap<u32, f64> {
    let mut result = HashMap::new();

    for edge in graph.edges(node) {
        let neighbor = edge.target();
        let weight = *edge.weight();
        let comm = *communities.get(&neighbor).unwrap();

        *result.entry(comm).or_insert(0.0) += weight;
    }

    result
}

fn calculate_modularity_gain(
    graph: &Graph<String, f64, Directed>,
    communities: &HashMap<NodeIndex, u32>,
    node: NodeIndex,
    current_comm: u32,
    new_comm: u32,
    edge_weight_to_new: f64,
    resolution: f64,
) -> f64 {
    // ΔQ = [Σ_in + k_i,in / 2m - (Σ_tot + k_i / 2m)²] - [Σ_in / 2m - (Σ_tot / 2m)² - (k_i / 2m)²]
    // Simplified: ΔQ = (edge_weight_to_new / m) - (degree_new_comm * degree_node / (2 * m²))

    let m = graph.edge_count() as f64;
    let k_i = graph.edges(node).map(|e| *e.weight()).sum::<f64>();

    let degree_new_comm: f64 = graph.node_indices()
        .filter(|n| *communities.get(n).unwrap() == new_comm)
        .map(|n| graph.edges(n).map(|e| *e.weight()).sum::<f64>())
        .sum();

    let gain = (edge_weight_to_new / m) - resolution * (degree_new_comm * k_i / (2.0 * m * m));

    gain
}

fn aggregate_communities(
    graph: &Graph<String, f64, Directed>,
    communities: &HashMap<NodeIndex, u32>,
) -> (Graph<String, f64, Directed>, HashMap<u32, NodeIndex>) {
    let mut new_graph = Graph::new();
    let mut comm_to_node: HashMap<u32, NodeIndex> = HashMap::new();

    // Create community nodes
    for (&comm, _) in communities.iter().map(|(_, c)| (c, ())).collect::<HashMap<_, _>>() {
        let node = new_graph.add_node(format!("Community:{}", comm));
        comm_to_node.insert(comm, node);
    }

    // Aggregate edges between communities
    let mut edge_weights: HashMap<(u32, u32), f64> = HashMap::new();

    for edge in graph.edge_indices() {
        let (source, target) = graph.edge_endpoints(edge).unwrap();
        let source_comm = *communities.get(&source).unwrap();
        let target_comm = *communities.get(&target).unwrap();
        let weight = *graph.edge_weight(edge).unwrap();

        let key = if source_comm <= target_comm {
            (source_comm, target_comm)
        } else {
            (target_comm, source_comm)
        };

        *edge_weights.entry(key).or_insert(0.0) += weight;
    }

    // Add aggregated edges
    for ((comm1, comm2), weight) in edge_weights {
        let node1 = *comm_to_node.get(&comm1).unwrap();
        let node2 = *comm_to_node.get(&comm2).unwrap();
        new_graph.add_edge(node1, node2, weight);
    }

    (new_graph, comm_to_node)
}

fn calculate_modularity(
    graph: &Graph<String, f64, Directed>,
    communities: &HashMap<NodeIndex, u32>,
    resolution: f64,
) -> f64 {
    let m = graph.edge_count() as f64;
    let mut q = 0.0;

    for edge in graph.edge_indices() {
        let (source, target) = graph.edge_endpoints(edge).unwrap();
        let weight = *graph.edge_weight(edge).unwrap();
        let source_comm = *communities.get(&source).unwrap();
        let target_comm = *communities.get(&target).unwrap();

        if source_comm == target_comm {
            let k_source = graph.edges(source).map(|e| *e.weight()).sum::<f64>();
            let k_target = graph.edges(target).map(|e| *e.weight()).sum::<f64>();

            q += weight - resolution * (k_source * k_target / (2.0 * m));
        }
    }

    q / (2.0 * m)
}

fn calculate_community_stats(
    graph: &Graph<String, f64, Directed>,
    node_to_comm: &HashMap<String, u32>,
) -> Vec<CommunityStat> {
    // Implementation...
    vec![]
}
```

### Step 2: Integrate with KuzuDB (Day 5-7)

```rust
// crates/gitnexus-graph/src/lib.rs (additions)

#[wasm_bindgen]
impl GraphDatabase {
    pub async fn detect_communities(&self, config: JsValue) -> Result<JsResult, JsValue> {
        let config: LouvainConfig = serde_wasm_bindgen::from_value(config)
            .unwrap_or_default();

        // Export graph to petgraph
        let graph = self.export_to_petgraph().await?;

        // Run Louvain
        let result = louvain::detect_communities(&graph, &config);

        // Store communities in KuzuDB
        for (comm_id, stat) in result.community_stats.iter().enumerate() {
            let props = serde_json::json!({
                "id": format!("Community:{}", comm_id),
                "label": format!("Community {}", comm_id),
                "heuristicLabel": stat.dominant_label,
                "cohesion": stat.cohesion,
                "symbolCount": stat.size as u32,
            });

            self.create_node("Community", serde_wasm_bindgen::to_value(&props).unwrap()).await?;
        }

        // Create MemberOf relationships
        for (node_id, comm_id) in &result.node_to_community {
            let rel_props = serde_json::json!({
                "confidence": 1.0,
            });

            self.create_relationship(
                node_id,
                &format!("Community:{}", comm_id),
                "MemberOf",
                Some(serde_wasm_bindgen::to_value(&rel_props).unwrap()),
            ).await?;
        }

        Ok(JsResult::ok(&serde_json::json!({
            "modularity": result.modularity,
            "communities": result.community_stats.len(),
            "levels": result.levels,
        })))
    }

    async fn export_to_petgraph(&self) -> Result<Graph<String, f64, Directed>, JsValue> {
        let query = "MATCH (n)-[r:CodeRelation]->(m) RETURN n.id AS sourceId, m.id AS targetId, r.confidence AS confidence";
        let rows = self.query(query).await?;

        let mut graph = Graph::new();
        let mut node_indices: HashMap<String, NodeIndex> = HashMap::new();

        for row in rows {
            let source_id = row.get("sourceId").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let target_id = row.get("targetId").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let confidence = row.get("confidence").and_then(|v| v.as_f64()).unwrap_or(0.5);

            let source_idx = *node_indices.entry(source_id.clone()).or_insert_with(|| {
                graph.add_node(source_id)
            });

            let target_idx = *node_indices.entry(target_id.clone()).or_insert_with(|| {
                graph.add_node(target_id)
            });

            graph.add_edge(source_idx, target_idx, confidence);
        }

        Ok(graph)
    }
}
```

### Step 3: Heuristic Labeling (Day 8-9)

```rust
pub fn generate_heuristic_label(
    members: &[&str],  // Node names in community
    member_types: &[&str], // Node types (Function, Class, etc.)
) -> String {
    // TF-IDF on member names
    let mut term_freq: HashMap<String, usize> = HashMap::new();

    for name in members {
        // Tokenize name (camelCase, snake_case)
        let terms = tokenize_identifier(name);
        for term in terms {
            *term_freq.entry(term).or_insert(0) += 1;
        }
    }

    // Find most distinctive terms (not common across all code)
    let mut scored_terms: Vec<(String, f64)> = term_freq.iter()
        .map(|(term, freq)| {
            let tf = *freq as f64 / members.len() as f64;
            let idf = 1.0; // Simplified — would need corpus stats
            (term.clone(), tf * idf)
        })
        .collect();

    scored_terms.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    // Format top 2-3 terms into label
    let top_terms: Vec<String> = scored_terms.iter()
        .take(3)
        .map(|(term, _)| capitalize_first(term))
        .collect();

    if top_terms.is_empty() {
        "Miscellaneous".to_string()
    } else {
        top_terms.join("")
    }
}

fn tokenize_identifier(name: &str) -> Vec<String> {
    // Split camelCase and snake_case
    let mut terms = Vec::new();
    let mut current = String::new();

    for (i, ch) in name.chars().enumerate() {
        if ch == '_' || ch == '-' {
            if !current.is_empty() {
                terms.push(current.to_lowercase());
                current.clear();
            }
        } else if ch.is_uppercase() && i > 0 && !current.is_empty() {
            terms.push(current.to_lowercase());
            current = ch.to_lowercase().to_string();
        } else {
            current.push(ch.to_lowercase().next().unwrap_or(ch));
        }
    }

    if !current.is_empty() {
        terms.push(current);
    }

    terms
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}
```

### Step 4: UI Color Coding (Day 10-11)

```typescript
// web/src/components/GraphView.tsx
function getNodeColor(node: any): string {
    const communityColors: Record<number, string> = {
        0: '#ef4444', // red
        1: '#f97316', // orange
        2: '#f59e0b', // amber
        3: '#84cc16', // lime
        4: '#22c55e', // green
        5: '#14b8a6', // teal
        6: '#06b6d4', // cyan
        7: '#3b82f6', // blue
        8: '#8b5cf6', // violet
        9: '#d946ef', // fuchsia
    };

    return communityColors[node.community] || '#94a3b8';
}
```

---

## Acceptance Criteria

- [ ] Louvain modularity >0.3 on 1000-node test graph
- [ ] Communities have >3 members each
- [ ] Heuristic labels are meaningful (e.g., "AuthService", not "Community 5")
- [ ] Community detection on 1000-node graph in <2s
- [ ] Graph visualization colors nodes by community
- [ ] Community nodes stored in KuzuDB with `MemberOf` relationships
- [ ] Search can filter by community

---

## Deliverables

1. `crates/gitnexus-graph/src/louvain.rs` — Louvain implementation
2. `crates/gitnexus-graph/src/lib.rs` — Modified (community methods)
3. `web/src/components/GraphView.tsx` — Modified (community colors)
4. `docs/ADR/003-community-detection.md` — Architecture decision record
