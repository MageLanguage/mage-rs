mod flatten;
pub use flatten::*;

mod flatten_tests;

#[derive(Debug)]
pub enum Error {
    ParseError(String),
}
