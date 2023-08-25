use std::env;
//mod db;
mod error;
mod ui;
mod rank;
use error::Error;
use error::Result;
//use db::DB;
use ui::UI;
use ui::visual_pack::VisualPack;

fn main() -> Result<()> {
    let mut cache_path = None;
    let mut target_file = None;
    let mut vp = VisualPack::ExtendedUnicode;

    let args: Vec<String> = env::args().collect();
    for (i, arg) in env::args().enumerate() {
        match arg.as_str() {
            "--cache-path" => {
                if i + 1 < args.len() {
                    cache_path = Some(args[i + 1].as_str());
                } else {
                    return Err(Error::CliArgs("Bad args : --cache-path".to_string()))
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
            _ => {}
        }
    }

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
