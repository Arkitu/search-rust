use std::{path::PathBuf, sync::{Arc, RwLock, Mutex}, thread, collections::{BinaryHeap, HashSet}, io::Read};
use crate::error::{Result, Error};
use rust_bert::pipelines::sentence_embeddings::{builder::SentenceEmbeddingsBuilder, SentenceEmbeddingsModel, SentenceEmbeddingsModelType::AllMiniLmL12V2};
use kdtree::{KdTree, distance::squared_euclidean};
use dotext::{self, MsDoc, doc::OpenOfficeDoc};

pub type EmbedderCache = KdTree<f32, PathBuf, Arc<[f32]>>;

pub struct Task {
    path: PathBuf,
    // lower is higher
    priority: f32
}
impl Task {
    pub fn new(path: PathBuf, priority: f32) -> Self {
        Self {
            path,
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
    tasks: Arc<RwLock<BinaryHeap<Task>>>,
    embedded_paths: Arc<RwLock<HashSet<PathBuf>>>
}
impl Embedder {
    pub fn new() -> Result<Self> {
        Ok(Self {
            model: Arc::new(Mutex::new(SentenceEmbeddingsBuilder::remote(AllMiniLmL12V2).create_model()?)),
            cache: Arc::new(RwLock::new(KdTree::new(384))),
            tasks: Arc::new(RwLock::new(BinaryHeap::new())),
            embedded_paths: Arc::new(RwLock::new(HashSet::new()))
        })
    }
    pub fn embed_to_cache<S>(&self, sentences: &[S], path: &PathBuf) -> Result<()>
    where S: AsRef<str> + Sync {
        let embeds = self.embed(sentences)?;

        for embed in embeds {
            self.cache.write()?.add(embed, path.clone())?;
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

    pub fn path_to_cache(&self, path: PathBuf) -> Result<()> {
        if self.embedded_paths.read()?.contains(&path) {
            return Ok(());
        }
        let mut prompts = vec![];
        let filename = match path.file_name() {
            None => return Ok(()),
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

                if let Ok(content) = Self::read_file_content(&path) {
                    if !content.is_empty() {
                        prompts.push(content);
                    }
                }
            }
        }

        //eprintln!("{}", prompts.join(" / "));
        
        self.embed_to_cache(&prompts, &path).unwrap();

        self.embedded_paths.write()?.insert(path);

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

        self.path_to_cache(task.path).unwrap();

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
        let nearest = cache.nearest(embed, count, &squared_euclidean)?;
        Ok(nearest.into_iter().map(|(s, p)| (s, p.to_owned())).collect())
    }
}
