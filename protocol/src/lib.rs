//! Cross-tier protocol contracts, domain entities, and cryptographic hashing utilities.

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

pub mod package;
pub use package::{PackageManifest, PackageManifestView, PackageSection};

/// Protocol domain error representations.
#[derive(Debug, Error)]
pub enum ProtocolError {
    /// Failure during serialization or deserialization of entity payloads.
    #[error("serialization failure: {0}")]
    Serialization(String),

    /// Missing or malformed entity identification attributes.
    #[error("invalid identifier: {0}")]
    InvalidIdentifier(String),
}

/// Fundamental digital book metadata entity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Book {
    /// Cryptographic content identifier of the primary book asset.
    pub id: String,

    /// Human-readable title of the publication.
    pub title: String,

    /// Author or primary contributor attribution string.
    pub author: String,

    /// Synopsis or introductory summary of the publication.
    pub description: String,

    /// Total count of recognized structural chapters.
    pub chapters: u32,
}

/// Lightweight book summary entity for high-speed shelf catalog hydration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BookSummary {
    /// Cryptographic content identifier of the primary book asset.
    pub id: String,

    /// Human-readable title of the publication.
    pub title: String,

    /// Author or primary contributor attribution string.
    pub author: String,

    /// Synopsis or introductory summary of the publication.
    pub description: String,

    /// Total count of recognized structural chapters.
    pub chapters: u32,

    /// Publication file container format ("TXT" | "EPUB").
    pub format: String,

    /// Preview or title of the first section.
    pub first_chapter: String,
}

/// Structural chapter segment within a digital publication.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Chapter {
    /// Stable chapter identifier formatted as `{book_id}:{index}`.
    pub id: String,

    /// Zero-based sequential chapter ordering position.
    pub index: u32,

    /// Display heading or chapter title.
    pub title: String,

    /// Total count of sentence chunks partitioned within this chapter.
    pub chunks: u32,
}

/// Detailed chapter section containing body text and constituent sentence chunks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Section {
    /// Stable chapter identifier formatted as `{book_id}:{index}`.
    pub id: String,

    /// Zero-based sequential chapter ordering position.
    pub index: u32,

    /// Display heading or chapter title.
    pub title: String,

    /// Volume grouping attribution (e.g. "第一卷", or "正文" if not subdivided).
    pub volume: String,

    /// Full chapter body text.
    pub body: String,

    /// Sentence-level semantic chunks partitioned within this chapter.
    pub chunks: Vec<Chunk>,
}

/// Complete parsed publication entity resulting from file ingestion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Novel {
    /// Foundational book metadata entity.
    pub book: Book,

    /// Ordered sequence of parsed chapter sections.
    pub sections: Vec<Section>,
}

/// Punctuation-bounded semantic sentence chunk for rendering and audio alignment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Chunk {
    /// Globally unique span element identifier.
    pub id: String,

    /// Parent chapter identifier ownership.
    pub chapter: String,

    /// Zero-based ordinal index within the parent chapter.
    pub index: u32,

    /// Zero-based paragraph ordinal within the parent chapter to preserve layout.
    pub paragraph: u32,

    /// Cleaned textual body representation.
    pub text: String,
}

/// Temporal span mapping an active sentence chunk to audio playback intervals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Span {
    /// Target chunk identifier matching a rendered DOM element.
    pub id: String,

    /// Playback start offset in milliseconds.
    pub start: u64,

    /// Playback end offset in milliseconds.
    pub end: u64,
}

/// Audio narrative alignment manifest binding text chapters to CAS audio assets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Manifest {
    /// Cryptographic BLAKE3 hash of the synthesized audio stream.
    pub audio: String,

    /// Associated chapter identifier.
    pub chapter: String,

    /// Audio duration in total milliseconds.
    pub duration: u64,

    /// Monotonically ordered temporal alignment intervals.
    pub spans: Vec<Span>,
}

/// Category of anomalies detected during volume and chapter sequence analysis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum AnomalyType {
    /// Chapter numbers skipped one or more indices (e.g. 第4章 -> 第6章).
    SequenceGap,
    /// Chapter numbers decreased unexpectedly (e.g. 第15章 -> 第10章).
    SequenceRegression,
    /// Repeated chapter number or identical title in the same scope; content may differ.
    DuplicateHeading,
    /// Volume numbering appeared out of sequential order or regressed.
    VolumeDisorder,
    /// Suspiciously short section text (< 10 characters), possibly a TOC entry or dialogue line.
    SuspiciousShortSection,
}

/// Detailed structural anomaly finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ValidationAnomaly {
    /// Content-bound identity used to persist an explicit retain decision.
    #[serde(default)]
    pub key: String,
    /// User retained this finding for the currently matching source content.
    #[serde(default)]
    pub retained: bool,
    pub anomaly_type: AnomalyType,
    pub section_index: u32,
    pub title: String,
    pub volume: String,
    pub description: String,
}

/// Comprehensive validation summary report for imported novel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct NovelValidation {
    /// Whether structural anomalies are absent; retained findings remain in this report.
    pub is_valid: bool,
    /// Total count of recognized volumes.
    pub total_volumes: u32,
    /// Total count of recognized chapters.
    pub total_chapters: u32,
    /// Human-readable volume numbering description (e.g. "标准连续单卷", "多卷连续递增", "未编号卷").
    pub volume_style: String,
    /// Human-readable chapter numbering description (e.g. "全书全局连续递增", "每卷重置从第1章递增", "无序或无数字章节").
    pub chapter_style: String,
    /// Diagnostic summary warning messages.
    pub warnings: Vec<String>,
    /// Concrete anomaly items flagged for user inspection.
    pub anomalies: Vec<ValidationAnomaly>,
}

/// Comprehensive import response payload containing the novel entity and validation report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct NovelImportResult {
    pub novel: Novel,
    pub validation: NovelValidation,
}

/// Live progress payload streamed to UI during multi-phase import and validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ImportProgress {
    /// Phase key: reading, hashing, caching, decoding, parsing, validating, persisting, completed.
    pub phase: String,
    /// Human-readable phase title (e.g. "正在导入", "正在校验", "正在落盘").
    pub title: String,
    /// Detailed step description.
    pub detail: String,
    /// Weighted pipeline progress (0 to 100); reading/parsing measure work, other phases indicate activity.
    pub percent: u32,
}

/// Computes the standard hexadecimal BLAKE3 cryptographic hash for given binary data.
pub fn hash(data: &[u8]) -> String {
    let digest = blake3::hash(data);
    digest.to_hex().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blake3_hashing() {
        let sample = b"hello saga";
        let digest = hash(sample);
        assert_eq!(digest.len(), 64);
    }

    #[test]
    fn test_book_instantiation() {
        let book = Book {
            id: "b3_sample".to_string(),
            title: "Test Book".to_string(),
            author: "Author".to_string(),
            description: "Sample description".to_string(),
            chapters: 10,
        };
        assert_eq!(book.chapters, 10);
    }
}
