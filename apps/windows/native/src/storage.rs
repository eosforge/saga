//! Saga SQLite local storage persistence engine.

use protocol::{Book, BookSummary, Novel, Section};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

/// Target chunk file byte budget for cross-platform segmented storage (256 KB).
pub const CHUNK_TARGET_SIZE: usize = 256 * 1024;

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
         PRAGMA cache_size = -64000;",
    )
    .map_err(|e| format!("failed to set sqlite pragmas: {}", e))?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS books (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            author TEXT NOT NULL,
            chapters INTEGER NOT NULL,
            format TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            created_at INTEGER NOT NULL
        );",
        [],
    )
    .map_err(|e| format!("failed to create books table: {}", e))?;

    // Older books tables lack description. Detect that schema before the additive migration.
    let description: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('books') WHERE name = 'description')",
            [],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if !description {
        conn.execute("ALTER TABLE books ADD COLUMN description TEXT NOT NULL DEFAULT '';", [])
            .map_err(|error| format!("cannot add book descriptions: {error}"))?;
    }

    // Check if sections table exists and has the chunked schema
    let chunk_schema: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('sections') WHERE name = 'chunk_index')",
            [],
            |row| row.get(0),
        )
        .unwrap_or(false);

    if !chunk_schema {
        conn.execute("DROP TABLE IF EXISTS sections;", [])
            .map_err(|e| format!("cannot drop legacy sections table: {e}"))?;
        conn.execute(
            "CREATE TABLE sections (
                id TEXT PRIMARY KEY,
                book_id TEXT NOT NULL,
                section_index INTEGER NOT NULL,
                title TEXT NOT NULL,
                volume TEXT NOT NULL,
                chunk_index INTEGER NOT NULL,
                byte_offset INTEGER NOT NULL,
                byte_length INTEGER NOT NULL,
                chunks_json TEXT NOT NULL DEFAULT ''
            );",
            [],
        )
        .map_err(|e| format!("failed to create chunked sections table: {e}"))?;
    } else {
        let has_chunks_json: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM pragma_table_info('sections') WHERE name = 'chunks_json')",
                [],
                |row| row.get(0),
            )
            .unwrap_or(false);
        if !has_chunks_json {
            conn.execute("ALTER TABLE sections ADD COLUMN chunks_json TEXT NOT NULL DEFAULT '';", [])
                .map_err(|e| format!("cannot add chunks_json column: {e}"))?;
        }
    }

    conn.execute("CREATE INDEX IF NOT EXISTS idx_sections_book ON sections(book_id, section_index);", [])
        .map_err(|e| format!("failed to create idx_sections_book index: {}", e))?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS reviews (
        book_id TEXT NOT NULL,
        fingerprint TEXT NOT NULL,
        PRIMARY KEY (book_id, fingerprint)
    );",
    )
    .map_err(|error| error.to_string())?;

    Ok(())
}

struct SectionMeta<'a> {
    id: &'a str,
    index: u32,
    title: &'a str,
    volume: &'a str,
    chunk_index: u32,
    byte_offset: u32,
    byte_length: u32,
    chunks_json: String,
}

fn write_chunks<'a>(book_dir: &Path, sections: &'a [Section]) -> Result<(std::path::PathBuf, Vec<SectionMeta<'a>>), String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let staging_dir = book_dir.with_extension(format!("staging-{}-{}", std::process::id(), stamp));
    if staging_dir.exists() {
        let _ = std::fs::remove_dir_all(&staging_dir);
    }
    std::fs::create_dir_all(&staging_dir).map_err(|e| format!("cannot create {}: {e}", staging_dir.display()))?;

    let mut current_chunk_idx: u32 = 0;
    let mut current_chunk_buf = Vec::new();
    let mut metas = Vec::with_capacity(sections.len());

    for sec in sections {
        let body_bytes = sec.body.as_bytes();
        if !current_chunk_buf.is_empty() && current_chunk_buf.len() + body_bytes.len() > CHUNK_TARGET_SIZE {
            let chunk_file = staging_dir.join(format!("{current_chunk_idx}.chunk"));
            std::fs::write(&chunk_file, &current_chunk_buf).map_err(|e| format!("cannot write {}: {e}", chunk_file.display()))?;
            current_chunk_buf.clear();
            current_chunk_idx += 1;
        }
        let byte_offset = current_chunk_buf.len() as u32;
        let byte_length = body_bytes.len() as u32;
        current_chunk_buf.extend_from_slice(body_bytes);
        let chunks_json = if sec.chunks.is_empty() {
            String::new()
        } else {
            serde_json::to_string(&sec.chunks).map_err(|error| error.to_string())?
        };
        metas.push(SectionMeta {
            id: &sec.id,
            index: sec.index,
            title: &sec.title,
            volume: &sec.volume,
            chunk_index: current_chunk_idx,
            byte_offset,
            byte_length,
            chunks_json,
        });
    }

    if !current_chunk_buf.is_empty() {
        let chunk_file = staging_dir.join(format!("{current_chunk_idx}.chunk"));
        std::fs::write(&chunk_file, &current_chunk_buf).map_err(|e| format!("cannot write {}: {e}", chunk_file.display()))?;
    }

    Ok((staging_dir, metas))
}

fn commit_chunks(staging_dir: &Path, target_dir: &Path) -> Result<(), String> {
    if target_dir.exists() {
        std::fs::remove_dir_all(target_dir).map_err(|e| format!("cannot remove {}: {e}", target_dir.display()))?;
    }
    std::fs::rename(staging_dir, target_dir).map_err(|e| format!("cannot rename {} to {}: {e}", staging_dir.display(), target_dir.display()))?;
    Ok(())
}

/// Atomically replaces a book and its complete chapter set, including shorter reimports.
pub(super) fn save(conn: &Connection, novel: &Novel, format: &str, books_dir: &Path) -> Result<(), String> {
    validate(&novel.sections)?;
    if novel.book.chapters as usize != novel.sections.len() {
        return Err("chapter count differs from the supplied sections".into());
    }
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
    let book_dir = books_dir.join(&novel.book.id);
    let (staging_dir, metas) = write_chunks(&book_dir, &novel.sections)?;

    let run_tx = || -> Result<(), String> {
        let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
        tx.execute(
            "INSERT OR REPLACE INTO books (id, title, author, chapters, format, description, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                novel.book.id,
                novel.book.title,
                novel.book.author,
                novel.book.chapters,
                format,
                novel.book.description,
                now
            ],
        )
        .map_err(|e| format!("failed to save book to db: {}", e))?;

        tx.execute("DELETE FROM sections WHERE book_id = ?1", params![novel.book.id])
            .map_err(|e| format!("failed to replace old sections: {}", e))?;
        {
            let mut stmt = tx
                .prepare(
                    "INSERT INTO sections (id, book_id, section_index, title, volume, chunk_index, byte_offset, byte_length, chunks_json)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                )
                .map_err(|e| e.to_string())?;

            for meta in metas {
                stmt.execute(params![
                    meta.id,
                    novel.book.id,
                    meta.index,
                    meta.title,
                    meta.volume,
                    meta.chunk_index,
                    meta.byte_offset,
                    meta.byte_length,
                    meta.chunks_json
                ])
                .map_err(|e| e.to_string())?;
            }
        }
        crate::review::refresh(&tx, &novel.book.id, &novel.sections)?;
        tx.commit().map_err(|e| format!("failed to commit sections transaction: {}", e))?;
        Ok(())
    };

    match run_tx() {
        Ok(()) => {
            commit_chunks(&staging_dir, &book_dir)?;
            Ok(())
        }
        Err(e) => {
            let _ = std::fs::remove_dir_all(&staging_dir);
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
    let book_dir = books_dir.join(book_id);
    let (staging_dir, metas) = write_chunks(&book_dir, sections)?;

    let run_tx = || -> Result<protocol::NovelValidation, String> {
        let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
        if !tx
            .query_row("SELECT EXISTS(SELECT 1 FROM books WHERE id = ?1)", params![book_id], |row| {
                row.get::<_, bool>(0)
            })
            .map_err(|error| error.to_string())?
        {
            return Err("book no longer exists".into());
        }
        {
            // 1. Delete all existing sections for this book
            tx.execute("DELETE FROM sections WHERE book_id = ?1", params![book_id])
                .map_err(|e| format!("failed to delete old sections: {}", e))?;

            // 2. Insert updated sections
            let mut stmt = tx
                .prepare(
                    "INSERT INTO sections (id, book_id, section_index, title, volume, chunk_index, byte_offset, byte_length, chunks_json)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                )
                .map_err(|e| e.to_string())?;

            for meta in metas {
                stmt.execute(params![
                    meta.id,
                    book_id,
                    meta.index,
                    meta.title,
                    meta.volume,
                    meta.chunk_index,
                    meta.byte_offset,
                    meta.byte_length,
                    meta.chunks_json
                ])
                .map_err(|e| e.to_string())?;
            }

            // 3. Update chapter count in books table
            tx.execute("UPDATE books SET chapters = ?1 WHERE id = ?2", params![sections.len() as u32, book_id])
                .map_err(|e| format!("failed to update book chapters count: {}", e))?;
        }
        let report = crate::review::replace(&tx, book_id, sections, retained)?;
        tx.commit().map_err(|e| format!("failed to commit updated sections transaction: {}", e))?;
        Ok(report)
    };

    match run_tx() {
        Ok(report) => {
            commit_chunks(&staging_dir, &book_dir)?;
            Ok(report)
        }
        Err(e) => {
            let _ = std::fs::remove_dir_all(&staging_dir);
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

/// Deletes database records atomically. Cache removal failure rolls back the database change.
pub(super) fn delete(conn: &Connection, book_id: &str, cache_dir: &Path, books_dir: &Path) -> Result<(), String> {
    if book_id.is_empty() || !book_id.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')) {
        return Err("invalid book identity for cache deletion".into());
    }
    let tx = conn.unchecked_transaction().map_err(|error| error.to_string())?;
    tx.execute("DELETE FROM reviews WHERE book_id = ?1", params![book_id])
        .map_err(|error| error.to_string())?;
    tx.execute("DELETE FROM sections WHERE book_id = ?1", params![book_id])
        .map_err(|error| error.to_string())?;
    tx.execute("DELETE FROM books WHERE id = ?1", params![book_id])
        .map_err(|error| error.to_string())?;
    let cache_file = cache_dir.join(format!("{book_id}.txt"));
    match std::fs::remove_file(&cache_file) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("cannot remove {}: {error}", cache_file.display())),
    }
    let book_dir = books_dir.join(book_id);
    match std::fs::remove_dir_all(&book_dir) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("cannot remove {}: {error}", book_dir.display())),
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
        let sections = sections(conn, &book.id, books_dir)?;

        novels.push(Novel { book, sections });
    }

    Ok(novels)
}

/// Loads only shelf metadata and the first title, without hydrating chapter bodies.
pub(super) fn summaries(conn: &Connection) -> Result<Vec<BookSummary>, String> {
    let mut statement = conn
        .prepare(
            "SELECT books.id, books.title, books.author, books.chapters, books.format, books.description,
         COALESCE((SELECT sections.title FROM sections WHERE sections.book_id = books.id ORDER BY section_index LIMIT 1), '正文')
         FROM books ORDER BY created_at DESC, books.id",
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

    let sections = sections(conn, &book.id, books_dir)?;
    Ok(Some(Novel { book, sections }))
}

/// Decodes stored sections from segmented chunk files.
fn sections(conn: &Connection, book_id: &str, books_dir: &Path) -> Result<Vec<Section>, String> {
    let mut sec_stmt = conn
        .prepare(
            "SELECT id, section_index, title, volume, chunk_index, byte_offset, byte_length, chunks_json
             FROM sections WHERE book_id = ?1 ORDER BY section_index ASC",
        )
        .map_err(|e| e.to_string())?;

    let sec_rows = sec_stmt
        .query_map(params![book_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, u32>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, u32>(4)?,
                row.get::<_, u32>(5)?,
                row.get::<_, u32>(6)?,
                row.get::<_, String>(7)?,
            ))
        })
        .map_err(|e| e.to_string())?;

    let book_dir = books_dir.join(book_id);
    let mut sections = Vec::new();
    let mut cached_chunk_idx: Option<u32> = None;
    let mut cached_chunk_buf: Vec<u8> = Vec::new();

    for row_res in sec_rows {
        let (id, index, title, volume, chunk_index, byte_offset, byte_length, chunks_str) = row_res.map_err(|e| e.to_string())?;
        if cached_chunk_idx != Some(chunk_index) {
            let chunk_file = book_dir.join(format!("{chunk_index}.chunk"));
            cached_chunk_buf = std::fs::read(&chunk_file).map_err(|e| format!("cannot read {}: {e}", chunk_file.display()))?;
            cached_chunk_idx = Some(chunk_index);
        }
        let start = byte_offset as usize;
        let end = start + byte_length as usize;
        if end > cached_chunk_buf.len() {
            return Err(format!("byte range {}..{} exceeds chunk length {}", start, end, cached_chunk_buf.len()));
        }
        let body = std::str::from_utf8(&cached_chunk_buf[start..end])
            .map_err(|e| format!("invalid UTF-8 in chunk {chunk_index}: {e}"))?
            .to_string();

        let chunks: Vec<protocol::Chunk> = if chunks_str.is_empty() {
            Vec::new()
        } else {
            serde_json::from_str(&chunks_str).map_err(|error| format!("corrupted chunks in section {id}: {error}"))?
        };

        sections.push(Section {
            id,
            index,
            title,
            volume,
            body,
            chunks,
        });
    }

    Ok(sections)
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
                .map(|index| Section {
                    id: format!("fixture:{}", index),
                    index,
                    title: format!("Chapter {}", index),
                    volume: "Volume".into(),
                    body: "Text".into(),
                    chunks: Vec::new(),
                })
                .collect(),
        }
    }

    fn test_books_dir() -> std::path::PathBuf {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("saga-storage-test-{}-{}", std::process::id(), stamp));
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
            "CREATE TRIGGER reject_import BEFORE INSERT ON sections WHEN NEW.title = 'Reject' BEGIN SELECT RAISE(ABORT, 'fixture failure'); END;",
        )
        .unwrap();
        let mut replacement = sample(2);
        replacement.book.title = "Replacement".into();
        replacement.sections[1].title = "Reject".into();
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
        assert!(delete(&conn, "fixture", Path::new("unused-cache"), &dir).is_err());
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
        // Overwrite chunk file with truncated bytes
        let chunk_file = dir.join("fixture").join("0.chunk");
        std::fs::write(&chunk_file, b"").unwrap();
        assert!(load(&conn, "fixture", &dir).is_err());
        assert!(all(&conn, &dir).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn old_schema_is_migrated_without_hiding_errors_or_losing_metadata() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE books (id TEXT PRIMARY KEY, title TEXT NOT NULL, author TEXT NOT NULL, chapters INTEGER NOT NULL, format TEXT NOT NULL, created_at INTEGER NOT NULL); INSERT INTO books VALUES ('old', 'Old title', 'Author', 0, 'TXT', 1);").unwrap();
        initialize(&conn).unwrap();
        initialize(&conn).unwrap();
        let dir = test_books_dir();
        let book = load(&conn, "old", &dir).unwrap().unwrap().book;
        assert_eq!(book.title, "Old title");
        assert_eq!(book.description, "");
        let summaries = summaries(&conn).unwrap();
        assert_eq!(summaries[0].first_chapter, "正文");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_segmented_256kb_chunking() {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        let dir = test_books_dir();

        // Create 4 chapters of ~100 KB text each (total ~400 KB > 256 KB)
        let large_body = "字".repeat(34_000);
        let sections: Vec<Section> = (0..4)
            .map(|i| Section {
                id: format!("large:{i}"),
                index: i,
                title: format!("Chapter {i}"),
                volume: "正文".to_string(),
                body: format!("Chapter {i} body: {large_body}"),
                chunks: vec![],
            })
            .collect();

        let novel = Novel {
            book: Book {
                id: "large".to_string(),
                title: "Large Book".to_string(),
                author: "Author".to_string(),
                description: String::new(),
                chapters: 4,
            },
            sections,
        };

        save(&conn, &novel, "TXT", &dir).unwrap();

        // 1. Verify multiple chunk files were created
        let chunk0 = dir.join("large").join("0.chunk");
        let chunk1 = dir.join("large").join("1.chunk");
        assert!(chunk0.exists());
        assert!(chunk1.exists());

        // 2. Verify sections table schema and chunk_index partition
        let chunk_indices: Vec<u32> = conn
            .prepare("SELECT chunk_index FROM sections WHERE book_id = 'large' ORDER BY section_index ASC")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(chunk_indices, vec![0, 0, 1, 1]);

        // 3. Verify load reconstructs novel perfectly
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

        // Verify chunk file exists
        assert!(dir.join("test_book_1").join("0.chunk").exists());

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
        let loaded_updated = load(&conn, "test_book_1", &dir).unwrap().unwrap();
        assert_eq!(loaded_updated.book.chapters, 2);
        assert_eq!(loaded_updated.sections.len(), 2);
        assert_eq!(loaded_updated.sections[0].title, "第一章 (已核验校正)");
        assert_eq!(loaded_updated.sections[0].body, "修正后正文。");
        assert_eq!(loaded_updated.sections[1].body, "第二章正文。");

        // 4. Create mock cache file
        let cache_file = dir.join("test_book_1.txt");
        std::fs::write(&cache_file, b"test content").unwrap();
        assert!(cache_file.exists());

        // 5. Delete
        delete(&conn, "test_book_1", &dir, &dir).unwrap();
        let loaded_after = all(&conn, &dir).unwrap();
        assert_eq!(loaded_after.len(), 0);
        assert!(!cache_file.exists());
        assert!(!dir.join("test_book_1").exists());

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
