mod flatify;
pub use flatify::*;

mod flatify_tests;

#[derive(Debug)]
pub enum Error {
    ParseError(String),
}
