use serde::Serialize;
use serde_text::ToSection;
use std::{fmt, io};

use crate::bytecode::ReadError;

#[derive(Debug, PartialEq, Eq, Serialize, ToSection)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LoadError {
    NotFound {
        message: String,
    },
    PermissionDenied {
        message: String,
    },
    InvalidData {
        message: String,
    },
    UnsupportedExtension {
        path: String,
    },
    UnsupportedStage {
        stage: String,
    },
    InvalidBytecode {
        #[serde(skip_serializing_if = "Option::is_none")]
        code: Option<u64>,
    },
}

impl From<io::Error> for LoadError {
    fn from(error: io::Error) -> Self {
        let message = error.to_string();
        match error.kind() {
            io::ErrorKind::NotFound => LoadError::NotFound { message },
            io::ErrorKind::PermissionDenied => LoadError::PermissionDenied { message },
            _ => LoadError::InvalidData { message },
        }
    }
}

impl From<ReadError> for LoadError {
    fn from(error: ReadError) -> Self {
        match error {
            ReadError::UnexpectedEndOfBytecode => LoadError::InvalidBytecode { code: None },
            ReadError::InvalidCode { code } => LoadError::InvalidBytecode { code: Some(code) },
        }
    }
}

impl fmt::Display for LoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::NotFound { message } => write!(formatter, "{}", message),
            LoadError::PermissionDenied { message } => write!(formatter, "{}", message),
            LoadError::InvalidData { message } => write!(formatter, "{}", message),
            LoadError::UnsupportedExtension { path } => {
                write!(
                    formatter,
                    "unsupported file extension for '{}', expected '.hex' or '.bytecode'",
                    path
                )
            }
            LoadError::UnsupportedStage { stage } => {
                write!(
                    formatter,
                    "'--stage {}' is only supported for '.hex' inputs",
                    stage
                )
            }
            LoadError::InvalidBytecode { code: None } => {
                write!(formatter, "unexpected end of bytecode")
            }
            LoadError::InvalidBytecode { code: Some(code) } => {
                write!(formatter, "invalid instruction code: {}", code)
            }
        }
    }
}

impl std::error::Error for LoadError {}
