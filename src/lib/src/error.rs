use std::error;
use std::fmt;
use std::io;

#[derive(Debug)]
pub enum OxenError {
    IO(io::Error),
    Basic(String),
    TomlSer(toml::ser::Error),
    TomlDe(toml::de::Error),
    URI(http::uri::InvalidUri),
    JSON(serde_json::Error),
    HTTP(reqwest::Error),
    Encoding(std::str::Utf8Error),
    DB(rocksdb::Error),
    ENV(std::env::VarError),
}

impl OxenError {
    pub fn basic_str(s: &str) -> Self {
        OxenError::Basic(String::from(s))
    }
}

impl fmt::Display for OxenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let OxenError::Basic(err) = self {
            write!(f, "{:?}", err)
        } else {
            write!(f, "{:?}", self)
        }
    }
}

// Defers to default method impls, compiler will fill in the blanks
impl error::Error for OxenError {}

// if you do not want to call .map_err, implement the std::convert::From trait
impl From<io::Error> for OxenError {
    fn from(error: io::Error) -> Self {
        OxenError::IO(error)
    }
}

impl From<String> for OxenError {
    fn from(error: String) -> Self {
        OxenError::Basic(error)
    }
}

impl From<toml::ser::Error> for OxenError {
    fn from(error: toml::ser::Error) -> Self {
        OxenError::TomlSer(error)
    }
}

impl From<toml::de::Error> for OxenError {
    fn from(error: toml::de::Error) -> Self {
        OxenError::TomlDe(error)
    }
}

impl From<http::uri::InvalidUri> for OxenError {
    fn from(error: http::uri::InvalidUri) -> Self {
        OxenError::URI(error)
    }
}

impl From<serde_json::Error> for OxenError {
    fn from(error: serde_json::Error) -> Self {
        OxenError::JSON(error)
    }
}

impl From<std::str::Utf8Error> for OxenError {
    fn from(error: std::str::Utf8Error) -> Self {
        OxenError::Encoding(error)
    }
}

impl From<reqwest::Error> for OxenError {
    fn from(error: reqwest::Error) -> Self {
        OxenError::HTTP(error)
    }
}

impl From<rocksdb::Error> for OxenError {
    fn from(error: rocksdb::Error) -> Self {
        OxenError::DB(error)
    }
}

impl From<std::env::VarError> for OxenError {
    fn from(error: std::env::VarError) -> Self {
        OxenError::ENV(error)
    }
}