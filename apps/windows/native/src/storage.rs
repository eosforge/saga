//! Saga SQLite local storage persistence engine.

use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use flate2::Compression;
use protocol::{Book, BookSummary, Novel, Section};
use rusqlite::{params, Connection, OptionalExtension};
use std::io::{Read, Write};
use std::path::Path;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

/// Standard MIME type signature for .saga archive containers.
pub const SAGA_MIMETYPE: &[u8] = b"application/x-saga-book";

/// Target maximum chunk file size on disk (strictly <= 256 KB).
pub const MAX_COMPRESSED_CHUNK_SIZE: usize = 256 * 1024;

/// Opens the selected database; callers own directory selection and creation.
pub(super) fn open(path: &Path) -> Result<Connection, String> {
    let connection = Connection::open(path).map_err(|error| format!("cannot open {}: {error}", path.display()))?;
    initialize(&connection)?;
    Ok(connection)
}

/// Initializes database tables for books and section records with high performance index.
pub(super) fn initialize(conn: &Connection) -> Result<(), String> {
    // High performance SQLite pragmas: WAL mode, memory temp store, 64MB cache
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA temp_store = MEMORY;
         PRAGMA cache_size = -64000;
         CREATE TABLE IF NOT EXISTS books (
             id TEXT PRIMARY KEY,
             title TEXT NOT NULL,
             author TEXT NOT NULL,
             chapters INTEGER NOT NULL,
             format TEXT NOT NULL,
             description TEXT NOT NULL DEFAULT '',
             first_chapter TEXT NOT NULL DEFAULT '正文',
             created_at INTEGER NOT NULL
         );
         CREATE TABLE IF NOT EXISTS reviews (
             book_id TEXT NOT NULL,
             fingerprint TEXT NOT NULL,
             PRIMARY KEY (book_id, fingerprint)
         );
         DROP TABLE IF EXISTS sections;",
    )
    .map_err(|e| format!("failed to initialize database schema: {}", e))?;

    let _ = conn.execute("ALTER TABLE books ADD COLUMN first_chapter TEXT NOT NULL DEFAULT '正文'", []);

    let freelist: i64 = conn.query_row("PRAGMA freelist_count", [], |r| r.get(0)).unwrap_or(0);
    if freelist > 0 {
        let _ = conn.execute_batch("VACUUM;");
    }

    Ok(())
}

struct SectionMeta<'a> {
    title: &'a str,
    volume: &'a str,
    chunk_index: u32,
    byte_offset: u32,
    byte_length: u32,
}

/// Compresses raw byte buffer using Deflate algorithm.
fn compress_payload(input: &[u8]) -> Result<Vec<u8>, String> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(input).map_err(|error| format!("failed to compress chunk payload: {error}"))?;
    encoder
        .finish()
        .map_err(|error| format!("failed to finalize compressed chunk payload: {error}"))
}

/// Decompresses Deflate compressed chunk bytes.
fn decompress_payload(compressed: &[u8]) -> Result<Vec<u8>, String> {
    let mut decoder = DeflateDecoder::new(compressed);
    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .map_err(|error| format!("failed to decompress chunk payload: {error}"))?;
    Ok(decompressed)
}

/// In-memory compressed chunk payload strictly bounded by 256 KB.
struct PackedChunk {
    index: u32,
    compressed: Vec<u8>,
}

fn flush_chunk<'a>(
    chunk_idx: u32,
    raw_buf: &[u8],
    cached_compressed: Option<Vec<u8>>,
    chunk_sections: &[(&'a Section, u32, u32)],
    packed_chunks: &mut Vec<PackedChunk>,
    metas: &mut Vec<SectionMeta<'a>>,
) -> Result<(), String> {
    let compressed = match cached_compressed {
        Some(data) => data,
        None => compress_payload(raw_buf)?,
    };
    packed_chunks.push(PackedChunk { index: chunk_idx, compressed });

    for &(sec, byte_offset, byte_length) in chunk_sections {
        metas.push(SectionMeta {
            title: &sec.title,
            volume: &sec.volume,
            chunk_index: chunk_idx,
            byte_offset,
            byte_length,
        });
    }
    Ok(())
}

fn pack_chunks<'a>(sections: &'a [Section]) -> Result<(Vec<PackedChunk>, Vec<SectionMeta<'a>>), String> {
    let mut current_chunk_idx: u32 = 0;
    let mut current_raw_buf: Vec<u8> = Vec::new();
    let mut current_chunk_sections: Vec<(&'a Section, u32, u32)> = Vec::new();
    let mut last_valid_compressed: Option<Vec<u8>> = None;
    let mut current_compressed_len: usize = 0;
    let mut packed_chunks = Vec::new();
    let mut metas = Vec::with_capacity(sections.len());

    for sec in sections {
        let body_bytes = sec.body.as_bytes();
        let needed_raw = body_bytes.len();

        if current_raw_buf.is_empty() {
            let offset = 0;
            let len = needed_raw as u32;
            current_raw_buf.extend_from_slice(body_bytes);
            current_chunk_sections.push((sec, offset, len));

            let compressed = compress_payload(&current_raw_buf)?;
            if compressed.len() >= MAX_COMPRESSED_CHUNK_SIZE {
                flush_chunk(
                    current_chunk_idx,
                    &current_raw_buf,
                    Some(compressed),
                    &current_chunk_sections,
                    &mut packed_chunks,
                    &mut metas,
                )?;
                current_raw_buf.clear();
                current_chunk_sections.clear();
                last_valid_compressed = None;
                current_compressed_len = 0;
                current_chunk_idx += 1;
            } else {
                current_compressed_len = compressed.len();
                last_valid_compressed = Some(compressed);
            }
        } else {
            // Deflate expansion invariant: additional raw bytes cannot expand compressed size
            // by more than (needed_raw + 64). As long as current_compressed_len + needed_raw + 64
            // is <= MAX_COMPRESSED_CHUNK_SIZE, this chunk is mathematically guaranteed to fit,
            // entirely eliminating redundant O(N) trial compressions on every single chapter.
            if current_compressed_len + needed_raw + 64 <= MAX_COMPRESSED_CHUNK_SIZE {
                let offset = current_raw_buf.len() as u32;
                current_raw_buf.extend_from_slice(body_bytes);
                current_chunk_sections.push((sec, offset, needed_raw as u32));
                current_compressed_len += needed_raw;
                last_valid_compressed = None;
            } else {
                let offset = current_raw_buf.len() as u32;
                current_raw_buf.extend_from_slice(body_bytes);
                let test_compressed = compress_payload(&current_raw_buf)?;

                if test_compressed.len() <= MAX_COMPRESSED_CHUNK_SIZE {
                    current_chunk_sections.push((sec, offset, needed_raw as u32));
                    current_compressed_len = test_compressed.len();
                    last_valid_compressed = Some(test_compressed);
                } else {
                    // Revert the trial chapter in O(1) without cloning
                    current_raw_buf.truncate(offset as usize);
                    flush_chunk(
                        current_chunk_idx,
                        &current_raw_buf,
                        last_valid_compressed.take(),
                        &current_chunk_sections,
                        &mut packed_chunks,
                        &mut metas,
                    )?;
                    current_raw_buf.clear();
                    current_chunk_sections.clear();
                    current_chunk_idx += 1;

                    // Start fresh chunk with current section
                    current_raw_buf.extend_from_slice(body_bytes);
                    current_chunk_sections.push((sec, 0, needed_raw as u32));

                    let compressed = compress_payload(&current_raw_buf)?;
                    if compressed.len() >= MAX_COMPRESSED_CHUNK_SIZE {
                        flush_chunk(
                            current_chunk_idx,
                            &current_raw_buf,
                            Some(compressed),
                            &current_chunk_sections,
                            &mut packed_chunks,
                            &mut metas,
                        )?;
                        current_raw_buf.clear();
                        current_chunk_sections.clear();
                        last_valid_compressed = None;
                        current_compressed_len = 0;
                        current_chunk_idx += 1;
                    } else {
                        current_compressed_len = compressed.len();
                        last_valid_compressed = Some(compressed);
                    }
                }
            }
        }
    }

    if !current_raw_buf.is_empty() {
        flush_chunk(
            current_chunk_idx,
            &current_raw_buf,
            last_valid_compressed.take(),
            &current_chunk_sections,
            &mut packed_chunks,
            &mut metas,
        )?;
    }

    Ok((packed_chunks, metas))
}

/// Serializes and writes a complete .saga archive container atomically to the destination path.
fn write_saga_file<'a>(dest_path: &Path, book: &Book, metas: &[SectionMeta<'a>], chunks: &[PackedChunk]) -> Result<(), String> {
    if let Some(parent) = dest_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let file = std::fs::File::create(dest_path).map_err(|e| format!("cannot create saga package {}: {e}", dest_path.display()))?;
    let mut zip = ZipWriter::new(file);
    let stored_opt = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

    // 1. mimetype (STORE)
    zip.start_file("mimetype", stored_opt)
        .map_err(|e| format!("failed to write mimetype to zip: {e}"))?;
    zip.write_all(SAGA_MIMETYPE).map_err(|e| format!("failed to write mimetype content: {e}"))?;

    // 2. manifest.bin (STORE)
    let mut total_chars = 0u32;
    let mut volumes: Vec<String> = Vec::new();
    let mut volume_map = std::collections::HashMap::new();
    for s in metas {
        if !s.volume.is_empty() && !volume_map.contains_key(s.volume) {
            let id = volumes.len() as u16;
            volume_map.insert(s.volume, id);
            volumes.push(s.volume.to_string());
        }
    }

    let package_sections: Vec<protocol::PackageSection> = metas
        .iter()
        .map(|s| {
            let char_count = s.byte_length.saturating_div(3);
            total_chars += char_count;
            let volume_id = if s.volume.is_empty() { 0 } else { *volume_map.get(s.volume).unwrap_or(&0) };
            protocol::PackageSection {
                title: s.title.to_string(),
                volume_id,
                chunk_index: s.chunk_index as u16,
                chunk_offset: s.byte_offset,
                byte_length: s.byte_length,
                char_count,
                has_audio: false,
                audio_duration_ms: 0,
                timeline_offset: 0,
            }
        })
        .collect();

    let manifest = protocol::PackageManifest {
        title: book.title.clone(),
        author: book.author.clone(),
        description: book.description.clone(),
        total_chars,
        chunks_count: chunks.len() as u32,
        volumes,
        sections: package_sections,
    };
    let manifest_bytes = manifest.encode();

    zip.start_file("manifest.bin", stored_opt)
        .map_err(|e| format!("failed to write manifest.bin to zip: {e}"))?;
    zip.write_all(&manifest_bytes).map_err(|e| format!("failed to write manifest.bin bytes: {e}"))?;

    // 3. texts/{chunk_index}.chunk (STORE, chunk is already Deflate compressed)
    for chunk in chunks {
        zip.start_file(format!("texts/{}.chunk", chunk.index), stored_opt)
            .map_err(|e| format!("failed to write chunk {}: {e}", chunk.index))?;
        zip.write_all(&chunk.compressed)
            .map_err(|e| format!("failed to write chunk data {}: {e}", chunk.index))?;
    }

    zip.finish().map_err(|e| format!("failed to finalize saga package: {e}"))?;

    Ok(())
}

/// Atomically replaces a book and its complete chapter set into a single .saga file on disk.
pub(super) fn save(conn: &Connection, novel: &Novel, format: &str, books_dir: &Path) -> Result<(), String> {
    validate(&novel.sections)?;
    if novel.book.chapters as usize != novel.sections.len() {
        return Err("chapter count differs from the supplied sections".into());
    }
    let (packed_chunks, metas) = pack_chunks(&novel.sections)?;

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let staging_saga = books_dir.join(format!("staging-{}-{}.saga", novel.book.id, stamp));
    let target_saga = books_dir.join(format!("{}.saga", novel.book.id));

    write_saga_file(&staging_saga, &novel.book, &metas, &packed_chunks)?;

    let run_tx = || -> Result<(), String> {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
        let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
        let first_chapter = novel.sections.first().map(|s| s.title.as_str()).unwrap_or("正文");
        tx.execute(
            "INSERT OR REPLACE INTO books (id, title, author, chapters, format, description, first_chapter, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                novel.book.id,
                novel.book.title,
                novel.book.author,
                novel.book.chapters,
                format,
                novel.book.description,
                first_chapter,
                now
            ],
        )
        .map_err(|e| format!("failed to save book to db: {}", e))?;

        crate::review::refresh(&tx, &novel.book.id, &novel.sections)?;
        tx.commit().map_err(|e| format!("failed to commit save book transaction: {}", e))?;
        Ok(())
    };

    match run_tx() {
        Ok(()) => {
            if target_saga.exists() {
                let _ = std::fs::remove_file(&target_saga);
            }
            std::fs::rename(&staging_saga, &target_saga).map_err(|e| format!("cannot commit saga package to {}: {e}", target_saga.display()))?;
            Ok(())
        }
        Err(e) => {
            let _ = std::fs::remove_file(&staging_saga);
            Err(e)
        }
    }
}

/// Updates and replaces all sections for an existing novel (e.g. after user audit/resolution).
pub(super) fn update(
    conn: &Connection,
    book_id: &str,
    sections: &[Section],
    retained: &[String],
    books_dir: &Path,
) -> Result<protocol::NovelValidation, String> {
    validate(sections)?;
    let book = conn
        .query_row(
            "SELECT id, title, author, chapters, description FROM books WHERE id = ?1",
            params![book_id],
            |row| {
                Ok(Book {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    author: row.get(2)?,
                    chapters: row.get(3)?,
                    description: row.get(4)?,
                })
            },
        )
        .map_err(|e| format!("book {book_id} no longer exists: {e}"))?;

    let (packed_chunks, metas) = pack_chunks(sections)?;

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let staging_saga = books_dir.join(format!("staging-{}-{}.saga", book_id, stamp));
    let target_saga = books_dir.join(format!("{book_id}.saga"));

    write_saga_file(&staging_saga, &book, &metas, &packed_chunks)?;

    let run_tx = || -> Result<protocol::NovelValidation, String> {
        let first_chapter = sections.first().map(|s| s.title.as_str()).unwrap_or("正文");
        let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
        tx.execute(
            "UPDATE books SET chapters = ?1, first_chapter = ?2 WHERE id = ?3",
            params![sections.len() as u32, first_chapter, book_id],
        )
        .map_err(|e| format!("failed to update book chapters count: {}", e))?;

        let report = crate::review::replace(&tx, book_id, sections, retained)?;
        tx.commit().map_err(|e| format!("failed to commit updated sections transaction: {}", e))?;
        Ok(report)
    };

    match run_tx() {
        Ok(report) => {
            if target_saga.exists() {
                let _ = std::fs::remove_file(&target_saga);
            }
            std::fs::rename(&staging_saga, &target_saga).map_err(|e| format!("cannot commit updated saga to {}: {e}", target_saga.display()))?;
            Ok(report)
        }
        Err(e) => {
            let _ = std::fs::remove_file(&staging_saga);
            Err(e)
        }
    }
}

/// Checks positional and identity invariants before mutating persisted data or review keys.
fn validate(sections: &[Section]) -> Result<(), String> {
    let mut identities = std::collections::HashSet::new();
    for (index, section) in sections.iter().enumerate() {
        if section.index as usize != index || section.id.is_empty() || !identities.insert(&section.id) {
            return Err("section positions or identities are inconsistent".into());
        }
        let mut chunks = std::collections::HashSet::new();
        for (position, chunk) in section.chunks.iter().enumerate() {
            if chunk.chapter != section.id || chunk.index as usize != position || chunk.id.is_empty() || !chunks.insert(&chunk.id) {
                return Err("chunk positions or identities are inconsistent".into());
            }
        }
    }
    Ok(())
}

/// Deletes database records and the .saga package archive atomically.
pub(super) fn delete(conn: &Connection, book_id: &str, books_dir: &Path) -> Result<(), String> {
    if book_id.is_empty() || !book_id.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')) {
        return Err("invalid book identity for deletion".into());
    }
    let tx = conn.unchecked_transaction().map_err(|error| error.to_string())?;
    tx.execute("DELETE FROM reviews WHERE book_id = ?1", params![book_id])
        .map_err(|error| error.to_string())?;
    tx.execute("DELETE FROM books WHERE id = ?1", params![book_id])
        .map_err(|error| error.to_string())?;

    let saga_path = books_dir.join(format!("{book_id}.saga"));
    match std::fs::remove_file(&saga_path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("cannot remove {}: {error}", saga_path.display())),
    }
    tx.commit().map_err(|error| format!("cannot commit book deletion: {error}"))
}

/// Loads all persisted novels and their sections from SQLite and chunk files.
pub(super) fn all(conn: &Connection, books_dir: &Path) -> Result<Vec<Novel>, String> {
    let mut stmt = conn
        .prepare("SELECT id, title, author, chapters, description FROM books ORDER BY created_at DESC, id")
        .map_err(|e| e.to_string())?;

    let book_iter = stmt
        .query_map([], |row| {
            Ok(Book {
                id: row.get(0)?,
                title: row.get(1)?,
                author: row.get(2)?,
                chapters: row.get(3)?,
                description: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut novels = Vec::new();
    for book_res in book_iter {
        let book = book_res.map_err(|e| e.to_string())?;
        let sections = sections(&book.id, books_dir)?;

        novels.push(Novel { book, sections });
    }

    Ok(novels)
}

/// Loads only shelf metadata and the first title, without hydrating chapter bodies.
pub(super) fn summaries(conn: &Connection) -> Result<Vec<BookSummary>, String> {
    let mut statement = conn
        .prepare(
            "SELECT id, title, author, chapters, format, description, first_chapter
             FROM books ORDER BY created_at DESC, id",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok(BookSummary {
                id: row.get(0)?,
                title: row.get(1)?,
                author: row.get(2)?,
                chapters: row.get(3)?,
                format: row.get(4)?,
                description: row.get(5)?,
                first_chapter: row.get(6)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<_, _>>().map_err(|error| error.to_string())
}

/// Loads complete Novel entity by book ID for active reading.
pub(super) fn load(conn: &Connection, book_id: &str, books_dir: &Path) -> Result<Option<Novel>, String> {
    let book_opt = conn
        .query_row(
            "SELECT id, title, author, chapters, description FROM books WHERE id = ?1",
            params![book_id],
            |row| {
                Ok(Book {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    author: row.get(2)?,
                    chapters: row.get(3)?,
                    description: row.get(4)?,
                })
            },
        )
        .optional()
        .map_err(|e| e.to_string())?;

    let Some(book) = book_opt else {
        return Ok(None);
    };

    let sections = sections(&book.id, books_dir)?;
    Ok(Some(Novel { book, sections }))
}

/// Decodes stored sections directly from manifest.bin and segmented compressed chunks inside the .saga archive container.
fn sections(book_id: &str, books_dir: &Path) -> Result<Vec<Section>, String> {
    let saga_path = books_dir.join(format!("{book_id}.saga"));
    let saga_file = std::fs::File::open(&saga_path).map_err(|e| format!("cannot open saga package {}: {e}", saga_path.display()))?;
    let mut zip = ZipArchive::new(saga_file).map_err(|e| format!("cannot open saga zip archive {}: {e}", saga_path.display()))?;

    let manifest_bytes = {
        let mut manifest_entry = zip
            .by_name("manifest.bin")
            .map_err(|e| format!("cannot find manifest.bin in {}: {e}", saga_path.display()))?;
        let mut bytes = Vec::new();
        manifest_entry.read_to_end(&mut bytes).map_err(|e| format!("cannot read manifest.bin: {e}"))?;
        bytes
    };

    let manifest_view =
        protocol::PackageManifestView::parse(&manifest_bytes).map_err(|e| format!("failed to parse manifest.bin in {}: {e}", saga_path.display()))?;

    let count = manifest_view.sections_count();
    let mut sections = Vec::with_capacity(count as usize);
    let mut cached_chunk_idx: Option<u32> = None;
    let mut cached_raw_buf: Vec<u8> = Vec::new();

    for i in 0..count {
        let pkg_sec = manifest_view
            .section_at(i)
            .map_err(|e| format!("failed to read section {i} from manifest: {e}"))?;
        let chunk_idx = pkg_sec.chunk_index as u32;

        if cached_chunk_idx != Some(chunk_idx) {
            let entry_name = format!("texts/{chunk_idx}.chunk");
            let compressed = {
                let mut chunk_entry = zip
                    .by_name(&entry_name)
                    .map_err(|e| format!("cannot find chunk entry {entry_name} in {}: {e}", saga_path.display()))?;
                let mut compressed = Vec::new();
                chunk_entry
                    .read_to_end(&mut compressed)
                    .map_err(|e| format!("cannot read chunk entry {entry_name}: {e}"))?;
                compressed
            };
            cached_raw_buf = decompress_payload(&compressed)?;
            cached_chunk_idx = Some(chunk_idx);
        }

        let start = pkg_sec.chunk_offset as usize;
        let end = start + pkg_sec.byte_length as usize;
        if end > cached_raw_buf.len() {
            return Err(format!(
                "byte range {}..{} exceeds decompressed chunk length {}",
                start,
                end,
                cached_raw_buf.len()
            ));
        }

        let body = std::str::from_utf8(&cached_raw_buf[start..end])
            .map_err(|e| format!("invalid UTF-8 in decompressed chunk {chunk_idx}: {e}"))?
            .to_string();

        let volume = manifest_view.volume_at(pkg_sec.volume_id).unwrap_or("").to_string();

        let sec_id = format!("{book_id}:{i}");
        let chunks = reader::split_sentences(&body, &sec_id);

        sections.push(Section {
            id: sec_id,
            index: i,
            title: pkg_sec.title,
            volume,
            body,
            chunks,
        });
    }

    Ok(sections)
}

/// Exports an existing persisted book (.saga archive container) to destination path.
pub(super) fn export_saga(_conn: &Connection, book_id: &str, books_dir: &Path, output_path: &Path) -> Result<(), String> {
    let source_saga = books_dir.join(format!("{book_id}.saga"));
    if !source_saga.exists() {
        return Err(format!("cannot find saga package for book {book_id} at {}", source_saga.display()));
    }
    if let Some(parent) = output_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::copy(&source_saga, output_path).map_err(|e| format!("cannot export saga package to {}: {e}", output_path.display()))?;
    Ok(())
}

/// Imports a `.saga` package archive container into the local library.
pub(super) fn import_saga(conn: &Connection, archive_path: &Path, books_dir: &Path) -> Result<protocol::NovelImportResult, String> {
    let archive_bytes = std::fs::read(archive_path).map_err(|e| format!("cannot open saga archive {}: {e}", archive_path.display()))?;

    // Content-addressed cryptographic book identity from archive bytes
    let book_id = protocol::hash(&archive_bytes);

    let cursor = std::io::Cursor::new(&archive_bytes);
    let mut zip = ZipArchive::new(cursor).map_err(|e| format!("invalid zip container {}: {e}", archive_path.display()))?;

    // 1. Verify mimetype
    let mut mime_file = zip.by_name("mimetype").map_err(|_| "未识别的归档文件：缺少 mimetype 格式标识".to_string())?;
    let mut mime_bytes = Vec::new();
    mime_file.read_to_end(&mut mime_bytes).map_err(|e| format!("failed to read mimetype: {e}"))?;
    let clean_mime = mime_bytes.strip_suffix(b"\n").unwrap_or(&mime_bytes);
    let clean_mime = clean_mime.strip_suffix(b"\r").unwrap_or(clean_mime);
    if clean_mime != SAGA_MIMETYPE {
        return Err("非法的 Saga 归档格式或已被篡改".into());
    }
    drop(mime_file);

    // 2. Read and decode manifest.bin
    let mut manifest_file = zip.by_name("manifest.bin").map_err(|_| "归档损坏：缺少 manifest.bin 调度描述".to_string())?;
    let mut manifest_bytes = Vec::new();
    manifest_file
        .read_to_end(&mut manifest_bytes)
        .map_err(|e| format!("failed to read manifest.bin: {e}"))?;
    let manifest = protocol::PackageManifest::decode(&manifest_bytes).map_err(|e| format!("解析 manifest.bin 失败: {e}"))?;
    drop(manifest_file);

    // 3. Verify all chunk entries exist
    for chunk_idx in 0..manifest.chunks_count {
        let chunk_zip_path = format!("texts/{chunk_idx}.chunk");
        zip.by_name(&chunk_zip_path).map_err(|e| format!("归档损坏：缺少分片 {chunk_zip_path}: {e}"))?;
    }

    // 4. Staging write to target .saga file (NO FOLDER!)
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let staging_saga = books_dir.join(format!("staging-{}-{}.saga", book_id, stamp));
    let target_saga = books_dir.join(format!("{book_id}.saga"));
    std::fs::write(&staging_saga, &archive_bytes).map_err(|e| format!("cannot write staging saga package {}: {e}", staging_saga.display()))?;

    // 5. Populate SQLite tables: books & sections
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64;

    let book = Book {
        id: book_id.clone(),
        title: if manifest.title.is_empty() {
            archive_path.file_stem().and_then(|s| s.to_str()).unwrap_or("未命名书籍").to_string()
        } else {
            manifest.title
        },
        author: manifest.author,
        chapters: manifest.sections.len() as u32,
        description: manifest.description,
    };

    let run_tx = || -> Result<(), String> {
        let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
        let first_chapter = manifest.sections.first().map(|s| s.title.as_str()).unwrap_or("正文");
        tx.execute(
            "INSERT OR REPLACE INTO books (id, title, author, chapters, format, description, first_chapter, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![book.id, book.title, book.author, book.chapters, "SAGA", book.description, first_chapter, now],
        )
        .map_err(|e| format!("failed to insert book record: {e}"))?;

        tx.commit().map_err(|e| format!("failed to commit saga import transaction: {e}"))?;
        Ok(())
    };

    match run_tx() {
        Ok(()) => {
            if target_saga.exists() {
                let _ = std::fs::remove_file(&target_saga);
            }
            std::fs::rename(&staging_saga, &target_saga).map_err(|e| format!("cannot commit saga package to {}: {e}", target_saga.display()))?;
        }
        Err(e) => {
            let _ = std::fs::remove_file(&staging_saga);
            return Err(e);
        }
    }

    // 6. Hydrate sections for Novel and validation directly from target_saga
    let loaded_sections = sections(&book_id, books_dir)?;
    let validation = crate::review::inspect(conn, &book_id, &loaded_sections)?;

    Ok(protocol::NovelImportResult {
        novel: Novel {
            book,
            sections: loaded_sections,
        },
        validation,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(count: u32) -> Novel {
        Novel {
            book: Book {
                id: "fixture".into(),
                title: "Fixture".into(),
                author: "Author".into(),
                description: String::new(),
                chapters: count,
            },
            sections: (0..count)
                .map(|index| {
                    let id = format!("fixture:{}", index);
                    let body = "Text".to_string();
                    let chunks = reader::split_sentences(&body, &id);
                    Section {
                        id,
                        index,
                        title: format!("Chapter {}", index),
                        volume: "Volume".into(),
                        body,
                        chunks,
                    }
                })
                .collect(),
        }
    }

    fn test_books_dir() -> std::path::PathBuf {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("saga-storage-test-{}-{}-{id}", std::process::id(), stamp));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn reimport_replaces_old_sections_instead_of_leaving_a_stale_tail() {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        let dir = test_books_dir();
        save(&conn, &sample(4), "TXT", &dir).unwrap();
        save(&conn, &sample(2), "TXT", &dir).unwrap();
        let loaded = load(&conn, "fixture", &dir).unwrap().unwrap();
        assert_eq!(loaded, sample(2));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn failed_reimport_preserves_previous_metadata_and_sections() {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        let dir = test_books_dir();
        let original = sample(4);
        save(&conn, &original, "TXT", &dir).unwrap();
        conn.execute_batch(
            "CREATE TRIGGER reject_import BEFORE INSERT ON books WHEN NEW.title = 'Replacement' BEGIN SELECT RAISE(ABORT, 'fixture failure'); END;",
        )
        .unwrap();
        let mut replacement = sample(2);
        replacement.book.title = "Replacement".into();
        assert!(save(&conn, &replacement, "TXT", &dir).is_err());
        assert_eq!(load(&conn, "fixture", &dir).unwrap().unwrap(), original);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn rejected_deletion_preserves_the_entire_book() {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        let dir = test_books_dir();
        let original = sample(2);
        save(&conn, &original, "TXT", &dir).unwrap();
        conn.execute_batch("CREATE TRIGGER reject_delete BEFORE DELETE ON books BEGIN SELECT RAISE(ABORT, 'fixture rejection'); END;")
            .unwrap();
        assert!(delete(&conn, "fixture", &dir).is_err());
        assert_eq!(load(&conn, "fixture", &dir).unwrap().unwrap(), original);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn inconsistent_section_positions_are_rejected_before_writing() {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        let dir = test_books_dir();
        let original = sample(2);
        save(&conn, &original, "TXT", &dir).unwrap();
        let mut sections = original.sections.clone();
        sections[1].index = 0;
        assert!(update(&conn, "fixture", &sections, &[], &dir).is_err());
        assert_eq!(load(&conn, "fixture", &dir).unwrap().unwrap(), original);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn corrupted_chunk_file_is_reported_as_error() {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        let dir = test_books_dir();
        let original = sample(1);
        save(&conn, &original, "TXT", &dir).unwrap();
        // Overwrite .saga archive file with truncated bytes
        let saga_file = dir.join("fixture.saga");
        assert!(saga_file.exists());
        std::fs::write(&saga_file, b"corrupted bytes").unwrap();
        assert!(load(&conn, "fixture", &dir).is_err());
        assert!(all(&conn, &dir).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_segmented_256kb_chunking() {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        let dir = test_books_dir();

        // Generate chapters with varied text so compressed size exceeds 256 KB across multiple chapters
        let mut seed: u64 = 0xdeadbeef_cafe1234;
        let mut next_text = |words: usize| -> String {
            let mut buf = String::with_capacity(words * 9);
            for _ in 0..words {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let val = (seed >> 32) as u32;
                buf.push_str(&format!("{:08x} ", val));
            }
            buf
        };

        // 12 chapters of ~72 KB text each (total ~864 KB raw text)
        // With Deflate on hex strings (~50% ratio), total compressed size will be ~430 KB > 256 KB,
        // which forces greedy packing across at least 2 chunk files.
        let sections: Vec<Section> = (0..12)
            .map(|i| {
                let id = format!("large:{i}");
                let body = format!("Chapter {i} body: {}", next_text(8_000));
                let chunks = reader::split_sentences(&body, &id);
                Section {
                    id,
                    index: i,
                    title: format!("Chapter {i}"),
                    volume: "正文".to_string(),
                    body,
                    chunks,
                }
            })
            .collect();

        let novel = Novel {
            book: Book {
                id: "large".to_string(),
                title: "Large Book".to_string(),
                author: "Author".to_string(),
                description: String::new(),
                chapters: 12,
            },
            sections,
        };

        save(&conn, &novel, "TXT", &dir).unwrap();

        // 1. Verify single .saga archive exists and NO subfolder was created
        let saga_path = dir.join("large.saga");
        assert!(saga_path.exists(), "large.saga archive file must exist on disk");
        assert!(!dir.join("large").exists(), "large book subfolder must NOT exist on disk");

        // 2. Open archive and verify chunk entries exist and size <= 256 KB
        let file = std::fs::File::open(&saga_path).unwrap();
        let mut zip = ZipArchive::new(file).unwrap();
        let mut chunk_count = 0;
        for i in 0..zip.len() {
            let entry = zip.by_index(i).unwrap();
            let name = entry.name().to_string();
            if name.starts_with("texts/") && name.ends_with(".chunk") {
                chunk_count += 1;
                let size = entry.size() as usize;
                assert!(
                    size <= MAX_COMPRESSED_CHUNK_SIZE,
                    "chunk {} size {} exceeds MAX_COMPRESSED_CHUNK_SIZE {}",
                    name,
                    size,
                    MAX_COMPRESSED_CHUNK_SIZE
                );
            }
        }
        assert!(chunk_count >= 2, "must have created at least 2 chunks");

        // 3. Verify manifest.bin chunk_index monotonicity
        let saga_bytes = std::fs::read(&saga_path).unwrap();
        let mut zip = ZipArchive::new(std::io::Cursor::new(&saga_bytes)).unwrap();
        let manifest_bytes = {
            let mut manifest_entry = zip.by_name("manifest.bin").unwrap();
            let mut manifest_buf = Vec::new();
            manifest_entry.read_to_end(&mut manifest_buf).unwrap();
            manifest_buf
        };
        let manifest_view = protocol::PackageManifestView::parse(&manifest_bytes).unwrap();
        assert_eq!(manifest_view.sections_count(), 12);
        let chunk_indices: Vec<u16> = (0..manifest_view.sections_count())
            .map(|i| manifest_view.section_at(i).unwrap().chunk_index)
            .collect();
        assert_eq!(chunk_indices.len(), 12);
        assert!(chunk_indices.contains(&0) && chunk_indices.contains(&1));
        assert!(chunk_indices.windows(2).all(|w| w[0] <= w[1]));

        // 4. Verify load reconstructs novel perfectly
        let loaded = load(&conn, "large", &dir).unwrap().unwrap();
        assert_eq!(loaded, novel);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_sqlite_novel_crud_lifecycle() {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        let dir = test_books_dir();

        let book = Book {
            id: "test_book_1".to_string(),
            title: "测试小说".to_string(),
            author: "测试作者".to_string(),
            description: "测试简介".to_string(),
            chapters: 1,
        };

        let section = Section {
            id: "test_book_1:0".to_string(),
            index: 0,
            title: "第一章".to_string(),
            volume: "第一卷".to_string(),
            body: "测试正文。".to_string(),
            chunks: vec![],
        };

        let novel = Novel { book, sections: vec![section] };

        // 1. Save
        save(&conn, &novel, "TXT", &dir).unwrap();

        // Verify .saga package exists and NO subfolder exists
        assert!(dir.join("test_book_1.saga").exists(), "test_book_1.saga must exist");
        assert!(!dir.join("test_book_1").exists(), "test_book_1 subfolder must NOT exist");

        // 2. Load
        let loaded = all(&conn, &dir).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].book.title, "测试小说");
        assert_eq!(loaded[0].sections[0].volume, "第一卷");
        assert_eq!(loaded[0].sections[0].body, "测试正文。");

        // 3. Update sections (simulate audit edit: rename & add chapter)
        let updated_sections = vec![
            Section {
                id: "test_book_1:0".to_string(),
                index: 0,
                title: "第一章 (已核验校正)".to_string(),
                volume: "正文卷".to_string(),
                body: "修正后正文。".to_string(),
                chunks: vec![],
            },
            Section {
                id: "test_book_1:1".to_string(),
                index: 1,
                title: "第二章 新增".to_string(),
                volume: "正文卷".to_string(),
                body: "第二章正文。".to_string(),
                chunks: vec![],
            },
        ];
        update(&conn, "test_book_1", &updated_sections, &[], &dir).unwrap();
        assert!(dir.join("test_book_1.saga").exists());
        assert!(!dir.join("test_book_1").exists());
        let loaded_updated = load(&conn, "test_book_1", &dir).unwrap().unwrap();
        assert_eq!(loaded_updated.book.chapters, 2);
        assert_eq!(loaded_updated.sections.len(), 2);
        assert_eq!(loaded_updated.sections[0].title, "第一章 (已核验校正)");
        assert_eq!(loaded_updated.sections[0].body, "修正后正文。");
        assert_eq!(loaded_updated.sections[1].body, "第二章正文。");

        // 4. Delete
        delete(&conn, "test_book_1", &dir).unwrap();
        let loaded_after = all(&conn, &dir).unwrap();
        assert_eq!(loaded_after.len(), 0);
        assert!(!dir.join("test_book_1.saga").exists());
        assert!(!dir.join("test_book_1").exists());

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_saga_package_export_and_import_roundtrip() {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        let dir = test_books_dir();

        // 1. Prepare sample novel with 3 chapters
        let original_novel = Novel {
            book: Book {
                id: "saga_sample_book".into(),
                title: "星际纪元".into(),
                author: "科幻作家".into(),
                description: "这是一部史诗巨著。".into(),
                chapters: 3,
            },
            sections: vec![
                Section {
                    id: "saga_sample_book:0".into(),
                    index: 0,
                    title: "第一章 启航".into(),
                    volume: "第一卷".into(),
                    body: "飞船在星云间穿梭，引擎发出低沉的轰鸣。".into(),
                    chunks: Vec::new(),
                },
                Section {
                    id: "saga_sample_book:1".into(),
                    index: 1,
                    title: "第二章 异星遭遇".into(),
                    volume: "第一卷".into(),
                    body: "雷达屏幕上出现了一个未知的脉冲信号。".into(),
                    chunks: Vec::new(),
                },
                Section {
                    id: "saga_sample_book:2".into(),
                    index: 2,
                    title: "第三章 跃迁".into(),
                    volume: "第一卷".into(),
                    body: "空间曲率在剧烈扭曲，光束撕裂了黑暗。".into(),
                    chunks: Vec::new(),
                },
            ],
        };

        // 2. Save original novel to DB & chunk storage
        save(&conn, &original_novel, "TXT", &dir).unwrap();
        assert!(dir.join("saga_sample_book.saga").exists());
        assert!(!dir.join("saga_sample_book").exists());

        // 3. Export to .saga package
        let package_path = dir.join("star_epoch.saga");
        export_saga(&conn, "saga_sample_book", &dir, &package_path).unwrap();
        assert!(package_path.exists());
        assert!(std::fs::metadata(&package_path).unwrap().len() > 100);

        // 4. In a separate isolated database and directory, import the .saga package
        let import_conn = Connection::open_in_memory().unwrap();
        initialize(&import_conn).unwrap();
        let import_dir = test_books_dir();

        let result = import_saga(&import_conn, &package_path, &import_dir).unwrap();
        let imported_novel = result.novel;

        // Verify that import_dir contains ONLY {book_id}.saga, NO subfolders!
        let imported_saga_path = import_dir.join(format!("{}.saga", imported_novel.book.id));
        assert!(imported_saga_path.exists());
        assert!(!import_dir.join(&imported_novel.book.id).exists());

        assert_eq!(imported_novel.book.title, "星际纪元");
        assert_eq!(imported_novel.book.author, "科幻作家");
        assert_eq!(imported_novel.book.description, "这是一部史诗巨著。");
        assert_eq!(imported_novel.book.chapters, 3);
        assert_eq!(imported_novel.sections.len(), 3);
        assert_eq!(imported_novel.sections[0].title, "第一章 启航");
        assert_eq!(imported_novel.sections[0].body, "飞船在星云间穿梭，引擎发出低沉的轰鸣。");
        assert_eq!(imported_novel.sections[1].title, "第二章 异星遭遇");
        assert_eq!(imported_novel.sections[1].body, "雷达屏幕上出现了一个未知的脉冲信号。");
        assert_eq!(imported_novel.sections[2].title, "第三章 跃迁");
        assert_eq!(imported_novel.sections[2].body, "空间曲率在剧烈扭曲，光束撕裂了黑暗。");

        // Verify summary format
        let summaries = summaries(&import_conn).unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].title, "星际纪元");
        assert_eq!(summaries[0].format, "SAGA");

        // Clean up
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&import_dir);
    }
}
