//! ONNX Runtime WASM embeddings for GitNexus
//!
//! Provides semantic text embeddings using ONNX Runtime Web.
//! Uses all-MiniLM-L6-v2 (384-dim) or similar lightweight models.

use wasm_bindgen::prelude::*;
use js_sys::{Function, Promise, Reflect, Array, Float32Array};
use web_sys::{console, Request, RequestInit, RequestMode, Response};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use once_cell::sync::OnceCell;
use log::{info, warn, error};

use gitnexus_shared::*;

// ============================================================================
// ONNX Runtime Web Bridge
// ============================================================================

/// Global ONNX Runtime instance
static ONNX_RUNTIME: OnceCell<JsValue> = OnceCell::new();

/// Global embedding session
static EMBEDDING_SESSION: OnceCell<JsValue> = OnceCell::new();

/// Model configuration
const MODEL_URL: &str = "./assets/all-MiniLM-L6-v2-quantized.onnx";
const EMBEDDING_DIM: usize = 384;
const MAX_SEQ_LENGTH: usize = 512;

#[wasm_bindgen]
pub struct EmbeddingEngine {
    tokenizer: JsValue,     // Tokenizer instance
    session: JsValue,       // ONNX InferenceSession
    ready: bool,
}

#[wasm_bindgen]
impl EmbeddingEngine {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            tokenizer: JsValue::NULL,
            session: JsValue::NULL,
            ready: false,
        }
    }

    /// Initialize the embedding engine
    pub async fn init(&mut self, model_url: Option<String>) -> Result<(), JsValue> {
        console_error_panic_hook::set_once();

        let url = model_url.unwrap_or_else(|| MODEL_URL.to_string());
        info!("Initializing embedding engine with model: {}", url);

        // Load ONNX Runtime
        let ort = self.load_onnx_runtime().await?;

        // Create inference session
        let session_options = self.create_session_options(&ort)?;
        let session = self.create_session(&ort, &url, session_options).await?;
        self.session = session;

        // Load tokenizer (simplified - in production use tokenizers-rs)
        self.tokenizer = self.create_simple_tokenizer();

        self.ready = true;
        info!("Embedding engine ready");

        Ok(())
    }

    async fn load_onnx_runtime(&self) -> Result<JsValue, JsValue> {
        // Check if ort is already loaded globally
        let window = web_sys::window().ok_or("No window")?;
        let ort = Reflect::get(&window, &"ort".into())?;

        if !ort.is_undefined() {
            info!("ONNX Runtime already loaded");
            return Ok(ort);
        }

        // Load ONNX Runtime script dynamically
        let document = window.document().ok_or("No document")?;
        let script = document.create_element("script")?;
        script.set_attribute("src", "https://cdn.jsdelivr.net/npm/onnxruntime-web@1.17.0/dist/ort.min.js")?;
        script.set_attribute("crossorigin", "anonymous")?;

        let promise = Promise::new(&mut |resolve, _reject| {
            let closure = Closure::once_into_js(move || {
                resolve.call0(&JsValue::NULL).unwrap_or(JsValue::NULL);
            });
            let _ = Reflect::set(&script, &"onload".into(), &closure);
        });

        document.head().unwrap().append_child(&script)?;
        wasm_bindgen_futures::JsFuture::from(promise).await?;

        let ort = Reflect::get(&window, &"ort".into())?;
        if ort.is_undefined() {
            return Err(JsValue::from_str("Failed to load ONNX Runtime"));
        }

        // Configure WASM backend
        let env = Reflect::get(&ort, &"env".into())?;
        let wasm = Reflect::get(&env, &"wasm".into())?;
        Reflect::set(&wasm, &"numThreads".into(), &JsValue::from_f64(1.0))?; // Single thread for WASM
        Reflect::set(&wasm, &"simd".into(), &JsValue::from_bool(true))?;

        info!("ONNX Runtime loaded successfully");
        Ok(ort)
    }

    fn create_session_options(&self, ort: &JsValue) -> Result<JsValue, JsValue> {
        let session_options_class: js_sys::Function = Reflect::get(ort, &"Session".into())?.dyn_into()?;
        let session_options = Reflect::get(&session_options_class, &"prototype".into())?;

        // Create execution providers - prefer WASM, fallback to CPU
        let eps = Array::new();
        eps.push(&"wasm".into());

        let options = Object::new();
        Reflect::set(&options, &"executionProviders".into(), &eps)?;
        Reflect::set(&options, &"graphOptimizationLevel".into(), &"all".into())?;

        Ok(options.into())
    }

    async fn create_session(&self, ort: &JsValue, model_url: &str, options: JsValue) -> Result<JsValue, JsValue> {
        let inference_session: js_sys::Function = Reflect::get(ort, &"InferenceSession".into())?.dyn_into()?;
        let create_method: js_sys::Function = Reflect::get(&inference_session, &"create".into())?.dyn_into()?;

        let model_url_js = JsValue::from_str(model_url);
        let promise: Promise = create_method.call2(&JsValue::NULL, &model_url_js, &options)?.dyn_into()?;

        let session = wasm_bindgen_futures::JsFuture::from(promise).await?;
        info!("ONNX session created");

        Ok(session)
    }

    fn create_simple_tokenizer(&self) -> JsValue {
        // Simplified tokenizer - in production, use a proper BPE/WordPiece tokenizer
        // For now, we use basic whitespace + punctuation splitting
        Object::new().into()
    }

    /// Tokenize text (simplified)
    fn tokenize(&self, text: &str) -> Vec<String> {
        // Basic whitespace tokenization with lowercase
        text.to_lowercase()
            .replace(|c: char| !c.is_alphanumeric() && c != ''', " ")
            .split_whitespace()
            .map(|s| s.to_string())
            .collect()
    }

    /// Convert tokens to input IDs (simplified - no vocab, just hash-based)
    fn tokens_to_input_ids(&self, tokens: &[String]) -> Vec<i64> {
        tokens.iter()
            .map(|t| {
                // Simple hash-based ID (not real tokenization)
                // In production, use a real tokenizer vocabulary
                let hash = t.bytes().fold(0u32, |acc, b| {
                    acc.wrapping_mul(31).wrapping_add(b as u32)
                });
                (hash % 30000) as i64 // Clamp to vocab size
            })
            .collect()
    }

    /// Embed a single text
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>, JsValue> {
        if !self.ready {
            return Err(JsValue::from_str("Embedding engine not initialized"));
        }

        let tokens = self.tokenize(text);
        let input_ids = self.tokens_to_input_ids(&tokens);

        // Create input tensor
        let input_ids_array = Array::new();
        for id in &input_ids {
            input_ids_array.push(&JsValue::from_f64(*id as f64));
        }

        // Pad or truncate to MAX_SEQ_LENGTH
        let actual_length = input_ids.len().min(MAX_SEQ_LENGTH);
        let mut padded_ids = vec![0i64; MAX_SEQ_LENGTH];
        padded_ids[..actual_length].copy_from_slice(&input_ids[..actual_length]);

        // Create attention mask
        let mut attention_mask = vec![0i64; MAX_SEQ_LENGTH];
        attention_mask[..actual_length].fill(1);

        // Create tensor data as Float32Array
        let input_data = Float32Array::new_with_length((MAX_SEQ_LENGTH * 3) as u32);
        // In reality, you'd create proper tensor objects here
        // This is a simplified placeholder

        // Run inference
        let run_method: js_sys::Function = Reflect::get(&self.session, &"run".into())?.dyn_into()?;
        let feeds = Object::new();

        // Create actual tensor inputs
        let input_tensor = self.create_tensor(&padded_ids, vec![1, MAX_SEQ_LENGTH as i64])?;
        let mask_tensor = self.create_tensor(&attention_mask, vec![1, MAX_SEQ_LENGTH as i64])?;

        Reflect::set(&feeds, &"input_ids".into(), &input_tensor)?;
        Reflect::set(&feeds, &"attention_mask".into(), &mask_tensor)?;

        let promise: Promise = run_method.call1(&self.session, &feeds.into())?.dyn_into()?;
        let outputs = wasm_bindgen_futures::JsFuture::from(promise).await?;

        // Extract embeddings from output
        let output_tensor = Reflect::get(&outputs, &"last_hidden_state".into())?;
        let embeddings = self.extract_embeddings(&output_tensor)?;

        Ok(embeddings)
    }

    fn create_tensor(&self, data: &[i64], shape: Vec<i64>) -> Result<JsValue, JsValue> {
        let window = web_sys::window().ok_or("No window")?;
        let ort = Reflect::get(&window, &"ort".into())?;
        let tensor_class: js_sys::Function = Reflect::get(&ort, &"Tensor".into())?.dyn_into()?;

        // Convert i64 data to BigInt64Array for int64 tensor
        let bigint_array = js_sys::BigInt64Array::new_with_length(data.len() as u32);
        for (i, &val) in data.iter().enumerate() {
            bigint_array.set_index(i as u32, js_sys::BigInt::from(val));
        }

        let shape_array = Array::new();
        for dim in &shape {
            shape_array.push(&JsValue::from_f64(*dim as f64));
        }

        let tensor = tensor_class.new2(&"int64".into(), &bigint_array.into(), &shape_array.into())?;
        Ok(tensor)
    }

    fn extract_embeddings(&self, tensor: &JsValue) -> Result<Vec<f32>, JsValue> {
        // Get the data from the tensor
        let data_method: js_sys::Function = Reflect::get(tensor, &"data".into())?.dyn_into()?;
        let data_promise: Promise = data_method.call0(tensor)?.dyn_into()?;

        // For synchronous extraction (in WASM, data() returns immediately for CPU tensors)
        let data = data_promise; // Simplified - in reality this might be async
        let float_array = js_sys::Float32Array::from(&data);

        let mut embeddings = Vec::with_capacity(EMBEDDING_DIM);
        for i in 0..EMBEDDING_DIM.min(float_array.length() as usize) {
            embeddings.push(float_array.get_index(i as u32));
        }

        // Normalize embeddings (L2 norm)
        let norm: f32 = embeddings.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for val in &mut embeddings {
                *val /= norm;
            }
        }

        Ok(embeddings)
    }

    /// Embed multiple texts in batch
    pub async fn embed_batch(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, JsValue> {
        let mut results = Vec::new();
        for text in texts {
            let embedding = self.embed(&text).await?;
            results.push(embedding);
        }
        Ok(results)
    }

    /// Check if engine is ready
    pub fn is_ready(&self) -> bool {
        self.ready
    }

    /// Get embedding dimension
    pub fn dimension(&self) -> usize {
        EMBEDDING_DIM
    }
}

// ============================================================================
// Text Chunking for Long Documents
// ============================================================================

#[wasm_bindgen]
pub struct TextChunker {
    chunk_size: usize,
    overlap: usize,
}

#[wasm_bindgen]
impl TextChunker {
    #[wasm_bindgen(constructor)]
    pub fn new(chunk_size: usize, overlap: usize) -> Self {
        Self { chunk_size, overlap }
    }

    /// Chunk text into overlapping segments
    pub fn chunk(&self, text: &str) -> Vec<TextChunk> {
        let words: Vec<&str> = text.split_whitespace().collect();
        let mut chunks = Vec::new();
        let mut start = 0;
        let mut chunk_index = 0;

        while start < words.len() {
            let end = (start + self.chunk_size).min(words.len());
            let chunk_text = words[start..end].join(" ");

            chunks.push(TextChunk {
                text: chunk_text,
                chunk_index,
                start_word: start,
                end_word: end,
            });

            if end >= words.len() {
                break;
            }

            start = end.saturating_sub(self.overlap);
            chunk_index += 1;
        }

        chunks
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextChunk {
    pub text: String,
    pub chunk_index: u32,
    pub start_word: usize,
    pub end_word: usize,
}

// ============================================================================
// Embedding Pipeline
// ============================================================================

#[wasm_bindgen]
pub struct EmbeddingPipeline {
    engine: EmbeddingEngine,
    chunker: TextChunker,
}

#[wasm_bindgen]
impl EmbeddingPipeline {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            engine: EmbeddingEngine::new(),
            chunker: TextChunker::new(512, 64),
        }
    }

    pub async fn init(&mut self, model_url: Option<String>) -> Result<(), JsValue> {
        self.engine.init(model_url).await
    }

    /// Generate embeddings for code nodes and store in graph
    pub async fn embed_nodes(
        &self,
        nodes: JsValue,
        progress_callback: JsValue,
    ) -> Result<JsResult, JsValue> {
        let node_list: Vec<HashMap<String, serde_json::Value>> = serde_wasm_bindgen::from_value(nodes)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        let callback: js_sys::Function = progress_callback.dyn_into()?;
        let total = node_list.len();
        let mut processed = 0;

        for node in &node_list {
            let content = node.get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let name = node.get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let label = node.get("label")
                .and_then(|v| v.as_str())
                .unwrap_or("CodeElement");

            // Generate embedding text with metadata
            let embed_text = format!("{} {} {}", label, name, content);

            // Chunk if too long
            let chunks = self.chunker.chunk(&embed_text);

            for (chunk_idx, chunk) in chunks.iter().enumerate() {
                let embedding = self.engine.embed(&chunk.text).await?;

                // Store in graph (would call graph DB here)
                // For now, just track progress
            }

            processed += 1;

            // Report progress
            let progress = EmbeddingProgress {
                phase: "embedding".to_string(),
                percent: ((processed as f32 / total as f32) * 100.0) as u8,
                nodes_processed: Some(processed as u32),
                total_nodes: Some(total as u32),
                error: None,
            };

            let js_progress = serde_wasm_bindgen::to_value(&progress).unwrap_or(JsValue::NULL);
            let _ = callback.call1(&JsValue::NULL, &js_progress);
        }

        Ok(JsResult::ok(&serde_json::json!({"processed": processed, "total": total})))
    }

    pub fn is_ready(&self) -> bool {
        self.engine.is_ready()
    }
}
