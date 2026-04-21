//! Minimal BPE implementation for gitnexus-tokenize
//!
//! Designed to load HuggingFace `tokenizer.json` and perform subword tokenization.

use std::collections::HashMap;
use serde::Deserialize;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TokenizerError {
    #[error("Failed to parse JSON: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("Invalid tokenizer format")]
    InvalidFormat,
}

pub struct BpeProcessor {
    vocab: HashMap<String, u32>,
    merges: HashMap<(String, String), u32>,
    unk_token: String,
    _special_tokens: HashMap<String, u32>,
}

#[derive(Deserialize)]
struct TokenizerJson {
    model: ModelJson,
    #[serde(default)]
    added_tokens: Vec<AddedToken>,
}

#[derive(Deserialize)]
struct ModelJson {
    #[serde(rename = "type")]
    _model_type: String, // "BPE" or "WordPiece"
    vocab: HashMap<String, u32>,
    merges: Option<Vec<String>>,
    unk_token: Option<String>,
}

#[derive(Deserialize)]
struct AddedToken {
    id: u32,
    content: String,
}

impl BpeProcessor {
    pub fn from_json(json: &str) -> Result<Self, TokenizerError> {
        let decoded: TokenizerJson = serde_json::from_str(json)?;
        
        let mut merges = HashMap::new();
        if let Some(m_list) = decoded.model.merges {
            for (idx, m) in m_list.iter().enumerate() {
                let parts: Vec<&str> = m.split(' ').collect();
                if parts.len() == 2 {
                    merges.insert((parts[0].to_string(), parts[1].to_string()), idx as u32);
                }
            }
        }

        let mut special_tokens = HashMap::new();
        for t in decoded.added_tokens {
            special_tokens.insert(t.content, t.id);
        }

        Ok(BpeProcessor {
            vocab: decoded.model.vocab,
            merges,
            unk_token: decoded.model.unk_token.unwrap_or_else(|| "[UNK]".to_string()),
            _special_tokens: special_tokens,
        })
    }

    pub fn tokenize(&self, text: &str) -> Vec<String> {
        // Simplified: Split by whitespace then apply BPE
        let mut result = Vec::new();
        for word in text.split_whitespace() {
            let mut subwords = self.bpe_word(word);
            result.append(&mut subwords);
        }
        result
    }

    fn bpe_word(&self, word: &str) -> Vec<String> {
        // Greedy BPE merge logic
        let mut symbols: Vec<String> = word.chars().map(|c| c.to_string()).collect();
        // Add Ġ prefix for some tokenizers if needed, but here we assume simple word split
        
        if symbols.is_empty() { return vec![]; }

        loop {
            let mut best_pair = None;
            let mut min_rank = u32::MAX;

            for i in 0..symbols.len() - 1 {
                let pair = (symbols[i].clone(), symbols[i+1].clone());
                if let Some(&rank) = self.merges.get(&pair) {
                    if rank < min_rank {
                        min_rank = rank;
                        best_pair = Some((i, pair));
                    }
                }
            }

            if let Some((i, pair)) = best_pair {
                let new_symbol = format!("{}{}", pair.0, pair.1);
                symbols[i] = new_symbol;
                symbols.remove(i + 1);
            } else {
                break;
            }
        }
        symbols
    }

    pub fn tokens_to_ids(&self, tokens: &[String]) -> Vec<u32> {
        let unk_id = *self.vocab.get(&self.unk_token).unwrap_or(&0u32);
        tokens.iter().map(|t| {
            self.vocab.get(t).cloned().unwrap_or(unk_id)
        }).collect()
    }
}
