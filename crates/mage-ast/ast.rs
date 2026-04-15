pub mod decode;
pub mod encode;

#[cfg(test)]
mod decode_test;
#[cfg(test)]
mod encode_test;

pub use mage_contract::{
    ast::*, ast_error, ast_error::*, line_index, line_index::*, source_locations,
    source_locations::*,
};

pub fn decode(code: &str) -> Result<(FlatRoot, SourceLocations), DecodeError> {
    decode::Decoder::decode(code)
}

pub fn decode_recovering(code: &str) -> (FlatRoot, SourceLocations, Vec<DecodeError>) {
    decode::Decoder::decode_recovering(code)
}
