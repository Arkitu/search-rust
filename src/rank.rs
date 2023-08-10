use std::{path::Path, fs::read_dir};
use crate::error::Result;

pub fn get_results(input: &str, result_count: usize) -> Result<Vec<String>> {
    let mut results = Vec::new();

    let input = input.trim();

    let path = Path::new(input);

    // If exact path exists, add it to results
    if path.try_exists()? {
        results.push(input.to_owned());
    }

    // Check if there are enough results
    if results.len() >= result_count {
        return Ok(results[0..result_count].to_vec());
    }

    // If input is a directory, add all its children to results
    if path.is_dir() {
        for entry in read_dir(input)? {
            let entry = entry?;
            let path = entry.path();
            results.push(path.to_str().unwrap().to_string());
        }
    }

    // Check if there are enough results
    if results.len() >= result_count {
        return Ok(results[0..result_count].to_vec());
    }

    Ok(results)
}