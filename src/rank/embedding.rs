use std::{path::PathBuf, sync::{Arc, RwLock, Mutex}, thread, collections::BinaryHeap, io::Read};
use crate::error::{Result, Error};
use rust_bert::pipelines::sentence_embeddings::{builder::SentenceEmbeddingsBuilder, SentenceEmbeddingsModel, SentenceEmbeddingsModelType::AllMiniLmL12V2};
use dotext::{self, MsDoc, doc::OpenOfficeDoc};

mod cache;
use cache::{Cache, Id};
pub use cache::{EmbeddingState, CacheItem};

#[derive(Debug)]
pub struct Task {
    item: CacheItem,
    // lower is higher
    priority: f32
}
impl Task {
    pub fn new(item: CacheItem, priority: f32) -> Self {
        Self {
            item,
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
    cache: Arc<Mutex<Cache>>,
    /// (path to embed, priority (lower is higher))
    tasks: Arc<RwLock<BinaryHeap<Task>>>
}
impl Embedder {
    pub fn new(db_path: Option<String>) -> Self {
        Self {
            model: Arc::new(Mutex::new(SentenceEmbeddingsBuilder::remote(AllMiniLmL12V2).create_model().expect("Can't create model"))),
            cache: Arc::new(Mutex::new(Cache::new(db_path))),
            tasks: Arc::new(RwLock::new(BinaryHeap::new()))
        }
    }
    pub fn embed<S>(&self, sentences: &[S]) -> Vec<Arc<[f32; 384]>>
    where S: AsRef<str> + Sync {
        let embeds = self.model.lock().expect("Can't lock model").encode(sentences).expect("Can't embed with model");
        let embeds: Vec<Arc<[f32; 384]>> = embeds.into_iter().map(|embed|{
            Arc::new(embed.as_slice().try_into().unwrap())
        }).collect();
        embeds
    }
    pub fn add_sentences_to_id<S>(&self, sentences: &[S], id: Id)
    where S: AsRef<str> + Sync {
        let embeds = self.embed(sentences);
        let mut cache = self.cache.lock().expect("Can't lock cache");
        for embed in embeds {
            cache.add_embed_to_id(embed, id);
        }
    }
    pub fn add_sentences_to_path<S>(&self, sentences: &[S], path: &PathBuf)
    where S: AsRef<str> + Sync {
        let cache = self.cache.lock().expect("Can't lock cache");
        let id = cache.get_id_by_path(path).expect("Trying to get id of unknown path");
        drop(cache);
        self.add_sentences_to_id(sentences, id)
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
                std::fs::read_to_string(&path).unwrap_or(String::new())
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

    pub fn execute_task(&self, task: Task) {
        if self.cache.lock().expect("Can't aquire cache lock").contains(&task.item) {
            return;
        }

        let prompts = match task.item.state {
            EmbeddingState::None => Ok(Vec::new()),
            EmbeddingState::Name => self.get_file_name_prompts(&task.item.path),
            EmbeddingState::Paragraphs(nb) => self.get_file_paragraphs_prompts(&task.item.path, nb),
            _ => Err(Error::NotImplementedYet)
        };

        if let Ok(prompts) = prompts {
            if prompts.len() > 0 {
                self.cache.lock().expect("Can't aquire cache lock").create_or_update_item(&task.item);
                self.add_sentences_to_path(&prompts, &task.item.path);
            }
        }
    }

    pub fn execute_tasks(&self) -> Result<()> {
        let clone = self.clone();
        thread::spawn(move || {
            loop {
                clone.next_task();
            }
        });
        Ok(())
    }

    fn next_task(&self) {
        let mut tasks = self.tasks.write().expect("Can't lock tasks");
        if tasks.len() == 0 {
            return;
        }
        let task = tasks.pop().unwrap();
        drop(tasks);

        self.execute_task(task);
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

    pub fn nearest<S>(&mut self, sentence: &S, count: usize) -> Vec<(f32, PathBuf)>
    where S: AsRef<str> + Sync + ?Sized {
        let embeds = self.embed(&[sentence]);
        let embed = embeds[0].as_ref();
        let cache = self.cache.lock().expect("Can't lock cache");
        cache.nearest(embed, count)
    }
}
