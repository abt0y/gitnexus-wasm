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
use gitnexus_tokenize::{CodeTokenizer, Encoding};

// ============================================================================
// ONNX Runtime Web Bridge
// ============================================================================

/// Model configuration
const MODEL_URL: &str = "./assets/all-MiniLM-L6-v2-quantized.onnx";
const TOKENIZER_URL: &str = "./assets/tokenizer.json";
const EMBEDDING_DIM: usize = 384;
const MAX_SEQ_LENGTH: usize = 512;

#[wasm_bindgen]
pub struct EmbeddingEngine {
    #[wasm_bindgen(skip)]
    pub tokenizer: Option<CodeTokenizer>,
    #[wasm_bindgen(skip)]
    pub session: JsValue,       // ONNX InferenceSession
    ready: bool,
}

#[wasm_bindgen]
impl EmbeddingEngine {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            tokenizer: None,
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

        // Load tokenizer.json
        let tokenizer_json = self.fetch_asset(TOKENIZER_URL).await?;
        self.tokenizer = Some(CodeTokenizer::new(&tokenizer_json, MAX_SEQ_LENGTH)?);

        self.ready = true;
        info!("Embedding engine ready");

        Ok(())
    }

    async fn fetch_asset(&self, url: &str) -> Result<String, JsValue> {
        let mut opts = RequestInit::new();
        opts.method("GET");
        opts.mode(RequestMode::Cors);

        let request = Request::new_with_str_and_init(url, &opts)?;
        let window = web_sys::window().unwrap();
        let resp_value = wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&request)).await?;
        let resp: Response = resp_value.dyn_into().unwrap();

        if !resp.ok() {
            return Err(JsValue::from_str(&format!("Failed to fetch asset: {}", url)));
        }

        let text = wasm_bindgen_futures::JsFuture::from(resp.text()?).await?;
        Ok(text.as_string().unwrap_or_default())
    }

    async fn load_onnx_runtime(&self) -> Result<JsValue, JsValue> {
        let window = web_sys::window().ok_or("No window")?;
        let ort = Reflect::get(&window, &"ort".into())?;

        if !ort.is_undefined() {
            return Ok(ort);
        }

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

        let env = Reflect::get(&ort, &"env".into())?;
        let wasm = Reflect::get(&env, &"wasm".into())?;
        Reflect::set(&wasm, &"numThreads".into(), &JsValue::from_f64(1.0))?;
        Reflect::set(&wasm, &"simd".into(), &JsValue::from_bool(true))?;

        Ok(ort)
    }

    fn create_session_options(&self, ort: &JsValue) -> Result<JsValue, JsValue> {
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
        Ok(session)
    }

    /// Embed a single text
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>, JsValue> {
        if !self.ready {
            return Err(JsValue::from_str("Embedding engine not initialized"));
        }

        let tokenizer = self.tokenizer.as_ref().ok_or("Tokenizer not ready")?;
        let encoding_js = tokenizer.encode(text)?;
        let encoding: Encoding = serde_wasm_bindgen::from_value(encoding_js)?;

        let mut input_ids = vec![0i64; MAX_SEQ_LENGTH];
        let mut attention_mask = vec![0i64; MAX_SEQ_LENGTH];
        let mut token_type_ids = vec![0i64; MAX_SEQ_LENGTH];

        let len = encoding.input_ids.len().min(MAX_SEQ_LENGTH);
        for i in 0..len {
            input_ids[i] = encoding.input_ids[i] as i64;
            attention_mask[i] = encoding.attention_mask[i] as i64;
            token_type_ids[i] = encoding.token_type_ids[i] as i64;
        }

        // Run inference
        let run_method: js_sys::Function = Reflect::get(&self.session, &"run".into())?.dyn_into()?;
        let feeds = Object::new();

        let input_tensor = self.create_tensor(&input_ids, vec![1, MAX_SEQ_LENGTH as i64])?;
        let mask_tensor = self.create_tensor(&attention_mask, vec![1, MAX_SEQ_LENGTH as i64])?;
        let type_tensor = self.create_tensor(&token_type_ids, vec![1, MAX_SEQ_LENGTH as i64])?;

        Reflect::set(&feeds, &"input_ids".into(), &input_tensor)?;
        Reflect::set(&feeds, &"attention_mask".into(), &mask_tensor)?;
        Reflect::set(&feeds, &"token_type_ids".into(), &type_tensor)?;

        let promise: Promise = run_method.call1(&self.session, &feeds.into())?.dyn_into()?;
        let outputs = wasm_bindgen_futures::JsFuture::from(promise).await?;

        // Extract embeddings from output (usually last_hidden_state or pooler_output)
        let output_tensor = Reflect::get(&outputs, &"last_hidden_state".into())?;
        if output_tensor.is_undefined() {
             // Fallback for some models
             let fallback = Reflect::get(&outputs, &"output_0".into())?;
             return self.extract_embeddings(&fallback);
        }
        
        self.extract_embeddings(&output_tensor)
    }

    fn create_tensor(&self, data: &[i64], shape: Vec<i64>) -> Result<JsValue, JsValue> {
        let window = web_sys::window().unwrap();
        let ort = Reflect::get(&window, &"ort".into())?;
        let tensor_class: js_sys::Function = Reflect::get(&ort, &"Tensor".into())?.dyn_into()?;

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
        let data_val = Reflect::get(tensor, &"data".into())?;
        let float_array = js_sys::Float32Array::from(&data_val);

        let mut embeddings = Vec::with_capacity(EMBEDDING_DIM);
        // Average pooling if it's (1, seq, dim)
        // For simplicity, we just take the [CLS] token at index 0 if it's the full state
        for i in 0..EMBEDDING_DIM {
            embeddings.push(float_array.get_index(i as u32));
        }

        // Normalize
        let norm: f32 = embeddings.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for val in &mut embeddings {
                *val /= norm;
            }
        }

        Ok(embeddings)
    }

    pub async fn embed_batch(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, JsValue> {
        let mut results = Vec::new();
        for text in texts {
            let embedding = self.embed(&text).await?;
            results.push(embedding);
        }
        Ok(results)
    }

    pub fn is_ready(&self) -> bool {
        self.ready
    }

    pub fn dimension(&self) -> usize {
        EMBEDDING_DIM
    }
}

// Chunker remains largely the same
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
            let content = node.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let name = node.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let label = node.get("label").and_then(|v| v.as_str()).unwrap_or("CodeElement");

            let embed_text = format!("{} {} {}", label, name, content);
            let chunks = self.chunker.chunk(&embed_text);

            for (chunk_idx, chunk) in chunks.iter().enumerate() {
                let _embedding = self.engine.embed(&chunk.text).await?;
                // TODO: Store in graph
            }

            processed += 1;
            let progress = EmbeddingProgress {
                phase: "embedding".to_string(),
                percent: ((processed as f32 / total as f32) * 100.0) as u8,
                nodes_processed: Some(processed as u32),
                total_nodes: Some(total as u32),
                error: None,
            };
            let _ = callback.call1(&JsValue::NULL, &serde_wasm_bindgen::to_value(&progress).unwrap());
        }

        Ok(JsResult::ok(&serde_json::json!({"processed": processed})))
    }

    pub fn is_ready(&self) -> bool {
        self.engine.is_ready()
    }
}
