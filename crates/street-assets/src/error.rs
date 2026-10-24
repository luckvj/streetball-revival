use std::{fmt, io};

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    /// The bytes do not have the layout the parser expects.
    Format(String),
    /// A named file or archive entry does not exist.
    NotFound(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub(crate) fn format(msg: impl Into<String>) -> Self {
        Error::Format(msg.into())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "i/o error: {e}"),
            Error::Format(msg) => write!(f, "unexpected data: {msg}"),
            Error::NotFound(name) => write!(f, "not found: {name}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}
