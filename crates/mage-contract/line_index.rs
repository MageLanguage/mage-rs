use memchr::memchr_iter;

use crate::ast_error::Location;

/// Lines and columns are 1-based to match `Location`.
#[derive(Debug, Clone)]
pub struct LineIndex {
    line_starts: Vec<u32>,
    crlf_cr_offsets: Vec<u32>,
}

impl LineIndex {
    pub fn new(source: &str) -> Self {
        let bytes = source.as_bytes();
        let mut line_starts = Vec::with_capacity(source.len() / 40 + 1);
        let mut crlf_cr_offsets = Vec::new();
        line_starts.push(0);

        for offset in memchr_iter(b'\n', bytes) {
            if offset > 0 && bytes[offset - 1] == b'\r' {
                crlf_cr_offsets.push((offset - 1) as u32);
            }
            line_starts.push((offset + 1) as u32);
        }

        Self {
            line_starts,
            crlf_cr_offsets,
        }
    }

    #[inline]
    pub fn line_col(&self, byte_offset: usize) -> (usize, usize) {
        let offset = byte_offset as u32;

        let line_index = self.line_starts.partition_point(|&start| start <= offset);
        let line_index = line_index.saturating_sub(1);

        let line_start = self.line_starts[line_index] as usize;
        let line = line_index + 1;
        let mut column = byte_offset - line_start + 1;

        if !self.crlf_cr_offsets.is_empty()
            && self
                .crlf_cr_offsets
                .binary_search(&(byte_offset as u32))
                .is_ok()
        {
            column = column.saturating_sub(1);
        }

        (line, column)
    }

    #[inline]
    pub fn byte_offset(&self, line: usize, column: usize) -> Option<usize> {
        if line == 0 || line > self.line_starts.len() {
            return None;
        }

        let line_start = self.line_starts[line - 1] as usize;
        Some(line_start + column - 1)
    }

    #[inline]
    pub fn location(&self, byte_offset: usize) -> Location {
        let (line, column) = self.line_col(byte_offset);
        Location::new(byte_offset, line, column)
    }

    #[inline]
    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }
}

impl Default for LineIndex {
    fn default() -> Self {
        Self {
            line_starts: vec![0],
            crlf_cr_offsets: Vec::new(),
        }
    }
}
