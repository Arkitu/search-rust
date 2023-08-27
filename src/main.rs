use std::env;
mod error;
mod ui;
mod rank;
mod build;
mod embedding;
use annoy_rs::AnnoyIndex;
use annoy_rs::AnnoyIndexSearchApi;
use embedding::Embedder;
use error::Error;
use error::Result;
use ui::UI;
use ui::visual_pack::VisualPack;

#[tokio::main]
async fn main() -> Result<()> {
    let embedder = Embedder::new(None, None).await;
    let annoy = AnnoyIndex::load(384, "cache.ann", annoy_rs::IndexType::Euclidean).unwrap();
    println!("{}", annoy.size);
    let result = annoy.get_nearest(embedder.embed(&["test"]).await[0].as_ref(), 18, 10, true);
    println!("{:#?}", result.id_list);
    println!("{:#?}", result.distance_list);
    Ok(())
}

//#[tokio::main]
async fn mainr() -> Result<()> {
    let mut cache_path = None;
    let mut db_path = None;
    let mut target_file = None;
    let mut vp = VisualPack::ExtendedUnicode;

    let args: Vec<String> = env::args().collect();
    for (i, arg) in env::args().enumerate() {
        match arg.as_str() {
            "--cache-path" => {
                if i + 1 < args.len() {
                    cache_path = Some(args[i + 1].clone());
                } else {
                    return Err(Error::CliArgs("Bad args : --cache-path".to_string()))
                }
            },
            "--db-path" => {
                if i + 1 < args.len() {
                    db_path = Some(args[i + 1].clone());
                } else {
                    return Err(Error::CliArgs("Bad args : --db-path".to_string()))
                }
            },
            "--target-file" => {
                if i + 1 < args.len() {
                    target_file = Some(args[i + 1].as_str());
                } else {
                    return Err(Error::CliArgs("Bad args : --target-file".to_string()))
                }
            },
            "--style" => {
                if i + 1 < args.len() {
                    vp = match args[i + 1].as_str() {
                        "extended_unicode" => {
                            VisualPack::ExtendedUnicode
                        },
                        "common_unicode" => {
                            VisualPack::CommonUnicode
                        },
                        "ascii" => {
                            VisualPack::Ascii
                        },
                        _ => {
                            return Err(Error::CliArgs("Bad args : unknown style".to_string()));
                        }
                    }
                } else {
                    return Err(Error::CliArgs("Bad args : --style".to_string()))
                }
            }
            "--build" => {
                if i + 2 < args.len() {
                    let level = match args[i + 1].as_str() {
                        "none" => {
                            embedding::EmbeddingState::None
                        },
                        "name" => {
                            embedding::EmbeddingState::Name
                        },
                        "content" => {
                            embedding::EmbeddingState::Paragraphs(1)
                        },
                        "paragraphs" => {
                            if i + 3 < args.len() {
                                let n = match args[i + 2].parse::<usize>() {
                                    Ok(n) => n,
                                    Err(_) => {
                                        return Err(Error::CliArgs("Bad args : paragraphs level must be an integer".to_string()));
                                    }
                                };
                                embedding::EmbeddingState::Paragraphs(n)
                            } else {
                                return Err(Error::CliArgs("Bad args : paragraphs level must be an integer".to_string()));
                            }
                        },
                        _ => {
                            return Err(Error::CliArgs("Bad args : unknown level".to_string()));
                        }
                    };
                    let target = args[i + 2].as_str();
                    let cache_path = match cache_path {
                        Some(path) => path,
                        None => {
                            return Err(Error::CliArgs("Bad args : --cache-path is required".to_string()));
                        }
                    };
                    let db_path = match db_path {
                        Some(ref path) => path.clone(),
                        None => {
                            return Err(Error::CliArgs("Bad args : --db-path is required".to_string()));
                        }
                    };
                    build::build(target, level, &cache_path, db_path).await;
                    return Ok(());
                }
            }
            _ => {}
        }
    }

    let mut ui = UI::new(vp, db_path, cache_path);
    let path = ui.run().await;

    if let Some(path) = path {
        // Write path to target file
        if let Some(target_file) = target_file {
            std::fs::write(target_file, path.display().to_string()).expect("Can't write to target file");
        }
    }

    Ok(())
}
