use std::sync::Arc;
use rusqlite::params;
use tokio_rusqlite::Connection;
use crate::error::Result;

#[derive(Clone)]
pub struct DB {
    conn: Arc<Connection>
}
impl<'a> DB {
    /// If None is used as a path, the database is opened in memory
    pub async fn new(path: Option<&str>) -> Result<Self> {
        let conn = match path {
            Some(path) => Connection::open(path).await?,
            None => Connection::open_in_memory().await?
        };

        let db = Self{conn: Arc::new(conn)};

        db.create_tables().await?;

        Ok(db)
    }

    pub async fn create_tables(&self) -> Result<()> {
        self.conn.call(|conn| {
            conn.execute("
                CREATE TABLE IF NOT EXISTS elements (
                    path TEXT PRIMARY KEY,
                    is_dir BOOLEAN NOT NULL DEFAULT FALSE
                );
            ", [])?;
            conn.execute("
                CREATE TABLE IF NOT EXISTS embeddings (
                    sentence TEXT PRIMARY KEY,
                    embedding BLOB NOT NULL
                );
            ", [])?;
            conn.execute("
                CREATE TABLE IF NOT EXISTS element_embeddings (
                    elementPath TEXT,
                    embeddingSentence TEXT,
                    weight REAL NOT NULL DEFAULT 1,
                    FOREIGN KEY (elementPath) REFERENCES elements (path),
                    FOREIGN KEY (embeddingSentence) REFERENCES embeddings (sentence),
                    PRIMARY KEY (elementPath, embeddingSentence)
                );
            ", [])?;

            Ok(())
        }).await?;

        Ok(())
    }

    pub async fn execute<I>(&'a self, sql: &'static str, params: I) -> Result<()>
    where
        I: Send + 'static + rusqlite::Params
    {
        self.conn.clone().call(|conn|{
            conn.execute(sql, params)
        }).await?;
        Ok(())
    }

    pub async fn insert_element(&'a self, path: String, is_dir: bool) -> Result<()> {
        self.conn.clone().call(move |conn|{
            conn.execute("INSERT INTO elements (path, is_dir) VALUES (?1, ?2)", params![path, is_dir])
        }).await?;

        Ok(())
    }
}