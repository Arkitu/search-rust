use std::{path::{Path, PathBuf}, fs::read_dir, collections::{HashMap, BinaryHeap}, thread};

use crate::error::{Result, Error};

pub mod embedding;
use embedding::{Embedder, Task, EmbeddingState};

#[derive(Clone, Copy, Debug)]
pub enum RankSource {
    ExactPath,
    InDir,
    StartLikePath,
    Semantic
}

#[derive(Clone, Debug)]
pub struct RankResult {
    pub path: PathBuf,
    pub source: RankSource,
    pub score: f32
}
impl RankResult {
    pub fn new(path: PathBuf, score: f32, source: RankSource) -> Self {
        Self {
            path: path.canonicalize().unwrap_or(path),
            score,
            source
        }
    }
    pub fn is_dir(&self) -> bool {
        self.path.is_dir()
    }
    pub fn is_symlink(&self) -> bool {
        self.path.is_symlink()
    }
}

const TASK_NAME_SCORE_LIMIT: f32 = 8.;
const TASK_PARAGRAPHS_SCORE_LIMIT: f32 = 5.;
const TASK_SENTENCES_SCORE_LIMIT: f32 = 3.;
const MAX_TASKS: usize = 100;
fn walk_path_create_tasks(path: &PathBuf, score: f32, tasks: &mut BinaryHeap<Task>) -> Result<()> {
    if tasks.len() >= MAX_TASKS {
        return Ok(());
    }
    if score < TASK_NAME_SCORE_LIMIT {
        
        tasks.push(Task::new(path.clone(), score, EmbeddingState::Name));
    }
    if path.is_dir() && !path.is_symlink() {
        let dir_iter = match read_dir(path.clone()) {
            Ok(dir_iter) => dir_iter,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::PermissionDenied || e.kind() == std::io::ErrorKind::NotFound || e.raw_os_error() == Some(20) {
                    return Ok(());
                } else {
                    panic!("Error {:?} with path {:?}", e, path);
                }
            }
        };
        for entry in dir_iter {
            match entry {
                Err(e) => {
                    match e.kind() {
                        std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::NotFound => continue,
                        _ => panic!("Error {:?} with path {:?}", e, path)
                    }
                },
                Ok(entry) => {
                    let mut path = entry.path();
                    path = match path.canonicalize() {
                        Ok(path) => path,
                        Err(_) => continue
                    };
                    walk_path_create_tasks(&path, score+1., tasks).unwrap();
                }
            }
        }
    } else {
        if score < TASK_PARAGRAPHS_SCORE_LIMIT {
            if score > 0. {
                tasks.push(Task::new(path.clone(), score+2., EmbeddingState::Paragraphs((10./score).round() as usize)));
            }
        }
        if score < TASK_SENTENCES_SCORE_LIMIT {
            tasks.push(Task::new(path.clone(), score+3., EmbeddingState::Sentences));
        }
    }
    Ok(())
}

pub struct Ranker {
    embedder: Embedder,
    last_input: String
}
impl Ranker {
    pub fn new() -> Result<Self> {
        Ok(Self {
            embedder: Embedder::new().unwrap(),
            last_input: String::new()
        })
    }

    pub fn init(&mut self) -> Result<()> {
        let embedder = self.embedder.clone();
        thread::spawn(move || {
            embedder.execute_tasks().unwrap();
        });
        Ok(())
    }

    fn get_results_hashmap(&mut self, input: &str, result_count: usize) -> Result<HashMap<PathBuf, RankResult>> {
        let mut results: HashMap<PathBuf, RankResult> = HashMap::new();

        let mut input = input.trim();

        // If input is empty, return empty results
        if input.is_empty() {
            input = "."
        }

        let path = PathBuf::from(input);

        // If exact path exists, add it to results
        match path.try_exists() {
            Ok(true) => {
                results.insert(path.clone().canonicalize().unwrap(), RankResult::new(path.clone(), 0., RankSource::ExactPath));

                // If input is a directory, add all its children to results
                if path.is_dir() && !path.is_symlink() {
                    match read_dir(path.clone()) {
                        Err(e) => {
                            if e.kind() != std::io::ErrorKind::PermissionDenied && e.kind() != std::io::ErrorKind::NotFound {
                                panic!("Error {:?} with path {:?}", e, path)
                            }
                        },
                        Ok(dir_iter) => {
                            for entry in dir_iter {
                                match entry {
                                    Err(e) => {
                                        if e.kind() != std::io::ErrorKind::PermissionDenied && e.kind() != std::io::ErrorKind::NotFound {
                                            panic!("Error {:?} with path {:?}", e, path)
                                        }
                                    },
                                    Ok(entry) => {
                                        let mut path = entry.path();
                                        path = path.canonicalize().unwrap_or(path);
                                        if let Some(r) = results.get(&path) {
                                            if r.score > 2. {
                                                results.insert(path.clone(), RankResult::new(path, 2., RankSource::InDir));
                                            }
                                        } else {
                                            results.insert(path.clone(), RankResult::new(path, 2., RankSource::InDir));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            Ok(false) => {},
            Err(e) => {
                if e.kind() != std::io::ErrorKind::PermissionDenied && e.kind() != std::io::ErrorKind::NotFound {
                    panic!("Error {:?} with path {:?}", e, path)
                }
            }
        }

        // Check if there are paths that starts with input
        if !path.is_dir() && path.exists() {
            if let Some(mut dirname) = path.canonicalize().unwrap().parent() {
                // if path.is_relative() && dirname.to_str().is_some() && dirname.to_str().ok_or(Error::CannotConvertOsStr).unwrap().is_empty() {
                //     dirname = Path::new(".");
                // }
                match read_dir(dirname) {
                    Ok(dir_iter) => {
                        for entry in dir_iter {
                            match entry {
                                Err(e) => {
                                    if e.kind() != std::io::ErrorKind::PermissionDenied && e.kind() != std::io::ErrorKind::NotFound {
                                        panic!("Error {:?} with path {:?}", e, path);
                                    }
                                },
                                Ok(entry) => {
                                    let entry_path = entry.path();
                                    if entry_path.file_name().unwrap().to_str().ok_or(Error::CannotConvertOsStr).unwrap().starts_with(path.file_name().unwrap_or_default().to_str().ok_or(Error::CannotConvertOsStr).unwrap()) {
                                    
                                        if let Some(r) = results.get(&path) {
                                            if r.score > 1. {
                                                results.insert(entry_path.canonicalize().unwrap(), RankResult::new(entry_path, 1., RankSource::StartLikePath));
                                            }
                                        } else {
                                            results.insert(entry_path.canonicalize().unwrap(), RankResult::new(entry_path, 1., RankSource::StartLikePath));
                                        }
                                    }
                                }
                            }
                        }
                    },
                    Err(e) => {
                        if e.kind() != std::io::ErrorKind::PermissionDenied && e.kind() != std::io::ErrorKind::NotFound {
                            panic!("Error {:?} with path {:?}", e, path)
                        }
                    }
                }
            }
        }

        // If this is the first time we search for this input, don't check semantic to be faster
        if self.last_input.is_empty() {
            self.last_input = input.to_string();
            return Ok(results);
        }

        let current_dir = std::env::current_dir().unwrap();
        // Check semantic with embedder
        let nearests = self.embedder.nearest(input, result_count-results.len().min(result_count)).unwrap();
        for (score, item) in nearests {
            if let Some(r) = results.get(&item.path) {
                if r.score < 3. {
                    results.insert(r.path.clone(), RankResult::new(item.path.clone(), r.score-1.+score, r.source));
                } else if r.score > (3. + score) {
                    results.insert(r.path.clone(), RankResult::new(item.path.clone(), 3.+score, RankSource::Semantic));
                }
            } else {
                if item.path.starts_with(&current_dir) {
                    results.insert(item.path.clone().canonicalize().unwrap(), RankResult::new(item.path.clone(), 3.+score, RankSource::Semantic));
                }
            }
        }

        // Launch tasks to embed paths in embedder cache
        let mut tasks = BinaryHeap::new();
        for r in results.values() {
            walk_path_create_tasks(&r.path, r.score, &mut tasks).unwrap();
        }

        if self.last_input != input {
            self.embedder.set_tasks(tasks).unwrap();
        } else {
            for task in tasks {
                self.embedder.add_task(task).unwrap();
            }
        }

        self.last_input = input.to_string();

        Ok(results)
    }

    pub fn get_results(&mut self, input: &str, result_count: usize) -> Result<Vec<RankResult>> {
        let results_hashmap = self.get_results_hashmap(input, result_count).unwrap();
        
        let mut results: Vec<RankResult> = results_hashmap.into_values().collect();

        // Sort results by score and alphabetically if score is equal
        results.sort_by(|a, b| {
            if a.score == b.score {
                a.path.cmp(&b.path)
            } else {
                a.score.partial_cmp(&b.score).unwrap()
            }
        });

        if results.len() > result_count {
            results.truncate(result_count);
        }

        Ok(results.to_vec())
    }
}
