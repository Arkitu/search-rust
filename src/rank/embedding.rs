use std::path::PathBuf;
use crate::error::Result;
use rust_bert::pipelines::sentence_embeddings::{builder::SentenceEmbeddingsBuilder, SentenceEmbeddingsModel, SentenceEmbeddingsModelType::AllMiniLmL12V2};
use kdtree::KdTree;

pub struct Embedder<'a> {
    model: Option<SentenceEmbeddingsModel>,
    pub cache: KdTree<f32, &'a PathBuf, &'a [f32; 384]>,
    embeds: Vec<[f32; 384]>
}
impl<'a> Embedder<'a> {
    pub fn new() -> Result<Self> {
        Ok(Self {
            model: None,
            cache: KdTree::new(384),
            embeds: Vec::new()
        })
    }
    fn create_model(&mut self) -> Result<()> {
        self.model = Some(SentenceEmbeddingsBuilder::remote(AllMiniLmL12V2).create_model()?);
        Ok(())
    }
    pub fn embed_to_cache<S>(&mut self, sentences: &[S], path: &'a PathBuf) -> Result<()>
    where S: AsRef<str> + Sync {
        if let None = self.model {
            self.create_model()?;
        }
        let embeds = self.model.as_ref().unwrap().encode(sentences)?;
        let mut embeds: Vec<[f32; 384]> = embeds.into_iter().map(|embed|{
            embed.as_slice().try_into().unwrap()
        }).collect();

        self.embeds.append(&mut embeds);
        
        for embed in self.embeds[self.embeds.len() - sentences.len() +1..].iter() {
            self.cache.add(embed, path)?;
        }

        Ok(())
    }
    //pub fn embed<S>(&'a mut )
}
