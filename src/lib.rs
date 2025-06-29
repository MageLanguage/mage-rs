mod jit;
pub use jit::*;

mod virtual_machine;
pub use virtual_machine::*;

mod flatify;
pub use flatify::*;

mod tests;

#[derive(Debug)]
pub enum MageError {
    ParseError(String),
    TypeError(String),
    RuntimeError(String),
    Utf8Error(std::str::Utf8Error),
    JitError(zydis::Status),
}

#[derive(Debug)]
pub enum Error {
    ParseError(String),
}
