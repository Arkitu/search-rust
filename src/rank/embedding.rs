use crate::error::Result;
use rust_bert::

pub struct Embedder {
    model: 
}
impl Embeder {
    pub fn new() -> Result<Self> {
        Self {
            model: SentenceEmbeddingsBuilder::remote(AllMiniLmL12V2).create_model()?
        }
    }
    pub fn embed(&self, sentences: &[&str]) {
        self.model.predict(sentences)
    }
}
