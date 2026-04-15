use serde::Serialize;
use serde_text::ToSection;
use std::fmt;

use crate::line_index::LineIndex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Location {
    pub offset: usize,
    pub line: usize,
    pub column: usize,
}

impl Location {
    pub fn new(offset: usize, line: usize, column: usize) -> Self {
        Self {
            offset,
            line,
            column,
        }
    }
}

impl fmt::Display for Location {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}", self.line, self.column)
    }
}

#[derive(Debug, PartialEq, Eq, Serialize, ToSection)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DecodeError {
    InputTooLarge { length: usize },
    UnexpectedEndOfInput { offset: u32 },
    ExpectedCharacter { offset: u32, expected: char },
    ExpectedExpression { offset: u32 },
    ExpectedIdentifier { offset: u32 },
    InvalidNumber { offset: u32, literal: String },
    InvalidStringEscape { offset: u32, escape: String },
    NewlineInString { offset: u32 },
    UnmatchedDelimiter { offset: u32, delimiter: char },
    UnexpectedClosingDelimiter { offset: u32, delimiter: char },
}

impl DecodeError {
    pub fn input_too_large(length: usize) -> Self {
        Self::InputTooLarge { length }
    }

    pub fn unexpected_end_of_input(offset: u32) -> Self {
        Self::UnexpectedEndOfInput { offset }
    }

    pub fn expected_character(offset: u32, expected: char) -> Self {
        Self::ExpectedCharacter { offset, expected }
    }

    pub fn expected_expression(offset: u32) -> Self {
        Self::ExpectedExpression { offset }
    }

    pub fn expected_identifier(offset: u32) -> Self {
        Self::ExpectedIdentifier { offset }
    }

    pub fn invalid_number(offset: u32, literal: impl Into<String>) -> Self {
        Self::InvalidNumber {
            offset,
            literal: literal.into(),
        }
    }

    pub fn invalid_string_escape(offset: u32, escape: impl Into<String>) -> Self {
        Self::InvalidStringEscape {
            offset,
            escape: escape.into(),
        }
    }

    pub fn newline_in_string(offset: u32) -> Self {
        Self::NewlineInString { offset }
    }

    pub fn unmatched_delimiter(offset: u32, delimiter: char) -> Self {
        Self::UnmatchedDelimiter { offset, delimiter }
    }

    pub fn unexpected_closing_delimiter(offset: u32, delimiter: char) -> Self {
        Self::UnexpectedClosingDelimiter { offset, delimiter }
    }

    pub fn offset(&self) -> u32 {
        match self {
            DecodeError::InputTooLarge { .. } => 0,
            DecodeError::UnexpectedEndOfInput { offset } => *offset,
            DecodeError::ExpectedCharacter { offset, .. } => *offset,
            DecodeError::ExpectedExpression { offset } => *offset,
            DecodeError::ExpectedIdentifier { offset } => *offset,
            DecodeError::InvalidNumber { offset, .. } => *offset,
            DecodeError::InvalidStringEscape { offset, .. } => *offset,
            DecodeError::NewlineInString { offset } => *offset,
            DecodeError::UnmatchedDelimiter { offset, .. } => *offset,
            DecodeError::UnexpectedClosingDelimiter { offset, .. } => *offset,
        }
    }

    pub fn location(&self, line_index: &LineIndex) -> Location {
        line_index.location(self.offset() as usize)
    }

    pub fn display<'a>(&'a self, line_index: &'a LineIndex) -> DisplayError<'a> {
        DisplayError {
            error: self,
            line_index,
        }
    }
}

impl fmt::Display for DecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecodeError::InputTooLarge { length } => {
                write!(
                    formatter,
                    "input too large: {} bytes exceeds maximum of {} bytes",
                    length,
                    u32::MAX
                )
            }
            DecodeError::UnexpectedEndOfInput { .. } => {
                write!(formatter, "unexpected end of input")
            }
            DecodeError::ExpectedCharacter { expected, .. } => {
                write!(formatter, "expected '{}'", expected)
            }
            DecodeError::ExpectedExpression { .. } => {
                write!(formatter, "expected expression")
            }
            DecodeError::ExpectedIdentifier { .. } => {
                write!(formatter, "expected identifier")
            }
            DecodeError::InvalidNumber { literal, .. } => {
                write!(formatter, "invalid number literal: {}", literal)
            }
            DecodeError::InvalidStringEscape { escape, .. } => {
                write!(formatter, "invalid string escape: {}", escape)
            }
            DecodeError::NewlineInString { .. } => {
                write!(formatter, "newline in string literal")
            }
            DecodeError::UnmatchedDelimiter { delimiter, .. } => {
                write!(formatter, "unmatched '{}'", delimiter)
            }
            DecodeError::UnexpectedClosingDelimiter { delimiter, .. } => {
                write!(formatter, "unexpected closing '{}'", delimiter)
            }
        }
    }
}

impl std::error::Error for DecodeError {}

pub struct DisplayError<'a> {
    error: &'a DecodeError,
    line_index: &'a LineIndex,
}

impl fmt::Display for DisplayError<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let location = self.error.location(self.line_index);
        write!(formatter, "{}: {}", location, self.error)
    }
}
