use std::path::PathBuf;
use rusqlite::OptionalExtension;
use rusqlite::ToSql;
use rusqlite::params;
use rusqlite::Connection;
use rusqlite::types::FromSql;
use super::CacheItem;
use super::EmbeddingState;
use super::Id;

impl ToSql for EmbeddingState {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(match self {
            EmbeddingState::None => 0.into(),
            EmbeddingState::Name => 1.into(),
            EmbeddingState::Paragraphs(n) => (2 + (*n as u32)).into()
        })
    }
}
impl FromSql for EmbeddingState {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        Ok(match value.as_i64()? {
            0 => EmbeddingState::None,
            1 => EmbeddingState::Name,
            n => EmbeddingState::Paragraphs((n - 2) as usize)
        })
    }
}

#[derive(Debug)]
pub struct DB {
    conn: Connection
}
impl DB {
    /// If None is used as a path, the database is opened in memory
    pub fn new(path: Option<String>) -> Self {
        let conn = match path {
            Some(path) => Connection::open(path).expect("Cannot open DB"),
            None => Connection::open_in_memory().expect("Cannot open DB")
        };

        let db = Self{conn};
        db.create_tables();

        db
    }

    pub fn create_tables(&self) {
        self.conn.execute("
            CREATE TABLE IF NOT EXISTS items (
                id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
                path TEXT UNIQUE NOT NULL,
                state INTEGER NOT NULL DEFAULT 0
            );
        ", []).expect("Can't create DB tables");
    }

    pub fn insert_item(&self, item: &CacheItem) {
        self.conn.execute("INSERT INTO items (path, state) VALUES (?1, ?2)", params![item.path.to_string_lossy(), item.state]).expect("Can't insert element");
    }
    pub fn update_item(&self, id: Id, item: &CacheItem) {
        self.conn.execute("UPDATE items SET path = ?1, state = ?2 WHERE id = ?3", params![item.path.to_string_lossy(), item.state, id]).expect("Can't update element");
    }
    pub fn insert_or_update_item(&self, item: &CacheItem) {
        match self.get_id_by_path(&item.path) {
            Some(id) => self.update_item(id, item),
            None => self.insert_item(item)
        }
    }
    pub fn get_id_by_path(&self, path: &PathBuf) -> Option<Id> {
        self.conn.query_row("SELECT id FROM items WHERE path = ?1", params![path.to_str().expect("Can't do path to str")], |row| row.get(0)).optional().expect("Can't get id from path")
    }
    pub fn get_path_by_id(&self, id: Id) -> Option<PathBuf> {
        match self.conn.query_row("SELECT path FROM items WHERE id = ?1", params![id], |row| row.get::<_, String>(0)).optional().expect("Can't get path from id") {
            Some(path) => Some(path.into()),
            None => None
        }
    }
    pub fn get_state_by_path(&self, path: &PathBuf) -> Option<EmbeddingState> {
        self.conn.query_row("SELECT state FROM items WHERE path = ?1", params![path.to_string_lossy()], |row| row.get(0)).optional().expect("Can't get state from path")
    }
    pub fn get_item_by_id(&self, id: Id) -> Option<CacheItem> {
        self.conn.query_row("SELECT path, state FROM items WHERE id = ?1", params![id], |row| Ok(CacheItem {
            path: row.get::<_, String>(0).expect("Can't get path from id").into(),
            state: row.get(1).expect("Can't get state from id")
        })).optional().expect("Can't get item from id")
    }
}