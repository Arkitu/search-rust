use std::{path::PathBuf, sync::Arc, collections::BinaryHeap, io::Read};
use crate::error::{Result, Error};
use rust_bert::pipelines::sentence_embeddings::{builder::SentenceEmbeddingsBuilder, SentenceEmbeddingsModel, SentenceEmbeddingsModelType::AllMiniLmL12V2};
use dotext::{self, MsDoc, doc::OpenOfficeDoc};
use tokio::{sync::{Mutex, RwLock}, task::spawn_blocking};

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
    /// Queue to permit high priority lock of the model as described in https://stackoverflow.com/a/11673600/16207028
    model_queue: Arc<Mutex<()>>,
    pub cache: Arc<Mutex<Cache>>,
    /// (path to embed, priority (lower is higher))
    tasks: Arc<RwLock<BinaryHeap<Task>>>
}
impl Embedder {
    pub async fn new(db_path: Option<String>, cache_path: Option<String>) -> Self {
        let model = spawn_blocking(move || {
            SentenceEmbeddingsBuilder::remote(AllMiniLmL12V2).create_model().unwrap()
        }).await.expect("Can't create model");
        Self {
            model: Arc::new(Mutex::new(model)),
            model_queue: Arc::new(Mutex::new(())),
            cache: Arc::new(Mutex::new(Cache::new(db_path, cache_path))),
            tasks: Arc::new(RwLock::new(BinaryHeap::new()))
        }
    }
    pub async fn embed_high_priotity<S>(&self, sentences: &[S]) -> Vec<Arc<[f32; 384]>>
    where S: AsRef<str> + Sync {
        let embeds = self.model.lock().await.encode(sentences).expect("Can't embed with model");
        let embeds: Vec<Arc<[f32; 384]>> = embeds.into_iter().map(|embed|{
            Arc::new(embed.as_slice().try_into().unwrap())
        }).collect();
        embeds
    }
    pub async fn embed<S>(&self, sentences: &[S]) -> Vec<Arc<[f32; 384]>>
    where S: AsRef<str> + Sync {
        let queue = self.model_queue.lock().await;
        let embeds = self.embed_high_priotity(sentences).await;
        drop(queue);
        embeds
    }
    pub async fn add_sentences_to_id<S>(&self, sentences: &[S], id: Id)
    where S: AsRef<str> + Sync {
        let embeds = self.embed(sentences).await;
        let mut cache = self.cache.lock().await;
        for embed in embeds {
            cache.add_embed_to_id(embed, id);
        }
    }
    pub async fn add_sentences_to_path<S>(&self, sentences: &[S], path: &PathBuf)
    where S: AsRef<str> + Sync {
        let cache = self.cache.lock().await;
        let id = cache.get_id_by_path(path).expect("Trying to get id of unknown path");
        drop(cache);
        self.add_sentences_to_id(sentences, id).await;
    }

    pub async fn read_file_content(path: &PathBuf) -> Result<String> {
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
                tokio::fs::read_to_string(&path).await.unwrap_or(String::new())
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

    async fn get_file_content_prompts(&self, path: &PathBuf) -> Result<Vec<String>> {
        let mut prompts = Vec::new();
        if !path.is_dir() {
            if let Ok(content) = Self::read_file_content(&path).await {
                if !content.is_empty() {
                    prompts.push(content);
                }
            }
        }
        Ok(prompts)
    }

    async fn get_file_paragraphs_prompts(&self, path: &PathBuf, nb: usize) -> Result<Vec<String>> {
        if nb == 1 {
            return self.get_file_content_prompts(path).await;
        }
        let mut prompts = Vec::new();
        if !path.is_dir() {
            if let Ok(content) = Self::read_file_content(&path).await {
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

    pub async fn get_prompts(&self, task: &Task) -> std::result::Result<Vec<String>, Error> {
        let prompts = match task.item.state {
            EmbeddingState::None => Ok(Vec::new()),
            EmbeddingState::Name => self.get_file_name_prompts(&task.item.path),
            EmbeddingState::Paragraphs(nb) => self.get_file_paragraphs_prompts(&task.item.path, nb).await,
            _ => Err(Error::NotImplementedYet)
        };
        prompts
    }

    pub async fn execute_task(&self, task: Task) {
        if self.cache.lock().await.contains(&task.item) {
            return;
        }

        let prompts = self.get_prompts(&task).await;

        if let Ok(prompts) = prompts {
            if prompts.len() > 0 {
                self.cache.lock().await.create_or_update_item(&task.item);
                self.add_sentences_to_path(&prompts, &task.item.path).await;
            }
        }
    }

    pub fn execute_tasks(&self) -> Result<()> {
        let clone = self.clone();
        tokio::spawn(async move {
            loop {
                if let Some(task) = clone.tasks.write().await.pop() {
                    let clone = clone.clone();
                    tokio::spawn(async move {
                        clone.execute_task(task).await;
                    });
                }
                //clone.next_task();
            }
        });
        Ok(())
    }

    // fn next_task(&self) {
    //     let mut tasks = self.tasks.write().expect("Can't lock tasks");
    //     if tasks.len() == 0 {
    //         return;
    //     }
    //     let task = tasks.pop().unwrap();
    //     drop(tasks);

    //     self.execute_task(task);
    // }

    pub async fn add_task(&self, task: Task) {
        let mut tasks = self.tasks.write().await;
        tasks.push(task);
    }
    pub async fn add_tasks(&self, tasks: Vec<Task>) {
        let mut old_tasks = self.tasks.write().await;
        for task in tasks {
            old_tasks.push(task);
        }
    }

    pub async fn set_tasks(&self, tasks: BinaryHeap<Task>) {
        *self.tasks.write().await = tasks;
    }

    pub async fn nearest<S>(&mut self, sentence: &S, count: usize) -> Vec<(f32, PathBuf)>
    where S: AsRef<str> + Sync + ?Sized {
        let embeds = self.embed_high_priotity(&[sentence]).await;
        let embed = embeds[0].as_ref();
        let cache = self.cache.lock().await;
        cache.nearest(embed, count)
    }
}
