//! High-performance binary package manifest specification and encoder/decoder.
//!
//! Provides the canonical `SGMF` binary format used in `.saga` asset packages.
//! Supports instant O(1) arithmetic section seeking without full deserialization.

use crate::ProtocolError;

/// Binary magic signature for Saga package manifests: "SGMF".
pub const MANIFEST_MAGIC: [u8; 4] = *b"SGMF";

/// Current manifest format revision.
pub const MANIFEST_VERSION: u16 = 1;

/// Fixed byte size of the manifest header.
pub const HEADER_SIZE: usize = 64;

/// Fixed byte size of each section TOC entry (32 bytes = 1 << 5).
pub const TOC_ENTRY_SIZE: usize = 32;

/// Section-level feature flags.
pub const SECTION_FLAG_AUDIO: u16 = 1 << 0;

/// High-level manifest data structure describing a `.saga` package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageManifest {
    pub title: String,
    pub author: String,
    pub description: String,
    pub total_chars: u32,
    pub chunks_count: u32,
    pub sections: Vec<PackageSection>,
}

/// Metadata for a single chapter section inside a `.saga` package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageSection {
    pub title: String,
    pub volume_id: u16,
    pub chunk_index: u16,
    pub chunk_offset: u32,
    pub byte_length: u32,
    pub char_count: u32,
    pub has_audio: bool,
    pub audio_duration_ms: u32,
    pub timeline_offset: u32,
}

impl PackageManifest {
    /// Encodes the complete package manifest into the canonical binary `SGMF` format.
    pub fn encode(&self) -> Vec<u8> {
        let mut string_pool = Vec::new();

        // 1. Pack global strings
        let title_bytes = self.title.as_bytes();
        let title_offset = string_pool.len() as u32;
        let title_len = title_bytes.len() as u16;
        string_pool.extend_from_slice(title_bytes);

        let author_bytes = self.author.as_bytes();
        let author_offset = string_pool.len() as u32;
        let author_len = author_bytes.len() as u16;
        string_pool.extend_from_slice(author_bytes);

        let desc_bytes = self.description.as_bytes();
        let desc_offset = string_pool.len() as u32;
        let desc_len = desc_bytes.len() as u16;
        string_pool.extend_from_slice(desc_bytes);

        // 2. Prepare TOC entries and append section titles to string pool
        let sections_count = self.sections.len() as u32;
        let toc_offset = HEADER_SIZE as u64;
        let toc_size = self.sections.len() * TOC_ENTRY_SIZE;
        let string_pool_offset = toc_offset + toc_size as u64;

        let mut toc_bytes = Vec::with_capacity(toc_size);
        for sec in &self.sections {
            let s_title_bytes = sec.title.as_bytes();
            let s_title_offset = string_pool.len() as u32;
            let s_title_len = s_title_bytes.len() as u16;
            string_pool.extend_from_slice(s_title_bytes);

            let mut flags = 0u16;
            if sec.has_audio {
                flags |= SECTION_FLAG_AUDIO;
            }

            // Write 32-byte entry
            toc_bytes.extend_from_slice(&sec.chunk_index.to_le_bytes()); // 0..2
            toc_bytes.extend_from_slice(&flags.to_le_bytes()); // 2..4
            toc_bytes.extend_from_slice(&sec.chunk_offset.to_le_bytes()); // 4..8
            toc_bytes.extend_from_slice(&sec.byte_length.to_le_bytes()); // 8..12
            toc_bytes.extend_from_slice(&sec.char_count.to_le_bytes()); // 12..16
            toc_bytes.extend_from_slice(&sec.volume_id.to_le_bytes()); // 16..18
            toc_bytes.extend_from_slice(&s_title_len.to_le_bytes()); // 18..20
            toc_bytes.extend_from_slice(&s_title_offset.to_le_bytes()); // 20..24
            toc_bytes.extend_from_slice(&sec.audio_duration_ms.to_le_bytes()); // 24..28
            toc_bytes.extend_from_slice(&sec.timeline_offset.to_le_bytes()); // 28..32
        }

        // 3. Assemble Header (64 bytes)
        let mut buffer = Vec::with_capacity(HEADER_SIZE + toc_size + string_pool.len());
        buffer.extend_from_slice(&MANIFEST_MAGIC); // 0..4
        buffer.extend_from_slice(&MANIFEST_VERSION.to_le_bytes()); // 4..6
        buffer.extend_from_slice(&0u16.to_le_bytes()); // 6..8: flags
        buffer.extend_from_slice(&sections_count.to_le_bytes()); // 8..12
        buffer.extend_from_slice(&self.chunks_count.to_le_bytes()); // 12..16
        buffer.extend_from_slice(&self.total_chars.to_le_bytes()); // 16..20
        buffer.extend_from_slice(&0u32.to_le_bytes()); // 20..24: reserved
        buffer.extend_from_slice(&toc_offset.to_le_bytes()); // 24..32
        buffer.extend_from_slice(&string_pool_offset.to_le_bytes()); // 32..40
        buffer.extend_from_slice(&title_offset.to_le_bytes()); // 40..44
        buffer.extend_from_slice(&title_len.to_le_bytes()); // 44..46
        buffer.extend_from_slice(&0u16.to_le_bytes()); // 46..48: volume_count/reserved
        buffer.extend_from_slice(&author_offset.to_le_bytes()); // 48..52
        buffer.extend_from_slice(&author_len.to_le_bytes()); // 52..54
        buffer.extend_from_slice(&0u16.to_le_bytes()); // 54..56: reserved
        buffer.extend_from_slice(&desc_offset.to_le_bytes()); // 56..60
        buffer.extend_from_slice(&desc_len.to_le_bytes()); // 60..62
        buffer.extend_from_slice(&0u16.to_le_bytes()); // 62..64: reserved

        debug_assert_eq!(buffer.len(), HEADER_SIZE);

        // 4. Combine Header + TOC + String Pool
        buffer.extend_from_slice(&toc_bytes);
        buffer.extend_from_slice(&string_pool);

        buffer
    }

    /// Deserializes a full package manifest from bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let view = PackageManifestView::parse(bytes)?;

        let mut sections = Vec::with_capacity(view.sections_count() as usize);
        for i in 0..view.sections_count() {
            sections.push(view.section_at(i)?);
        }

        Ok(Self {
            title: view.title()?.to_string(),
            author: view.author()?.to_string(),
            description: view.description()?.to_string(),
            total_chars: view.total_chars(),
            chunks_count: view.chunks_count(),
            sections,
        })
    }
}

/// Zero-copy read-only view over a binary `SGMF` manifest buffer.
/// Enables O(1) arithmetic section lookup without allocating section arrays.
pub struct PackageManifestView<'a> {
    bytes: &'a [u8],
    sections_count: u32,
    chunks_count: u32,
    total_chars: u32,
    toc_offset: usize,
    string_pool_offset: usize,
    title_offset: usize,
    title_len: usize,
    author_offset: usize,
    author_len: usize,
    desc_offset: usize,
    desc_len: usize,
}

impl<'a> PackageManifestView<'a> {
    /// Validates the header and constructs an O(1) query view.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, ProtocolError> {
        if bytes.len() < HEADER_SIZE {
            return Err(ProtocolError::Serialization("manifest buffer is smaller than 64-byte header".into()));
        }

        if bytes[0..4] != MANIFEST_MAGIC {
            return Err(ProtocolError::Serialization("invalid magic signature; expected SGMF".into()));
        }

        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        if version != MANIFEST_VERSION {
            return Err(ProtocolError::Serialization(format!(
                "unsupported manifest version {version}; expected {MANIFEST_VERSION}"
            )));
        }

        let sections_count = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
        let chunks_count = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
        let total_chars = u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
        let toc_offset = u64::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27], bytes[28], bytes[29], bytes[30], bytes[31]]) as usize;
        let string_pool_offset = u64::from_le_bytes([bytes[32], bytes[33], bytes[34], bytes[35], bytes[36], bytes[37], bytes[38], bytes[39]]) as usize;

        let title_offset = u32::from_le_bytes([bytes[40], bytes[41], bytes[42], bytes[43]]) as usize;
        let title_len = u16::from_le_bytes([bytes[44], bytes[45]]) as usize;

        let author_offset = u32::from_le_bytes([bytes[48], bytes[49], bytes[50], bytes[51]]) as usize;
        let author_len = u16::from_le_bytes([bytes[52], bytes[53]]) as usize;

        let desc_offset = u32::from_le_bytes([bytes[56], bytes[57], bytes[58], bytes[59]]) as usize;
        let desc_len = u16::from_le_bytes([bytes[60], bytes[61]]) as usize;

        let min_required = toc_offset + (sections_count as usize * TOC_ENTRY_SIZE);
        if bytes.len() < min_required {
            return Err(ProtocolError::Serialization(format!(
                "manifest truncated: required at least {min_required} bytes, got {}",
                bytes.len()
            )));
        }

        Ok(Self {
            bytes,
            sections_count,
            chunks_count,
            total_chars,
            toc_offset,
            string_pool_offset,
            title_offset,
            title_len,
            author_offset,
            author_len,
            desc_offset,
            desc_len,
        })
    }

    pub fn sections_count(&self) -> u32 {
        self.sections_count
    }

    pub fn chunks_count(&self) -> u32 {
        self.chunks_count
    }

    pub fn total_chars(&self) -> u32 {
        self.total_chars
    }

    pub fn title(&self) -> Result<&'a str, ProtocolError> {
        self.get_string(self.title_offset, self.title_len)
    }

    pub fn author(&self) -> Result<&'a str, ProtocolError> {
        self.get_string(self.author_offset, self.author_len)
    }

    pub fn description(&self) -> Result<&'a str, ProtocolError> {
        self.get_string(self.desc_offset, self.desc_len)
    }

    /// O(1) arithmetic seek to a specific section entry.
    pub fn section_at(&self, index: u32) -> Result<PackageSection, ProtocolError> {
        if index >= self.sections_count {
            return Err(ProtocolError::InvalidIdentifier(format!(
                "section index {index} out of bounds (count {})",
                self.sections_count
            )));
        }

        // Fast arithmetic seek: index * 32 = index << 5
        let entry_start = self.toc_offset + ((index as usize) << 5);
        let entry_bytes = &self.bytes[entry_start..entry_start + TOC_ENTRY_SIZE];

        let chunk_index = u16::from_le_bytes([entry_bytes[0], entry_bytes[1]]);
        let flags = u16::from_le_bytes([entry_bytes[2], entry_bytes[3]]);
        let chunk_offset = u32::from_le_bytes([entry_bytes[4], entry_bytes[5], entry_bytes[6], entry_bytes[7]]);
        let byte_length = u32::from_le_bytes([entry_bytes[8], entry_bytes[9], entry_bytes[10], entry_bytes[11]]);
        let char_count = u32::from_le_bytes([entry_bytes[12], entry_bytes[13], entry_bytes[14], entry_bytes[15]]);
        let volume_id = u16::from_le_bytes([entry_bytes[16], entry_bytes[17]]);
        let title_len = u16::from_le_bytes([entry_bytes[18], entry_bytes[19]]) as usize;
        let title_offset = u32::from_le_bytes([entry_bytes[20], entry_bytes[21], entry_bytes[22], entry_bytes[23]]) as usize;
        let audio_duration_ms = u32::from_le_bytes([entry_bytes[24], entry_bytes[25], entry_bytes[26], entry_bytes[27]]);
        let timeline_offset = u32::from_le_bytes([entry_bytes[28], entry_bytes[29], entry_bytes[30], entry_bytes[31]]);

        let title = self.get_string(title_offset, title_len)?.to_string();

        Ok(PackageSection {
            title,
            volume_id,
            chunk_index,
            chunk_offset,
            byte_length,
            char_count,
            has_audio: (flags & SECTION_FLAG_AUDIO) != 0,
            audio_duration_ms,
            timeline_offset,
        })
    }

    fn get_string(&self, pool_offset: usize, length: usize) -> Result<&'a str, ProtocolError> {
        let abs_start = self.string_pool_offset + pool_offset;
        let abs_end = abs_start + length;
        if abs_end > self.bytes.len() {
            return Err(ProtocolError::Serialization("string pool reference exceeds buffer boundary".into()));
        }
        std::str::from_utf8(&self.bytes[abs_start..abs_end]).map_err(|e| ProtocolError::Serialization(format!("invalid utf-8 string in pool: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_roundtrip_and_o1_lookup() {
        let manifest = PackageManifest {
            title: "宿命之环".to_string(),
            author: "爱潜水的乌贼".to_string(),
            description: "诡秘之主第二部".to_string(),
            total_chars: 2_450_000,
            chunks_count: 8,
            sections: vec![
                PackageSection {
                    title: "引子".to_string(),
                    volume_id: 0,
                    chunk_index: 0,
                    chunk_offset: 0,
                    byte_length: 1240,
                    char_count: 580,
                    has_audio: true,
                    audio_duration_ms: 180_000,
                    timeline_offset: 0,
                },
                PackageSection {
                    title: "第一章 游民".to_string(),
                    volume_id: 1,
                    chunk_index: 0,
                    chunk_offset: 1240,
                    byte_length: 6400,
                    char_count: 3100,
                    has_audio: true,
                    audio_duration_ms: 540_000,
                    timeline_offset: 320,
                },
                PackageSection {
                    title: "第二章 旅舍".to_string(),
                    volume_id: 1,
                    chunk_index: 0,
                    chunk_offset: 7640,
                    byte_length: 5800,
                    char_count: 2890,
                    has_audio: false,
                    audio_duration_ms: 0,
                    timeline_offset: 0,
                },
            ],
        };

        let encoded = manifest.encode();
        assert_eq!(&encoded[0..4], b"SGMF");

        // 1. Full decode test
        let decoded = PackageManifest::decode(&encoded).expect("decode failed");
        assert_eq!(decoded.title, manifest.title);
        assert_eq!(decoded.author, manifest.author);
        assert_eq!(decoded.description, manifest.description);
        assert_eq!(decoded.sections.len(), 3);
        assert_eq!(decoded.sections[1].title, "第一章 游民");
        assert_eq!(decoded.sections[1].audio_duration_ms, 540_000);
        assert!(decoded.sections[1].has_audio);
        assert!(!decoded.sections[2].has_audio);

        // 2. Zero-copy O(1) lookup view test
        let view = PackageManifestView::parse(&encoded).expect("parse view failed");
        assert_eq!(view.title().unwrap(), "宿命之环");
        assert_eq!(view.author().unwrap(), "爱潜水的乌贼");
        assert_eq!(view.sections_count(), 3);

        let sec2 = view.section_at(2).expect("lookup section 2 failed");
        assert_eq!(sec2.title, "第二章 旅舍");
        assert_eq!(sec2.chunk_offset, 7640);
        assert_eq!(sec2.byte_length, 5800);
        assert!(!sec2.has_audio);
    }

    #[test]
    fn test_corrupted_header_handling() {
        let mut bad_magic = vec![0u8; 64];
        bad_magic[0..4].copy_from_slice(b"BAD!");
        assert!(PackageManifestView::parse(&bad_magic).is_err());

        let short_buffer = vec![0u8; 30];
        assert!(PackageManifestView::parse(&short_buffer).is_err());
    }
}
