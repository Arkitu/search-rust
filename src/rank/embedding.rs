use std::{path::PathBuf, sync::{Arc, RwLock, Mutex, atomic::{AtomicI32, Ordering}}, thread, collections::{BinaryHeap, HashSet, HashMap}, io::Read};
use crate::error::{Result, Error};
use rust_bert::pipelines::sentence_embeddings::{builder::SentenceEmbeddingsBuilder, SentenceEmbeddingsModel, SentenceEmbeddingsModelType::AllMiniLmL12V2};
use kdtree::{KdTree, distance::squared_euclidean};
use dotext::{self, MsDoc, doc::OpenOfficeDoc};

pub type Id = i32;
pub type TempCache = KdTree<f32, Id, Arc<[f32]>>;

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

pub struct CacheItem {
    pub path: PathBuf,
    state: EmbeddingState
}

pub struct Cache {
    temp_cache: TempCache,
    items: HashMap<Id, Arc<CacheItem>>,
    id_counter: AtomicI32
}
impl Cache {
    pub fn new() -> Self {
        Self {
            temp_cache: KdTree::new(384),
            items: HashMap::new(),
            id_counter: AtomicI32::new(0)
        }
    }
    pub fn get_item(&self, id: &Id) -> Arc<CacheItem> {
        self.items[id]
    }
    pub fn add(&mut self, embed: Arc<[f32; 384]>, item: CacheItem) {
        let id = self.id_counter.fetch_add(1, Ordering::Relaxed);
        self.temp_cache.add(embed, id);
        self.items.insert(id, Arc::new(item));
    }
    pub fn nearest(&self, embed: &[f32; 384], count: usize) -> Vec<(f32, Arc<CacheItem>)> {
        let nearest = self.temp_cache.nearest(embed, count, &squared_euclidean).expect("Can't get nearest in temp cache");
        nearest.into_iter().map(|(score, id)| (
            score,
            self.get_item(id)
        )).collect()
    }
    pub fn contains(&self, item: &CacheItem) -> bool {
        self.items.values().any(|p| p.path == item.path && p.state >= item.state)
    }
}



pub struct Task {
    item: CacheItem,
    // lower is higher
    priority: f32,
    kind: EmbeddingState
}
impl Task {
    pub fn new(item: CacheItem, priority: f32, kind: EmbeddingState) -> Self {
        Self {
            item,
            priority,
            kind
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
    cache: Arc<RwLock<Cache>>,
    /// (path to embed, priority (lower is higher))
    tasks: Arc<RwLock<BinaryHeap<Task>>>,
    pub embedded_paths: Arc<RwLock<HashSet<(PathBuf, EmbeddingState)>>>
}
impl Embedder {
    pub fn new() -> Result<Self> {
        Ok(Self {
            model: Arc::new(Mutex::new(SentenceEmbeddingsBuilder::remote(AllMiniLmL12V2).create_model()?)),
            cache: Arc::new(RwLock::new(Cache::new())),
            tasks: Arc::new(RwLock::new(BinaryHeap::new())),
            embedded_paths: Arc::new(RwLock::new(HashSet::new()))
        })
    }
    pub fn embed_to_cache<S>(&self, sentences: &[S], path: &PathBuf) -> Result<()>
    where S: AsRef<str> + Sync {
        let embeds = self.embed(sentences)?;

        for embed in embeds {
            self.cache.write()?.add(embed, path.clone()).expect("Can't add item to cache");
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
        if self.embedded_paths.read()?.contains(&(task.path.clone(), task.kind)) {
            return Ok(());
        }

        //eprintln!("embedding with level {:?} : {} ", task.kind, task.path.display());

        let prompts = match task.kind {
            EmbeddingState::Name => self.get_file_name_prompts(&task.path),
            EmbeddingState::Paragraphs(nb) => self.get_file_paragraphs_prompts(&task.path, nb),
            _ => Err(Error::NotImplementedYet)
        };

        if let Ok(prompts) = prompts {
            if prompts.len() > 0 {
                self.embed_to_cache(&prompts, &task.path).unwrap();
            }
        }

        self.embedded_paths.write()?.insert((task.path, task.kind));

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

    pub fn nearest<S>(&mut self, sentence: &S, count: usize) -> Result<Vec<(f32, PathBuf)>>
    where S: AsRef<str> + Sync + ?Sized {
        let embeds = self.embed(&[sentence])?;
        let embed = embeds[0].as_ref();
        let cache = self.cache.read()?;
        let nearest = cache.nearest(embed, count, &squared_euclidean).expect("Can't get nearest elements in temp cache");
        Ok(nearest.into_iter().map(|(s, p)| (s, p.to_owned())).collect())
    }
}
