mod mage;
pub use mage::*;

mod cli;
pub use cli::*;

mod general;
pub use general::*;

mod lsp;
pub use lsp::*;

mod parse;
pub use parse::*;

mod flatten;
pub use flatten::*;

mod jit;
pub use jit::*;

#[cfg(test)]
mod flatten_tests;
