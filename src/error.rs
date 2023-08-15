#[derive(Debug)]
pub enum Error {
    //Rusqlite(rusqlite::Error),
    Io(std::io::Error),
    RustBert(rust_bert::RustBertError),
    KdTree(kdtree::ErrorKind),
    CliArgs(String)
}

//impl From<rusqlite::Error> for Error {
//    fn from(value: rusqlite::Error) -> Self {
//        Self::Rusqlite(value)
//    }
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

impl From<kdtree::ErrorKind> for Error {
    fn from(value: kdtree::ErrorKind) -> Self {
        Self::KdTree(value)
    }
}

// impl<T: ToString> From<T> for Error {
//     fn from(value: T) -> Self {
//         Self(value.to_string())
//     }
// }

pub type Result<T> = std::result::Result<T, Error>;
