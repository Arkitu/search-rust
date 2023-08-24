use std::{path::PathBuf, sync::{Arc, RwLock, Mutex}, thread, collections::BinaryHeap, io::Read};
use crate::error::{Result, Error};
use rust_bert::pipelines::sentence_embeddings::{builder::SentenceEmbeddingsBuilder, SentenceEmbeddingsModel, SentenceEmbeddingsModelType::AllMiniLmL12V2};
use kiddo::{distance::squared_euclidean, float::kdtree::KdTree};
use dotext::{self, MsDoc, doc::OpenOfficeDoc};

pub struct CacheItem {
    pub path: PathBuf,
    state: EmbeddingState
}

pub struct EmbedderCache {
    // <axisUnit, itemID, dimensionNumber, numberOfItemAtTheSamePositionOnOneAxis, maxNumberOfItems>
    kdtree: KdTree<f32, u32, 384, 256, u32>,
    paths: Vec<Arc<CacheItem>>
}
impl EmbedderCache {
    pub fn new() -> Self {
        Self {
            kdtree: KdTree::new(),
            paths: Vec::new()
        }
    }
    pub fn add(&mut self, embed: &[f32; 384], item: CacheItem) {
        let id = self.paths.len() as u32;
        self.kdtree.add(embed, id);
        self.paths.push(Arc::new(item));
    }
    pub fn nearest(&self, embed: &[f32; 384], count: usize) -> Vec<(f32, Arc<CacheItem>)> {
        let nearest = self.kdtree.nearest_n(embed, count, &squared_euclidean);
        nearest.into_iter().map(|n| (n.distance, self.paths[n.item as usize].clone())).collect()
    }
    pub fn contains(&self, item: &CacheItem) -> bool {
        self.paths.iter().any(|p| p.path == item.path && p.state >= item.state)
    }
}


#[derive(PartialEq, Eq, Hash, Debug, Clone, Copy)]
pub enum EmbeddingState {
    None,
    Name,
    // (nb of paragraphs) to avoid too much embedding with files with a lot of paragraphs. If the file has more paragraphs the paragraphs will be grouped
    Paragraphs(usize),
    Sentences
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
            (EmbeddingState::Paragraphs(_), EmbeddingState::Sentences) => Some(std::cmp::Ordering::Less),
            (EmbeddingState::Sentences, EmbeddingState::Paragraphs(_)) => Some(std::cmp::Ordering::Greater),
            (EmbeddingState::Paragraphs(a), EmbeddingState::Paragraphs(b)) => Some(a.cmp(b)),
            (EmbeddingState::Sentences, EmbeddingState::Sentences) => Some(std::cmp::Ordering::Equal)
        }
    }
}
impl Ord for EmbeddingState {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

pub struct Task {
    item: CacheItem,
    // lower is higher
    priority: f32
}
impl Task {
    pub fn new(path: PathBuf, priority: f32, kind: EmbeddingState) -> Self {
        Self {
            item: CacheItem { path, state: kind },
            priority
        }
    }
}
impl PartialEq for Task {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority
    }
}
impl Eq for Task {}
impl PartialOrd for Task {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        // reverse because lower is higher
        match self.priority.partial_cmp(&other.priority) {
            Some(o) => Some(o.reverse()),
            None => None
        }
    }
}
impl Ord for Task {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.priority.partial_cmp(&other.priority).unwrap()
    }
}

#[derive(Clone)]
pub struct Embedder {
    model: Arc<Mutex<SentenceEmbeddingsModel>>,
    cache: Arc<RwLock<EmbedderCache>>,
    /// (path to embed, priority (lower is higher))
    tasks: Arc<RwLock<BinaryHeap<Task>>>
}
impl Embedder {
    pub fn new() -> Result<Self> {
        Ok(Self {
            model: Arc::new(Mutex::new(SentenceEmbeddingsBuilder::remote(AllMiniLmL12V2).create_model()?)),
            cache: Arc::new(RwLock::new(EmbedderCache::new())),
            tasks: Arc::new(RwLock::new(BinaryHeap::new()))
        })
    }
    pub fn embed_to_cache<S>(&self, sentences: &[S], path: &PathBuf) -> Result<()>
    where S: AsRef<str> + Sync {
        let embeds = self.embed(sentences)?;

        for embed in embeds {
            self.cache.write()?.add(&embed, CacheItem { path: path.clone(), state: EmbeddingState::None });
        }

        Ok(())
    }
    pub fn embed<S>(&self, sentences: &[S]) -> Result<Vec<Arc<[f32; 384]>>>
    where S: AsRef<str> + Sync {
        let embeds = self.model.lock()?.encode(sentences)?;
        let embeds: Vec<Arc<[f32; 384]>> = embeds.into_iter().map(|embed|{
            Arc::new(embed.as_slice().try_into().unwrap())
        }).collect();

        Ok(embeds)
    }

    pub fn read_file_content(path: &PathBuf) -> Result<String> {
        let extension = match path.extension() {
            None => return Ok("".to_string()),
            Some(extension) => extension.to_str().unwrap_or("")
        };
        let content = match extension {
            "docx" => {
                let mut content = String::new();
                dotext::Docx::open(&path)?.read_to_string(&mut content)?;
                content
            },
            "xlsx" => {
                let mut content = String::new();
                dotext::Xlsx::open(&path)?.read_to_string(&mut content)?;
                content
            },
            "pptx" => {
                let mut content = String::new();
                dotext::Pptx::open(&path)?.read_to_string(&mut content)?;
               content
            },
            "odt" => {
                let mut content = String::new();
                dotext::Odt::open(&path)?.read_to_string(&mut content)?;
                content
            },
            "odp" => {
                let mut content = String::new();
                dotext::Odp::open(&path)?.read_to_string(&mut content)?;
                content
            },
            _ => {
                std::fs::read_to_string(&path).unwrap_or_default()
            }
        };
        Ok(content)
    }

    fn get_file_name_prompts(&self, path: &PathBuf) -> Result<Vec<String>> {
        let mut prompts = Vec::new();
        let filename = match path.file_name() {
            None => return Ok(prompts),
            Some(filename) => filename.to_str().ok_or(Error::CannotConvertOsStr)?
        };
        if path.is_dir() {
            prompts.push("directory: ".to_string() + filename);
        } else {
            prompts.push("file: ".to_string() + filename);
        }

        let name = path.file_stem().ok_or(Error::CannotGetFileStem)?.to_str().ok_or(Error::CannotConvertOsStr)?.replace('_', " ");
        prompts.push("name: ".to_string() + &name);

        if !path.is_dir() {
            if let Some(e) = path.extension() {
                let e = e.to_str().ok_or(Error::CannotConvertOsStr)?;
                prompts.push("extension: ".to_string() + e);
            }
        }

        Ok(prompts)
    }

    fn get_file_content_prompts(&self, path: &PathBuf) -> Result<Vec<String>> {
        let mut prompts = Vec::new();
        if !path.is_dir() {
            if let Ok(content) = Self::read_file_content(&path) {
                if !content.is_empty() {
                    prompts.push(content);
                }
            }
        }
        Ok(prompts)
    }

    fn get_file_paragraphs_prompts(&self, path: &PathBuf, nb: usize) -> Result<Vec<String>> {
        if nb == 1 {
            return self.get_file_content_prompts(path);
        }
        let mut prompts = Vec::new();
        if !path.is_dir() {
            if let Ok(content) = Self::read_file_content(&path) {
                if !content.is_empty() {
                    let mut content: Vec<String> = content.split("\n\n").map(|p| p.to_string()).collect();

                    // Remove empty paragraphs
                    content.retain(|p| !p.is_empty());

                    // if there is more paragraphs than nb, group them 2 by 2 until there is nb paragraphs
                    if content.len() > nb {
                        while content.len() > nb {
                            let mut new_content: Vec<String> = Vec::new();
                            for double in content.chunks(2) {
                                new_content.push(double.join("\n\n"));
                            }
                            content = new_content;
                        }
                    }

                    for p in content {
                        prompts.push(p.to_string());
                    }
                }
            }
        }
        Ok(prompts)
    }

    pub fn execute_task(&self, task: Task) -> Result<()> {
        if self.cache.read()?.contains(&task.item) {
            return Ok(());
        }

        //eprintln!("embedding with level {:?} : {} ", task.kind, task.path.display());

        let prompts = match task.item.state {
            EmbeddingState::Name => self.get_file_name_prompts(&task.item.path),
            EmbeddingState::Paragraphs(nb) => self.get_file_paragraphs_prompts(&task.item.path, nb),
            _ => Err(Error::NotImplementedYet)
        };

        if let Ok(prompts) = prompts {
            if prompts.len() > 0 {
                self.embed_to_cache(&prompts, &task.item.path).unwrap();
            }
        }

        Ok(())
    }

    pub fn execute_tasks(&self) -> Result<()> {
        let clone = self.clone();
        thread::spawn(move || {
            loop {
                clone.next_task().unwrap();
            }
        });
        Ok(())
    }

    fn next_task(&self) -> Result<()> {
        let mut tasks = self.tasks.write()?;
        if tasks.len() == 0 {
            return Ok(());
        }
        let task = tasks.pop().unwrap();
        drop(tasks);

        self.execute_task(task).unwrap();

        Ok(())
    }

    pub fn add_task(&self, task: Task) -> Result<()> {
        let mut tasks = self.tasks.write()?;
        tasks.push(task);
        Ok(())
    }

    pub fn set_tasks(&self, tasks: BinaryHeap<Task>) -> Result<()> {
        *self.tasks.write()? = tasks;
        Ok(())
    }

    pub fn empty_tasks(&self) -> Result<()> {
        *self.tasks.write()? = BinaryHeap::new();
        Ok(())
    }

    pub fn nearest<S>(&mut self, sentence: &S, count: usize) -> Result<Vec<(f32, Arc<CacheItem>)>>
    where S: AsRef<str> + Sync + ?Sized {
        let embeds = self.embed(&[sentence])?;
        let embed = embeds[0].as_ref();
        let cache = self.cache.read()?;
        Ok(cache.nearest(embed, count))
    }
}
