use std::{path::{Path, PathBuf}, fs::read_dir, collections::HashSet};
use crate::error::Result;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResultType {
    Dir,
    File
}
impl From<PathBuf> for ResultType {
    fn from(path: PathBuf) -> Self {
        if path.is_dir() {
            Self::Dir
        } else {
            Self::File
        }
    }
}

pub fn get_results(input: &str, result_count: usize) -> Result<Vec<(ResultType, String, usize)>> {
    // (ResultType, path, score)
    // Lower score is better
    let mut results: HashSet<(ResultType, String, usize)> = HashSet::new();

    let input = input.trim();

    let path = PathBuf::from(input);

    // If exact path exists, add it to results
    if path.try_exists()? {
        results.insert((path.clone().into(), path.to_str().unwrap().to_string(), 0));
    }

    // Check if there are enough results
    if results.len() >= result_count {
        return Ok(results.drain().collect::<Vec<(ResultType, String, usize)>>()[0..result_count].to_vec());
    }

    // Check if there are paths that starts with input
    if let Some(dirname) = path.parent() {
        for entry in read_dir(dirname)? {
            let entry = entry?;
            let path = entry.path();
            if path.to_str().unwrap().starts_with(input) {
                results.insert((path.clone().into(), path.to_str().unwrap().to_string()));
            }
        }
    }

    // Check if there are enough results
    if results.len() >= result_count {
        return Ok(results.drain().collect::<Vec<(ResultType, String)>>()[0..result_count].to_vec());
    }

    // If input is a directory, add all its children to results
    if path.is_dir() {
        for entry in read_dir(input)? {
            let entry = entry?;
            let path = entry.path();
            results.insert((path.clone().into(), path.to_str().unwrap().to_string()));
        }
    }

    // Check if there are enough results
    if results.len() >= result_count {
        return Ok(results.drain().collect::<Vec<(ResultType, String)>>()[0..result_count].to_vec());
    }

    Ok(results.drain().collect::<Vec<(ResultType, String)>>())
}