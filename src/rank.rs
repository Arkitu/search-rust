use std::{path::{Path, PathBuf}, fs::read_dir, collections::HashMap};
use crate::error::Result;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum PathType {
    Dir,
    File
}
impl From<&PathBuf> for PathType {
    fn from(path: &PathBuf) -> Self {
        if path.is_dir() {
            Self::Dir
        } else {
            Self::File
        }
    }
}

#[derive(Clone)]
pub struct RankResult {
    pub path: PathBuf,
    pub result_type: PathType,
    pub score: f32
}

fn get_results_hashmap(input: &str, result_count: usize) -> Result<HashMap<PathBuf, (PathType, f32)>> {
    // path -> (ResultType, score)
    // Lower score is better
    let mut results: HashMap<PathBuf, (PathType, f32)> = HashMap::new();

    let mut input = input.trim();

    // If input is empty, return empty results
    if input.is_empty() {
        input = "."
    }

    let path = PathBuf::from(input);

    // If exact path exists, add it to results
    if path.try_exists()? {
        results.insert(path.clone().canonicalize()?, ((&path).into(), 0.));

        // If input is a directory, add all its children to results
        if path.is_dir() {
            for entry in read_dir(input)? {
                let entry = entry?;
                let path = entry.path();
                results.insert(path.clone().canonicalize()?, ((&path).into(), 2.));
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
                    results.insert(entry_path.clone().canonicalize()?, ((&entry_path).into(), 1.));
                }
            }
        }
    }

    // Check if there are enough results
    if results.len() >= result_count {
        return Ok(results);
    }

    Ok(results)
}

pub fn get_results(input: &str, result_count: usize) -> Result<Vec<RankResult>> {
    let mut results = Vec::new();

    let results_hashmap = get_results_hashmap(input, result_count)?;

    for (path, (result_type, score)) in results_hashmap {
        results.push(RankResult {
            path,
            result_type,
            score
        });
    }

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
