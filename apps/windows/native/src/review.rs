//! Content-bound retain decisions, independent of chapter text and numbering.

use protocol::{NovelValidation, Section};
use rusqlite::{params, Connection};
use std::collections::HashSet;

/// Builds stable finding identities from the affected section and its source context.
fn analyze(sections: &[Section]) -> Result<NovelValidation, String> {
    let mut report = reader::validate_novel_sequence(sections);
    let hashes: Vec<String> = sections
        .iter()
        .map(|section| {
            serde_json::to_vec(&(&section.title, &section.volume, &section.body))
                .map(|bytes| protocol::hash(&bytes))
                .map_err(|error| error.to_string())
        })
        .collect::<Result<_, _>>()?;
    for anomaly in &mut report.anomalies {
        let index = anomaly.section_index as usize;
        let related: Vec<&String> = sections
            .iter()
            .zip(&hashes)
            .filter(|(section, _)| section.title == anomaly.title && section.volume == anomaly.volume)
            .map(|(_, hash)| hash)
            .collect();
        let bytes = serde_json::to_vec(&(
            "retain-v1",
            &anomaly.anomaly_type,
            index,
            &anomaly.description,
            hashes.get(index),
            index.checked_sub(1).and_then(|previous| hashes.get(previous)),
            related,
        ))
        .map_err(|error| error.to_string())?;
        anomaly.key = protocol::hash(&bytes);
    }
    Ok(report)
}

/// Loads the compact decision set without hashing the book again.
fn keys(conn: &Connection, book: &str) -> Result<HashSet<String>, String> {
    let mut statement = conn
        .prepare("SELECT fingerprint FROM reviews WHERE book_id = ?1")
        .map_err(|error| error.to_string())?;
    let collected = statement
        .query_map(params![book], |row| row.get::<_, String>(0))
        .map_err(|error| error.to_string())?
        .collect::<Result<HashSet<_>, _>>()
        .map_err(|error| error.to_string());
    collected
}

/// Reads persisted decisions without suppressing the underlying validation findings.
pub fn inspect(conn: &Connection, book: &str, sections: &[Section]) -> Result<NovelValidation, String> {
    let mut report = analyze(sections)?;
    let keys = keys(conn, book)?;
    for anomaly in &mut report.anomalies {
        anomaly.retained = keys.contains(&anomaly.key);
    }
    Ok(report)
}

/// Prunes stale decisions on reimport; first imports have no decisions to revalidate.
pub fn refresh(conn: &Connection, book: &str, sections: &[Section]) -> Result<(), String> {
    let keys: Vec<String> = keys(conn, book)?.into_iter().collect();
    if !keys.is_empty() {
        replace(conn, book, sections, &keys)?;
    }
    Ok(())
}

/// Replaces decisions within the caller's section transaction; stale keys cannot hide new findings.
pub fn replace(conn: &Connection, book: &str, sections: &[Section], keys: &[String]) -> Result<NovelValidation, String> {
    let mut report = analyze(sections)?;
    conn.execute("DELETE FROM reviews WHERE book_id = ?1", params![book])
        .map_err(|error| error.to_string())?;
    let requested: HashSet<&String> = keys.iter().collect();
    for anomaly in &mut report.anomalies {
        if requested.contains(&anomaly.key) {
            conn.execute(
                "INSERT OR IGNORE INTO reviews (book_id, fingerprint) VALUES (?1, ?2)",
                params![book, anomaly.key],
            )
            .map_err(|error| error.to_string())?;
            anomaly.retained = true;
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage;

    fn sample() -> protocol::Novel {
        reader::parse_txt_str(
            "第2656章 前篇\n这是第一篇完整正文，与后篇内容不同。\n第2656章 后篇\n这是衔接前篇的另一段完整正文。",
            "retention",
            None,
        )
        .unwrap()
    }

    fn setup() -> (Connection, protocol::Novel) {
        let conn = Connection::open_in_memory().unwrap();
        storage::initialize(&conn).unwrap();
        let novel = sample();
        storage::save(&conn, &novel, "TXT").unwrap();
        (conn, novel)
    }

    fn keep(conn: &Connection, novel: &protocol::Novel) -> String {
        let report = inspect(conn, &novel.book.id, &novel.sections).unwrap();
        assert_eq!(report.anomalies.len(), 1);
        let key = report.anomalies[0].key.clone();
        storage::update(conn, &novel.book.id, &novel.sections, std::slice::from_ref(&key)).unwrap();
        key
    }

    #[test]
    fn retain_preserves_text_and_survives_reimport_without_erasing_the_finding() {
        let (conn, novel) = setup();
        keep(&conn, &novel);
        storage::save(&conn, &novel, "TXT").unwrap();
        assert_eq!(storage::load(&conn, &novel.book.id).unwrap().unwrap(), novel);
        let report = inspect(&conn, &novel.book.id, &novel.sections).unwrap();
        assert!(!report.is_valid);
        assert!(report.anomalies[0].retained);
        assert!(!inspect(&conn, "other", &novel.sections).unwrap().anomalies[0].retained);
    }

    #[test]
    fn changed_title_body_or_previous_context_requires_review_again() {
        for field in 0..3 {
            let (conn, mut novel) = setup();
            let stale = keep(&conn, &novel);
            match field {
                0 => novel.sections[1].title.push_str(" 修订"),
                1 => novel.sections[1].body.push_str("新内容。"),
                _ => novel.sections[0].body.push_str("前文变化。"),
            }
            let report = storage::update(&conn, &novel.book.id, &novel.sections, &[stale]).unwrap();
            assert!(!report.anomalies[0].retained);
            assert_eq!(conn.query_row("SELECT COUNT(*) FROM reviews", [], |row| row.get::<_, i64>(0)).unwrap(), 0);
        }
    }

    #[test]
    fn revocation_and_failed_commit_preserve_the_correct_decisions() {
        let (conn, mut novel) = setup();
        keep(&conn, &novel);
        let original = novel.clone();
        conn.execute_batch("CREATE TRIGGER reject_review BEFORE DELETE ON reviews BEGIN SELECT RAISE(ABORT, 'review failure'); END;")
            .unwrap();
        novel.sections[1].body.push_str("本次修改应当回滚。");
        assert!(storage::update(&conn, &novel.book.id, &novel.sections, &[]).is_err());
        assert_eq!(storage::load(&conn, &novel.book.id).unwrap().unwrap(), original);
        assert!(inspect(&conn, &novel.book.id, &original.sections).unwrap().anomalies[0].retained);
        conn.execute_batch("DROP TRIGGER reject_review").unwrap();
        let report = storage::update(&conn, &novel.book.id, &original.sections, &[]).unwrap();
        assert!(!report.anomalies[0].retained);
        assert!(storage::update(&conn, "missing", &novel.sections, &[]).is_err());
    }

    #[test]
    #[ignore = "requires SAGA_SAMPLE; uses an isolated in-memory database"]
    fn original_book_retains_only_the_two_reviewed_numbering_conflicts() {
        let bytes = std::fs::read(std::env::var_os("SAGA_SAMPLE").expect("SAGA_SAMPLE required")).unwrap();
        let novel = reader::parse_txt(&bytes, None).unwrap();
        let conn = Connection::open_in_memory().unwrap();
        storage::initialize(&conn).unwrap();
        storage::save(&conn, &novel, "TXT").unwrap();
        let report = inspect(&conn, &novel.book.id, &novel.sections).unwrap();
        assert!(report.warnings.is_empty());
        assert_eq!(report.anomalies.len(), 2);
        assert_eq!(report.anomalies[0].title, "第2656章 我就知道遇上霸宋不会有好事");
        assert_eq!(report.anomalies[1].title, "第2680章 长生柱：虚日柱！");
        let keys: Vec<String> = report.anomalies.iter().map(|item| item.key.clone()).collect();
        let partial = storage::update(&conn, &novel.book.id, &novel.sections, &keys[..1]).unwrap();
        assert!(partial.anomalies[0].retained);
        assert!(!partial.anomalies[1].retained);
        storage::update(&conn, &novel.book.id, &novel.sections, &keys).unwrap();
        storage::save(&conn, &novel, "TXT").unwrap();
        let final_report = inspect(&conn, &novel.book.id, &novel.sections).unwrap();
        assert!(final_report.anomalies.iter().all(|item| item.retained));
        assert_eq!(storage::load(&conn, &novel.book.id).unwrap().unwrap(), novel);
        println!(
            "sample_bytes={} sections={} retained=2 pending=0 original_sections_unchanged=true",
            bytes.len(),
            novel.sections.len()
        );
    }

    #[test]
    fn retain_survives_closing_and_reopening_the_database() {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("saga-review-{}-{}.db", std::process::id(), stamp));
        let novel = sample();
        {
            let conn = Connection::open(&path).unwrap();
            storage::initialize(&conn).unwrap();
            storage::save(&conn, &novel, "TXT").unwrap();
            keep(&conn, &novel);
        }
        {
            let conn = Connection::open(&path).unwrap();
            storage::initialize(&conn).unwrap();
            let loaded = storage::load(&conn, &novel.book.id).unwrap().unwrap();
            assert!(inspect(&conn, &novel.book.id, &loaded.sections).unwrap().anomalies[0].retained);
        }
        std::fs::remove_file(path).unwrap();
    }
}
