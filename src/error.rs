#[derive(Debug)]
pub enum Error {
    TokioRusqlite(tokio_rusqlite::Error),
    Io(std::io::Error)
}

impl From<tokio_rusqlite::Error> for Error {
    fn from(value: tokio_rusqlite::Error) -> Self {
        Self::TokioRusqlite(value)
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

// impl<T: ToString> From<T> for Error {
//     fn from(value: T) -> Self {
//         Self(value.to_string())
//     }
// }

pub type Result<T> = std::result::Result<T, Error>;