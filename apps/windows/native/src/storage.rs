//! Saga SQLite local storage persistence engine.

use flate2::write::DeflateEncoder;
use flate2::{Compression, Decompress, FlushDecompress, Status};
use protocol::{Book, BookSummary, Novel, Section};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    RwLock,
};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

/// Standard MIME type signature for .saga archive containers.
pub const SAGA_MIMETYPE: &[u8] = b"application/x-saga-book";

/// Exclusive target for compressed multi-chapter payloads; a single chapter may exceed it.
pub const MAX_COMPRESSED_CHUNK_SIZE: usize = 256 * 1024;

/// Serializes archive changes with readers in this process; it is not a crash journal.
static ACCESS: RwLock<()> = RwLock::new(());

/// Distinguishes simultaneous staging operations even when the wall clock repeats.
static SERIAL: AtomicU64 = AtomicU64::new(0);

/// Opens the selected database; callers own directory selection and creation.
pub(super) fn open(path: &Path) -> Result<Connection, String> {
    let connection = Connection::open(path).map_err(|error| format!("cannot open {}: {error}", path.display()))?;
    connection.busy_timeout(std::time::Duration::from_secs(5)).map_err(|error| error.to_string())?;
    initialize(&connection)?;
    Ok(connection)
}

/// Configures a connection and applies only missing additive catalog columns.
pub(super) fn initialize(conn: &Connection) -> Result<(), String> {
    let _access = ACCESS.write().map_err(|_| "storage access is poisoned")?;
    // High performance SQLite pragmas: WAL mode, memory temp store, 64MB cache
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA temp_store = MEMORY;
         PRAGMA cache_size = -64000;",
    )
    .map_err(|error| error.to_string())?;
    let complete: bool = conn
        .query_row(
            "SELECT count(*) = 2 FROM sqlite_master WHERE type = 'table' AND name IN ('books', 'reviews')",
            [],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if !complete {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS books (
             id TEXT PRIMARY KEY,
             title TEXT NOT NULL,
             author TEXT NOT NULL,
             chapters INTEGER NOT NULL,
             format TEXT NOT NULL,
             description TEXT NOT NULL DEFAULT '',
             first_chapter TEXT NOT NULL DEFAULT '正文',
             last_chapter_index INTEGER NOT NULL DEFAULT 0,
             last_paragraph_index INTEGER NOT NULL DEFAULT 0,
             last_scroll_ratio REAL NOT NULL DEFAULT 0.0,
             last_read_time INTEGER NOT NULL DEFAULT 0,
             created_at INTEGER NOT NULL
         );
         CREATE TABLE IF NOT EXISTS reviews (
             book_id TEXT NOT NULL,
             fingerprint TEXT NOT NULL,
             PRIMARY KEY (book_id, fingerprint)
         );",
        )
        .map_err(|e| format!("failed to initialize database schema: {}", e))?;
    }

    let mut statement = conn.prepare("PRAGMA table_info(books)").map_err(|error| error.to_string())?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| error.to_string())?
        .collect::<Result<std::collections::HashSet<_>, _>>()
        .map_err(|error| error.to_string())?;
    drop(statement);
    for (name, definition) in [
        ("first_chapter", "TEXT NOT NULL DEFAULT '正文'"),
        ("last_chapter_index", "INTEGER NOT NULL DEFAULT 0"),
        ("last_paragraph_index", "INTEGER NOT NULL DEFAULT 0"),
        ("last_scroll_ratio", "REAL NOT NULL DEFAULT 0.0"),
        ("last_read_time", "INTEGER NOT NULL DEFAULT 0"),
    ] {
        if !columns.contains(name) {
            conn.execute(&format!("ALTER TABLE books ADD COLUMN {name} {definition}"), [])
                .map_err(|error| format!("cannot add catalog column {name}: {error}"))?;
        }
    }

    Ok(())
}

struct SectionMeta<'a> {
    section: &'a Section,
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

/// Requires a complete Deflate stream and exactly the byte span declared by its sections.
fn decompress_payload(compressed: &[u8], expected: usize) -> Result<Vec<u8>, String> {
    let mut decoder = Decompress::new(false);
    let mut output = Vec::new();
    let mut buffer = [0u8; 8192];
    loop {
        let before = (decoder.total_in(), decoder.total_out());
        let status = decoder
            .decompress(&compressed[before.0 as usize..], &mut buffer, FlushDecompress::None)
            .map_err(|error| format!("failed to decompress chunk payload: {error}"))?;
        let written = (decoder.total_out() - before.1) as usize;
        if output.len().checked_add(written).is_none_or(|size| size > expected) {
            return Err("decompressed chunk exceeds its declared section spans".into());
        }
        output.extend_from_slice(&buffer[..written]);
        if status == Status::StreamEnd {
            if decoder.total_in() as usize != compressed.len() || output.len() != expected {
                return Err("chunk length differs from its complete Deflate stream or section spans".into());
            }
            return Ok(output);
        }
        if (decoder.total_in(), decoder.total_out()) == before {
            return Err("truncated Deflate chunk".into());
        }
    }
}

/// Packed Deflate bytes; an oversized single chapter is kept in its own chunk.
struct PackedChunk {
    index: u32,
    compressed: Vec<u8>,
}

fn flush_chunk<'a>(
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
    // Headroom is only a batching hint. Final compressed bytes decide multi-chapter admission.
    if compressed.len() >= MAX_COMPRESSED_CHUNK_SIZE && chunk_sections.len() > 1 {
        let middle = chunk_sections.len() / 2;
        let offset = chunk_sections[middle].1;
        flush_chunk(&raw_buf[..offset as usize], None, &chunk_sections[..middle], packed_chunks, metas)?;
        let tail: Vec<_> = chunk_sections[middle..]
            .iter()
            .map(|&(section, start, length)| (section, start - offset, length))
            .collect();
        return flush_chunk(&raw_buf[offset as usize..], None, &tail, packed_chunks, metas);
    }
    let chunk_idx = u16::try_from(packed_chunks.len()).map_err(|_| "book exceeds the v1 limit of 65536 text chunks")? as u32;
    packed_chunks.push(PackedChunk { index: chunk_idx, compressed });

    for &(sec, byte_offset, byte_length) in chunk_sections {
        metas.push(SectionMeta {
            section: sec,
            title: &sec.title,
            volume: &sec.volume,
            chunk_index: chunk_idx,
            byte_offset,
            byte_length,
        });
    }
    Ok(())
}

/// Restricts catalog identities to one local filename component.
fn identify(book: &str) -> Result<(), String> {
    if book.is_empty() || !book.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')) {
        return Err("invalid book identity".into());
    }
    Ok(())
}

/// Generates a sibling path without reusing any active operation's name.
fn temporary(target: &Path, role: &str) -> PathBuf {
    let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    target.with_extension(format!("{role}-{}-{stamp}-{serial}", std::process::id()))
}

/// Owns only an unpublished staging file and removes it on every ordinary exit.
struct Staging(PathBuf);

impl Drop for Staging {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_file(&self.0) {
            if error.kind() != std::io::ErrorKind::NotFound {
                eprintln!("cannot remove unpublished staging file {}: {error}", self.0.display());
            }
        }
    }
}

/// Restores the old archive after a rejected replacement or SQLite commit.
fn restore(target: &Path, backup: Option<&Path>, replaced: bool, cause: String) -> String {
    let mut errors = Vec::new();
    if replaced {
        if let Err(error) = std::fs::remove_file(target) {
            if error.kind() != std::io::ErrorKind::NotFound {
                errors.push(format!("cannot remove replacement {}: {error}", target.display()));
            }
        }
    }
    if let Some(backup) = backup {
        if let Err(error) = std::fs::rename(backup, target) {
            errors.push(format!(
                "cannot restore {}; original retained at {}: {error}",
                target.display(),
                backup.display()
            ));
        }
    }
    if errors.is_empty() {
        cause
    } else {
        format!("{cause}; archive recovery incomplete: {}", errors.join("; "))
    }
}

/// Publishes a prepared replacement or deletion, restoring the old file if commit fails.
/// Callers hold ACCESS for writing; process termination and external writers are outside this guarantee.
fn publish<T>(conn: &Connection, target: &Path, staged: Option<&Path>, change: impl FnOnce(&Connection) -> Result<T, String>) -> Result<T, String> {
    let transaction = Transaction::new_unchecked(conn, TransactionBehavior::Immediate).map_err(|error| error.to_string())?;
    let result = change(&transaction)?;
    let backup = match std::fs::symlink_metadata(target) {
        Ok(metadata) if metadata.is_file() => {
            let backup = temporary(target, "backup");
            std::fs::rename(target, &backup).map_err(|error| format!("cannot stage existing archive {}: {error}", target.display()))?;
            Some(backup)
        }
        Ok(_) => return Err(format!("archive path is not a regular file: {}", target.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(format!("cannot inspect archive {}: {error}", target.display())),
    };
    if let Some(staged) = staged {
        if let Err(error) = std::fs::rename(staged, target) {
            return Err(restore(target, backup.as_deref(), false, format!("cannot publish archive: {error}")));
        }
    }
    if let Err(error) = transaction.commit() {
        return Err(restore(target, backup.as_deref(), staged.is_some(), format!("cannot commit catalog: {error}")));
    }
    if let Some(backup) = backup {
        if let Err(error) = std::fs::remove_file(&backup) {
            // A retained backup does not undo the acknowledged catalog/archive commit.
            eprintln!("catalog committed; cannot remove backup {}: {error}", backup.display());
        }
    }
    Ok(result)
}

fn pack_chunks<'a>(sections: &'a [Section]) -> Result<(Vec<PackedChunk>, Vec<SectionMeta<'a>>), String> {
    let mut current_raw_buf: Vec<u8> = Vec::new();
    let mut current_chunk_sections: Vec<(&'a Section, u32, u32)> = Vec::new();
    let mut last_valid_compressed: Option<Vec<u8>> = None;
    let mut current_compressed_len: usize = 0;
    let mut packed_chunks = Vec::new();
    let mut metas = Vec::with_capacity(sections.len());

    for sec in sections {
        let body_bytes = sec.body.as_bytes();
        let needed_raw = body_bytes.len();
        if needed_raw > u32::MAX as usize {
            return Err("chapter text exceeds the v1 byte length range".into());
        }
        if current_raw_buf.len().checked_add(needed_raw).is_none_or(|size| size > u32::MAX as usize) {
            flush_chunk(
                &current_raw_buf,
                last_valid_compressed.take(),
                &current_chunk_sections,
                &mut packed_chunks,
                &mut metas,
            )?;
            current_raw_buf.clear();
            current_chunk_sections.clear();
            current_compressed_len = 0;
        }

        if current_chunk_sections.is_empty() {
            let offset = 0;
            let len = needed_raw as u32;
            current_raw_buf.extend_from_slice(body_bytes);
            current_chunk_sections.push((sec, offset, len));

            let compressed = compress_payload(&current_raw_buf)?;
            if compressed.len() >= MAX_COMPRESSED_CHUNK_SIZE {
                flush_chunk(&current_raw_buf, Some(compressed), &current_chunk_sections, &mut packed_chunks, &mut metas)?;
                current_raw_buf.clear();
                current_chunk_sections.clear();
                last_valid_compressed = None;
                current_compressed_len = 0;
            } else {
                current_compressed_len = compressed.len();
                last_valid_compressed = Some(compressed);
            }
        } else {
            // Defer trial compression while there is ample headroom; flush_chunk checks the result.
            if current_compressed_len + needed_raw + 64 < MAX_COMPRESSED_CHUNK_SIZE {
                let offset = current_raw_buf.len() as u32;
                current_raw_buf.extend_from_slice(body_bytes);
                current_chunk_sections.push((sec, offset, needed_raw as u32));
                current_compressed_len += needed_raw;
                last_valid_compressed = None;
            } else {
                let offset = current_raw_buf.len() as u32;
                current_raw_buf.extend_from_slice(body_bytes);
                let test_compressed = compress_payload(&current_raw_buf)?;

                if test_compressed.len() < MAX_COMPRESSED_CHUNK_SIZE {
                    current_chunk_sections.push((sec, offset, needed_raw as u32));
                    current_compressed_len = test_compressed.len();
                    last_valid_compressed = Some(test_compressed);
                } else {
                    // Revert the trial chapter in O(1) without cloning
                    current_raw_buf.truncate(offset as usize);
                    flush_chunk(
                        &current_raw_buf,
                        last_valid_compressed.take(),
                        &current_chunk_sections,
                        &mut packed_chunks,
                        &mut metas,
                    )?;
                    current_raw_buf.clear();
                    current_chunk_sections.clear();

                    // Start fresh chunk with current section
                    current_raw_buf.extend_from_slice(body_bytes);
                    current_chunk_sections.push((sec, 0, needed_raw as u32));

                    let compressed = compress_payload(&current_raw_buf)?;
                    if compressed.len() >= MAX_COMPRESSED_CHUNK_SIZE {
                        flush_chunk(&current_raw_buf, Some(compressed), &current_chunk_sections, &mut packed_chunks, &mut metas)?;
                        current_raw_buf.clear();
                        current_chunk_sections.clear();
                        last_valid_compressed = None;
                        current_compressed_len = 0;
                    } else {
                        current_compressed_len = compressed.len();
                        last_valid_compressed = Some(compressed);
                    }
                }
            }
        }
    }

    if !current_chunk_sections.is_empty() {
        flush_chunk(
            &current_raw_buf,
            last_valid_compressed.take(),
            &current_chunk_sections,
            &mut packed_chunks,
            &mut metas,
        )?;
    }

    Ok((packed_chunks, metas))
}

/// Writes an unpublished archive, preserving resources and unchanged-text audio routing on edits.
fn write_saga_file(
    dest_path: &Path,
    book: &Book,
    metas: &[SectionMeta<'_>],
    chunks: &[PackedChunk],
    source: Option<(&mut ZipArchive<std::fs::File>, &protocol::PackageManifest, &[Section])>,
) -> Result<(), String> {
    let mut total_chars = 0u32;
    let mut volumes: Vec<String> = Vec::new();
    let mut volume_map = std::collections::HashMap::new();
    for s in metas {
        if !volume_map.contains_key(s.volume) {
            let id = u16::try_from(volumes.len()).map_err(|_| "too many volumes for a v1 manifest")?;
            volume_map.insert(s.volume, id);
            volumes.push(s.volume.to_string());
        }
    }

    // Matching unique, unchanged text keeps routing valid even when earlier chapters are deleted.
    // Changed or ambiguous text loses the active route; its original resource bytes are still retained.
    let mut routes = std::collections::HashMap::new();
    if let Some((_, manifest, sections)) = source.as_ref() {
        for (section, route) in sections.iter().zip(&manifest.sections) {
            routes.entry(section.body.as_str()).and_modify(|value| *value = None).or_insert(Some(route));
        }
    }
    let package_sections: Vec<protocol::PackageSection> = metas
        .iter()
        .map(|s| {
            let char_count = u32::try_from(s.section.body.chars().count()).map_err(|_| "chapter character count exceeds v1 range")?;
            total_chars = total_chars.checked_add(char_count).ok_or("book character count exceeds v1 range")?;
            let route = routes.get(s.section.body.as_str()).copied().flatten();
            Ok(protocol::PackageSection {
                title: s.title.to_string(),
                volume_id: volume_map[s.volume],
                chunk_index: u16::try_from(s.chunk_index).map_err(|_| "text chunk index exceeds v1 range")?,
                chunk_offset: s.byte_offset,
                byte_length: s.byte_length,
                char_count,
                has_audio: route.is_some_and(|route| route.has_audio),
                audio_duration_ms: route.map_or(0, |route| route.audio_duration_ms),
                timeline_offset: route.map_or(0, |route| route.timeline_offset),
            })
        })
        .collect::<Result<_, String>>()?;

    let manifest = protocol::PackageManifest {
        title: book.title.clone(),
        author: book.author.clone(),
        description: book.description.clone(),
        total_chars,
        chunks_count: chunks.len() as u32,
        volumes,
        sections: package_sections,
    };
    let manifest_bytes = manifest.encode().map_err(|error| error.to_string())?;

    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| format!("cannot create archive directory: {error}"))?;
    }
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dest_path)
        .map_err(|error| format!("cannot create staging archive {}: {error}", dest_path.display()))?;
    let mut zip = ZipWriter::new(file);
    let stored_opt = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    zip.start_file("mimetype", stored_opt).map_err(|error| error.to_string())?;
    zip.write_all(SAGA_MIMETYPE).map_err(|error| error.to_string())?;

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

    if let Some((source, previous, _)) = source {
        let replaced: std::collections::HashSet<_> = (0..previous.chunks_count).map(|index| format!("texts/{index}.chunk")).collect();
        let generated: std::collections::HashSet<_> = chunks.iter().map(|chunk| format!("texts/{}.chunk", chunk.index)).collect();
        for index in 0..source.len() {
            let entry = source.by_index(index).map_err(|error| error.to_string())?;
            if entry.name() == "mimetype" || entry.name() == "manifest.bin" || replaced.contains(entry.name()) {
                continue;
            }
            if generated.contains(entry.name()) {
                return Err(format!("new text chunk would overwrite an additional archive resource: {}", entry.name()));
            }
            zip.raw_copy_file(entry).map_err(|error| format!("cannot preserve archive resource: {error}"))?;
        }
    }
    zip.finish()
        .map_err(|error| format!("failed to finalize saga package: {error}"))?
        .sync_all()
        .map_err(|error| format!("failed to sync staging archive: {error}"))?;

    Ok(())
}

/// Replaces a complete book; ordinary file/commit failures preserve the previous catalog and archive.
pub(super) fn save(conn: &Connection, novel: &Novel, format: &str, books_dir: &Path) -> Result<protocol::NovelValidation, String> {
    let _access = ACCESS.write().map_err(|_| "storage access is poisoned")?;
    identify(&novel.book.id)?;
    validate(&novel.sections)?;
    if novel.book.chapters as usize != novel.sections.len() {
        return Err("chapter count differs from the supplied sections".into());
    }
    let (packed_chunks, metas) = pack_chunks(&novel.sections)?;

    let target_saga = books_dir.join(format!("{}.saga", novel.book.id));
    let staging = Staging(temporary(&target_saga, "pending"));
    write_saga_file(&staging.0, &novel.book, &metas, &packed_chunks, None)?;

    publish(conn, &target_saga, Some(&staging.0), |transaction| {
        record(transaction, &novel.book, format, &novel.sections)?;
        crate::review::refresh(transaction, &novel.book.id, &novel.sections)?;
        crate::review::inspect(transaction, &novel.book.id, &novel.sections)
    })
}

/// Upserts publication metadata without discarding a valid reading anchor.
fn record(conn: &Connection, book: &Book, format: &str, sections: &[Section]) -> Result<(), String> {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
    let first = sections.first().map(|section| section.title.as_str()).unwrap_or("正文");
    conn.execute(
        "INSERT INTO books (id, title, author, chapters, format, description, first_chapter, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET
                 title = excluded.title,
                 author = excluded.author,
                 chapters = excluded.chapters,
                 format = excluded.format,
                 description = excluded.description,
                 first_chapter = excluded.first_chapter",
        params![book.id, book.title, book.author, sections.len() as u32, format, book.description, first, now],
    )
    .map_err(|error| format!("failed to save book to db: {error}"))?;
    normalize(conn, &book.id, sections)
}

/// Clamps stale persisted positions only after the replacement chapter graph is known.
fn normalize(conn: &Connection, book: &str, sections: &[Section]) -> Result<(), String> {
    let (chapter, paragraph, ratio): (u32, u32, f64) = conn
        .query_row(
            "SELECT last_chapter_index, last_paragraph_index, last_scroll_ratio FROM books WHERE id = ?1",
            [book],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|error| error.to_string())?;
    let index = (chapter as usize).min(sections.len().saturating_sub(1));
    let count = sections
        .get(index)
        .map(|section| {
            reader::split_sentences(&section.body, &section.id)
                .last()
                .map_or(0, |chunk| chunk.paragraph as usize + 1)
        })
        .unwrap_or(0);
    let position = if index == chapter as usize {
        (paragraph as usize).min(count.saturating_sub(1))
    } else {
        0
    };
    let ratio = if index != chapter as usize || position != paragraph as usize || !ratio.is_finite() || !(0.0..=1.0).contains(&ratio) {
        0.0
    } else {
        ratio
    };
    conn.execute(
        "UPDATE books SET last_chapter_index = ?1, last_paragraph_index = ?2, last_scroll_ratio = ?3 WHERE id = ?4",
        params![index as u32, position as u32, ratio, book],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

/// Updates and replaces all sections for an existing novel (e.g. after user audit/resolution).
pub(super) fn update(
    conn: &Connection,
    book_id: &str,
    sections: &[Section],
    retained: &[String],
    books_dir: &Path,
) -> Result<protocol::NovelValidation, String> {
    let _access = ACCESS.write().map_err(|_| "storage access is poisoned")?;
    identify(book_id)?;
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

    let target_saga = books_dir.join(format!("{book_id}.saga"));
    let file = std::fs::File::open(&target_saga).map_err(|error| error.to_string())?;
    let mut archive = ZipArchive::new(file).map_err(|error| error.to_string())?;
    let (manifest, previous) = decode(&mut archive, book_id)?;
    let change = |transaction: &Connection| -> Result<protocol::NovelValidation, String> {
        let first_chapter = sections.first().map(|s| s.title.as_str()).unwrap_or("正文");
        transaction
            .execute(
                "UPDATE books SET chapters = ?1, first_chapter = ?2 WHERE id = ?3",
                params![sections.len() as u32, first_chapter, book_id],
            )
            .map_err(|e| format!("failed to update book chapters count: {}", e))?;

        normalize(transaction, book_id, sections)?;
        crate::review::replace(transaction, book_id, sections, retained)
    };

    if previous.len() == sections.len()
        && previous
            .iter()
            .zip(sections)
            .all(|(old, new)| old.title == new.title && old.volume == new.volume && old.body == new.body)
    {
        let transaction = Transaction::new_unchecked(conn, TransactionBehavior::Immediate).map_err(|error| error.to_string())?;
        let report = change(&transaction)?;
        transaction.commit().map_err(|error| error.to_string())?;
        return Ok(report);
    }
    let (packed_chunks, metas) = pack_chunks(sections)?;
    let staging = Staging(temporary(&target_saga, "pending"));
    write_saga_file(&staging.0, &book, &metas, &packed_chunks, Some((&mut archive, &manifest, &previous)))?;
    drop(archive);
    publish(conn, &target_saga, Some(&staging.0), change)
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

/// Deletes catalog records and the archive with rollback on ordinary operation failures.
pub(super) fn delete(conn: &Connection, book_id: &str, books_dir: &Path) -> Result<(), String> {
    let _access = ACCESS.write().map_err(|_| "storage access is poisoned")?;
    identify(book_id)?;
    let saga_path = books_dir.join(format!("{book_id}.saga"));
    publish(conn, &saga_path, None, |transaction| {
        transaction
            .execute("DELETE FROM reviews WHERE book_id = ?1", params![book_id])
            .map_err(|error| error.to_string())?;
        transaction
            .execute("DELETE FROM books WHERE id = ?1", params![book_id])
            .map_err(|error| error.to_string())?;
        Ok(())
    })
}

/// Loads all persisted novels and their sections from SQLite and chunk files.
pub(super) fn all(conn: &Connection, books_dir: &Path) -> Result<Vec<Novel>, String> {
    let _access = ACCESS.read().map_err(|_| "storage access is poisoned")?;
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
    let _access = ACCESS.read().map_err(|_| "storage access is poisoned")?;
    let mut statement = conn
        .prepare(
            "SELECT id, title, author, chapters, format, description, first_chapter,
                    last_chapter_index, last_paragraph_index, last_scroll_ratio
             FROM books
             ORDER BY CASE WHEN last_read_time > 0 THEN last_read_time ELSE created_at END DESC, id",
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
                last_chapter_index: row.get(7)?,
                last_paragraph_index: row.get(8)?,
                last_scroll_ratio: row.get(9)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<_, _>>().map_err(|error| error.to_string())
}

/// Updates user's durable reading progress for a given book.
pub(super) fn update_progress(conn: &Connection, book_id: &str, chapter_index: u32, paragraph_index: u32, scroll_ratio: f64) -> Result<(), String> {
    let _access = ACCESS.read().map_err(|_| "storage access is poisoned")?;
    if !scroll_ratio.is_finite() || !(0.0..=1.0).contains(&scroll_ratio) {
        return Err("scroll ratio must be finite and between zero and one".into());
    }
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
    let changed = conn
        .execute(
            "UPDATE books
         SET last_chapter_index = ?1,
             last_paragraph_index = ?2,
             last_scroll_ratio = ?3,
             last_read_time = ?4
         WHERE id = ?5 AND ((chapters > 0 AND ?1 < chapters) OR (chapters = 0 AND ?1 = 0 AND ?2 = 0 AND ?3 = 0))",
            params![chapter_index, paragraph_index, scroll_ratio, now, book_id],
        )
        .map_err(|e| format!("failed to update reading progress: {e}"))?;
    if changed != 1 {
        return Err("book is missing or reading chapter is out of range".into());
    }
    Ok(())
}

/// Loads complete Novel entity by book ID for active reading.
pub(super) fn load(conn: &Connection, book_id: &str, books_dir: &Path) -> Result<Option<Novel>, String> {
    let _access = ACCESS.read().map_err(|_| "storage access is poisoned")?;
    identify(book_id)?;
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
    identify(book_id)?;
    let saga_path = books_dir.join(format!("{book_id}.saga"));
    let saga_file = std::fs::File::open(&saga_path).map_err(|e| format!("cannot open saga package {}: {e}", saga_path.display()))?;
    let mut zip = ZipArchive::new(saga_file).map_err(|e| format!("cannot open saga zip archive {}: {e}", saga_path.display()))?;

    decode(&mut zip, book_id).map(|(_, sections)| sections)
}

/// Validates all archive entries and complete text streams before exposing a book or committing it.
fn decode<R: Read + Seek>(zip: &mut ZipArchive<R>, book_id: &str) -> Result<(protocol::PackageManifest, Vec<Section>), String> {
    let mut names = std::collections::HashSet::new();
    for index in 0..zip.len() {
        let entry = zip.by_index(index).map_err(|error| error.to_string())?;
        if !names.insert(entry.name().to_string()) {
            return Err(format!("duplicate archive entry: {}", entry.name()));
        }
    }
    let mut mime = Vec::new();
    zip.by_name("mimetype")
        .map_err(|_| "archive is missing mimetype")?
        .take(SAGA_MIMETYPE.len() as u64 + 3)
        .read_to_end(&mut mime)
        .map_err(|error| error.to_string())?;
    let clean = mime.strip_suffix(b"\n").unwrap_or(&mime);
    if clean.strip_suffix(b"\r").unwrap_or(clean) != SAGA_MIMETYPE {
        return Err("invalid Saga archive mimetype".into());
    }
    let manifest_bytes = {
        let mut manifest_entry = zip.by_name("manifest.bin").map_err(|e| format!("cannot find manifest.bin: {e}"))?;
        let mut bytes = Vec::new();
        manifest_entry.read_to_end(&mut bytes).map_err(|e| format!("cannot read manifest.bin: {e}"))?;
        bytes
    };

    let manifest = protocol::PackageManifest::decode(&manifest_bytes).map_err(|error| error.to_string())?;
    let declared: std::collections::HashSet<_> = (0..manifest.chunks_count).map(|index| format!("texts/{index}.chunk")).collect();
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index).map_err(|error| error.to_string())?;
        if entry.name() != "mimetype" && entry.name() != "manifest.bin" && !declared.contains(entry.name()) {
            std::io::copy(&mut entry, &mut std::io::sink()).map_err(|error| format!("damaged archive resource: {error}"))?;
        }
    }
    let mut routes = vec![Vec::new(); manifest.chunks_count as usize];
    let mut sections = Vec::with_capacity(manifest.sections.len());
    for (index, entry) in manifest.sections.iter().enumerate() {
        routes[entry.chunk_index as usize].push(index);
        sections.push(Section {
            id: format!("{book_id}:{index}"),
            index: index as u32,
            title: entry.title.clone(),
            volume: manifest.volumes.get(entry.volume_id as usize).cloned().unwrap_or_default(),
            body: String::new(),
            chunks: Vec::new(),
        });
    }
    for (index, positions) in routes.iter().enumerate() {
        let name = format!("texts/{index}.chunk");
        let expected = positions
            .iter()
            .map(|&position| {
                let entry = &manifest.sections[position];
                entry.chunk_offset as usize + entry.byte_length as usize
            })
            .max()
            .unwrap_or(0);
        let mut compressed = Vec::new();
        zip.by_name(&name)
            .map_err(|error| format!("missing text chunk {name}: {error}"))?
            .read_to_end(&mut compressed)
            .map_err(|error| error.to_string())?;
        let bytes = decompress_payload(&compressed, expected)?;
        for &position in positions {
            let entry = &manifest.sections[position];
            let start = entry.chunk_offset as usize;
            let body = std::str::from_utf8(&bytes[start..start + entry.byte_length as usize])
                .map_err(|error| format!("invalid UTF-8 in text chunk {name}: {error}"))?
                .to_string();
            let section = &mut sections[position];
            // Match TXT ingestion: eager projection only for the first five chapters.
            if position < 5 {
                section.chunks = reader::split_sentences(&body, &section.id);
            }
            section.body = body;
        }
    }
    Ok((manifest, sections))
}

/// Exports an existing persisted book (.saga archive container) to destination path.
pub(super) fn export_saga(_conn: &Connection, book_id: &str, books_dir: &Path, output_path: &Path) -> Result<(), String> {
    let _access = ACCESS.read().map_err(|_| "storage access is poisoned")?;
    identify(book_id)?;
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
    let _access = ACCESS.write().map_err(|_| "storage access is poisoned")?;
    let archive_bytes = std::fs::read(archive_path).map_err(|e| format!("cannot open saga archive {}: {e}", archive_path.display()))?;

    // Content-addressed cryptographic book identity from archive bytes
    let book_id = protocol::hash(&archive_bytes);

    let cursor = std::io::Cursor::new(&archive_bytes);
    let mut zip = ZipArchive::new(cursor).map_err(|e| format!("invalid zip container {}: {e}", archive_path.display()))?;

    // Resolve every text span before creating any persistent shelf state.
    let (manifest, loaded_sections) = decode(&mut zip, &book_id)?;

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

    let target = books_dir.join(format!("{book_id}.saga"));
    let staging = Staging(temporary(&target, "pending"));
    std::fs::create_dir_all(books_dir).map_err(|error| error.to_string())?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&staging.0)
        .map_err(|error| error.to_string())?;
    file.write_all(&archive_bytes).map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    drop(file);
    let validation = publish(conn, &target, Some(&staging.0), |transaction| {
        record(transaction, &book, "SAGA", &loaded_sections)?;
        crate::review::refresh(transaction, &book_id, &loaded_sections)?;
        crate::review::inspect(transaction, &book_id, &loaded_sections)
    })?;

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

        // 2. Every multi-chapter compressed entry is below 256 KiB.
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
                    size < MAX_COMPRESSED_CHUNK_SIZE,
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

        // 4. Stored bodies/metadata round-trip, including sentences derived lazily after chapter five.
        let mut loaded = load(&conn, "large", &dir).unwrap().unwrap();
        for section in &mut loaded.sections {
            if section.chunks.is_empty() {
                section.chunks = reader::split_sentences(&section.body, &section.id);
            }
        }
        assert!(loaded == novel, "segmented publication differs after full sentence projection");

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

    #[test]
    fn test_durable_reading_progress_persistence() {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        let dir = test_books_dir();
        let mut novel = sample(5);
        // Paragraph 12 is a real, restorable position in the replacement fixture.
        novel.sections[3].body = (0..13).map(|index| format!("Paragraph {index}.")).collect::<Vec<_>>().join("\n");
        novel.sections[3].chunks = reader::split_sentences(&novel.sections[3].body, &novel.sections[3].id);
        save(&conn, &novel, "TXT", &dir).unwrap();

        let s0 = summaries(&conn).unwrap();
        assert_eq!(s0[0].last_chapter_index, 0);
        assert_eq!(s0[0].last_paragraph_index, 0);
        assert_eq!(s0[0].last_scroll_ratio, 0.0);

        update_progress(&conn, "fixture", 3, 12, 0.45).unwrap();

        let s1 = summaries(&conn).unwrap();
        assert_eq!(s1[0].last_chapter_index, 3);
        assert_eq!(s1[0].last_paragraph_index, 12);
        assert!((s1[0].last_scroll_ratio - 0.45).abs() < f64::EPSILON);

        // Reimport preserves progress
        save(&conn, &novel, "TXT", &dir).unwrap();
        let s2 = summaries(&conn).unwrap();
        assert_eq!(s2[0].last_chapter_index, 3);
        assert_eq!(s2[0].last_paragraph_index, 12);

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
