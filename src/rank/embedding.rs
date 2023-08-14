use crate::error::Result;
use rust_bert::pipelines::sentence_embeddings::{builder::SentenceEmbeddingsBuilder, SentenceEmbeddingsModel, SentenceEmbeddingsModelType::AllMiniLmL12V2};

pub struct Embedder {
    model: SentenceEmbeddingsModel
}
impl Embedder {
    pub fn new() -> Result<Self> {
        Ok(Self {
            model: SentenceEmbeddingsBuilder::remote(AllMiniLmL12V2).create_model()?
        })
    }
    pub fn embed<S>(&self, sentences: &[S]) -> Result<Vec<Vec<f32>>>
    where S: AsRef<str> + Sync {
        Ok(self.model.encode(sentences)?)
    }
}
