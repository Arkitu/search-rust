use std::{path::{PathBuf, Path}, sync::RwLock, rc::Rc};
use crate::error::Result;
use rust_bert::pipelines::sentence_embeddings::{builder::SentenceEmbeddingsBuilder, SentenceEmbeddingsModel, SentenceEmbeddingsModelType::AllMiniLmL12V2};
use kdtree::{KdTree, distance::squared_euclidean};

use super::RankResult;

pub type EmbedderCache = KdTree<f32, PathBuf, Rc<[f32]>>;

pub struct Embedder {
    model: Option<SentenceEmbeddingsModel>,
    cache: EmbedderCache
}
impl Embedder {
    pub fn new() -> Result<Self> {
        Ok(Self {
            model: None,
            cache: KdTree::new(384)
        })
    }
    fn create_model(&mut self) -> Result<()> {
        self.model = Some(SentenceEmbeddingsBuilder::remote(AllMiniLmL12V2).create_model()?);
        Ok(())
    }
    fn create_model_if_not_exists(&mut self) -> Result<()> {
        if let None = self.model {
            self.create_model()?;
        }
        Ok(())
    }
    pub fn embed_to_cache<S>(&mut self, sentences: &[S], path: &Path) -> Result<()>
    where S: AsRef<str> + Sync {
        let embeds = self.embed(sentences)?;

        for embed in embeds {
            self.cache.add(embed, path.to_path_buf())?;
        }

        Ok(())
    }
    pub fn embed<S>(&mut self, sentences: &[S]) -> Result<Vec<Rc<[f32; 384]>>>
    where S: AsRef<str> + Sync {
        self.create_model_if_not_exists()?;
        let embeds = self.model.as_ref().unwrap().encode(sentences)?;
        let mut embeds: Vec<Rc<[f32; 384]>> = embeds.into_iter().map(|embed|{
            Rc::new(embed.as_slice().try_into().unwrap())
        }).collect();

        Ok(embeds)
    }

    pub fn nearest<S>(&mut self, sentence: &S, count: usize) -> Result<Vec<(f32, &PathBuf)>>
    where S: AsRef<str> + Sync + ?Sized {
        let embeds = self.embed(&[sentence])?;
        let embed = embeds[0].as_ref();
        let nearest = self.cache.nearest(embed, count, &squared_euclidean)?;
        Ok(nearest)
    }
}
