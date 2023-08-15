use std::env;
use std::path::PathBuf;
//mod db;
mod error;
mod ui;
mod rank;
use error::Error;
use error::Result;
use rank::embedding::Embedder;
//use db::DB;
use ui::UI;
use ui::visual_pack::VisualPack;

fn main() -> Result<()> {
    let mut embedder = Embedder::new()?;
    embedder.embed_to_cache(&["coucou c'est la vie"], &PathBuf::from("/coucou.txt"))?;
    println!("{:#?}", embedder.cache);
    Ok(())
}

fn maint() -> Result<()> {
    let mut db_path = None;
    let mut target_file = None;
    let mut vp = VisualPack::ExtendedUnicode;

    let args: Vec<String> = env::args().collect();
    for (i, arg) in env::args().enumerate() {
        match arg.as_str() {
            "--db-path" => {
                if i + 1 < args.len() {
                    db_path = Some(args[i + 1].as_str());
                } else {
                    return Err(Error::CliArgs("Bad args : --db_path".to_string()))
                }
            },
            "--target-file" => {
                if i + 1 < args.len() {
                    target_file = Some(args[i + 1].as_str());
                } else {
                    return Err(Error::CliArgs("Bad args : --target_file".to_string()))
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
            _ => {}
        }
    }

    // let db = DB::new(db_path).await?;
    // let mut futures = Vec::new();
    // if let Err(e) = ScanDir::all().walk(target_path, |iter| {
    //     for (entry, _) in iter {
    //         let is_dir = entry.file_type().expect("cannot determine file type").is_dir();
    //         let path = entry.path().to_str().unwrap().to_owned();
    //         futures.push(db.insert_element(path, is_dir));
    //     }
    // }) {
    //     //return Err(Error::from(e[0].to_string()));
    // };
    // join_all(futures).await;

    let mut ui = UI::new(vp)?;
    let path = ui.run()?;

    if let Some(path) = path {
        // Write path to target file
        if let Some(target_file) = target_file {
            std::fs::write(target_file, path.display().to_string())?;
        }
    }

    Ok(())
}
