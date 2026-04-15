pub mod ast;
pub mod ast_error;
pub mod bytecode;
pub mod code;
pub mod compile_error;
pub mod error;
pub mod line_index;
pub mod load_error;
pub mod source_locations;
pub mod source_map;

#[cfg(test)]
mod bytecode_test;
#[cfg(test)]
mod contract_test;
#[cfg(test)]
mod line_index_test;
#[cfg(test)]
mod source_map_test;

pub use ast::*;
pub use ast_error::*;
pub use bytecode::*;
pub use code::*;
pub use compile_error::*;
pub use error::*;
pub use line_index::*;
pub use load_error::*;
pub use source_locations::*;
pub use source_map::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Load,
    Flatten,
    Compile,
    Execute,
}

impl Stage {
    pub fn name(self) -> &'static str {
        match self {
            Self::Load => "load",
            Self::Flatten => "flatten",
            Self::Compile => "compile",
            Self::Execute => "execute",
        }
    }
}

impl std::fmt::Display for Stage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.name())
    }
}

pub(crate) fn read_u32(data: &[u8], start: usize) -> u32 {
    u32::from_le_bytes([
        data[start],
        data[start + 1],
        data[start + 2],
        data[start + 3],
    ])
}
