use crate::source_map::SourceMap;

#[test]
fn empty_source_map() {
    let map = SourceMap::default();
    assert!(map.is_empty());
    assert_eq!(map.source_range(0), None);
    assert_eq!(map.bytecode_offset(0), None);
}

#[test]
fn single_entry() {
    let mut map = SourceMap::default();
    map.push(0, 10, 20);

    assert_eq!(map.source_range(0), Some((10, 20)));
    assert_eq!(map.bytecode_offset(15), Some(0));
    assert_eq!(map.bytecode_offset(9), None);
    assert_eq!(map.bytecode_offset(20), None);
}

#[test]
fn multiple_entries_forward_lookup() {
    let mut map = SourceMap::default();
    map.push(0, 0, 10);
    map.push(24, 12, 20);
    map.push(48, 22, 30);

    assert_eq!(map.source_range(0), Some((0, 10)));
    assert_eq!(map.source_range(24), Some((12, 20)));
    assert_eq!(map.source_range(48), Some((22, 30)));

    assert_eq!(map.source_range(10), Some((0, 10)));
    assert_eq!(map.source_range(30), Some((12, 20)));
    assert_eq!(map.source_range(100), Some((22, 30)));
}

#[test]
fn reverse_lookup() {
    let mut map = SourceMap::default();
    map.push(0, 0, 10);
    map.push(24, 12, 20);
    map.push(48, 22, 30);

    assert_eq!(map.bytecode_offset(5), Some(0));
    assert_eq!(map.bytecode_offset(15), Some(24));
    assert_eq!(map.bytecode_offset(25), Some(48));

    assert_eq!(map.bytecode_offset(10), None);
    assert_eq!(map.bytecode_offset(11), None);
}

#[test]
fn deduplication_consecutive_same_source_range() {
    let mut map = SourceMap::default();
    map.push(0, 10, 20);
    map.push(8, 10, 20);
    map.push(16, 10, 20);
    map.push(24, 30, 40);

    assert_eq!(map.len(), 2);
    assert_eq!(map.source_range(0), Some((10, 20)));
    assert_eq!(map.source_range(24), Some((30, 40)));
}

#[test]
fn deduplication_same_offset_replaces_in_place() {
    let mut map = SourceMap::default();
    map.push(0, 10, 20);
    map.push(0, 30, 40);

    assert_eq!(map.len(), 1);
    assert_eq!(map.source_range(0), Some((30, 40)));
}

#[test]
fn deduplication_same_offset_then_advance() {
    let mut map = SourceMap::default();
    map.push(0, 0, 5);
    map.push(0, 5, 10);
    map.push(0, 10, 15);
    map.push(24, 20, 30);

    assert_eq!(map.len(), 2);
    assert_eq!(map.source_range(0), Some((10, 15)));
    assert_eq!(map.source_range(24), Some((20, 30)));
}

#[test]
fn deduplication_same_offset_reverse_lookup() {
    let mut map = SourceMap::default();
    map.push(0, 0, 10);
    map.push(0, 10, 20);
    map.push(24, 20, 30);

    assert_eq!(map.bytecode_offset(5), None);
    assert_eq!(map.bytecode_offset(15), Some(0));
    assert_eq!(map.bytecode_offset(25), Some(24));
}

#[test]
fn deduplication_same_range_after_different_is_kept() {
    let mut map = SourceMap::default();
    map.push(0, 10, 20);
    map.push(24, 30, 40);
    map.push(48, 10, 20);

    assert_eq!(map.len(), 3);
}

#[test]
fn lookup_before_first_entry() {
    let mut map = SourceMap::default();
    map.push(24, 10, 20);
    map.push(48, 30, 40);

    assert_eq!(map.source_range(0), None);
    assert_eq!(map.source_range(16), None);
    assert_eq!(map.source_range(23), None);

    assert_eq!(map.source_range(24), Some((10, 20)));
    assert_eq!(map.source_range(30), Some((10, 20)));
    assert_eq!(map.source_range(48), Some((30, 40)));
}

#[test]
fn serialization_round_trip() {
    let mut map = SourceMap::default();
    map.push(0, 0, 15);
    map.push(24, 20, 35);

    let json = serde_json::to_string(&map).unwrap();
    let restored: SourceMap = serde_json::from_str(&json).unwrap();

    assert_eq!(restored.len(), 2);
    assert_eq!(restored.source_range(0), Some((0, 15)));
    assert_eq!(restored.source_range(24), Some((20, 35)));
}

#[test]
fn out_of_order_source_start() {
    let mut map = SourceMap::default();
    map.push(0, 50, 60);
    map.push(10, 10, 20);
    map.push(20, 30, 40);

    assert_eq!(map.bytecode_offset(55), Some(0));
    assert_eq!(map.bytecode_offset(15), Some(10));
    assert_eq!(map.bytecode_offset(35), Some(20));
}
