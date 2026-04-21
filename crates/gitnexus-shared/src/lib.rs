pub mod hash;
//! Shared types and utilities for GitNexus WASM
//!
//! This crate contains all data structures that cross the WASM/JS boundary,
//! ensuring type safety and zero-copy serialization where possible.

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

// ============================================================================
// Graph Node Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GraphNode {
    pub id: String,
    pub label: NodeLabel,
    pub properties: NodeProperties,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NodeLabel {
    File,
    Folder,
    Function,
    Class,
    Interface,
    Method,
    Struct,
    Enum,
    Trait,
    Impl,
    TypeAlias,
    Const,
    Static,
    Property,
    Record,
    Delegate,
    Annotation,
    Constructor,
    Template,
    Module,
    Route,
    Tool,
    Community,
    Process,
    CodeElement,
    Namespace,
    Macro,
    Union,
    Typedef,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NodeProperties {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heuristic_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cohesion: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry_point_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_keys: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_keys: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub middleware: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    // Method/function metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_exported: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameter_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_static: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_abstract: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_async: Option<bool>,
}

// ============================================================================
// Graph Relationship Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GraphRelationship {
    pub id: String,
    pub source_id: String,
    pub target_id: String,
    #[serde(rename = "type")]
    pub rel_type: RelationType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RelationType {
    Calls,
    Imports,
    Extends,
    Implements,
    HasMethod,
    HasProperty,
    MethodOverrides,
    Overrides,
    MethodImplements,
    Accesses,
    HandlesRoute,
    Fetches,
    HandlesTool,
    EntryPointOf,
    Wraps,
    MemberOf,
    StepInProcess,
    Defines,
    Contains,
}

// ============================================================================
// Analysis Pipeline Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineProgress {
    pub phase: String,
    pub percent: u8,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<PipelineStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineStats {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files_processed: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_files: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nodes_created: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edges_created: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisResult {
    pub repo_name: String,
    pub repo_path: String,
    pub indexed_at: String, // ISO 8601
    pub stats: IndexStats,
    pub community_count: u32,
    pub process_count: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexStats {
    pub files: u32,
    pub nodes: u32,
    pub edges: u32,
    pub communities: u32,
    pub clusters: u32,
    pub processes: u32,
}

// ============================================================================
// Search Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub node_id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub file_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_line: Option<u32>,
    pub score: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distance: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sources: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connections: Option<Connections>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cluster: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub processes: Option<Vec<ProcessRef>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Connections {
    pub outgoing: Vec<Connection>,
    pub incoming: Vec<Connection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    pub name: String,
    #[serde(rename = "type")]
    pub rel_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessRef {
    pub id: String,
    pub label: String,
    pub step: u32,
    pub step_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    pub query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<SearchMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchMode {
    Hybrid,
    Semantic,
    Bm25,
}

// ============================================================================
// Impact Analysis Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImpactResult {
    pub target: TargetSymbol,
    pub direction: Direction,
    pub impacted_count: u32,
    pub risk: RiskLevel,
    pub summary: ImpactSummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partial: Option<bool>,
    pub affected_processes: Vec<AffectedProcess>,
    pub affected_modules: Vec<AffectedModule>,
    pub by_depth: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetSymbol {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub symbol_type: String,
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Upstream,
    Downstream,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImpactSummary {
    pub direct: u32,
    pub processes_affected: u32,
    pub modules_affected: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AffectedProcess {
    pub name: String,
    #[serde(rename = "type")]
    pub entry_type: String,
    pub file_path: String,
    pub affected_process_count: u32,
    pub total_hits: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub earliest_broken_step: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AffectedModule {
    pub name: String,
    pub hits: u32,
    pub impact: String, // "direct" or "indirect"
}

// ============================================================================
// Context Tool Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextResult {
    pub status: String,
    pub symbol: SymbolDetail,
    pub incoming: serde_json::Map<String, serde_json::Value>,
    pub outgoing: serde_json::Map<String, serde_json::Value>,
    pub processes: Vec<ProcessRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SymbolDetail {
    pub uid: String,
    pub name: String,
    pub kind: String,
    pub file_path: String,
    pub start_line: u32,
    pub end_line: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method_metadata: Option<serde_json::Map<String, serde_json::Value>>,
}

// ============================================================================
// File System Types (Browser)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub path: String,
    pub name: String,
    pub is_directory: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoImport {
    pub name: String,
    pub files: Vec<FileEntry>,
    pub is_git_repo: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_branch: Option<String>,
}

// ============================================================================
// Embedding Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingProgress {
    pub phase: String,
    pub percent: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nodes_processed: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_nodes: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingConfig {
    pub batch_size: u32,
    pub chunk_size: u32,
    pub overlap: u32,
    pub model_name: String,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            batch_size: 32,
            chunk_size: 512,
            overlap: 64,
            model_name: "all-MiniLM-L6-v2".to_string(),
        }
    }
}

// ============================================================================
// WASM-JS Bridge Helpers
// ============================================================================

/// Convert a Rust Result to a JS-compatible result object
#[wasm_bindgen]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl JsResult {
    pub fn ok<T: Serialize>(data: &T) -> Self {
        Self {
            success: true,
            data: Some(serde_json::to_string(data).unwrap_or_default()),
            error: None,
        }
    }

    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(msg.into()),
        }
    }
}

/// Progress callback trait for WASM
#[wasm_bindgen]
pub struct ProgressCallback {
    #[wasm_bindgen(skip)]
    pub closure: js_sys::Function,
}

#[wasm_bindgen]
impl ProgressCallback {
    #[wasm_bindgen(constructor)]
    pub fn new(closure: js_sys::Function) -> Self {
        Self { closure }
    }

    pub fn call(&self, progress: &PipelineProgress) {
        let this = JsValue::NULL;
        let js_value = serde_wasm_bindgen::to_value(progress).unwrap_or(JsValue::NULL);
        let _ = self.closure.call1(&this, &js_value);
    }
}
