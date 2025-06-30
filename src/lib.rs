mod flatten;
pub use flatten::*;

#[cfg(test)]
mod flatten_tests;

#[derive(Debug)]
pub enum Error {
    FlattenError(String),
}
