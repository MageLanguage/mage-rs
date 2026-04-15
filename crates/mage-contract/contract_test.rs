use crate::{Bytecode, MAP_HEADER_SIZE, MAP_VERSION, MapError, SourceMap, read_map, write_map};

#[test]
fn round_trip_write_read() {
    let source = "return 0d42;";
    let bytecode = Bytecode::new(vec![1, 2, 3, 4, 5, 6, 7, 8]);
    let mut source_map = SourceMap::default();
    source_map.push(0, 0, 12);

    let map_data = write_map(source, &bytecode, &source_map);
    let map_file = read_map(&map_data, &bytecode).unwrap();

    assert_eq!(map_file.source, source);
    assert_eq!(map_file.source_map.len(), 1);
    assert_eq!(map_file.source_map.source_range(0), Some((0, 12)));
}

#[test]
fn bytecode_hash_is_correct() {
    let source = "return 0d42;";
    let bytecode = Bytecode::new(vec![1, 2, 3, 4, 5, 6, 7, 8]);
    let source_map = SourceMap::default();

    let map_data = write_map(source, &bytecode, &source_map);
    let map_file = read_map(&map_data, &bytecode).unwrap();

    let expected_bytecode_hash = blake3::hash(bytecode.data());
    assert_eq!(map_file.bytecode_hash, *expected_bytecode_hash.as_bytes());
}

#[test]
fn bytecode_hash_at_correct_offset() {
    let source = "return 0d1;";
    let bytecode = Bytecode::new(vec![1, 2, 3]);
    let source_map = SourceMap::default();

    let map_data = write_map(source, &bytecode, &source_map);

    let expected_bytecode_hash = blake3::hash(bytecode.data());
    assert_eq!(&map_data[4..36], expected_bytecode_hash.as_bytes());
}

#[test]
fn read_succeeds_with_matching_bytecode() {
    let source = "x = 0d10;";
    let bytecode = Bytecode::new(vec![10, 20, 30, 40]);
    let source_map = SourceMap::default();

    let map_data = write_map(source, &bytecode, &source_map);
    let map = read_map(&map_data, &bytecode).unwrap();

    assert_eq!(map.source, source);
}

#[test]
fn read_fails_with_tampered_bytecode() {
    let source = "x = 0d10;";
    let bytecode = Bytecode::new(vec![10, 20, 30, 40]);
    let source_map = SourceMap::default();

    let map_data = write_map(source, &bytecode, &source_map);
    let tampered = Bytecode::new(vec![10, 20, 30, 99]);

    let result = read_map(&map_data, &tampered);
    assert_eq!(result.unwrap_err(), MapError::BytecodeMismatch);
}

#[test]
fn read_fails_with_truncated_bytecode() {
    let source = "x = 0d10;";
    let bytecode = Bytecode::new(vec![10, 20, 30, 40]);
    let source_map = SourceMap::default();

    let map_data = write_map(source, &bytecode, &source_map);
    let truncated = Bytecode::new(vec![10, 20, 30]);

    let result = read_map(&map_data, &truncated);
    assert_eq!(result.unwrap_err(), MapError::BytecodeMismatch);
}

#[test]
fn multiple_source_map_entries_preserved() {
    let source = "x = 0d10;\ny = 0d20;\nreturn x + y;";
    let bytecode = Bytecode::new(vec![0u8; 64]);
    let mut source_map = SourceMap::default();
    source_map.push(0, 0, 9);
    source_map.push(24, 10, 19);
    source_map.push(48, 20, 33);

    let map_data = write_map(source, &bytecode, &source_map);
    let map_file = read_map(&map_data, &bytecode).unwrap();

    assert_eq!(map_file.source_map.len(), 3);
    assert_eq!(map_file.source_map.source_range(0), Some((0, 9)));
    assert_eq!(map_file.source_map.source_range(24), Some((10, 19)));
    assert_eq!(map_file.source_map.source_range(48), Some((20, 33)));
    assert_eq!(map_file.source_map.bytecode_offset(5), Some(0));
    assert_eq!(map_file.source_map.bytecode_offset(15), Some(24));
    assert_eq!(map_file.source_map.bytecode_offset(25), Some(48));
}

#[test]
fn empty_source_map() {
    let source = "return 0d0;";
    let bytecode = Bytecode::new(vec![1, 2, 3]);
    let source_map = SourceMap::default();

    let map_data = write_map(source, &bytecode, &source_map);
    let map_file = read_map(&map_data, &bytecode).unwrap();

    assert_eq!(map_file.source, source);
    assert_eq!(map_file.source_map.len(), 0);
    assert!(map_file.source_map.is_empty());
}

#[test]
fn empty_source() {
    let source = "";
    let bytecode = Bytecode::new(vec![0u8; 16]);
    let source_map = SourceMap::default();

    let map_data = write_map(source, &bytecode, &source_map);
    assert_eq!(map_data.len(), MAP_HEADER_SIZE);

    let map_file = read_map(&map_data, &bytecode).unwrap();
    assert_eq!(map_file.source, "");
    assert!(map_file.source_map.is_empty());
}

#[test]
fn header_has_correct_version() {
    let map_data = write_map(
        "x = 0d1;",
        &Bytecode::new(vec![1, 2]),
        &SourceMap::default(),
    );

    assert_eq!(
        u32::from_le_bytes(map_data[0..4].try_into().unwrap()),
        MAP_VERSION
    );
}

#[test]
fn written_size_matches_expected() {
    let source = "hello";
    let bytecode = Bytecode::new(vec![1, 2, 3]);
    let mut source_map = SourceMap::default();
    source_map.push(0, 0, 5);
    source_map.push(8, 3, 5);

    let map_data = write_map(source, &bytecode, &source_map);
    let expected = MAP_HEADER_SIZE + 2 * 12 + source.len();
    assert_eq!(map_data.len(), expected);
}

#[test]
fn error_too_small() {
    let result = read_map(&[0u8; 10], &Bytecode::default());
    assert_eq!(result.unwrap_err(), MapError::TooSmall);
}

#[test]
fn error_empty_input() {
    let result = read_map(&[], &Bytecode::default());
    assert_eq!(result.unwrap_err(), MapError::TooSmall);
}

#[test]
fn error_unsupported_version() {
    let mut data = vec![0u8; MAP_HEADER_SIZE];
    data[0..4].copy_from_slice(&99u32.to_le_bytes());

    let result = read_map(&data, &Bytecode::default());
    assert_eq!(
        result.unwrap_err(),
        MapError::UnsupportedVersion { version: 99 }
    );
}

#[test]
fn error_truncated_source_map_entries() {
    let mut data = vec![0u8; MAP_HEADER_SIZE];
    data[0..4].copy_from_slice(&MAP_VERSION.to_le_bytes());
    data[36..40].copy_from_slice(&5u32.to_le_bytes());
    data[40..44].copy_from_slice(&0u32.to_le_bytes());

    let result = read_map(&data, &Bytecode::default());
    assert_eq!(result.unwrap_err(), MapError::Truncated);
}

#[test]
fn error_truncated_source_text() {
    let mut data = vec![0u8; MAP_HEADER_SIZE];
    data[0..4].copy_from_slice(&MAP_VERSION.to_le_bytes());
    data[36..40].copy_from_slice(&0u32.to_le_bytes());
    data[40..44].copy_from_slice(&100u32.to_le_bytes());

    let result = read_map(&data, &Bytecode::default());
    assert_eq!(result.unwrap_err(), MapError::Truncated);
}

#[test]
fn unicode_source_preserved() {
    let source = "счётчик = 0d0;\nカウンター = 0d1;";
    let bytecode = Bytecode::new(vec![1, 2, 3, 4]);
    let mut source_map = SourceMap::default();
    source_map.push(0, 0, 10);

    let map_data = write_map(source, &bytecode, &source_map);
    let map_file = read_map(&map_data, &bytecode).unwrap();

    assert_eq!(map_file.source, source);
}

#[test]
fn large_source_map_round_trip() {
    let source = "test";
    let bytecode = Bytecode::new(vec![0u8; 32]);
    let mut source_map = SourceMap::default();
    for i in 0..1000u32 {
        source_map.push(i * 8, i, i + 1);
    }

    let map_data = write_map(source, &bytecode, &source_map);
    let map_file = read_map(&map_data, &bytecode).unwrap();

    assert_eq!(map_file.source_map.len(), 1000);
    assert_eq!(map_file.source_map.source_range(0), Some((0, 1)));
    assert_eq!(map_file.source_map.source_range(7992), Some((999, 1000)));
}

#[test]
fn different_bytecode_produces_different_hash() {
    let source = "return 0d1;";
    let source_map = SourceMap::default();

    let bytecode_a = Bytecode::new(vec![1, 2, 3]);
    let bytecode_b = Bytecode::new(vec![4, 5, 6]);

    let encoded_a = write_map(source, &bytecode_a, &source_map);
    let encoded_b = write_map(source, &bytecode_b, &source_map);

    let decoded_a = read_map(&encoded_a, &bytecode_a).unwrap();
    let decoded_b = read_map(&encoded_b, &bytecode_b).unwrap();

    assert_ne!(decoded_a.bytecode_hash, decoded_b.bytecode_hash);
}

#[test]
fn extra_trailing_bytes_are_ignored() {
    let source = "x = 0d1;";
    let bytecode = Bytecode::new(vec![1, 2, 3]);
    let source_map = SourceMap::default();

    let mut map_data = write_map(source, &bytecode, &source_map);
    map_data.extend_from_slice(&[0xFF; 100]);

    let map_file = read_map(&map_data, &bytecode).unwrap();
    assert_eq!(map_file.source, source);
}

// Source is not recoverable from .map alone

#[test]
fn source_not_stored_as_raw_text() {
    let source = "fibonacci : (procedure {n : U64}, U64) { return n; };";
    let bytecode = Bytecode::new(vec![7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
    let source_map = SourceMap::default();

    let map_data = write_map(source, &bytecode, &source_map);
    let payload = &map_data[MAP_HEADER_SIZE..];

    assert_ne!(payload, source.as_bytes());

    let raw = String::from_utf8_lossy(payload);
    assert_ne!(raw.as_ref(), source);
}

#[test]
fn source_not_recoverable_with_wrong_bytecode() {
    let source = "return 0d42;";
    let bytecode = Bytecode::new(vec![1, 2, 3, 4, 5]);
    let wrong_bytecode = Bytecode::new(vec![99, 98, 97, 96, 95]);
    let source_map = SourceMap::default();

    let map_data = write_map(source, &bytecode, &source_map);
    let result = read_map(&map_data, &wrong_bytecode);

    assert_eq!(result.unwrap_err(), MapError::BytecodeMismatch);
}

#[test]
fn source_not_recoverable_with_empty_bytecode() {
    let source = "x = 0d10;";
    let bytecode = Bytecode::new(vec![10, 20, 30]);
    let source_map = SourceMap::default();

    let map_data = write_map(source, &bytecode, &source_map);
    let result = read_map(&map_data, &Bytecode::default());

    assert_eq!(result.unwrap_err(), MapError::BytecodeMismatch);
}

#[test]
fn different_bytecode_produces_different_encrypted_source() {
    let source = "return 0d1;";
    let source_map = SourceMap::default();

    let encoded_a = write_map(source, &Bytecode::new(vec![1, 2, 3]), &source_map);
    let encoded_b = write_map(source, &Bytecode::new(vec![4, 5, 6]), &source_map);

    let payload_a = &encoded_a[MAP_HEADER_SIZE..];
    let payload_b = &encoded_b[MAP_HEADER_SIZE..];

    assert_eq!(payload_a.len(), payload_b.len());
    assert_ne!(payload_a, payload_b);
}

#[test]
fn encrypted_source_has_same_length_as_original() {
    let source = "hello world 0d42";
    let bytecode = Bytecode::new(vec![1, 2, 3, 4]);
    let source_map = SourceMap::default();

    let map_data = write_map(source, &bytecode, &source_map);

    assert_eq!(map_data.len(), MAP_HEADER_SIZE + source.len());
}

// Corruption detection

#[test]
fn corrupted_encrypted_source_detected() {
    let source = "return 0d42;";
    let bytecode = Bytecode::new(vec![1, 2, 3, 4, 5, 6, 7, 8]);
    let source_map = SourceMap::default();

    let mut map_data = write_map(source, &bytecode, &source_map);

    let last = map_data.len() - 1;
    map_data[last] ^= 0xFF;

    let result = read_map(&map_data, &bytecode);
    assert_eq!(result.unwrap_err(), MapError::InvalidSourceUtf8);
}
