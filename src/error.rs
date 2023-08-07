#[derive(Debug)]
pub struct Error(String);

impl<T: ToString> From<T> for Error {
    fn from(value: T) -> Self {
        Self(value.to_string())
    }
}

impl<T: ToString> From<Vec<T>> for Vec<Error> {
    fn from(value: Vec<T>) -> Self {
        value.into_iter().map(|e| Error::from(e)).collect()
    }
}

pub type Result<T> = std::result::Result<T, Error>;