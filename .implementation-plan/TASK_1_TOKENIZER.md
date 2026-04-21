# Task 1: Real Tokenizer — Implementation Guide

**Priority**: P0 (Critical Path)  
**Estimated Effort**: 2 weeks  
**Skill Level**: Expert (Rust tokenization, WASM FFI)  
**Dependencies**: None  
**Blocks**: Task 5 (Semantic Search)

---

## Problem Statement

Current implementation uses a **fake tokenizer** that hashes words to generate input IDs:

```rust
// BAD — gitnexus-embed/src/lib.rs (current)
fn tokens_to_input_ids(&self, tokens: &[String]) -> Vec<i64> {
    tokens.iter().map(|t| {
        let hash = t.bytes().fold(0u32, |acc, b| {
            acc.wrapping_mul(31).wrapping_add(b as u32)
        });
        (hash % 30000) as i64
    }).collect()
}
```

This produces **random embeddings** with no semantic meaning. We need a real **BPE (Byte-Pair Encoding)** or **WordPiece** tokenizer that matches HuggingFace's `transformers` exactly.

---

## Solution Approaches

### Option A: Port `tokenizers` crate to WASM (RECOMMENDED)

The HuggingFace `tokenizers` crate is the gold standard. It supports BPE, WordPiece, and Unigram tokenization.

**Pros**:
- 100% compatible with HuggingFace models
- Fast Rust implementation
- Supports all special tokens, truncation, padding

**Cons**:
- Uses `onig` regex library which may not compile to WASM
- May need patches for `getrandom` (WASM entropy)

**Implementation**:

```toml
# crates/gitnexus-tokenize/Cargo.toml
[package]
name = "gitnexus-tokenize"
version = "0.1.0"
edition = "2021"

[dependencies]
tokenizers = { version = "0.15", default-features = false, features = ["unstable_wasm"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
wasm-bindgen = "0.2"
js-sys = "0.3"
```

```rust
// crates/gitnexus-tokenize/src/lib.rs
use tokenizers::Tokenizer;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct WasmTokenizer {
    tokenizer: Tokenizer,
}

#[wasm_bindgen]
impl WasmTokenizer {
    #[wasm_bindgen(constructor)]
    pub fn from_json(json: &str) -> Result<WasmTokenizer, JsValue> {
        let tokenizer = Tokenizer::from_str(json)
            .map_err(|e| JsValue::from_str(&format!("Failed to load tokenizer: {}", e)))?;

        Ok(Self { tokenizer })
    }

    pub fn encode(&self, text: &str, add_special_tokens: bool) -> Result<JsValue, JsValue> {
        let encoding = self.tokenizer.encode(text, add_special_tokens)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        let result = serde_json::json!({
            "ids": encoding.get_ids(),
            "attention_mask": encoding.get_attention_mask(),
            "type_ids": encoding.get_type_ids(),
            "tokens": encoding.get_tokens(),
            "word_ids": encoding.get_word_ids(),
            "special_tokens_mask": encoding.get_special_tokens_mask(),
            "offset": encoding.get_offsets(),
            "overflowing": encoding.get_overflowing().len(),
        });

        serde_wasm_bindgen::to_value(&result)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn encode_batch(&self, texts: Vec<String>, add_special_tokens: bool) -> Result<JsValue, JsValue> {
        let encodings = self.tokenizer.encode_batch(texts, add_special_tokens)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        let results: Vec<_> = encodings.iter().map(|e| serde_json::json!({
            "ids": e.get_ids(),
            "attention_mask": e.get_attention_mask(),
            "type_ids": e.get_type_ids(),
        })).collect();

        serde_wasm_bindgen::to_value(&results)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn decode(&self, ids: Vec<u32>, skip_special_tokens: bool) -> Result<String, JsValue> {
        self.tokenizer.decode(&ids, skip_special_tokens)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn vocab_size(&self) -> usize {
        self.tokenizer.get_vocab_size(true)
    }
}
```

**WASM Compatibility Patches**:

The `tokenizers` crate may fail to compile for WASM due to:
1. `onig` regex (C dependency) → Replace with `regex` crate
2. `getrandom` → Enable `wasm-bindgen` feature: `getrandom = { version = "0.2", features = ["js"] }`
3. `rayon` (parallelism) → Disable: `default-features = false`

Create a patch in `Cargo.toml`:
```toml
[patch.crates-io]
tokenizers = { git = "https://github.com/huggingface/tokenizers", branch = "main" }
```

Or fork and patch:
```bash
git clone https://github.com/huggingface/tokenizers.git
cd tokenizers/tokenizers
# Edit Cargo.toml: replace onig with regex, disable rayon
# Edit src/utils/mod.rs: conditionally compile regex backend
```

### Option B: Custom Minimal BPE in Rust

If `tokenizers` crate is too heavy or incompatible.

**Pros**:
- Full control, no dependencies
- Tiny WASM binary (<100KB)
- No FFI issues

**Cons**:
- Must implement BPE merge algorithm
- Must load vocab from JSON manually
- No truncation/padding helpers

**Implementation Sketch**:

```rust
pub struct BpeTokenizer {
    vocab: HashMap<String, u32>,
    merges: Vec<(String, String)>,
    special_tokens: HashMap<String, u32>,
}

impl BpeTokenizer {
    pub fn from_huggingface_json(json: &str) -> Result<Self, Error> {
        // Parse tokenizer.json format
        // Extract vocab, merges, special_tokens
    }

    pub fn encode(&self, text: &str) -> Vec<u32> {
        // 1. Pre-tokenize (Unicode regex split)
        // 2. Byte-level BPE for each word
        // 3. Apply merges in order
        // 4. Map tokens to IDs
        // 5. Add [CLS] and [SEP]
    }

    fn bpe_word(&self, word: &str) -> Vec<String> {
        // Start with chars as tokens
        // Greedily apply merges
        // Return final token sequence
    }
}
```

Reference: [GPT-2 BPE implementation](https://github.com/openai/gpt-2/blob/master/src/encoder.py) (Python, port to Rust)

### Option C: JS Tokenizer via wasm-bindgen (Fallback)

Use `@xenova/transformers` tokenizer in JS, pass IDs to Rust.

**Pros**:
- Guaranteed working
- HuggingFace maintains it

**Cons**:
- Extra JS dependency
- Memory copy overhead JS→WASM
- Not self-contained

---

## Step-by-Step Implementation

### Step 1: Create `gitnexus-tokenize` crate (Day 1-2)

```bash
cd crates
cargo new --lib gitnexus-tokenize
cd gitnexus-tokenize
```

Add to workspace `Cargo.toml`:
```toml
members = [
    ...
    "crates/gitnexus-tokenize",
]
```

### Step 2: Download tokenizer.json (Day 2)

```bash
# From HuggingFace
wget https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/tokenizer.json   -O web/public/assets/tokenizer.json

# Verify it's BPE (not WordPiece)
python3 -c "import json; t=json.load(open('tokenizer.json')); print(t['model']['type'])"
# Expected: "BPE"
```

### Step 3: Implement tokenizer (Day 3-5)

Choose Option A or B. Implement with tests.

**Unit Tests**:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_simple() {
        let tokenizer = load_test_tokenizer();
        let ids = tokenizer.encode("Hello world", true);
        assert_eq!(ids.len(), 4); // [CLS] hello world [SEP]
        assert_eq!(ids[0], 101); // [CLS] ID
        assert_eq!(ids[ids.len()-1], 102); // [SEP] ID
    }

    #[test]
    fn test_encode_code() {
        let tokenizer = load_test_tokenizer();
        let ids = tokenizer.encode("function foo() { return 42; }", true);
        assert!(!ids.is_empty());
        // Verify no unknown tokens (all IDs < vocab_size)
        for id in &ids {
            assert!(*id < tokenizer.vocab_size() as u32);
        }
    }

    #[test]
    fn test_matches_python() {
        // Compare with transformers.AutoTokenizer
        let rust_ids = tokenizer.encode("def calculate(x): return x * 2", true);
        let python_ids = [101, 2258, 12345, ...]; // From Python script
        assert_eq!(rust_ids, python_ids);
    }
}
```

### Step 4: Integrate with Embedding Engine (Day 6-8)

Modify `gitnexus-embed/src/lib.rs`:

```rust
use gitnexus_tokenize::WasmTokenizer;

pub struct EmbeddingEngine {
    tokenizer: Option<WasmTokenizer>,
    session: JsValue,
    ready: bool,
}

pub async fn init(&mut self, model_url: Option<String>, tokenizer_url: Option<String>) -> Result<(), JsValue> {
    // Load tokenizer
    let tokenizer_json = fetch_tokenizer_json(tokenizer_url.unwrap_or("./assets/tokenizer.json")).await?;
    self.tokenizer = Some(WasmTokenizer::from_json(&tokenizer_json)?);

    // Load ONNX session (existing code)
    ...

    self.ready = true;
    Ok(())
}

pub async fn embed(&self, text: &str) -> Result<Vec<f32>, JsValue> {
    let tokenizer = self.tokenizer.as_ref()
        .ok_or_else(|| JsValue::from_str("Tokenizer not loaded"))?;

    let encoding = tokenizer.encode(text, true)?;
    let encoding_obj: serde_json::Value = serde_wasm_bindgen::from_value(encoding)?;

    let input_ids: Vec<i64> = encoding_obj["ids"].as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_i64().unwrap())
        .collect();

    let attention_mask: Vec<i64> = encoding_obj["attention_mask"].as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_i64().unwrap())
        .collect();

    // Create tensors and run ONNX (existing code)
    let input_tensor = self.create_tensor(&input_ids, vec![1, input_ids.len() as i64])?;
    let mask_tensor = self.create_tensor(&attention_mask, vec![1, attention_mask.len() as i64])?;

    // ... run inference ...
}
```

### Step 5: Handle Truncation (Day 9)

Tokenizer must truncate to model's max length (512 for MiniLM):

```rust
// In WasmTokenizer::encode
let mut encoding = self.tokenizer.encode(text, add_special_tokens)?;

if encoding.get_ids().len() > MAX_LENGTH {
    // Truncate from the end
    let mut ids = encoding.get_ids().to_vec();
    ids.truncate(MAX_LENGTH - 1);
    ids.push(self.special_tokens["[SEP]"]); // Ensure [SEP] at end
    encoding = self.tokenizer.encode_with_offsets(
        &self.decode(&ids, true)?, 
        add_special_tokens
    )?;
}
```

### Step 6: Batch Processing (Day 10-12)

```rust
pub async fn embed_batch(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, JsValue> {
    let tokenizer = self.tokenizer.as_ref().unwrap();

    // Encode all texts
    let encodings = tokenizer.encode_batch(texts, true)?;

    // Find max length for padding
    let max_len = encodings.iter()
        .map(|e| e.get_ids().len())
        .max()
        .unwrap_or(0);

    // Pad all to max_len
    let mut batch_input_ids = Vec::new();
    let mut batch_attention_mask = Vec::new();

    for enc in &encodings {
        let mut ids = enc.get_ids().to_vec();
        let mut mask = enc.get_attention_mask().to_vec();

        // Pad
        while ids.len() < max_len {
            ids.push(0); // [PAD] token ID
            mask.push(0);
        }

        batch_input_ids.extend(ids.into_iter().map(|i| i as i64));
        batch_attention_mask.extend(mask.into_iter().map(|i| i as i64));
    }

    // Create batch tensor [batch_size, max_len]
    let batch_size = encodings.len() as i64;
    let input_tensor = self.create_tensor(&batch_input_ids, vec![batch_size, max_len as i64])?;
    let mask_tensor = self.create_tensor(&batch_attention_mask, vec![batch_size, max_len as i64])?;

    // Run batch inference
    let outputs = self.run_onnx(vec![input_tensor, mask_tensor]).await?;

    // Extract embeddings for each item
    self.extract_batch_embeddings(&outputs, batch_size as usize)
}
```

### Step 7: Testing & Validation (Day 13-14)

**Create validation script**:

```python
# scripts/validate_tokenizer.py
from transformers import AutoTokenizer
import json

# Load same tokenizer.json
hf_tokenizer = AutoTokenizer.from_pretrained("sentence-transformers/all-MiniLM-L6-v2")

# Test cases
test_cases = [
    "Hello world",
    "function foo() { return 42; }",
    "class UserService { async getUser(id: string) { ... } }",
    "import { useState } from 'react'",
    "def calculate(x: int) -> int: return x * 2",
    "// TODO: fix this bug",
    "SELECT * FROM users WHERE id = 1",
]

results = []
for text in test_cases:
    hf_ids = hf_tokenizer.encode(text, add_special_tokens=True)
    results.append({"text": text, "expected_ids": hf_ids})

with open("test/tokenizer_test_cases.json", "w") as f:
    json.dump(results, f, indent=2)
```

**Rust test**:
```rust
#[wasm_bindgen_test]
async fn test_tokenizer_matches_huggingface() {
    let tokenizer = load_tokenizer().await;
    let test_cases = include_str!("../../test/tokenizer_test_cases.json");

    for case in test_cases {
        let rust_encoding = tokenizer.encode(&case.text, true).unwrap();
        let rust_ids: Vec<u32> = serde_json::from_value(rust_encoding).unwrap();

        assert_eq!(rust_ids, case.expected_ids, 
            "Mismatch for text: {}", case.text);
    }
}
```

---

## Acceptance Criteria

- [ ] Tokenizer loads `tokenizer.json` from URL in <500ms
- [ ] Encoding "function foo() { return 42; }" produces valid IDs (all < vocab_size)
- [ ] 100% match with HuggingFace `AutoTokenizer` on 100 test strings
- [ ] Truncation works: "a " * 1000 → exactly 512 tokens
- [ ] Padding works: batch of ["hi", "hello world"] → both 512 tokens with attention_mask
- [ ] Batch encoding of 32 texts in <100ms
- [ ] WASM binary increase <500KB (tokenizer only)
- [ ] Embeddings produce cosine similarity >0.7 for similar code snippets

---

## Deliverables

1. `crates/gitnexus-tokenize/` — New crate
2. `crates/gitnexus-embed/src/lib.rs` — Modified (integrated tokenizer)
3. `web/public/assets/tokenizer.json` — Downloaded from HuggingFace
4. `test/tokenizer_test_cases.json` — Validation data
5. `docs/ADR/001-tokenizer.md` — Architecture decision record

---

## References

- [HuggingFace Tokenizers](https://github.com/huggingface/tokenizers)
- [BPE Paper](https://arxiv.org/abs/1508.07909)
- [GPT-2 Encoder](https://github.com/openai/gpt-2/blob/master/src/encoder.py)
- [WASM tokenizers issue](https://github.com/huggingface/tokenizers/issues/991)
- [ONNX Runtime Web Tokenizer Example](https://github.com/microsoft/onnxruntime-inference-examples/tree/main/js/chat)
