use std::env::args as argv;
mod db;
mod error;
mod ui;
mod rank;
use error::Result;
use db::DB;
use ui::UI;

fn main() -> Result<()> {
    let mut db_path = None;
    let mut target_path = ".";

    let args: Vec<String> = argv().collect();
    for (i, arg) in argv().enumerate() {
        match arg.as_str() {
            "--db-path" => {
                if i + 1 < args.len() {
                    db_path = Some(args[i + 1].as_str());
                }
            },
            "--target-path" => {
                if i + 1 < args.len() {
                    target_path = args[i + 1].as_str();
                }
            },
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

    let mut ui = UI::new();
    ui.run()?;

    Ok(())
}
