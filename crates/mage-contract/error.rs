use serde::Serialize;
use serde_text::ToSection;
use std::{fmt, result};

use crate::{Stage, ast_error::DecodeError, compile_error::CompileError, load_error::LoadError};

#[derive(Debug, Serialize)]
#[serde(tag = "type", content = "error", rename_all = "snake_case")]
pub enum Error {
    Load(LoadError),
    Flatten(DecodeError),
    Compile(CompileError),
}

impl Error {
    pub fn stage(&self) -> Stage {
        match self {
            Error::Load(_) => Stage::Load,
            Error::Flatten(_) => Stage::Flatten,
            Error::Compile(_) => Stage::Compile,
        }
    }

    pub fn to_section(&self) -> serde_text::Section {
        let prefix = self.stage().name();
        match self {
            Error::Load(error) => error.to_section().prefix_name(prefix),
            Error::Flatten(error) => error.to_section().prefix_name(prefix),
            Error::Compile(error) => error.to_section().prefix_name(prefix),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Load(error) => write!(formatter, "{}", error),
            Error::Flatten(error) => write!(formatter, "{}", error),
            Error::Compile(error) => write!(formatter, "{}", error),
        }
    }
}

impl std::error::Error for Error {}

impl From<LoadError> for Error {
    fn from(error: LoadError) -> Self {
        Error::Load(error)
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Error::Load(LoadError::from(error))
    }
}

impl From<DecodeError> for Error {
    fn from(error: DecodeError) -> Self {
        Error::Flatten(error)
    }
}

impl From<CompileError> for Error {
    fn from(error: CompileError) -> Self {
        Error::Compile(error)
    }
}

pub type Result<T> = result::Result<T, Error>;
