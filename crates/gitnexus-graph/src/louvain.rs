//! Louvain community detection algorithm (Task 3)
//!
//! Maximises modularity Q via two alternating phases:
//! 1. Local optimization — greedily move nodes to neighbour communities
//! 2. Graph aggregation — collapse communities into single super-nodes
//!
//! Reference: Blondel et al. (2008), "Fast unfolding of communities in large networks"

use std::collections::HashMap;
use serde::{Serialize, Deserialize};


// ============================================================================
// Public API
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LouvainConfig {
    /// Resolution parameter γ (default 1.0)
    pub resolution: f64,
    /// Minimum community size to keep (default 3)
    pub min_community_size: usize,
    /// Maximum aggregation levels (default 10)
    pub max_levels: usize,
    /// Modularity improvement threshold to stop (default 1e-6)
    pub tolerance: f64,
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

#[derive(Debug, Clone)]
pub struct CommunityStat {
    pub id: u32,
    pub size: usize,
    pub cohesion: f64,
    pub dominant_label: String,
}

#[derive(Debug, Clone)]
pub struct LouvainResult {
    /// Maps node_id → community_id
    pub node_to_community: HashMap<String, u32>,
    pub community_stats: Vec<CommunityStat>,
    pub modularity: f64,
    pub levels: usize,
}

// ============================================================================
// Internal representation
// ============================================================================

/// Simple adjacency list for numeric node indices
struct SparseGraph {
    /// adj[i] = list of (neighbour_index, weight)
    adj: Vec<Vec<(usize, f64)>>,
    /// Original string IDs (index → id)
    _node_ids: Vec<String>,
    total_weight: f64,
}

impl SparseGraph {
    fn node_count(&self) -> usize {
        self.adj.len()
    }

    fn degree(&self, node: usize) -> f64 {
        self.adj[node].iter().map(|(_, w)| w).sum()
    }
}

// ============================================================================
// Entry point
// ============================================================================

/// Run Louvain on a list of (source_id, target_id, weight) edges.
pub fn detect_communities(
    edges: &[(String, String, f64)],
    config: &LouvainConfig,
) -> LouvainResult {
    if edges.is_empty() {
        return LouvainResult {
            node_to_community: HashMap::new(),
            community_stats: vec![],
            modularity: 0.0,
            levels: 0,
        };
    }

    // Build initial graph
    let graph = build_graph(edges);
    let n = graph.node_count();

    // Community assignment: each node in its own community
    let mut community: Vec<u32> = (0..n as u32).collect();
    let mut total_levels = 0;

    let mut current_graph = graph;

    for _level in 0..config.max_levels {
        let improved = local_phase(&current_graph, &mut community, config);
        total_levels += 1;
        if !improved {
            break;
        }
        let (new_graph, remap) = aggregate(&current_graph, &community);
        if new_graph.node_count() >= current_graph.node_count() {
            break;
        }
        // Remap community array to new smaller graph
        community = remap;
        current_graph = new_graph;
    }

    // Build final modularity & string mapping
    let q = modularity(&current_graph, &community, config.resolution);
    let string_map = build_string_map(edges, &community, config);
    let stats = build_stats(&string_map, config);

    LouvainResult {
        node_to_community: string_map,
        community_stats: stats,
        modularity: q,
        levels: total_levels,
    }
}

// ============================================================================
// Phase 1 – local optimisation
// ============================================================================

fn local_phase(graph: &SparseGraph, community: &mut Vec<u32>, config: &LouvainConfig) -> bool {
    let n = graph.node_count();
    let m2 = 2.0 * graph.total_weight; // 2m
    let mut changed = false;

    // Precompute degree for each node
    let degrees: Vec<f64> = (0..n).map(|i| graph.degree(i)).collect();

    // Σ_tot[c] = sum of degrees of nodes in community c
    let mut sigma_tot: HashMap<u32, f64> = HashMap::new();
    for (i, &c) in community.iter().enumerate() {
        *sigma_tot.entry(c).or_insert(0.0) += degrees[i];
    }

    let mut improved = true;
    while improved {
        improved = false;

        for node in 0..n {
            let cur_comm = community[node];
            let k_i = degrees[node];

            // Weight of edges from node to each neighbouring community
            let mut neighbour_weights: HashMap<u32, f64> = HashMap::new();
            for &(nb, w) in &graph.adj[node] {
                let c = community[nb];
                *neighbour_weights.entry(c).or_insert(0.0) += w;
            }

            // Remove node from current community
            let _k_i_cur = neighbour_weights.get(&cur_comm).copied().unwrap_or(0.0);
            *sigma_tot.entry(cur_comm).or_insert(0.0) -= k_i;

            // Find best community
            let mut best_comm  = cur_comm;
            let mut best_gain  = 0.0;

            for (&c, &k_i_c) in &neighbour_weights {
                if c == cur_comm { continue; }
                let sigma = sigma_tot.get(&c).copied().unwrap_or(0.0);
                let gain = (k_i_c / graph.total_weight)
                    - config.resolution * (sigma * k_i) / (m2 * graph.total_weight);
                if gain > best_gain {
                    best_gain = gain;
                    best_comm = c;
                }
            }

            // Move to best community if gain is positive
            *sigma_tot.entry(best_comm).or_insert(0.0) += k_i;
            if best_comm != cur_comm {
                community[node] = best_comm;
                improved  = true;
                changed   = true;
            }
        }
    }
    changed
}

// ============================================================================
// Phase 2 – aggregation
// ============================================================================

/// Build super-graph where each community becomes a node.
/// Returns (new_graph, community_per_super_node) where the latter starts
/// with each super-node in its own community.
fn aggregate(graph: &SparseGraph, community: &[u32]) -> (SparseGraph, Vec<u32>) {
    // Renumber communities 0..k
    let mut comm_to_idx: HashMap<u32, usize> = HashMap::new();
    for &c in community {
        let next = comm_to_idx.len();
        comm_to_idx.entry(c).or_insert(next);
    }
    let k = comm_to_idx.len();

    // Aggregate edges between super-nodes
    let mut super_edges: HashMap<(usize, usize), f64> = HashMap::new();
    for node in 0..graph.node_count() {
        let sc = comm_to_idx[&community[node]];
        for &(nb, w) in &graph.adj[node] {
            let tc = comm_to_idx[&community[nb]];
            let key = if sc <= tc { (sc, tc) } else { (tc, sc) };
            *super_edges.entry(key).or_insert(0.0) += w;
        }
    }

    let mut adj: Vec<Vec<(usize, f64)>> = vec![vec![]; k];
    let mut total = 0.0;
    for ((s, t), w) in super_edges {
        adj[s].push((t, w));
        if s != t { adj[t].push((s, w)); }
        total += w;
    }

    // Build node_ids for the super-graph (just stringify indices)
    let mut node_ids = vec![String::new(); k];
    for (c, idx) in &comm_to_idx {
        node_ids[*idx] = format!("SuperNode:{}", c);
    }

    let super_graph = SparseGraph { adj, _node_ids: node_ids, total_weight: total };
    let new_community: Vec<u32> = (0..k as u32).collect();
    (super_graph, new_community)
}

// ============================================================================
// Modularity
// ============================================================================

fn modularity(graph: &SparseGraph, community: &[u32], resolution: f64) -> f64 {
    let m2 = 2.0 * graph.total_weight;
    if m2 == 0.0 { return 0.0; }

    let mut q = 0.0;
    for node in 0..graph.node_count() {
        let k_i = graph.degree(node);
        for &(nb, w) in &graph.adj[node] {
            if community[node] == community[nb] {
                let k_j = graph.degree(nb);
                q += w - resolution * (k_i * k_j) / m2;
            }
        }
    }
    q / m2
}

// ============================================================================
// Helpers
// ============================================================================

fn build_graph(edges: &[(String, String, f64)]) -> SparseGraph {
    let mut node_index: HashMap<&str, usize> = HashMap::new();
    let mut node_ids: Vec<String> = Vec::new();

    let mut get_idx = |id: &str| -> usize {
        if let Some(&i) = node_index.get(id) { return i; }
        let i = node_ids.len();
        node_ids.push(id.to_owned());
        node_index.insert(unsafe { &*(id as *const str) }, i);
        i
    };

    let indexed: Vec<(usize, usize, f64)> = edges.iter().map(|(s, t, w)| {
        let si = get_idx(s.as_str());
        let ti = get_idx(t.as_str());
        (si, ti, *w)
    }).collect();

    let n = node_ids.len();
    let mut adj = vec![vec![]; n];
    let mut total = 0.0;

    for (s, t, w) in indexed {
        adj[s].push((t, w));
        if s != t { adj[t].push((s, w)); }
        total += w;
    }

    SparseGraph { adj, _node_ids: node_ids, total_weight: total }
}

/// Map original string node IDs to final community IDs.
fn build_string_map(
    edges: &[(String, String, f64)],
    community: &[u32],
    _config: &LouvainConfig,
) -> HashMap<String, u32> {
    // Re-build node index from original edges (same order as build_graph)
    let mut seen: Vec<String> = Vec::new();
    let mut node_index: HashMap<String, usize> = HashMap::new();
    for (s, t, _) in edges {
        for id in [s, t] {
            if !node_index.contains_key(id) {
                node_index.insert(id.clone(), seen.len());
                seen.push(id.clone());
            }
        }
    }

    let mut map = HashMap::new();
    for (id, &idx) in &node_index {
        if idx < community.len() {
            map.insert(id.clone(), community[idx]);
        }
    }
    map
}

fn build_stats(node_to_comm: &HashMap<String, u32>, config: &LouvainConfig) -> Vec<CommunityStat> {
    let mut sizes: HashMap<u32, usize> = HashMap::new();
    let mut names: HashMap<u32, Vec<String>> = HashMap::new();

    for (id, &c) in node_to_comm {
        *sizes.entry(c).or_insert(0) += 1;
        names.entry(c).or_default().push(id.clone());
    }

    let mut stats: Vec<CommunityStat> = sizes
        .iter()
        .filter(|(_, &s)| s >= config.min_community_size)
        .map(|(&id, &size)| {
            let members = names.get(&id).cloned().unwrap_or_default();
            CommunityStat {
                id,
                size,
                cohesion: 0.8, // placeholder; full calc needs intra/inter edge counts
                dominant_label: derive_label(&members),
            }
        })
        .collect();

    stats.sort_by_key(|s| std::cmp::Reverse(s.size));
    stats
}

/// Derive a human-readable label from member IDs via token frequency.
fn derive_label(members: &[String]) -> String {
    let mut freq: HashMap<String, usize> = HashMap::new();

    for m in members {
        // Strip type prefix (e.g. "Function:doLogin" → "doLogin")
        let name = m.splitn(2, ':').last().unwrap_or(m.as_str());
        for tok in tokenize_identifier(name) {
            *freq.entry(tok).or_insert(0) += 1;
        }
    }

    let mut sorted: Vec<_> = freq.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));

    let label: String = sorted
        .iter()
        .take(2)
        .map(|(t, _)| capitalize(t))
        .collect();

    if label.is_empty() { "Misc".to_string() } else { label }
}

fn tokenize_identifier(name: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut cur = String::new();

    for (i, ch) in name.chars().enumerate() {
        if ch == '_' || ch == '-' {
            if !cur.is_empty() { tokens.push(cur.to_lowercase()); cur.clear(); }
        } else if ch.is_uppercase() && i > 0 && !cur.is_empty() {
            tokens.push(cur.to_lowercase());
            cur = ch.to_lowercase().to_string();
        } else {
            cur.push(ch);
        }
    }
    if !cur.is_empty() { tokens.push(cur.to_lowercase()); }
    tokens
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().to_string() + c.as_str(),
    }
}


