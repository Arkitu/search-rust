use std::sync::{RwLockWriteGuard, PoisonError};

#[derive(Debug)]
pub enum Error {
    //Rusqlite(rusqlite::Error),
    Io(std::io::Error),
    RustBert(rust_bert::RustBertError),
    ScanDir(scan_dir::Error),
    ScanDirVec(Vec<scan_dir::Error>),
    KdTree(kdtree::ErrorKind),
    LockPoison(String),
    CliArgs(String)
}

//impl From<rusqlite::Error> for Error {
//    fn from(value: rusqlite::Error) -> Self {
//        Self::Rusqlite(value)
//    }²
//}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<rust_bert::RustBertError> for Error {
    fn from(value: rust_bert::RustBertError) -> Self {
        Self::RustBert(value)
    }
}

impl From<scan_dir::Error> for Error {
    fn from(value: scan_dir::Error) -> Self {
        Self::ScanDir(value)
    }
}

impl From<Vec<scan_dir::Error>> for Error {
    fn from(value: Vec<scan_dir::Error>) -> Self {
        Self::ScanDirVec(value)
    }
}

impl From<kdtree::ErrorKind> for Error {
    fn from(value: kdtree::ErrorKind) -> Self {
        Self::KdTree(value)
    }
}

impl<T> From<PoisonError<T>> for Error {
    fn from(value: PoisonError<T>) -> Self {
        Self::LockPoison(value.to_string())
    }
}

// impl<T: ToString> From<T> for Error {
//     fn from(value: T) -> Self {
//         Self(value.to_string())
//     }
// }

pub type Result<T> = std::result::Result<T, Error>;
