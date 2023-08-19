use std::{path::{Path, PathBuf}, fs::read_dir, collections::HashMap};
use scan_dir::ScanDir;

use crate::error::Result;

pub mod embedding;

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
            path: path.clone(),
            score,
            source
        }
    }
    pub fn is_dir(&self) -> bool {
        self.path.is_dir()
    }
}

pub struct Ranker {
    embedder: embedding::Embedder
}
impl Ranker {
    pub fn new() -> Result<Self> {
        Ok(Self {
            embedder: embedding::Embedder::new()?
        })
    }

    pub fn init(&mut self) -> Result<()> {
        ScanDir::all().skip_hidden(true).skip_backup(true).walk(".", |iter|{
            for (entry, filename) in iter {

                let mut prompts = vec![];

                prompts.push("file: ".to_string() + &filename);

                let name = filename.split('.').next().unwrap().replace('_', " ");
                
                prompts.push("name: ".to_string() + &name);

                if filename.contains('.') {
                    prompts.push("extension: ".to_string() + filename.split_once('.').unwrap().1);
                }

                eprintln!("{}", prompts.join(" / "));
                
                let entry_path = entry.path().canonicalize().unwrap();
                
                self.embedder.embed_to_cache(&prompts, &entry_path.as_path()).unwrap();
            }
        })?;
        Ok(())
    }

    fn get_results_hashmap(&mut self, input: &str, result_count: usize) -> Result<HashMap<PathBuf, RankResult>> {
        // path -> (ResultType, score)
        // Lower score is better
        let mut results: HashMap<PathBuf, RankResult> = HashMap::new();

        let mut input = input.trim();

        // If input is empty, return empty results
        if input.is_empty() {
            input = "."
        }

        let path = PathBuf::from(input);

        // If exact path exists, add it to results
        if path.try_exists()? {
            results.insert(path.clone().canonicalize()?, RankResult::new(path.clone(), 0., RankSource::ExactPath));

            // If input is a directory, add all its children to results
            if path.is_dir() {
                for entry in read_dir(input)? {
                    let entry = entry?;
                    let mut path = entry.path();
                    path = path.canonicalize()?;
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

        // Check if there are paths that starts with input
        if let Some(mut dirname) = path.parent() {
            if path.is_relative() && dirname.to_str().is_some() && dirname.to_str().unwrap().is_empty() {
                dirname = Path::new(".");
            }
            if dirname.try_exists()? {
                for entry in read_dir(dirname)? {
                    let entry = entry?;
                    let entry_path = entry.path();
                    if entry_path.file_name().unwrap().to_str().unwrap().starts_with(path.file_name().unwrap_or_default().to_str().unwrap()) {
                    
                        if let Some(r) = results.get(&path) {
                            if r.score > 1. {
                                results.insert(entry_path.clone().canonicalize()?, RankResult::new(entry_path, 1., RankSource::StartLikePath));
                            }
                        } else {
                            results.insert(entry_path.clone().canonicalize()?, RankResult::new(entry_path, 1., RankSource::StartLikePath));
                        }
                    }
                }
            }
        }

        // Check if there are enough results
        if results.len() >= result_count {
            return Ok(results);
        }

        // Check semantic with embedder
        let nearests = self.embedder.nearest(input, result_count-results.len())?;
        for n in nearests {
            let path: PathBuf = n.1.into();

            if let Some(r) = results.get(&path) {
                if r.score > (3. + n.0) {
                    results.insert(path.clone().canonicalize()?, RankResult::new(path, (3.+n.0), RankSource::Semantic));
                }
            } else {
                results.insert(path.clone().canonicalize()?, RankResult::new(path, (3.+n.0), RankSource::Semantic));
            }
        }

        Ok(results)
    }

    pub fn get_results(&mut self, input: &str, result_count: usize) -> Result<Vec<RankResult>> {
        let results_hashmap = self.get_results_hashmap(input, result_count)?;
        
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
