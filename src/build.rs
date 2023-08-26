/// The purpose of this module is to pre calculate the embedding cache and store it in an annoy file

use std::{path::PathBuf, fs::read_dir, sync::Arc};
use rannoy::Rannoy;
use tokio::sync::Mutex;
use crate::embedding::{Embedder, EmbeddingState, Task, CacheItem};
use async_recursion::async_recursion;

#[async_recursion]
async fn step(level: EmbeddingState, embedder: &Embedder, annoy: Arc<Mutex<Rannoy>>, path: PathBuf, recursion_level: usize) {
    //println!("{} scanning {}", recursion_level, path.display());
    let task = Task::new(CacheItem { path: path.clone(), state: EmbeddingState::None }, 0.);
    let prompts = match embedder.get_prompts(&task).await {
        Ok(ps) => ps,
        Err(_) => return
    };
    let cache = embedder.cache.lock().await;
    cache.create_or_update_item(&CacheItem { path: path.clone(), state: level });
    let id = cache.get_id_by_path(&path).expect("Can't get id of item just created");
    drop(cache);
    let embeds = if prompts.len() > 0 {
        embedder.embed(&prompts).await
    } else {
        Vec::new()
    };
    for embed in embeds {
        annoy.lock().await.add_item(id, embed.as_ref());
    }
    if let Ok(childs) = read_dir(path) {
        for child in childs {
            if let Ok(child) = child {
                step(level, embedder, annoy.clone(), child.path(), recursion_level+1).await;
            }
        }
    }
}

pub async fn build(target: &str, level: EmbeddingState, cache_path: &str, db_path: String) {
    let target = PathBuf::from(target);

    let embedder = Embedder::new(Some(db_path)).await;

    let annoy = Arc::new(Mutex::new(Rannoy::new(384)));
    annoy.lock().await.set_seed(123); // 123 is the seed for the random number generator

    step(level, &embedder, annoy.clone(), target, 0).await;

    let annoy = annoy.lock().await;
    annoy.build(30); // 30 is the number of trees (higher = more precision)

    annoy.save(cache_path);

    println!("Done!");
}