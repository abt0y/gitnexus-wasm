//! Shared types and utilities for GitNexus WASM
//!
//! This crate contains all data structures that cross the WASM/JS boundary,
//! ensuring type safety and zero-copy serialization where possible.

pub mod hash;

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
    pub confidence: Option<f64>,
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
    Overrides,
    Accesses,
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
    pub stats: Option<PipelineStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineStats {
    pub files_processed: Option<u32>,
    pub total_files: Option<u32>,
    pub nodes_created: Option<u32>,
    pub edges_created: Option<u32>,
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
// Search & Impact Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    pub query: String,
    pub semantic: Option<bool>,
    pub embedding: Option<Vec<f32>>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub node_id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub file_path: String,
    pub start_line: Option<u32>,
    pub end_line: Option<u32>,
}

// ============================================================================
// WASM-JS Bridge Helpers
// ============================================================================

/// Result object for WASM calls
#[wasm_bindgen(getter_with_clone)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsResult {
    pub success: bool,
    pub data: Option<String>,
    pub error: Option<String>,
}

#[wasm_bindgen]
impl JsResult {
    pub fn ok(data_str: String) -> Self {
        Self {
            success: true,
            data: Some(data_str),
            error: None,
        }
    }

    pub fn err(msg: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(msg),
        }
    }
}

/// Helper for passing JSON back to JS
pub fn to_js_result<T: Serialize>(res: Result<T, String>) -> JsResult {
    match res {
        Ok(v) => JsResult::ok(serde_json::to_string(&v).unwrap_or_default()),
        Err(e) => JsResult::err(e),
    }
}

// Chunker remains largely the same
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextChunk {
    pub text: String,
    pub chunk_index: u32,
    pub start_word: usize,
    pub end_word: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub path: String,
    pub name: String,
    pub is_directory: bool,
    pub content: Option<String>,
    pub size: Option<u64>,
}
