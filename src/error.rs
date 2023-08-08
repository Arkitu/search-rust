#[derive(Debug)]
pub struct Error(String);

impl<T: ToString> From<T> for Error {
    fn from(value: T) -> Self {
        Self(value.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;