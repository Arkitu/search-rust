use tokio::spawn;
use futures::future::join_all;
use scan_dir::ScanDir;
mod db;
mod error;
use error::Result;
use db::DB;

#[tokio::main]
async fn main() -> Result<()> {
    let db = DB::new(None).await?;
    let mut futures = Vec::new();
    ScanDir::all().walk(".", |iter| {
        for (entry, _) in iter {
            let path = entry.path().to_str().unwrap().to_owned();
            futures.push(db.insert_element(path));
        }
    })?;

    join_all(futures).await;

    Ok(())
}
