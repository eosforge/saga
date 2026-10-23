//! Regression cases for archive publication, format rejection, and retained user state.
use crate::storage;
use protocol::{Book, Novel, PackageManifest, PackageManifestView, PackageSection, Section};
use rusqlite::Connection;
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

fn sandbox(name: &str) -> Sandbox {
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let path = std::env::temp_dir().join(format!("saga-regression-{}-{stamp}-{name}", std::process::id()));
    fs::create_dir(&path).unwrap();
    eprintln!("fixture={}", path.display());
    Sandbox(path)
}

/// Owns one isolated fixture; preserve it when an assertion fails.
struct Sandbox(PathBuf);
impl std::ops::Deref for Sandbox {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        if std::thread::panicking() {
            return;
        }
        let root = std::env::temp_dir().canonicalize().unwrap();
        let path = self.0.canonicalize().unwrap();
        assert!(path.starts_with(&root) && path.file_name().unwrap().to_string_lossy().starts_with("saga-regression-"));
        fs::remove_dir_all(path).unwrap();
    }
}
fn database() -> Connection {
    let c = Connection::open_in_memory().unwrap();
    storage::initialize(&c).unwrap();
    c
}
fn novel() -> Novel {
    Novel {
        book: Book {
            id: "probe".into(),
            title: "Original".into(),
            author: "Author".into(),
            description: String::new(),
            chapters: 1,
        },
        sections: vec![Section {
            id: "probe:0".into(),
            index: 0,
            title: "Chapter 1".into(),
            volume: "Volume 1".into(),
            body: "Original body of the chapter.".into(),
            chunks: vec![],
        }],
    }
}
fn manifest() -> PackageManifest {
    PackageManifest {
        title: "Probe".into(),
        author: "Author".into(),
        description: String::new(),
        total_chars: 29,
        chunks_count: 1,
        volumes: vec!["Volume 1".into()],
        sections: vec![PackageSection {
            title: "Chapter 1".into(),
            volume_id: 0,
            chunk_index: 0,
            chunk_offset: 0,
            byte_length: 29,
            char_count: 29,
            has_audio: false,
            audio_duration_ms: 0,
            timeline_offset: 0,
        }],
    }
}
fn archive(path: &Path, m: &PackageManifest, payload: &[u8], extra: bool) {
    let mut z = ZipWriter::new(fs::File::create(path).unwrap());
    let o = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    z.start_file("mimetype", o).unwrap();
    z.write_all(b"application/x-saga-book").unwrap();
    z.start_file("manifest.bin", o).unwrap();
    z.write_all(&m.encode().unwrap()).unwrap();
    z.start_file("texts/0.chunk", o).unwrap();
    z.write_all(payload).unwrap();
    if extra {
        z.start_file("cover.jpg", o).unwrap();
        z.write_all(b"cover-fixture").unwrap();
        z.start_file("audio/0.m4a", o).unwrap();
        z.write_all(b"audio-fixture").unwrap();
        z.start_file("timeline.bin", o).unwrap();
        z.write_all(b"timeline-fixture").unwrap();
        z.start_file("texts/notes.txt", o).unwrap();
        z.write_all(b"notes-fixture").unwrap();
    }
    z.finish().unwrap();
}
fn compressed(body: &[u8]) -> Vec<u8> {
    let mut e = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
    e.write_all(body).unwrap();
    e.finish().unwrap()
}
#[test]
fn audit_manifest_arithmetic_overflow_must_return_error() {
    let mut b = manifest().encode().unwrap();
    b[24..32].copy_from_slice(&(u64::MAX - 15).to_le_bytes());
    let result = std::panic::catch_unwind(|| PackageManifestView::parse(&b));
    assert!(matches!(result, Ok(Err(_))), "malformed TOC offset panicked or was accepted");
}
#[test]
fn audit_manifest_string_length_must_not_silently_truncate() {
    let mut m = manifest();
    m.description = "文".repeat(21846);
    assert!(m.encode().is_err(), "oversized UTF-8 strings must be rejected before v1 encoding");
}
#[test]
#[cfg(windows)]
fn audit_failed_file_replace_must_preserve_database() {
    use std::os::windows::fs::OpenOptionsExt;
    let p = sandbox("locked-replace");
    let c = database();
    let original = novel();
    storage::save(&c, &original, "TXT", &p).unwrap();
    let target = p.join("probe.saga");
    let _held = fs::OpenOptions::new().read(true).share_mode(1).open(&target).unwrap();
    let mut changed = original.clone();
    changed.book.title = "Changed".into();
    changed.sections[0].body = "Replacement text".into();
    let result = storage::save(&c, &changed, "TXT", &p);
    assert!(result.is_err(), "test must exercise rejected file replacement");
    let title: String = c.query_row("SELECT title FROM books WHERE id='probe'", [], |r| r.get(0)).unwrap();
    assert_eq!(title, "Original", "failed file replacement published new database metadata: {result:?}");
}
#[test]
fn audit_corrupt_import_must_not_publish_book() {
    let p = sandbox("corrupt-import");
    let c = database();
    let source = p.join("source.saga");
    archive(&source, &manifest(), &[255, 255, 255], false);
    let result = storage::import_saga(&c, &source, &p);
    assert!(result.is_err(), "test must exercise rejected corrupt payload");
    let count: i64 = c.query_row("SELECT count(*) FROM books", [], |r| r.get(0)).unwrap();
    assert_eq!(count, 0, "corrupt import committed a shelf record before payload validation: {result:?}");
}
#[test]
fn audit_failed_delete_commit_must_preserve_archive() {
    let p = sandbox("delete-commit");
    let c = database();
    storage::save(&c, &novel(), "TXT", &p).unwrap();
    c.execute_batch(
        "PRAGMA foreign_keys=ON; CREATE TABLE held (book TEXT REFERENCES books(id) DEFERRABLE INITIALLY DEFERRED); INSERT INTO held VALUES ('probe');",
    )
    .unwrap();
    let result = storage::delete(&c, "probe", &p);
    assert!(result.is_err(), "test must exercise failed SQLite commit");
    let count: i64 = c.query_row("SELECT count(*) FROM books", [], |r| r.get(0)).unwrap();
    assert_eq!(count, 1, "database rolls back the failed deletion");
    assert!(p.join("probe.saga").exists(), "failed deletion removed the archive permanently: {result:?}");
}
#[test]
fn audit_heading_only_chapters_must_survive_save() {
    let p = sandbox("empty-bodies");
    let c = database();
    let n = reader::parse_txt_str("第1章 开始\n第2章 结束", "probe", None).unwrap();
    assert_eq!(n.sections.len(), 2);
    storage::save(&c, &n, "TXT", &p).unwrap();
    let loaded = storage::load(&c, "probe", &p).unwrap().unwrap();
    assert_eq!(loaded.sections.len(), 2, "empty-body chapter TOC entries were dropped");
}
#[test]
fn oversized_single_chapter_remains_supported() {
    let p = sandbox("oversized-chapter");
    let c = database();
    let mut n = novel();
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut state = 0x12345678u32;
    let bytes: Vec<u8> = (0..1_048_576)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            alphabet[(state & 63) as usize]
        })
        .collect();
    n.sections[0].body = String::from_utf8(bytes).unwrap();
    storage::save(&c, &n, "TXT", &p).unwrap();
    let mut z = ZipArchive::new(fs::File::open(p.join("probe.saga")).unwrap()).unwrap();
    let size = z.by_name("texts/0.chunk").unwrap().size();
    assert!(size > 256 * 1024, "fixture must exercise the accepted compressed-size exception");
    assert_eq!(storage::load(&c, "probe", &p).unwrap().unwrap().sections[0].body, n.sections[0].body);
}
#[test]
fn audit_empty_volume_must_not_be_assigned_to_first_named_volume() {
    let p = sandbox("volume-roundtrip");
    let c = database();
    let mut n = novel();
    n.sections[0].volume.clear();
    let mut second = n.sections[0].clone();
    second.index = 1;
    second.id = "probe:1".into();
    second.title = "Chapter 2".into();
    second.volume = "Volume 2".into();
    n.sections.push(second);
    n.book.chapters = 2;
    storage::save(&c, &n, "TXT", &p).unwrap();
    let loaded = storage::load(&c, "probe", &p).unwrap().unwrap();
    assert_eq!(loaded.sections[0].volume, "", "unnamed section was reassigned to another volume");
}
#[test]
fn audit_invalid_reading_progress_must_be_rejected() {
    let p = sandbox("invalid-progress");
    let c = database();
    storage::save(&c, &novel(), "TXT", &p).unwrap();
    assert!(
        storage::update_progress(&c, "probe", 99, 0, 2.0).is_err(),
        "accepted chapter 99 and scroll ratio 2.0 for a one-chapter book"
    );
}
#[test]
fn audit_retaining_review_must_preserve_other_package_resources() {
    let p = sandbox("retain-assets");
    let c = database();
    let source = p.join("source.saga");
    let mut m = manifest();
    m.sections[0].has_audio = true;
    m.sections[0].audio_duration_ms = 1000;
    archive(&source, &m, &compressed(b"Original body of the chapter."), true);
    let imported = storage::import_saga(&c, &source, &p).unwrap();
    let target = p.join(format!("{}.saga", imported.novel.book.id));
    let original = fs::read(&target).unwrap();
    storage::update(&c, &imported.novel.book.id, &imported.novel.sections, &[], &p).unwrap();
    assert_eq!(
        fs::read(&target).unwrap(),
        original,
        "review-only saves must preserve the exact publication bytes"
    );
    let mut z = ZipArchive::new(fs::File::open(target).unwrap()).unwrap();
    assert!(z.by_name("cover.jpg").is_ok(), "unchanged section save erased cover/audio/timeline resources");
}

#[test]
fn failed_publication_commit_restores_existing_and_absent_archives() {
    for existing in [false, true] {
        let directory = sandbox("publication-commit");
        let connection = database();
        let mut replacement = novel();
        let target = directory.join("probe.saga");
        if existing {
            storage::save(&connection, &replacement, "TXT", &directory).unwrap();
        }
        let original = fs::read(&target).ok();
        connection
            .execute_batch(
                "PRAGMA foreign_keys=ON;
            CREATE TABLE parent (id TEXT PRIMARY KEY);
            CREATE TABLE held (id TEXT REFERENCES parent(id) DEFERRABLE INITIALLY DEFERRED);
            CREATE TRIGGER fail_insert AFTER INSERT ON books BEGIN INSERT INTO held VALUES ('missing'); END;
            CREATE TRIGGER fail_update AFTER UPDATE OF title ON books BEGIN INSERT INTO held VALUES ('missing'); END;",
            )
            .unwrap();
        replacement.book.title = "Changed".into();
        replacement.sections[0].body = "Replacement body".into();
        assert!(storage::save(&connection, &replacement, "TXT", &directory).is_err());
        assert_eq!(fs::read(&target).ok(), original);
        let title: Option<String> = connection.query_row("SELECT title FROM books", [], |row| row.get(0)).ok();
        assert_eq!(title.as_deref(), existing.then_some("Original"));
        assert_eq!(fs::read_dir(&*directory).unwrap().count(), usize::from(existing));
    }
}

#[test]
fn malformed_streams_never_publish_files_or_catalog_rows() {
    let directory = sandbox("invalid-streams");
    let connection = database();
    let valid = compressed(b"Original body of the chapter.");
    let mut trailing = valid.clone();
    trailing.push(0);
    let invalid = [valid[..valid.len() - 1].to_vec(), trailing, compressed(&[255; 29]), compressed(b"short")];
    for (index, payload) in invalid.into_iter().enumerate() {
        let source = directory.join(format!("source-{index}.saga"));
        archive(&source, &manifest(), &payload, false);
        let identity = protocol::hash(&fs::read(&source).unwrap());
        assert!(storage::import_saga(&connection, &source, &directory).is_err(), "case {index}");
        assert!(!directory.join(format!("{identity}.saga")).exists());
    }
    let count: i64 = connection.query_row("SELECT count(*) FROM books", [], |row| row.get(0)).unwrap();
    assert_eq!(count, 0);
}

#[test]
fn damaged_nontext_resource_rejects_import() {
    let directory = sandbox("resource-crc");
    let connection = database();
    let source = directory.join("source.saga");
    archive(&source, &manifest(), &compressed(b"Original body of the chapter."), true);
    let mut bytes = fs::read(&source).unwrap();
    let offset = bytes.windows(b"cover-fixture".len()).position(|window| window == b"cover-fixture").unwrap();
    bytes[offset] ^= 1;
    fs::write(&source, bytes).unwrap();
    assert!(storage::import_saga(&connection, &source, &directory).is_err());
    let count: i64 = connection.query_row("SELECT count(*) FROM books", [], |row| row.get(0)).unwrap();
    assert_eq!(count, 0);
}

#[test]
fn edits_preserve_resources_and_only_valid_audio_routes() {
    let directory = sandbox("resource-edit");
    let connection = database();
    let source = directory.join("source.saga");
    let mut metadata = manifest();
    metadata.sections[0].has_audio = true;
    metadata.sections[0].audio_duration_ms = 1000;
    metadata.sections[0].timeline_offset = 123;
    archive(&source, &metadata, &compressed(b"Original body of the chapter."), true);
    let imported = storage::import_saga(&connection, &source, &directory).unwrap();
    let mut sections = imported.novel.sections;
    for edited in [false, true] {
        sections[0].title = "Corrected heading".into();
        if edited {
            sections[0].body = "Changed text".into();
            sections[0].chunks.clear();
        }
        storage::update(&connection, &imported.novel.book.id, &sections, &[], &directory).unwrap();
        let target = directory.join(format!("{}.saga", imported.novel.book.id));
        let mut archive = ZipArchive::new(fs::File::open(target).unwrap()).unwrap();
        for (name, expected) in [
            ("cover.jpg", "cover-fixture"),
            ("audio/0.m4a", "audio-fixture"),
            ("timeline.bin", "timeline-fixture"),
            ("texts/notes.txt", "notes-fixture"),
        ] {
            let mut content = String::new();
            archive.by_name(name).unwrap().read_to_string(&mut content).unwrap();
            assert_eq!(content, expected);
        }
        let mut bytes = Vec::new();
        archive.by_name("manifest.bin").unwrap().read_to_end(&mut bytes).unwrap();
        let current = PackageManifest::decode(&bytes).unwrap();
        assert_eq!(current.sections[0].has_audio, !edited);
        assert_eq!(current.sections[0].timeline_offset, if edited { 0 } else { 123 });
    }
}

#[test]
fn compressed_size_controls_packing_and_later_sentences_are_lazy() {
    let directory = sandbox("packed-boundaries");
    let connection = database();
    let mut publication = novel();
    publication.sections = (0..8)
        .map(|index| Section {
            id: format!("probe:{index}"),
            index,
            title: format!("Chapter {index}"),
            volume: String::new(),
            body: "a".repeat(300_000),
            chunks: Vec::new(),
        })
        .collect();
    publication.book.chapters = publication.sections.len() as u32;
    storage::save(&connection, &publication, "TXT", &directory).unwrap();
    let loaded = storage::load(&connection, "probe", &directory).unwrap().unwrap();
    assert_eq!(loaded.sections.len(), 8);
    for (index, section) in loaded.sections.iter().enumerate() {
        assert_eq!(section.body, publication.sections[index].body);
        assert_eq!(section.chunks.is_empty(), index >= 5);
    }
    let mut archive = ZipArchive::new(fs::File::open(directory.join("probe.saga")).unwrap()).unwrap();
    assert!(archive.by_name("texts/0.chunk").unwrap().size() < 256 * 1024);
    assert!(archive.by_name("texts/1.chunk").is_err(), "uncompressed size must not force splitting");
}

#[test]
fn replacement_clamps_removed_reading_anchors() {
    let directory = sandbox("progress-clamp");
    let connection = database();
    let mut publication = novel();
    publication.sections.push(Section {
        id: "probe:1".into(),
        index: 1,
        ..publication.sections[0].clone()
    });
    publication.book.chapters = 2;
    storage::save(&connection, &publication, "TXT", &directory).unwrap();
    storage::update_progress(&connection, "probe", 1, 9, 0.8).unwrap();
    storage::update(&connection, "probe", &publication.sections[..1], &[], &directory).unwrap();
    let summary = storage::summaries(&connection).unwrap().remove(0);
    assert_eq!(
        (summary.last_chapter_index, summary.last_paragraph_index, summary.last_scroll_ratio),
        (0, 0, 0.0)
    );
    for ratio in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
        assert!(storage::update_progress(&connection, "probe", 0, 0, ratio).is_err());
    }
    assert!(storage::update_progress(&connection, "missing", 0, 0, 0.0).is_err());
}

#[test]
fn repeated_initialization_does_not_change_schema_or_progress() {
    let directory = sandbox("schema");
    let connection = database();
    storage::save(&connection, &novel(), "TXT", &directory).unwrap();
    storage::update_progress(&connection, "probe", 0, 0, 0.6).unwrap();
    let before: u32 = connection.query_row("PRAGMA schema_version", [], |row| row.get(0)).unwrap();
    storage::initialize(&connection).unwrap();
    let after: u32 = connection.query_row("PRAGMA schema_version", [], |row| row.get(0)).unwrap();
    assert_eq!(before, after);
    assert_eq!(storage::summaries(&connection).unwrap()[0].last_scroll_ratio, 0.6);
}
#[test]
fn audit_preface_must_not_break_per_volume_reset_detection() {
    let n=reader::parse_txt_str("前言\n这是一段完整介绍，正文在后续两卷分别开始。\n第一卷 开始\n第1章 甲\n这是第一卷第一章足够长的正文内容。\n第2章 乙\n这是第一卷第二章足够长的正文内容。\n第二卷 继续\n第1章 丙\n这是第二卷第一章足够长的正文内容。","probe",None).unwrap();
    let r = reader::validate_novel_sequence(&n.sections);
    assert!(
        !r.anomalies.iter().any(|a| a.anomaly_type == protocol::AnomalyType::SequenceRegression),
        "ordinary per-volume reset was flagged because the first group was a preface: {r:?}"
    );
}
#[test]
fn audit_malformed_numerals_must_not_be_treated_as_valid_numbers() {
    assert_eq!(reader::validator::parse_chinese_numeral("十十"), None, "repeated unit was accepted");
}
#[test]
fn audit_ascii_character_count_must_be_exact() {
    let p = sandbox("character-count");
    let c = database();
    let mut n = novel();
    n.sections[0].body = "abcdef".into();
    storage::save(&c, &n, "TXT", &p).unwrap();
    let mut z = ZipArchive::new(fs::File::open(p.join("probe.saga")).unwrap()).unwrap();
    let mut b = Vec::new();
    z.by_name("manifest.bin").unwrap().read_to_end(&mut b).unwrap();
    let m = PackageManifest::decode(&b).unwrap();
    assert_eq!(m.total_chars, 6, "UTF-8 byte count divided by three is not a character count");
}
