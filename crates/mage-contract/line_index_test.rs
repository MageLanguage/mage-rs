use crate::line_index::LineIndex;

#[test]
fn empty_source() {
    let index = LineIndex::new("");
    assert_eq!(index.line_col(0), (1, 1));
    assert_eq!(index.line_count(), 1);
}

#[test]
fn single_line() {
    let index = LineIndex::new("hello");
    assert_eq!(index.line_col(0), (1, 1));
    assert_eq!(index.line_col(4), (1, 5));
}

#[test]
fn multiple_lines() {
    let index = LineIndex::new("aaa\nbbb\nccc");
    assert_eq!(index.line_col(0), (1, 1));
    assert_eq!(index.line_col(3), (1, 4));
    assert_eq!(index.line_col(4), (2, 1));
    assert_eq!(index.line_col(7), (2, 4));
    assert_eq!(index.line_col(8), (3, 1));
    assert_eq!(index.line_col(10), (3, 3));
    assert_eq!(index.line_count(), 3);
}

#[test]
fn trailing_newline() {
    let index = LineIndex::new("a\nb\n");
    assert_eq!(index.line_col(0), (1, 1));
    assert_eq!(index.line_col(2), (2, 1));
    assert_eq!(index.line_col(4), (3, 1));
    assert_eq!(index.line_count(), 3);
}

#[test]
fn empty_lines() {
    let index = LineIndex::new("\n\n\n");
    assert_eq!(index.line_col(0), (1, 1));
    assert_eq!(index.line_col(1), (2, 1));
    assert_eq!(index.line_col(2), (3, 1));
    assert_eq!(index.line_col(3), (4, 1));
    assert_eq!(index.line_count(), 4);
}

#[test]
fn byte_offset_round_trip() {
    let index = LineIndex::new("aaa\nbbb\nccc");
    for offset in 0..11 {
        let (line, column) = index.line_col(offset);
        assert_eq!(index.byte_offset(line, column), Some(offset));
    }
}

#[test]
fn byte_offset_out_of_range() {
    let index = LineIndex::new("hello");
    assert_eq!(index.byte_offset(0, 1), None);
    assert_eq!(index.byte_offset(2, 1), None);
}

#[test]
fn location_helper() {
    let index = LineIndex::new("ab\ncd");
    let location = index.location(3);
    assert_eq!(location.offset, 3);
    assert_eq!(location.line, 2);
    assert_eq!(location.column, 1);
}

#[test]
fn unicode_source() {
    // 'с'=2b, 'ч'=2b, 'ё'=2b, 'т'=2b, '\n'=1b → line 2 starts at byte 9
    let index = LineIndex::new("счёт\nОК");
    assert_eq!(index.line_col(0), (1, 1));
    assert_eq!(index.line_col(9), (2, 1));
    assert_eq!(index.line_count(), 2);
}

#[test]
fn crlf_line_count() {
    let index = LineIndex::new("aaa\r\nbbb\r\nccc");
    assert_eq!(index.line_count(), 3);
}

#[test]
fn crlf_line_col_visible_characters() {
    // '\r' before '\n' is clamped to the last visible column
    let index = LineIndex::new("hello\r\nworld\r\n");
    assert_eq!(index.line_col(0), (1, 1));
    assert_eq!(index.line_col(4), (1, 5));
    assert_eq!(index.line_col(5), (1, 5));
    assert_eq!(index.line_col(7), (2, 1));
    assert_eq!(index.line_col(11), (2, 5));
    assert_eq!(index.line_col(12), (2, 5));
}

#[test]
fn crlf_byte_offset_round_trip() {
    let index = LineIndex::new("ab\r\ncd\r\n");
    assert_eq!(index.byte_offset(1, 1), Some(0));
    assert_eq!(index.byte_offset(1, 2), Some(1));
    assert_eq!(index.byte_offset(2, 1), Some(4));
    assert_eq!(index.byte_offset(2, 2), Some(5));
}

#[test]
fn crlf_location_helper() {
    let index = LineIndex::new("ab\r\ncd");
    let location = index.location(4);
    assert_eq!(location.offset, 4);
    assert_eq!(location.line, 2);
    assert_eq!(location.column, 1);
}

#[test]
fn mixed_line_endings() {
    // '\r' before '\n' is clamped to the last visible column
    let index = LineIndex::new("aa\nbb\r\ncc\n");
    assert_eq!(index.line_col(0), (1, 1));
    assert_eq!(index.line_col(3), (2, 1));
    assert_eq!(index.line_col(4), (2, 2));
    assert_eq!(index.line_col(5), (2, 2));
    assert_eq!(index.line_col(7), (3, 1));
    assert_eq!(index.line_count(), 4);
}
