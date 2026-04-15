use memchr::{memchr, memchr3};
use rustc_hash::FxHashMap;
use unicode_ident::{is_xid_continue, is_xid_start};

use crate::{
    DecodeError, FlatAssign, FlatBinaryOperation, FlatBinaryOperationKind, FlatCall,
    FlatExpression, FlatIndex, FlatMember, FlatMultipleVariable, FlatRoot, FlatSource, FlatString,
    SourceLocations, SourceLocationsState,
};

/// Maximum input length the decoder can handle. All byte offsets in the AST
/// are stored as `u32`, so inputs beyond this limit would cause silent
/// truncation and corrupt source locations.
const MAX_INPUT_LENGTH: usize = u32::MAX as usize;

#[derive(Clone, Copy)]
struct CursorState {
    offset: usize,
}

#[derive(Clone, Copy)]
struct StringTableState {
    strings_len: usize,
    buffer_len: usize,
    expressions_len: usize,
    sources_len: usize,
    indices_len: usize,
    locations_state: SourceLocationsState,
}

struct Cursor<'a> {
    code: &'a str,
    offset: usize,
}

impl<'a> Cursor<'a> {
    fn new(code: &'a str) -> Self {
        Self { code, offset: 0 }
    }

    #[inline]
    fn at_end(&self) -> bool {
        self.offset >= self.code.len()
    }

    #[inline]
    fn save(&self) -> CursorState {
        CursorState {
            offset: self.offset,
        }
    }

    #[inline]
    fn restore(&mut self, state: CursorState) {
        self.offset = state.offset;
    }

    #[inline]
    fn peek_char(&self) -> Option<char> {
        self.code[self.offset..].chars().next()
    }

    #[inline]
    fn advance_char(&mut self) -> Option<char> {
        let character = self.peek_char()?;
        self.offset += character.len_utf8();
        Some(character)
    }

    #[inline]
    fn advance_byte(&mut self) {
        self.offset += 1;
    }

    #[inline]
    fn peek_byte(&self) -> Option<u8> {
        self.code.as_bytes().get(self.offset).copied()
    }

    #[inline]
    fn slice(&self, start: usize, end: usize) -> &'a str {
        &self.code[start..end]
    }
}

pub struct Decoder<'a> {
    cursor: Cursor<'a>,
    root: FlatRoot,
    locations: SourceLocations,
    interned: FxHashMap<&'a str, u32>,
    errors: Vec<DecodeError>,
}

impl<'a> Decoder<'a> {
    pub fn decode(code: &'a str) -> Result<(FlatRoot, SourceLocations), DecodeError> {
        let (root, locations, errors) = Self::decode_recovering(code);
        if let Some(error) = errors.into_iter().next() {
            Err(error)
        } else {
            Ok((root, locations))
        }
    }

    pub fn decode_recovering(code: &'a str) -> (FlatRoot, SourceLocations, Vec<DecodeError>) {
        if code.len() > MAX_INPUT_LENGTH {
            return (
                FlatRoot::default(),
                SourceLocations::default(),
                vec![DecodeError::input_too_large(code.len())],
            );
        }

        let len = code.len();
        let est_strings = (len / 16).max(4);
        let est_expressions = (len / 12).max(4);
        let est_indices = (len / 8).max(4);
        let est_sources = (len / 32).max(2);

        let mut root = FlatRoot::default();
        root.strings.reserve(est_strings);
        root.buffer.reserve(len / 4);
        root.expressions.reserve(est_expressions);
        root.indices.reserve(est_indices);
        root.sources.reserve(est_sources);

        let mut decoder = Decoder {
            cursor: Cursor::new(code),
            root,
            locations: SourceLocations::default(),
            interned: FxHashMap::with_capacity_and_hasher(est_strings, Default::default()),
            errors: Vec::new(),
        };

        decoder.root.sources.push(FlatSource::default());
        decoder.locations.push_source_empty();
        let (indexes, locations) = decoder.parse_source_file();
        let start = Self::usize_to_u32(decoder.root.indices.len());
        decoder.root.indices.extend(indexes);
        let end = Self::usize_to_u32(decoder.root.indices.len());
        decoder.root.sources[0] = FlatSource { start, end };
        decoder.locations.set_source(0, &locations);
        let errors = decoder.errors;
        (decoder.root, decoder.locations, errors)
    }

    #[inline]
    fn usize_to_u32(value: usize) -> u32 {
        debug_assert!(
            value <= MAX_INPUT_LENGTH,
            "decoder offset exceeded u32 range"
        );
        value as u32
    }

    #[inline]
    fn at_end(&self) -> bool {
        self.cursor.at_end()
    }

    #[inline]
    fn peek_next_byte(&self) -> Option<u8> {
        self.cursor
            .code
            .as_bytes()
            .get(self.cursor.offset + 1)
            .copied()
    }

    #[inline]
    fn check(&self, byte: u8) -> bool {
        self.cursor.peek_byte() == Some(byte)
    }

    #[inline]
    fn consume(&mut self, byte: u8) -> bool {
        if self.cursor.peek_byte() == Some(byte) {
            self.cursor.offset += 1;
            true
        } else {
            false
        }
    }

    #[inline]
    fn advance(&mut self) -> Option<char> {
        self.cursor.advance_char()
    }

    #[inline]
    fn advance_byte(&mut self) {
        self.cursor.advance_byte();
    }

    #[inline]
    fn offset(&self) -> u32 {
        Self::usize_to_u32(self.cursor.offset)
    }

    #[inline]
    fn offset_at(&self, offset: usize) -> u32 {
        Self::usize_to_u32(offset)
    }

    #[inline]
    fn save(&self) -> CursorState {
        self.cursor.save()
    }

    #[inline]
    fn restore(&mut self, state: CursorState) {
        self.cursor.restore(state);
    }

    #[inline]
    fn is_single_equals_assignment(&self) -> bool {
        self.check(b'=') && !matches!(self.peek_next_byte(), Some(b'=') | Some(b'>'))
    }

    fn expect_closing(&mut self, byte: u8, open_offset: usize) -> Result<(), DecodeError> {
        if self.consume(byte) {
            Ok(())
        } else {
            let opening = match byte {
                b')' => '(',
                b'}' => '{',
                _ => byte as char,
            };
            Err(DecodeError::unmatched_delimiter(
                self.offset_at(open_offset),
                opening,
            ))
        }
    }

    fn skip_to_sync_point(&mut self) {
        let mut depth: u32 = 0;
        let bytes = self.cursor.code.as_bytes();
        while !self.at_end() {
            let remaining = &bytes[self.cursor.offset..];

            let position = match (
                memchr3(b';', b'{', b'}', remaining),
                memchr3(b'(', b')', b'"', remaining),
            ) {
                (Some(a), Some(b)) => a.min(b),
                (Some(a), None) => a,
                (None, Some(b)) => b,
                (None, None) => {
                    self.cursor.offset = bytes.len();
                    break;
                }
            };

            self.cursor.offset += position;

            match bytes[self.cursor.offset] {
                b';' => {
                    if depth == 0 {
                        self.cursor.offset += 1;
                        break;
                    }
                    self.cursor.offset += 1;
                }
                b'}' => {
                    if depth == 0 {
                        break;
                    }
                    depth = depth.saturating_sub(1);
                    self.cursor.offset += 1;
                }
                b'{' | b'(' => {
                    depth += 1;
                    self.cursor.offset += 1;
                }
                b')' => {
                    depth = depth.saturating_sub(1);
                    self.cursor.offset += 1;
                }
                b'"' => {
                    self.cursor.offset += 1;
                    self.skip_string_content();
                }
                _ => {
                    self.cursor.offset += 1;
                }
            }
        }
    }

    fn skip_string_content(&mut self) {
        let bytes = self.cursor.code.as_bytes();
        loop {
            let remaining = &bytes[self.cursor.offset..];

            let position = match memchr3(b'"', b'\\', b'\n', remaining) {
                Some(position) => position,
                None => {
                    if let Some(cr_position) = memchr(b'\r', remaining) {
                        self.cursor.offset += cr_position;
                    } else {
                        self.cursor.offset = bytes.len();
                    }
                    return;
                }
            };

            if let Some(cr_position) = memchr(b'\r', &remaining[..position]) {
                self.cursor.offset += cr_position;
                return;
            }

            self.cursor.offset += position;

            match bytes[self.cursor.offset] {
                b'"' => {
                    self.cursor.offset += 1;
                    return;
                }
                b'\\' => {
                    self.cursor.offset += 1;
                    if !self.at_end() {
                        self.cursor.offset += 1;
                    }
                }
                b'\n' => return,
                _ => {
                    self.cursor.offset += 1;
                }
            }
        }
    }

    fn skip_whitespace(&mut self) {
        let bytes = self.cursor.code.as_bytes();
        while self.cursor.offset < bytes.len() {
            let byte = bytes[self.cursor.offset];
            if byte.is_ascii_whitespace() {
                self.cursor.offset += 1;
            } else if byte >= 0x80 {
                match self.cursor.peek_char() {
                    Some(character) if character.is_whitespace() => {
                        self.cursor.offset += character.len_utf8();
                    }
                    _ => break,
                }
            } else {
                break;
            }
        }
    }

    fn push_string(&mut self, start: usize, end: usize) -> u32 {
        let slice = self.cursor.slice(start, end);

        if let Some(&index) = self.interned.get(slice) {
            return index;
        }

        let index = Self::usize_to_u32(self.root.strings.len());
        let buffer_start = Self::usize_to_u32(self.root.buffer.len());
        self.root.buffer.push_str(slice);
        let buffer_end = Self::usize_to_u32(self.root.buffer.len());
        self.root.strings.push(FlatString {
            start: buffer_start,
            end: buffer_end,
        });
        self.interned.insert(slice, index);
        index
    }

    fn push_expression(&mut self, expression: FlatExpression, offset: u32) -> FlatIndex {
        let index = Self::usize_to_u32(self.root.expressions.len());
        self.root.expressions.push(expression);
        self.locations.push_expression(offset);
        FlatIndex::Expression(index)
    }

    fn push_indices(&mut self, indices: Vec<FlatIndex>) -> (u32, u32) {
        let start = Self::usize_to_u32(self.root.indices.len());
        self.root.indices.extend(indices);
        let end = Self::usize_to_u32(self.root.indices.len());
        (start, end)
    }

    fn push_source(&mut self, indices: Vec<FlatIndex>, locations: Vec<u32>) -> u32 {
        let (start, end) = self.push_indices(indices);
        let index = Self::usize_to_u32(self.root.sources.len());
        self.root.sources.push(FlatSource { start, end });
        self.locations.push_source(&locations);
        index
    }

    fn save_string_table(&self) -> StringTableState {
        StringTableState {
            strings_len: self.root.strings.len(),
            buffer_len: self.root.buffer.len(),
            expressions_len: self.root.expressions.len(),
            sources_len: self.root.sources.len(),
            indices_len: self.root.indices.len(),
            locations_state: self.locations.save(),
        }
    }

    /// Rolls back parser tables to a previously saved snapshot after a
    /// speculative parse fails.
    ///
    /// The `interned` keys borrow from `self.cursor.code`, not `self.root.buffer`,
    /// so reading discarded string contents before truncating the buffer is safe.
    fn restore_string_table(&mut self, state: StringTableState) {
        for string_index in state.strings_len..self.root.strings.len() {
            let string = &self.root.strings[string_index];
            let key: &str = &self.root.buffer[string.start as usize..string.end as usize];
            self.interned.remove(key);
        }
        self.root.strings.truncate(state.strings_len);
        self.root.buffer.truncate(state.buffer_len);
        self.root.expressions.truncate(state.expressions_len);
        self.root.sources.truncate(state.sources_len);
        self.root.indices.truncate(state.indices_len);
        self.locations.restore(&state.locations_state);
    }

    #[inline]
    fn is_identifier_start(character: char) -> bool {
        character == '_' || is_xid_start(character)
    }

    #[inline]
    fn is_identifier_char(character: char) -> bool {
        is_xid_continue(character)
    }

    #[inline]
    fn is_ascii_identifier_start(byte: u8) -> bool {
        matches!(byte, b'_' | b'a'..=b'z' | b'A'..=b'Z')
    }

    #[inline]
    fn is_ascii_identifier_continue(byte: u8) -> bool {
        matches!(byte, b'_' | b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9')
    }

    fn number_radix(&self) -> Option<u32> {
        let bytes = self.cursor.code.as_bytes();
        let offset = self.cursor.offset;
        if offset + 1 >= bytes.len() || bytes[offset] != b'0' {
            return None;
        }

        match bytes[offset + 1] {
            b'b' => Some(2),
            b'o' => Some(8),
            b'd' => Some(10),
            b'x' => Some(16),
            _ => None,
        }
    }

    #[inline]
    fn is_ascii_number_digit(byte: u8, radix: u32) -> bool {
        match byte {
            b'0'..=b'9' => (byte - b'0') < radix as u8,
            b'a'..=b'z' => radix > 10 && (byte - b'a') < (radix - 10) as u8,
            b'A'..=b'Z' => radix > 10 && (byte - b'A') < (radix - 10) as u8,
            _ => false,
        }
    }

    fn can_start_atom(&self) -> bool {
        match self.cursor.peek_byte() {
            None => false,
            Some(b'(' | b'{' | b'"' | b'0') => true,
            Some(byte) if byte < 0x80 => Self::is_ascii_identifier_start(byte),
            Some(_) => match self.cursor.peek_char() {
                Some(character) => Self::is_identifier_start(character),
                None => false,
            },
        }
    }

    fn parse_identifier(&mut self) -> Option<(usize, usize)> {
        let start = self.cursor.offset;
        let bytes = self.cursor.code.as_bytes();

        if self.cursor.offset >= bytes.len() {
            return None;
        }

        let first_byte = bytes[self.cursor.offset];
        if first_byte < 0x80 {
            if !Self::is_ascii_identifier_start(first_byte) {
                return None;
            }
            self.cursor.offset += 1;
        } else {
            match self.cursor.peek_char() {
                Some(character) if Self::is_identifier_start(character) => {
                    self.cursor.offset += character.len_utf8();
                }
                _ => return None,
            }
        }

        while self.cursor.offset < bytes.len() {
            let byte = bytes[self.cursor.offset];
            if byte < 0x80 {
                if !Self::is_ascii_identifier_continue(byte) {
                    break;
                }
                self.cursor.offset += 1;
                continue;
            }

            match self.cursor.peek_char() {
                Some(character) if Self::is_identifier_char(character) => {
                    self.cursor.offset += character.len_utf8();
                }
                _ => break,
            }
        }

        Some((start, self.cursor.offset))
    }

    fn parse_number_literal(&mut self) -> Result<u32, DecodeError> {
        let start = self.cursor.offset;

        let radix = if let Some(radix) = self.number_radix() {
            self.advance_byte();
            self.advance_byte();
            radix
        } else if self.check(b'0') {
            self.advance_byte();
            return Ok(self.push_string(start, self.cursor.offset));
        } else {
            return Err(DecodeError::invalid_number(self.offset_at(start), ""));
        };

        let mut saw_digit = false;
        let bytes = self.cursor.code.as_bytes();

        while self.cursor.offset < bytes.len() {
            let byte = bytes[self.cursor.offset];
            if byte == b'_' {
                self.cursor.offset += 1;
                continue;
            }
            if !Self::is_ascii_number_digit(byte, radix) {
                break;
            }
            saw_digit = true;
            self.cursor.offset += 1;
        }

        if !saw_digit {
            let literal = self.cursor.slice(start, self.cursor.offset).to_string();
            return Err(DecodeError::invalid_number(self.offset_at(start), literal));
        }

        if self.has_identifier_continuation() {
            self.consume_identifier_continuation();
            let literal = self.cursor.slice(start, self.cursor.offset).to_string();
            return Err(DecodeError::invalid_number(self.offset_at(start), literal));
        }

        Ok(self.push_string(start, self.cursor.offset))
    }

    fn has_identifier_continuation(&self) -> bool {
        match self.cursor.code.as_bytes().get(self.cursor.offset) {
            Some(&byte) if byte < 0x80 => Self::is_ascii_identifier_continue(byte),
            Some(_) => self
                .cursor
                .peek_char()
                .is_some_and(Self::is_identifier_char),
            None => false,
        }
    }

    fn consume_identifier_continuation(&mut self) {
        self.cursor.offset += self.cursor.peek_char().map_or(0, |c| c.len_utf8());
        while self.cursor.offset < self.cursor.code.len() {
            let byte = self.cursor.code.as_bytes()[self.cursor.offset];
            if byte < 0x80 {
                if !Self::is_ascii_identifier_continue(byte) {
                    break;
                }
                self.cursor.offset += 1;
            } else {
                match self.cursor.peek_char() {
                    Some(character) if Self::is_identifier_char(character) => {
                        self.cursor.offset += character.len_utf8();
                    }
                    _ => break,
                }
            }
        }
    }

    fn parse_string_literal(&mut self) -> Result<u32, DecodeError> {
        let start = self.cursor.offset;

        if !self.consume(b'"') {
            return Err(DecodeError::expected_character(self.offset(), '"'));
        }

        let bytes = self.cursor.code.as_bytes();
        loop {
            let remaining = &bytes[self.cursor.offset..];

            let position = match memchr3(b'"', b'\\', b'\n', remaining) {
                Some(position) => position,
                None => return self.string_unterminated(remaining, start),
            };

            if let Some(cr_position) = memchr(b'\r', &remaining[..position]) {
                self.cursor.offset += cr_position;
                return Err(DecodeError::newline_in_string(self.offset()));
            }

            self.cursor.offset += position;

            match bytes[self.cursor.offset] {
                b'"' => {
                    self.cursor.offset += 1;
                    return Ok(self.push_string(start, self.cursor.offset));
                }
                b'\n' => return Err(DecodeError::newline_in_string(self.offset())),
                b'\\' => self.handle_string_escape()?,
                _ => self.cursor.offset += 1,
            }
        }
    }

    fn string_unterminated(&mut self, remaining: &[u8], start: usize) -> Result<u32, DecodeError> {
        if let Some(position) = memchr(b'\r', remaining) {
            self.cursor.offset += position;
            return Err(DecodeError::newline_in_string(self.offset()));
        }
        self.cursor.offset = self.cursor.code.len();
        Err(DecodeError::unmatched_delimiter(self.offset_at(start), '"'))
    }

    fn handle_string_escape(&mut self) -> Result<(), DecodeError> {
        let escape_offset = self.cursor.offset;
        self.cursor.offset += 1;

        let escaped = self
            .advance()
            .ok_or_else(|| DecodeError::unexpected_end_of_input(self.offset()))?;

        match escaped {
            'n' | '"' | '\\' => Ok(()),
            '\n' | '\r' => Err(DecodeError::newline_in_string(self.offset())),
            other => Err(DecodeError::invalid_string_escape(
                self.offset_at(escape_offset),
                format!("\\{}", other),
            )),
        }
    }

    fn parse_member_chain(&mut self, mut base: FlatIndex) -> Result<FlatIndex, DecodeError> {
        while self.consume(b'.') {
            let dot_offset = self.offset() - 1;
            if let Some((start, end)) = self.parse_identifier() {
                let index = self.push_string(start, end);
                base = self.push_expression(
                    FlatExpression::Member(FlatMember {
                        name: base,
                        expression: FlatIndex::Identifier(index),
                    }),
                    dot_offset,
                );
            } else {
                return Err(DecodeError::expected_identifier(self.offset()));
            }
        }
        Ok(base)
    }

    /// Attempts to parse a single statement and push it to `indices`/`locations`.
    /// Returns `true` if the caller should check for a trailing semicolon.
    /// Returns `false` if the loop iteration should be skipped (error recovery
    /// or zero-progress advance already happened).
    fn parse_one_statement(
        &mut self,
        indices: &mut Vec<FlatIndex>,
        locations: &mut Vec<u32>,
    ) -> bool {
        let saved = self.save();
        match self.parse_statement() {
            Ok(index) => {
                if self.cursor.offset == saved.offset {
                    let error = DecodeError::expected_expression(self.offset());
                    self.errors.push(error);
                    self.advance();
                    return false;
                }

                if !matches!(index, FlatIndex::None) {
                    indices.push(index);
                    locations.push(self.offset_at(saved.offset));
                }
                true
            }
            Err(error) => {
                self.errors.push(error);
                self.skip_to_sync_point();
                false
            }
        }
    }

    fn parse_source_file(&mut self) -> (Vec<FlatIndex>, Vec<u32>) {
        let mut indices = Vec::with_capacity(8);
        let mut locations = Vec::with_capacity(8);

        while !self.at_end() {
            self.skip_whitespace();

            match self.cursor.peek_byte() {
                None => break,
                Some(b';') => {
                    self.cursor.offset += 1;
                    continue;
                }
                Some(b')') => {
                    let error = DecodeError::unexpected_closing_delimiter(self.offset(), ')');
                    self.errors.push(error);
                    self.cursor.offset += 1;
                    continue;
                }
                Some(b'}') => {
                    let error = DecodeError::unexpected_closing_delimiter(self.offset(), '}');
                    self.errors.push(error);
                    self.cursor.offset += 1;
                    continue;
                }
                _ => {}
            }

            if !self.parse_one_statement(&mut indices, &mut locations) {
                continue;
            }

            self.skip_whitespace();

            if !self.at_end() && !self.consume(b';') {
                let error = DecodeError::expected_character(self.offset(), ';');
                self.errors.push(error);
            }
        }

        (indices, locations)
    }

    fn parse_source(&mut self) -> (Vec<FlatIndex>, Vec<u32>) {
        let mut indices = Vec::with_capacity(4);
        let mut locations = Vec::with_capacity(4);

        while !self.at_end() {
            self.skip_whitespace();

            match self.cursor.peek_byte() {
                None => break,
                Some(b'}') => break,
                Some(b';') => {
                    self.cursor.offset += 1;
                    continue;
                }
                Some(b')') => {
                    let error = DecodeError::unexpected_closing_delimiter(self.offset(), ')');
                    self.errors.push(error);
                    self.cursor.offset += 1;
                    continue;
                }
                _ => {}
            }

            if !self.parse_one_statement(&mut indices, &mut locations) {
                continue;
            }

            self.skip_whitespace();

            if !self.at_end() && !self.check(b'}') && !self.consume(b';') {
                let error = DecodeError::expected_character(self.offset(), ';');
                self.errors.push(error);
            }
        }

        (indices, locations)
    }

    fn parse_statement(&mut self) -> Result<FlatIndex, DecodeError> {
        if let Some((identifier_start, identifier_end)) = self.parse_identifier() {
            let name_index = self.push_string(identifier_start, identifier_end);
            let mut name = FlatIndex::Identifier(name_index);
            name = self.parse_member_chain(name)?;

            self.skip_whitespace();

            if self.consume(b':') {
                self.skip_whitespace();
                let expression_index = self.parse_expression()?;
                return Ok(self.push_expression(
                    FlatExpression::Constant(FlatAssign {
                        name,
                        expression: expression_index,
                    }),
                    self.offset_at(identifier_start),
                ));
            }

            if self.is_single_equals_assignment() {
                self.advance_byte();
                self.skip_whitespace();
                let expression_index = self.parse_expression()?;
                return Ok(self.push_expression(
                    FlatExpression::Variable(FlatAssign {
                        name,
                        expression: expression_index,
                    }),
                    self.offset_at(identifier_start),
                ));
            }

            if let Some(result) = self.try_parse_multiple_variable(name, identifier_start)? {
                return Ok(result);
            }

            let index = self.parse_call_with_atom(name, self.offset_at(identifier_start))?;
            return self.parse_binary_rhs(index, 0);
        }

        self.parse_expression()
    }

    fn try_parse_multiple_variable(
        &mut self,
        first_name: FlatIndex,
        identifier_start: usize,
    ) -> Result<Option<FlatIndex>, DecodeError> {
        if !self.check(b',') {
            return Ok(None);
        }

        let multi_saved = self.save();
        let string_table_saved = self.save_string_table();
        let mut names = vec![first_name];
        let mut valid = true;

        while self.consume(b',') {
            self.skip_whitespace();
            let Some((name_start, name_end)) = self.parse_identifier() else {
                valid = false;
                break;
            };
            let name_index = self.push_string(name_start, name_end);
            let mut next_name = FlatIndex::Identifier(name_index);
            next_name = self.parse_member_chain(next_name)?;
            names.push(next_name);
            self.skip_whitespace();
        }

        if valid && self.is_single_equals_assignment() {
            self.advance_byte();
            self.skip_whitespace();
            let expression_index = self.parse_expression()?;
            let (names_start, names_end) = self.push_indices(names);
            return Ok(Some(self.push_expression(
                FlatExpression::MultipleVariable(FlatMultipleVariable {
                    names_start,
                    names_end,
                    expression: expression_index,
                }),
                self.offset_at(identifier_start),
            )));
        }

        self.restore(multi_saved);
        self.restore_string_table(string_table_saved);
        Ok(None)
    }

    fn peek_binary_operator(&self) -> Option<(FlatBinaryOperationKind, u8, u8)> {
        match self.cursor.peek_byte() {
            Some(b'*') => Some((FlatBinaryOperationKind::Multiply, 5, 6)),
            Some(b'/') => Some((FlatBinaryOperationKind::Divide, 5, 6)),
            Some(b'%') => Some((FlatBinaryOperationKind::Modulo, 5, 6)),
            Some(b'+') => Some((FlatBinaryOperationKind::Add, 3, 4)),
            Some(b'-') => Some((FlatBinaryOperationKind::Subtract, 3, 4)),
            Some(b'<') => match self.peek_next_byte() {
                Some(b'=') => Some((FlatBinaryOperationKind::LessThanOrEqual, 1, 2)),
                _ => Some((FlatBinaryOperationKind::LessThan, 1, 2)),
            },
            Some(b'>') => match self.peek_next_byte() {
                Some(b'=') => Some((FlatBinaryOperationKind::GreaterThanOrEqual, 1, 2)),
                _ => Some((FlatBinaryOperationKind::GreaterThan, 1, 2)),
            },
            Some(b'=') => match self.peek_next_byte() {
                Some(b'=') => Some((FlatBinaryOperationKind::Equal, 1, 2)),
                _ => None,
            },
            Some(b'!') => match self.peek_next_byte() {
                Some(b'=') => Some((FlatBinaryOperationKind::NotEqual, 1, 2)),
                _ => None,
            },
            _ => None,
        }
    }

    fn consume_binary_operator(&mut self, kind: FlatBinaryOperationKind) {
        self.advance_byte();
        match kind {
            FlatBinaryOperationKind::LessThanOrEqual
            | FlatBinaryOperationKind::GreaterThanOrEqual
            | FlatBinaryOperationKind::Equal
            | FlatBinaryOperationKind::NotEqual => {
                self.advance_byte();
            }
            _ => {}
        }
    }

    fn parse_expression(&mut self) -> Result<FlatIndex, DecodeError> {
        let left = self.parse_call()?;
        self.parse_binary_rhs(left, 0)
    }

    fn parse_binary_rhs(
        &mut self,
        mut left: FlatIndex,
        min_binding_power: u8,
    ) -> Result<FlatIndex, DecodeError> {
        loop {
            let saved = self.save();
            self.skip_whitespace();

            let Some((kind, left_binding_power, right_binding_power)) = self.peek_binary_operator()
            else {
                self.restore(saved);
                break;
            };

            if left_binding_power < min_binding_power {
                self.restore(saved);
                break;
            }

            let operator_offset = self.offset();
            self.consume_binary_operator(kind);
            self.skip_whitespace();
            let right_atom = self.parse_call()?;
            let right = self.parse_binary_rhs(right_atom, right_binding_power)?;

            left = self.push_expression(
                FlatExpression::BinaryOperation(FlatBinaryOperation { kind, left, right }),
                operator_offset,
            );

            // Comparisons are non-associative: allow at most one per expression level
            if left_binding_power <= 2 {
                break;
            }
        }

        Ok(left)
    }

    fn parse_call(&mut self) -> Result<FlatIndex, DecodeError> {
        self.skip_whitespace();
        let atom_offset = self.offset();
        let atom = self.parse_atom()?;
        self.parse_call_with_atom(atom, atom_offset)
    }

    fn parse_call_with_atom(
        &mut self,
        atom: FlatIndex,
        atom_offset: u32,
    ) -> Result<FlatIndex, DecodeError> {
        if !matches!(&atom, FlatIndex::Identifier(_) | FlatIndex::Expression(_)) {
            return Ok(atom);
        }

        let saved = self.save();
        self.skip_whitespace();

        if !self.can_start_atom() {
            self.restore(saved);
            return Ok(atom);
        }

        let mut arguments = Vec::with_capacity(4);
        arguments.push(self.parse_expression()?);
        self.skip_whitespace();

        while self.consume(b',') {
            self.skip_whitespace();
            arguments.push(self.parse_expression()?);
            self.skip_whitespace();
        }

        let (arguments_start, arguments_end) = self.push_indices(arguments);
        Ok(self.push_expression(
            FlatExpression::Call(FlatCall {
                name: atom,
                arguments_start,
                arguments_end,
            }),
            atom_offset,
        ))
    }

    fn parse_atom(&mut self) -> Result<FlatIndex, DecodeError> {
        self.skip_whitespace();

        if self.check(b'(') {
            let open_offset = self.cursor.offset;
            self.advance_byte();
            self.skip_whitespace();
            let inner = self.parse_statement()?;
            self.skip_whitespace();
            self.expect_closing(b')', open_offset)?;
            return Ok(inner);
        }

        if self.check(b'{') {
            let open_offset = self.cursor.offset;
            self.advance_byte();
            let (source_indices, source_locations) = self.parse_source();
            self.expect_closing(b'}', open_offset)?;
            let source_index = self.push_source(source_indices, source_locations);
            return Ok(FlatIndex::Source(source_index));
        }

        if self.check(b'"') {
            let string_index = self.parse_string_literal()?;
            return Ok(FlatIndex::String(string_index));
        }

        if self.number_radix().is_some() || self.check(b'0') {
            let number_index = self.parse_number_literal()?;
            return Ok(FlatIndex::Number(number_index));
        }

        if let Some((start, end)) = self.parse_identifier() {
            let index = self.push_string(start, end);
            return self.parse_member_chain(FlatIndex::Identifier(index));
        }

        if self.at_end() {
            return Ok(FlatIndex::None);
        }

        Err(DecodeError::expected_expression(self.offset()))
    }
}
