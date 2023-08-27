use std::{sync::Arc, path::PathBuf, fmt::{Debug, Formatter}};
use kdtree::{KdTree, distance::squared_euclidean};

mod db;
use db::DB;
use rannoy::Rannoy;

pub type Id = i32;
pub type TempCache = KdTree<f32, Id, Arc<[f32]>>;

#[derive(PartialEq, Eq, Hash, Debug, Clone, Copy)]
pub enum EmbeddingState {
    None,
    Name,
    // (nb of paragraphs) to avoid too much embedding with files with a lot of paragraphs. If the file has more paragraphs the paragraphs will be grouped
    Paragraphs(usize)
}

impl PartialOrd for EmbeddingState {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        if self == other {
            return Some(std::cmp::Ordering::Equal);
        }
        match (self, other) {
            (EmbeddingState::None, _) => Some(std::cmp::Ordering::Less),
            (_, EmbeddingState::None) => Some(std::cmp::Ordering::Greater),
            (EmbeddingState::Name, _) => Some(std::cmp::Ordering::Less),
            (_, EmbeddingState::Name) => Some(std::cmp::Ordering::Greater),
            (EmbeddingState::Paragraphs(a), EmbeddingState::Paragraphs(b)) => Some(a.cmp(b)),
        }
    }
}
impl Ord for EmbeddingState {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

#[derive(Debug)]
pub struct CacheItem {
    pub path: PathBuf,
    pub state: EmbeddingState
}

pub struct Cache {
    temp_cache: TempCache,
    annoy: Option<Rannoy>,
    db: DB
}
impl Debug for Cache {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cache")
            .field("temp_cache", &self.temp_cache)
            .field("db", &self.db)
            .finish()
    }
}
impl Cache {
    pub fn new(db_path: Option<String>, cache_path: Option<String>) -> Self {
        let annoy = match cache_path {
            Some(p) => {
                let a = Rannoy::new(384);
                a.set_seed(123);
                a.load(p.into());
                Some(a)
            },
            None => None
        };
        Self {
            temp_cache: KdTree::new(384),
            annoy,
            db: DB::new(db_path)
        }
    }
    pub fn create_item(&self, item: &CacheItem) {
        self.db.insert_item(item)
    }
    pub fn create_or_update_item(&self, item: &CacheItem) {
        self.db.insert_or_update_item(item)
    }
    pub fn add_embed_to_id(&mut self, embed: Arc<[f32; 384]>, id: Id) {
        self.temp_cache.add(embed, id).expect("Can't add item to temp cache")
    }
    pub fn nearest(&self, embed: &[f32; 384], count: usize) -> Vec<(f32, PathBuf)> {
        let nearest = match self.annoy {
            None => Vec::new(),
            Some(ref a) => {
                let result = a.get_nns_by_vector((*embed).into(), count.try_into().unwrap(), -1); // search_k can be changed to increase precision but it's slower
                eprintln!("{:#?}", result);
                result.1.into_iter().zip(result.0.into_iter().map(|id|{
                    self.db.get_path_by_id(id).expect(format!("Trying to get id that doesn't exist from db : {}", id).as_str())
                })).collect()
            }
        };
        let temp_nearest = self.temp_cache.nearest(embed, count, &squared_euclidean).expect("Can't get nearest in temp cache");
        let temp_nearest: Vec<(f32, PathBuf)> = temp_nearest.into_iter().map(|(score, id)| (
            score,
            self.db.get_path_by_id(*id).expect(format!("Trying to get id that doesn't exist from db : {}", id).as_str())
        )).collect();
        let mut nearest = nearest.into_iter().chain(temp_nearest.into_iter()).collect::<Vec<_>>();
        nearest.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        nearest.truncate(count);
        nearest
    }
    pub fn contains(&self, item: &CacheItem) -> bool {
        match self.db.get_state_by_path(&item.path) {
            Some(state) => state >= item.state,
            None => false
        }
    }
    pub fn get_id_by_path(&self, path: &PathBuf) -> Option<Id> {
        self.db.get_id_by_path(path)
    }
}