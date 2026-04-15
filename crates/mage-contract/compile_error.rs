use crate::{ast_error::Location, line_index::LineIndex};
use serde::Serialize;
use serde_text::ToSection;
use std::fmt;

#[derive(Debug, PartialEq, Eq, Serialize, ToSection)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CompileError {
    UnsupportedExpression { offset: u32 },
    UnsupportedIndex { offset: u32 },
    InvalidNumberLiteral { literal: String, offset: u32 },
    UndefinedVariable { name: String, offset: u32 },
    UndefinedProcedure { name: String, offset: u32 },
    IfArgumentCount { found: usize, offset: u32 },
    IfSecondArgumentNotBlock { offset: u32 },
    WhileArgumentCount { found: usize, offset: u32 },
    WhileSecondArgumentNotBlock { offset: u32 },
    UnresolvedWhileTargetLabel,
    UnresolvedWhileEndLabel,
    UnresolvedIfEndLabel,
    MissingProcedureBytecodeOffset,
    BreakOutsideBlock { offset: u32 },
    BreakUnresolvedTarget { name: String, offset: u32 },
    ContinueOutsideWhile { offset: u32 },
    ContinueUnresolvedTarget { name: String, offset: u32 },
    UnsupportedCallTarget { offset: u32 },
    MultipleVariableExpressionNotCall { offset: u32 },
    DuplicateProcedure { name: String, offset: u32 },
    DuplicateConstant { name: String, offset: u32 },
    AssignmentToConstant { name: String, offset: u32 },
    InvalidConstantTarget { offset: u32 },
    InvalidAssignmentTarget { offset: u32 },
    InvalidMultipleAssignmentTarget { offset: u32 },
    InvalidProcedureDeclaration { offset: u32 },
    ReturnOutsideProcedure { offset: u32 },
}

impl CompileError {
    pub fn unsupported_expression(offset: u32) -> Self {
        Self::UnsupportedExpression { offset }
    }

    pub fn unsupported_index(offset: u32) -> Self {
        Self::UnsupportedIndex { offset }
    }

    pub fn invalid_number_literal(offset: u32, literal: impl Into<String>) -> Self {
        Self::InvalidNumberLiteral {
            literal: literal.into(),
            offset,
        }
    }

    pub fn undefined_variable(offset: u32, name: impl Into<String>) -> Self {
        Self::UndefinedVariable {
            name: name.into(),
            offset,
        }
    }

    pub fn undefined_procedure(offset: u32, name: impl Into<String>) -> Self {
        Self::UndefinedProcedure {
            name: name.into(),
            offset,
        }
    }

    pub fn if_argument_count(offset: u32, found: usize) -> Self {
        Self::IfArgumentCount { found, offset }
    }

    pub fn if_second_argument_not_block(offset: u32) -> Self {
        Self::IfSecondArgumentNotBlock { offset }
    }

    pub fn while_argument_count(offset: u32, found: usize) -> Self {
        Self::WhileArgumentCount { found, offset }
    }

    pub fn while_second_argument_not_block(offset: u32) -> Self {
        Self::WhileSecondArgumentNotBlock { offset }
    }

    pub fn break_outside_block(offset: u32) -> Self {
        Self::BreakOutsideBlock { offset }
    }

    pub fn break_unresolved_target(offset: u32, name: impl Into<String>) -> Self {
        Self::BreakUnresolvedTarget {
            name: name.into(),
            offset,
        }
    }

    pub fn continue_outside_while(offset: u32) -> Self {
        Self::ContinueOutsideWhile { offset }
    }

    pub fn continue_unresolved_target(offset: u32, name: impl Into<String>) -> Self {
        Self::ContinueUnresolvedTarget {
            name: name.into(),
            offset,
        }
    }

    pub fn unsupported_call_target(offset: u32) -> Self {
        Self::UnsupportedCallTarget { offset }
    }

    pub fn multiple_variable_expression_not_call(offset: u32) -> Self {
        Self::MultipleVariableExpressionNotCall { offset }
    }

    pub fn duplicate_procedure(offset: u32, name: impl Into<String>) -> Self {
        Self::DuplicateProcedure {
            name: name.into(),
            offset,
        }
    }

    pub fn duplicate_constant(offset: u32, name: impl Into<String>) -> Self {
        Self::DuplicateConstant {
            name: name.into(),
            offset,
        }
    }

    pub fn assignment_to_constant(offset: u32, name: impl Into<String>) -> Self {
        Self::AssignmentToConstant {
            name: name.into(),
            offset,
        }
    }

    pub fn invalid_constant_target(offset: u32) -> Self {
        Self::InvalidConstantTarget { offset }
    }

    pub fn invalid_assignment_target(offset: u32) -> Self {
        Self::InvalidAssignmentTarget { offset }
    }

    pub fn invalid_multiple_assignment_target(offset: u32) -> Self {
        Self::InvalidMultipleAssignmentTarget { offset }
    }

    pub fn invalid_procedure_declaration(offset: u32) -> Self {
        Self::InvalidProcedureDeclaration { offset }
    }

    pub fn return_outside_procedure(offset: u32) -> Self {
        Self::ReturnOutsideProcedure { offset }
    }

    pub fn offset(&self) -> Option<u32> {
        match self {
            CompileError::UnsupportedExpression { offset, .. }
            | CompileError::UnsupportedIndex { offset, .. }
            | CompileError::InvalidNumberLiteral { offset, .. }
            | CompileError::UndefinedVariable { offset, .. }
            | CompileError::UndefinedProcedure { offset, .. }
            | CompileError::IfArgumentCount { offset, .. }
            | CompileError::IfSecondArgumentNotBlock { offset, .. }
            | CompileError::WhileArgumentCount { offset, .. }
            | CompileError::WhileSecondArgumentNotBlock { offset, .. }
            | CompileError::BreakOutsideBlock { offset, .. }
            | CompileError::BreakUnresolvedTarget { offset, .. }
            | CompileError::ContinueOutsideWhile { offset, .. }
            | CompileError::ContinueUnresolvedTarget { offset, .. }
            | CompileError::UnsupportedCallTarget { offset, .. }
            | CompileError::MultipleVariableExpressionNotCall { offset, .. }
            | CompileError::DuplicateProcedure { offset, .. }
            | CompileError::DuplicateConstant { offset, .. }
            | CompileError::AssignmentToConstant { offset, .. }
            | CompileError::InvalidConstantTarget { offset, .. }
            | CompileError::InvalidAssignmentTarget { offset, .. }
            | CompileError::InvalidMultipleAssignmentTarget { offset, .. }
            | CompileError::InvalidProcedureDeclaration { offset, .. }
            | CompileError::ReturnOutsideProcedure { offset, .. } => Some(*offset),
            CompileError::UnresolvedWhileTargetLabel
            | CompileError::UnresolvedWhileEndLabel
            | CompileError::UnresolvedIfEndLabel
            | CompileError::MissingProcedureBytecodeOffset => None,
        }
    }

    pub fn location(&self, line_index: &LineIndex) -> Option<Location> {
        self.offset()
            .map(|offset| line_index.location(offset as usize))
    }

    pub fn display<'a>(&'a self, line_index: &'a LineIndex) -> DisplayCompileError<'a> {
        DisplayCompileError {
            error: self,
            line_index,
        }
    }
}

impl fmt::Display for CompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompileError::UnsupportedExpression { .. } => {
                write!(formatter, "unsupported expression type")
            }
            CompileError::UnsupportedIndex { .. } => {
                write!(formatter, "unsupported index type")
            }
            CompileError::InvalidNumberLiteral { literal, .. } => {
                write!(formatter, "invalid number literal: {}", literal)
            }
            CompileError::UndefinedVariable { name, .. } => {
                write!(formatter, "undefined variable: {}", name)
            }
            CompileError::UndefinedProcedure { name, .. } => {
                write!(formatter, "undefined procedure: {}", name)
            }
            CompileError::IfArgumentCount { found, .. } => write!(
                formatter,
                "if expects 2 arguments (condition and block), found {}",
                found
            ),
            CompileError::IfSecondArgumentNotBlock { .. } => {
                write!(formatter, "if second argument must be a block source")
            }
            CompileError::WhileArgumentCount { found, .. } => write!(
                formatter,
                "while expects 2 arguments (condition and block), found {}",
                found
            ),
            CompileError::WhileSecondArgumentNotBlock { .. } => {
                write!(formatter, "while second argument must be a block source")
            }
            CompileError::UnresolvedWhileTargetLabel => {
                write!(formatter, "failed to resolve while target label")
            }
            CompileError::UnresolvedWhileEndLabel => {
                write!(formatter, "failed to resolve while end label")
            }
            CompileError::UnresolvedIfEndLabel => {
                write!(formatter, "failed to resolve if end label")
            }
            CompileError::MissingProcedureBytecodeOffset => {
                write!(formatter, "procedure has no bytecode offset")
            }
            CompileError::BreakOutsideBlock { .. } => {
                write!(formatter, "break used outside of a while or if block")
            }
            CompileError::BreakUnresolvedTarget { name, .. } => {
                write!(formatter, "break target not found: {}", name)
            }
            CompileError::ContinueOutsideWhile { .. } => {
                write!(formatter, "continue used outside of a while loop")
            }
            CompileError::ContinueUnresolvedTarget { name, .. } => {
                write!(formatter, "continue target not found: {}", name)
            }
            CompileError::UnsupportedCallTarget { .. } => {
                write!(formatter, "call target must be an identifier")
            }
            CompileError::MultipleVariableExpressionNotCall { .. } => {
                write!(
                    formatter,
                    "multiple variable assignment requires a procedure call expression"
                )
            }
            CompileError::DuplicateProcedure { name, .. } => {
                write!(formatter, "duplicate procedure: {}", name)
            }
            CompileError::DuplicateConstant { name, .. } => {
                write!(formatter, "constant already defined: {}", name)
            }
            CompileError::AssignmentToConstant { name, .. } => {
                write!(formatter, "cannot assign to constant: {}", name)
            }
            CompileError::InvalidConstantTarget { .. } => {
                write!(formatter, "constant target must be an identifier")
            }
            CompileError::InvalidAssignmentTarget { .. } => {
                write!(formatter, "assignment target must be an identifier")
            }
            CompileError::InvalidMultipleAssignmentTarget { .. } => {
                write!(formatter, "multiple assignment targets must be identifiers")
            }
            CompileError::InvalidProcedureDeclaration { .. } => {
                write!(formatter, "invalid procedure declaration")
            }
            CompileError::ReturnOutsideProcedure { .. } => {
                write!(formatter, "return used outside of a procedure")
            }
        }
    }
}

impl std::error::Error for CompileError {}

pub struct DisplayCompileError<'a> {
    error: &'a CompileError,
    line_index: &'a LineIndex,
}

impl fmt::Display for DisplayCompileError<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.error.location(self.line_index) {
            Some(location) => write!(formatter, "{}: {}", location, self.error),
            None => write!(formatter, "{}", self.error),
        }
    }
}
