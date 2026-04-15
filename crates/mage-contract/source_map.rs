use serde::{Deserialize, Serialize};
use std::fmt;

use crate::bytecode::Bytecode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceMapEntry {
    pub bytecode_offset: u32,
    pub source_start: u32,
    pub source_end: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SourceMap {
    entries: Vec<SourceMapEntry>,
}

impl SourceMap {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
        }
    }

    pub fn from_raw_entries(entries: Vec<SourceMapEntry>) -> Self {
        Self { entries }
    }

    pub fn push(&mut self, bytecode_offset: u32, source_start: u32, source_end: u32) {
        debug_assert!(
            source_start <= source_end,
            "source map ranges must be forward: {source_start}..{source_end}"
        );

        if let Some(last) = self.entries.last_mut() {
            debug_assert!(
                bytecode_offset >= last.bytecode_offset,
                "source map bytecode offsets must be nondecreasing"
            );
            if last.source_start == source_start && last.source_end == source_end {
                return;
            }
            // A no-op statement (e.g. procedure declaration) emits no bytecode,
            // so the next statement lands at the same offset. Replace in place
            // instead of appending a shadowed duplicate.
            if last.bytecode_offset == bytecode_offset {
                last.source_start = source_start;
                last.source_end = source_end;
                return;
            }
        }

        self.entries.push(SourceMapEntry {
            bytecode_offset,
            source_start,
            source_end,
        });
    }

    pub fn source_range(&self, bytecode_offset: u32) -> Option<(u32, u32)> {
        if self.entries.is_empty() {
            return None;
        }

        let index = self
            .entries
            .partition_point(|entry| entry.bytecode_offset <= bytecode_offset);

        if index == 0 {
            return None;
        }

        let entry = &self.entries[index - 1];

        Some((entry.source_start, entry.source_end))
    }

    pub fn bytecode_offset(&self, source_offset: u32) -> Option<u32> {
        if self.entries.is_empty() {
            return None;
        }

        for entry in self.entries.iter().rev() {
            if entry.source_start <= source_offset && source_offset < entry.source_end {
                return Some(entry.bytecode_offset);
            }
        }

        None
    }

    #[inline]
    pub fn entries(&self) -> &[SourceMapEntry] {
        &self.entries
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

// Map file format
//
// Header (44 bytes):
//   [0..4]    version: u32
//   [4..36]   bytecode_hash: [u8; 32] (BLAKE3 of .bytecode file)
//   [36..40]  source_map_count: u32
//   [40..44]  source_length: u32
//
// Source map entries (12 bytes each):
//   [bytecode_offset: u32, source_start: u32, source_end: u32] × source_map_count
//
// Encrypted source:
//   Source XOR BLAKE3 keystream derived from bytecode, source_length bytes

pub const MAP_VERSION: u32 = 1;
pub const MAP_HEADER_SIZE: usize = 44;
const MAP_ENTRY_SIZE: usize = 12;

#[derive(Debug, PartialEq, Eq)]
pub enum MapError {
    TooSmall,
    UnsupportedVersion { version: u32 },
    Truncated,
    InvalidSourceUtf8,
    BytecodeMismatch,
}

impl fmt::Display for MapError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MapError::TooSmall => write!(formatter, "map file too small"),
            MapError::UnsupportedVersion { version } => {
                write!(formatter, "unsupported map version: {version}")
            }
            MapError::Truncated => write!(formatter, "map file truncated"),
            MapError::InvalidSourceUtf8 => write!(formatter, "map source is not valid UTF-8"),
            MapError::BytecodeMismatch => {
                write!(formatter, "bytecode hash does not match map file")
            }
        }
    }
}

impl std::error::Error for MapError {}

#[derive(Debug)]
pub struct MapFile {
    pub bytecode_hash: [u8; 32],
    pub source_map: SourceMap,
    pub source: String,
}

fn source_keystream(bytecode: &[u8], length: usize) -> Vec<u8> {
    let mut hasher = blake3::Hasher::new_derive_key("mage map source v1");
    hasher.update(bytecode);
    let mut reader = hasher.finalize_xof();
    let mut keystream = vec![0u8; length];
    reader.fill(&mut keystream);
    keystream
}

pub fn write_map(source: &str, bytecode: &Bytecode, source_map: &SourceMap) -> Vec<u8> {
    let bytecode_hash = blake3::hash(bytecode.data());
    let entries = source_map.entries();

    let total_size = MAP_HEADER_SIZE + entries.len() * MAP_ENTRY_SIZE + source.len();
    let mut data = Vec::with_capacity(total_size);

    data.extend_from_slice(&MAP_VERSION.to_le_bytes());
    data.extend_from_slice(bytecode_hash.as_bytes());
    data.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    data.extend_from_slice(&(source.len() as u32).to_le_bytes());

    for entry in entries {
        data.extend_from_slice(&entry.bytecode_offset.to_le_bytes());
        data.extend_from_slice(&entry.source_start.to_le_bytes());
        data.extend_from_slice(&entry.source_end.to_le_bytes());
    }

    let keystream = source_keystream(bytecode.data(), source.len());
    let encrypted: Vec<u8> = source
        .as_bytes()
        .iter()
        .zip(keystream.iter())
        .map(|(s, k)| s ^ k)
        .collect();
    data.extend_from_slice(&encrypted);

    debug_assert_eq!(data.len(), total_size);
    data
}

pub fn read_map(data: &[u8], bytecode: &Bytecode) -> Result<MapFile, MapError> {
    if data.len() < MAP_HEADER_SIZE {
        return Err(MapError::TooSmall);
    }

    let version = crate::read_u32(data, 0);
    if version != MAP_VERSION {
        return Err(MapError::UnsupportedVersion { version });
    }

    let bytecode_hash: [u8; 32] = data[4..36].try_into().unwrap();
    let source_map_count = crate::read_u32(data, 36) as usize;
    let source_length = crate::read_u32(data, 40) as usize;

    let entries_end = MAP_HEADER_SIZE
        + source_map_count
            .checked_mul(MAP_ENTRY_SIZE)
            .ok_or(MapError::Truncated)?;
    let source_end = entries_end
        .checked_add(source_length)
        .ok_or(MapError::Truncated)?;

    if data.len() < source_end {
        return Err(MapError::Truncated);
    }

    let mut entries = Vec::with_capacity(source_map_count);
    let mut offset = MAP_HEADER_SIZE;
    for _ in 0..source_map_count {
        entries.push(SourceMapEntry {
            bytecode_offset: crate::read_u32(data, offset),
            source_start: crate::read_u32(data, offset + 4),
            source_end: crate::read_u32(data, offset + 8),
        });
        offset += MAP_ENTRY_SIZE;
    }

    let actual_bytecode_hash = blake3::hash(bytecode.data());
    if *actual_bytecode_hash.as_bytes() != bytecode_hash {
        return Err(MapError::BytecodeMismatch);
    }

    let keystream = source_keystream(bytecode.data(), source_length);
    let decrypted: Vec<u8> = data[entries_end..source_end]
        .iter()
        .zip(keystream.iter())
        .map(|(e, k)| e ^ k)
        .collect();

    let source = String::from_utf8(decrypted).map_err(|_| MapError::InvalidSourceUtf8)?;

    Ok(MapFile {
        bytecode_hash,
        source_map: SourceMap::from_raw_entries(entries),
        source,
    })
}
