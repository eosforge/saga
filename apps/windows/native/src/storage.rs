//! Saga SQLite local storage persistence engine.

use protocol::{Book, BookSummary, Chunk, Novel, Section};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

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

    conn.execute(
        "CREATE TABLE IF NOT EXISTS sections (
            id TEXT PRIMARY KEY,
            book_id TEXT NOT NULL,
            section_index INTEGER NOT NULL,
            title TEXT NOT NULL,
            volume TEXT NOT NULL,
            body TEXT NOT NULL,
            chunks_json TEXT NOT NULL
        );",
        [],
    )
    .map_err(|e| format!("failed to create sections table: {}", e))?;

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

/// Atomically replaces a book and its complete chapter set, including shorter reimports.
pub(super) fn save(conn: &Connection, novel: &Novel, format: &str) -> Result<(), String> {
    validate(&novel.sections)?;
    if novel.book.chapters as usize != novel.sections.len() {
        return Err("chapter count differs from the supplied sections".into());
    }
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64;

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
                "INSERT INTO sections (id, book_id, section_index, title, volume, body, chunks_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )
            .map_err(|e| e.to_string())?;

        for sec in &novel.sections {
            let chunks_json = if sec.chunks.is_empty() {
                "[]".to_string()
            } else {
                serde_json::to_string(&sec.chunks).map_err(|error| error.to_string())?
            };
            stmt.execute(params![sec.id, novel.book.id, sec.index, sec.title, sec.volume, sec.body, chunks_json])
                .map_err(|e| e.to_string())?;
        }
    }
    crate::review::refresh(&tx, &novel.book.id, &novel.sections)?;
    tx.commit().map_err(|e| format!("failed to commit sections transaction: {}", e))?;

    Ok(())
}

/// Updates and replaces all sections for an existing novel (e.g. after user audit/resolution).
pub(super) fn update(conn: &Connection, book_id: &str, sections: &[Section], retained: &[String]) -> Result<protocol::NovelValidation, String> {
    validate(sections)?;
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
                "INSERT INTO sections (id, book_id, section_index, title, volume, body, chunks_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )
            .map_err(|e| e.to_string())?;

        for (idx, sec) in sections.iter().enumerate() {
            let chunks_json = serde_json::to_string(&sec.chunks).map_err(|error| error.to_string())?;
            stmt.execute(params![sec.id, book_id, idx as u32, sec.title, sec.volume, sec.body, chunks_json])
                .map_err(|e| e.to_string())?;
        }

        // 3. Update chapter count in books table
        tx.execute("UPDATE books SET chapters = ?1 WHERE id = ?2", params![sections.len() as u32, book_id])
            .map_err(|e| format!("failed to update book chapters count: {}", e))?;
    }
    let report = crate::review::replace(&tx, book_id, sections, retained)?;
    tx.commit().map_err(|e| format!("failed to commit updated sections transaction: {}", e))?;

    Ok(report)
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
/// A commit failure after cache removal may leave a readable book without its optional raw cache.
pub(super) fn delete(conn: &Connection, book_id: &str, cache_dir: &Path) -> Result<(), String> {
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
    tx.commit().map_err(|error| format!("cannot commit book deletion: {error}"))
}
/// Loads all persisted novels and their sections from SQLite.
pub(super) fn all(conn: &Connection) -> Result<Vec<Novel>, String> {
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
        let sections = sections(conn, &book.id)?;

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
pub(super) fn load(conn: &Connection, book_id: &str) -> Result<Option<Novel>, String> {
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

    let sections = sections(conn, &book.id)?;
    Ok(Some(Novel { book, sections }))
}

/// Decodes stored sections and rejects corruption rather than silently dropping chunks.
fn sections(conn: &Connection, book_id: &str) -> Result<Vec<Section>, String> {
    let mut sec_stmt = conn
        .prepare(
            "SELECT id, section_index, title, volume, body, chunks_json
             FROM sections WHERE book_id = ?1 ORDER BY section_index ASC",
        )
        .map_err(|e| e.to_string())?;

    let sec_iter = sec_stmt
        .query_map(params![book_id], |row| {
            let chunks_str: String = row.get(5)?;
            let chunks: Vec<Chunk> = serde_json::from_str(&chunks_str)
                .map_err(|error| rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, Box::new(error)))?;
            Ok(Section {
                id: row.get(0)?,
                index: row.get(1)?,
                title: row.get(2)?,
                volume: row.get(3)?,
                body: row.get(4)?,
                chunks,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut sections = Vec::new();
    for sec_res in sec_iter {
        sections.push(sec_res.map_err(|e| e.to_string())?);
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

    #[test]
    fn reimport_replaces_old_sections_instead_of_leaving_a_stale_tail() {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        save(&conn, &sample(4), "TXT").unwrap();
        save(&conn, &sample(2), "TXT").unwrap();
        let loaded = load(&conn, "fixture").unwrap().unwrap();
        assert_eq!(loaded, sample(2));
    }

    #[test]
    fn failed_reimport_preserves_previous_metadata_and_sections() {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        let original = sample(4);
        save(&conn, &original, "TXT").unwrap();
        conn.execute_batch(
            "CREATE TRIGGER reject_import BEFORE INSERT ON sections WHEN NEW.title = 'Reject' BEGIN SELECT RAISE(ABORT, 'fixture failure'); END;",
        )
        .unwrap();
        let mut replacement = sample(2);
        replacement.book.title = "Replacement".into();
        replacement.sections[1].title = "Reject".into();
        assert!(save(&conn, &replacement, "TXT").is_err());
        assert_eq!(load(&conn, "fixture").unwrap().unwrap(), original);
    }

    #[test]
    fn rejected_deletion_preserves_the_entire_book() {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        let original = sample(2);
        save(&conn, &original, "TXT").unwrap();
        conn.execute_batch("CREATE TRIGGER reject_delete BEFORE DELETE ON books BEGIN SELECT RAISE(ABORT, 'fixture rejection'); END;")
            .unwrap();
        assert!(delete(&conn, "fixture", Path::new("unused-cache")).is_err());
        assert_eq!(load(&conn, "fixture").unwrap().unwrap(), original);
    }

    #[test]
    fn inconsistent_section_positions_are_rejected_before_writing() {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        let original = sample(2);
        save(&conn, &original, "TXT").unwrap();
        let mut sections = original.sections.clone();
        sections[1].index = 0;
        assert!(update(&conn, "fixture", &sections, &[]).is_err());
        assert_eq!(load(&conn, "fixture").unwrap().unwrap(), original);
    }

    #[test]
    fn corrupted_chunks_and_cache_cleanup_failure_are_not_success() {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        let original = sample(1);
        save(&conn, &original, "TXT").unwrap();
        conn.execute("UPDATE sections SET chunks_json = 'invalid json'", []).unwrap();
        assert!(load(&conn, "fixture").is_err());
        assert!(all(&conn).is_err());
        save(&conn, &original, "TXT").unwrap();
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let directory = std::env::temp_dir().join(format!("saga-cleanup-{}-{stamp}", std::process::id()));
        std::fs::create_dir_all(directory.join("fixture.txt")).unwrap();
        assert!(delete(&conn, "fixture", &directory).is_err());
        assert!(delete(&conn, "../escape", &directory).is_err());
        assert_eq!(load(&conn, "fixture").unwrap().unwrap(), original);
        std::fs::remove_dir(directory.join("fixture.txt")).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn old_schema_is_migrated_without_hiding_errors_or_losing_metadata() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE books (id TEXT PRIMARY KEY, title TEXT NOT NULL, author TEXT NOT NULL, chapters INTEGER NOT NULL, format TEXT NOT NULL, created_at INTEGER NOT NULL); INSERT INTO books VALUES ('old', 'Old title', 'Author', 0, 'TXT', 1);").unwrap();
        initialize(&conn).unwrap();
        initialize(&conn).unwrap();
        let book = load(&conn, "old").unwrap().unwrap().book;
        assert_eq!(book.title, "Old title");
        assert_eq!(book.description, "");
        let summaries = summaries(&conn).unwrap();
        assert_eq!(summaries[0].first_chapter, "正文");
    }

    #[test]
    fn test_sqlite_novel_crud_lifecycle() {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();

        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let temp_dir = std::env::temp_dir().join(format!("saga-crud-{}-{stamp}", std::process::id()));
        std::fs::create_dir(&temp_dir).unwrap();

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
            chunks: vec![Chunk {
                id: "test_book_1:0:c0".to_string(),
                chapter: "test_book_1:0".to_string(),
                index: 0,
                paragraph: 0,
                text: "测试正文。".to_string(),
            }],
        };

        let novel = Novel { book, sections: vec![section] };

        // 1. Save
        save(&conn, &novel, "TXT").unwrap();

        // 2. Load
        let loaded = all(&conn).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].book.title, "测试小说");
        assert_eq!(loaded[0].sections[0].volume, "第一卷");
        assert_eq!(loaded[0].sections[0].chunks.len(), 1);

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
        update(&conn, "test_book_1", &updated_sections, &[]).unwrap();
        let loaded_updated = load(&conn, "test_book_1").unwrap().unwrap();
        assert_eq!(loaded_updated.book.chapters, 2);
        assert_eq!(loaded_updated.sections.len(), 2);
        assert_eq!(loaded_updated.sections[0].title, "第一章 (已核验校正)");

        // 4. Create mock cache file
        let cache_file = temp_dir.join("test_book_1.txt");
        std::fs::write(&cache_file, b"test content").unwrap();
        assert!(cache_file.exists());

        // 5. Delete
        delete(&conn, "test_book_1", &temp_dir).unwrap();
        let loaded_after = all(&conn).unwrap();
        assert_eq!(loaded_after.len(), 0);
        assert!(!cache_file.exists());

        std::fs::remove_dir(&temp_dir).unwrap();
    }
}
