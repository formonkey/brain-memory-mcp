use anyhow::Result;
use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};
use std::env;

#[cfg(test)]
use sha2::{Digest, Sha256};

pub trait Embedder: Send {
    fn name(&self) -> &str;
    fn embed(&mut self, texts: &[String], input: EmbeddingInput) -> Result<Vec<Vec<f32>>>;
}

#[derive(Debug, Clone, Copy)]
pub enum EmbeddingInput {
    Passage,
    Query,
}

pub struct FastEmbedder {
    model_name: String,
    model: TextEmbedding,
}

impl FastEmbedder {
    pub fn new() -> Result<Self> {
        let model_name = env::var("BRAIN_MEMORY_MODEL")
            .unwrap_or_else(|_| "intfloat/multilingual-e5-small".to_string());
        let model = model_name
            .parse::<EmbeddingModel>()
            .unwrap_or(EmbeddingModel::MultilingualE5Small);
        let model = TextEmbedding::try_new(
            TextInitOptions::new(model)
                .with_show_download_progress(false)
                .with_intra_threads(4),
        )?;

        Ok(Self { model_name, model })
    }
}

impl Embedder for FastEmbedder {
    fn name(&self) -> &str {
        &self.model_name
    }

    fn embed(&mut self, texts: &[String], input: EmbeddingInput) -> Result<Vec<Vec<f32>>> {
        let prefixed: Vec<String> = texts
            .iter()
            .map(|text| match input {
                EmbeddingInput::Passage => format!("passage: {text}"),
                EmbeddingInput::Query => format!("query: {text}"),
            })
            .collect();
        let refs: Vec<&str> = prefixed.iter().map(String::as_str).collect();
        Ok(self.model.embed(refs, None)?)
    }
}

#[cfg(test)]
pub struct HashEmbedder {
    dimensions: usize,
    name: String,
}

#[cfg(test)]
impl HashEmbedder {
    pub fn new(dimensions: usize) -> Self {
        Self {
            dimensions,
            name: "hash-test-embedder".to_string(),
        }
    }
}

#[cfg(test)]
impl Embedder for HashEmbedder {
    fn name(&self) -> &str {
        &self.name
    }

    fn embed(&mut self, texts: &[String], _input: EmbeddingInput) -> Result<Vec<Vec<f32>>> {
        Ok(texts.iter().map(|text| self.embed_one(text)).collect())
    }
}

#[cfg(test)]
impl HashEmbedder {
    fn embed_one(&self, text: &str) -> Vec<f32> {
        let mut vector = vec![0.0; self.dimensions];
        for token in text.split_whitespace().map(|token| token.to_lowercase()) {
            let digest = Sha256::digest(token.as_bytes());
            let index = u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]) as usize
                % self.dimensions;
            vector[index] += 1.0;
        }
        normalize(vector)
    }
}

#[cfg(test)]
fn normalize(mut vector: Vec<f32>) -> Vec<f32> {
    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in &mut vector {
            *value /= norm;
        }
    }
    vector
}
