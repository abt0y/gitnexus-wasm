//! GitNexus Tokenize — Lightweight BPE tokenizer for WASM
//!
//! Loads HuggingFace `tokenizer.json` and performs subword tokenization.

pub mod bpe;

use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use bpe::BpeProcessor;

#[wasm_bindgen]
pub struct CodeTokenizer {
    processor: BpeProcessor,
    max_length: usize,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Encoding {
    pub input_ids: Vec<u32>,
    pub attention_mask: Vec<u32>,
    pub token_type_ids: Vec<u32>,
}

#[wasm_bindgen]
impl CodeTokenizer {
    #[wasm_bindgen(constructor)]
    pub fn new(json_content: &str, max_length: usize) -> Result<CodeTokenizer, JsValue> {
        let processor = BpeProcessor::from_json(json_content)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        
        Ok(CodeTokenizer {
            processor,
            max_length,
        })
    }

    pub fn encode(&self, text: &str) -> Result<JsValue, JsValue> {
        let tokens = self.processor.tokenize(text);
        let mut ids = self.processor.tokens_to_ids(&tokens);

        // Truncate
        if ids.len() > self.max_length {
            ids.truncate(self.max_length);
        }

        let actual_len = ids.len();
        let attention_mask = vec![1u32; actual_len];
        let token_type_ids = vec![0u32; actual_len];

        let encoding = Encoding {
            input_ids: ids,
            attention_mask,
            token_type_ids,
        };

        serde_wasm_bindgen::to_value(&encoding)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn encode_batch(&self, texts: Vec<String>) -> Result<JsValue, JsValue> {
        let mut results = Vec::new();
        for text in texts {
            let tokens = self.processor.tokenize(&text);
            let mut ids = self.processor.tokens_to_ids(&tokens);
            if ids.len() > self.max_length {
                ids.truncate(self.max_length);
            }
            results.push(ids);
        }
        serde_wasm_bindgen::to_value(&results)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }
}
