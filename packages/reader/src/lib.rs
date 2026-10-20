mod heading;
pub use heading::{is_chapter_heading, is_volume_heading};

pub mod validator;
pub use validator::validate_novel_sequence;

use encoding_rs::{GB18030, UTF_16BE, UTF_16LE, UTF_8};
use protocol::{Book, Chunk, Novel, Section};
use regex::Regex;
use std::sync::LazyLock;
use thiserror::Error;

/// Error representations for reader parsing operations.
#[derive(Debug, Error)]
pub enum ReaderError {
    /// Provided raw byte buffer was empty.
    #[error("input byte buffer is empty")]
    Empty,

    /// Unrecoverable character decoding error.
    #[error("failed to decode text stream: {0}")]
    Encoding(String),

    /// Missing or invalid structure.
    #[error("parsing failure: {0}")]
    Parse(String),
}

/// Automatically sniffs character encoding (UTF-8, UTF-8 BOM, UTF-16, GB18030/GBK) and decodes to UTF-8.
pub fn decode_bytes(bytes: &[u8]) -> Result<String, ReaderError> {
    if bytes.is_empty() {
        return Err(ReaderError::Empty);
    }

    // 1. Check for standard Byte Order Marks (BOM)
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        let (cow, _, malformed) = UTF_8.decode(&bytes[3..]);
        if !malformed {
            return Ok(cow.into_owned());
        }
    } else if bytes.starts_with(&[0xFF, 0xFE]) {
        let (cow, _, _) = UTF_16LE.decode(&bytes[2..]);
        return Ok(cow.into_owned());
    } else if bytes.starts_with(&[0xFE, 0xFF]) {
        let (cow, _, _) = UTF_16BE.decode(&bytes[2..]);
        return Ok(cow.into_owned());
    }

    // 2. Validate standard UTF-8 without BOM
    if let Ok(utf8_str) = std::str::from_utf8(bytes) {
        return Ok(utf8_str.to_string());
    }

    // 3. Fallback to GB18030 (superset of GBK and GB2312)
    let (cow, _, malformed) = GB18030.decode(bytes);
    if malformed {
        return Err(ReaderError::Encoding("malformed GB18030/UTF8 byte sequence".to_string()));
    }
    Ok(cow.into_owned())
}

/// Partitions a chapter's body text into paragraph-preserving semantic sentence chunks.
pub fn split_sentences(text: &str, chapter_id: &str) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let mut chunk_index = 0u32;
    let mut paragraph_index = 0u32;

    for line in text.lines() {
        let trimmed_line = line.trim();
        if trimmed_line.is_empty() {
            continue;
        }

        let chars: Vec<char> = trimmed_line.chars().collect();
        let total = chars.len();
        let mut i = 0;
        let mut current = String::new();

        while i < total {
            let c = chars[i];
            current.push(c);

            let is_terminal = matches!(c, '。' | '！' | '？' | '；');
            if is_terminal {
                // Adhere trailing quotation marks or closing brackets
                while i + 1 < total && matches!(chars[i + 1], '”' | '"' | '’' | '\'' | '）' | ')' | '』' | '」') {
                    i += 1;
                    current.push(chars[i]);
                }

                let trimmed = current.trim();
                if !trimmed.is_empty() {
                    chunks.push(Chunk {
                        id: format!("{}:c{}", chapter_id, chunk_index),
                        chapter: chapter_id.to_string(),
                        index: chunk_index,
                        paragraph: paragraph_index,
                        text: trimmed.to_string(),
                    });
                    chunk_index += 1;
                }
                current.clear();
            }
            i += 1;
        }

        let remaining = current.trim();
        if !remaining.is_empty() {
            chunks.push(Chunk {
                id: format!("{}:c{}", chapter_id, chunk_index),
                chapter: chapter_id.to_string(),
                index: chunk_index,
                paragraph: paragraph_index,
                text: remaining.to_string(),
            });
            chunk_index += 1;
        }

        paragraph_index += 1;
    }

    chunks
}

/// Parses raw TXT binary bytes into a complete structured `Novel` entity with chapters, volumes, and paragraph chunks.
pub fn parse_txt(bytes: &[u8], title_hint: Option<&str>) -> Result<Novel, ReaderError> {
    let decoded = decode_bytes(bytes)?;
    let book_id = protocol::hash(bytes);
    parse_txt_str(&decoded, &book_id, title_hint)
}

/// Parses decoded UTF-8 text into a complete structured `Novel`.
pub fn parse_txt_str(text: &str, book_id: &str, title_hint: Option<&str>) -> Result<Novel, ReaderError> {
    parse_txt_str_with_progress(text, book_id, title_hint, |_, _| {})
}

/// Parses text with monotonic progress shared between candidate discovery and body assembly.
pub fn parse_txt_str_with_progress(text: &str, book_id: &str, title_hint: Option<&str>, mut progress: impl FnMut(usize, usize)) -> Result<Novel, ReaderError> {
    let mut sections = Vec::new();
    let mut current_title = String::new();
    let mut current_volume = String::from("正文");
    let mut current_body = String::new();
    let mut chapter_index = 0u32;

    let mut has_found_first_chapter = false;
    let mut processed = 0;
    progress(0, text.len());
    let boundaries = heading::scan(text, |done| progress(done / 2, text.len()));
    let mut boundaries = boundaries.into_iter().peekable();

    // Helper to commit accumulated chapter lines
    let commit_chapter = |title: &str, volume: &str, body_text: &str, idx: &mut u32, secs: &mut Vec<Section>| {
        let trimmed_body = body_text.trim();
        if !trimmed_body.is_empty() || !title.is_empty() {
            let chapter_id = format!("{}:{}", book_id, *idx);
            // Eager chunking for first sections (< 5 chapters) for instant preview/verification,
            // while downstream chapters are chunked on-demand to maintain sub-second ingestion.
            let chunks = if *idx < 5 { split_sentences(trimmed_body, &chapter_id) } else { Vec::new() };
            secs.push(Section {
                id: chapter_id,
                index: *idx,
                title: title.to_string(),
                volume: volume.to_string(),
                body: trimmed_body.to_string(),
                chunks,
            });
            *idx += 1;
        }
    };

    for (index, segment) in text.split_inclusive('\n').enumerate() {
        // Match str::lines while retaining delimiters for exact byte accounting.
        let line = segment.strip_suffix('\n').unwrap_or(segment);
        let line = if segment.ends_with('\n') {
            line.strip_suffix('\r').unwrap_or(line)
        } else {
            line
        };
        let boundary = if boundaries.peek().is_some_and(|boundary| boundary.offset == processed) {
            boundaries.next()
        } else {
            None
        };
        if boundary.as_ref().is_some_and(|boundary| boundary.volume) {
            let heading = line.trim().to_string();
            // If we have actual content prior to volume switch, commit it
            let has_body = !current_body.trim().is_empty();
            if !current_title.is_empty() || has_body {
                let prev_title = if current_title.is_empty() { "前言 / 简介" } else { &current_title };
                commit_chapter(prev_title, &current_volume, &current_body, &mut chapter_index, &mut sections);
                current_title.clear();
                current_body.clear();
            }
            current_volume = heading;
            has_found_first_chapter = true;
        } else if boundary.is_some() {
            let heading = line.trim().to_string();
            if !has_found_first_chapter {
                // Content preceding the first identified chapter is grouped into Preface/Preamble
                let has_body = !current_body.trim().is_empty();
                if has_body {
                    commit_chapter("前言 / 简介", "前言", &current_body, &mut chapter_index, &mut sections);
                    current_body.clear();
                }
                has_found_first_chapter = true;
            } else {
                commit_chapter(&current_title, &current_volume, &current_body, &mut chapter_index, &mut sections);
                current_body.clear();
            }
            current_title = heading;
        } else {
            current_body.push_str(line);
            current_body.push('\n');
        }
        processed += segment.len();
        if index % 256 == 255 {
            progress(text.len() / 2 + processed - processed / 2, text.len());
        }
    }

    // Commit trailing chapter
    if !current_body.is_empty() || !current_title.is_empty() {
        let final_title = if current_title.is_empty() { "正文".to_string() } else { current_title };
        commit_chapter(&final_title, &current_volume, &current_body, &mut chapter_index, &mut sections);
    }

    let (meta_title, meta_author, meta_description) = extract_metadata(text, title_hint);

    let final_title = if !meta_title.is_empty() && meta_title != "未命名书籍" {
        meta_title
    } else {
        sections.first().map(|s| s.title.clone()).unwrap_or_else(|| "未命名书籍".to_string())
    };

    let book = Book {
        id: book_id.to_string(),
        title: final_title,
        author: meta_author,
        description: meta_description,
        chapters: sections.len() as u32,
    };

    progress(text.len(), text.len());
    Ok(Novel { book, sections })
}

/// Extracts clean publication title, author attribution, and synopsis description from filename hint and text.
pub fn extract_metadata(text: &str, title_hint: Option<&str>) -> (String, String, String) {
    let mut title = String::new();
    let mut author = String::new();
    let mut description = String::new();

    // 1. Try extracting from title_hint (filename)
    if let Some(hint) = title_hint {
        let clean_hint = hint.trim();
        static FILENAME_BOOK_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"《(.*?)》\s*(?:(?:作者|著|文)[：:])?\s*(.*)").expect("valid regex"));
        static FILENAME_AUTHOR_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(.*?)\s+(?:作者|著|文)[：:]\s*(.*)").expect("valid regex"));
        static FILENAME_BRACKET_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[\[【(（](.*?)[\]】)）]\s*(.*)").expect("valid regex"));
        static FILENAME_DASH_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(.*?)\s*[-_]\s*(?:作者[：:])?\s*(.*)").expect("valid regex"));

        if let Some(caps) = FILENAME_BOOK_REGEX.captures(clean_hint) {
            title = caps.get(1).map(|m| m.as_str().trim().to_string()).unwrap_or_default();
            let auth_candidate = caps.get(2).map(|m| m.as_str().trim()).unwrap_or_default();
            if !auth_candidate.is_empty() {
                author = auth_candidate
                    .trim_matches(|c| c == '【' || c == '】' || c == '(' || c == ')' || c == '（' || c == '）' || c == '[' || c == ']')
                    .trim()
                    .to_string();
            }
        } else if let Some(caps) = FILENAME_AUTHOR_REGEX.captures(clean_hint) {
            title = caps.get(1).map(|m| m.as_str().trim().to_string()).unwrap_or_default();
            author = caps.get(2).map(|m| m.as_str().trim().to_string()).unwrap_or_default();
        } else if let Some(caps) = FILENAME_BRACKET_REGEX.captures(clean_hint) {
            author = caps.get(1).map(|m| m.as_str().trim().to_string()).unwrap_or_default();
            title = caps.get(2).map(|m| m.as_str().trim().to_string()).unwrap_or_default();
        } else if let Some(caps) = FILENAME_DASH_REGEX.captures(clean_hint) {
            let left = caps.get(1).map(|m| m.as_str().trim().to_string()).unwrap_or_default();
            let right = caps.get(2).map(|m| m.as_str().trim().to_string()).unwrap_or_default();
            if !left.is_empty() && !right.is_empty() {
                title = left;
                author = right;
            }
        }

        if title.is_empty() && !clean_hint.is_empty() {
            title = clean_hint.to_string();
        }
    }

    // 2. Scan first 60 lines for text metadata tags: 书名, 作者, 简介
    static TEXT_TITLE_REGEX: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^[ \t]*(?:【?书名】?|小说名称|作品名称)\s*[：:]\s*(?:《)?([^》\n\r]+)(?:》)?").expect("valid regex"));
    static TEXT_AUTHOR_REGEX: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^[ \t]*(?:【?作者】?|作\s*者|著|文\s*[/／])\s*[：:]\s*(?:【|\[)?([^\s\n\r，,。】\]]{1,25})(?:】|\])?").expect("valid regex")
    });
    static TEXT_DESC_START_REGEX: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^[ \t]*(?:【?(?:内容简介|简介|文案|内容提要|作品简介)】?)\s*[：:]?\s*(.*)$").expect("valid regex"));

    let mut desc_collecting = false;
    let mut desc_lines: Vec<String> = Vec::new();

    for line in text.lines().take(60) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if desc_collecting && !desc_lines.is_empty() {
                desc_collecting = false;
            }
            continue;
        }

        // Stop scanning when encountering actual chapter headings
        if is_chapter_heading(trimmed) || is_volume_heading(trimmed) {
            break;
        }

        if title.is_empty() || title == "未命名书籍" {
            if let Some(caps) = TEXT_TITLE_REGEX.captures(trimmed) {
                if let Some(m) = caps.get(1) {
                    title = m.as_str().trim().to_string();
                }
            }
        }

        if author.is_empty() || author == "未知作者" {
            if let Some(caps) = TEXT_AUTHOR_REGEX.captures(trimmed) {
                if let Some(m) = caps.get(1) {
                    author = m.as_str().trim().to_string();
                }
            }
        }

        if let Some(caps) = TEXT_DESC_START_REGEX.captures(trimmed) {
            desc_collecting = true;
            if let Some(m) = caps.get(1) {
                let rest = m.as_str().trim();
                if !rest.is_empty() {
                    desc_lines.push(rest.to_string());
                }
            }
            continue;
        }

        if desc_collecting {
            if trimmed.starts_with("书名")
                || trimmed.starts_with("作者")
                || trimmed.starts_with("类型")
                || trimmed.starts_with("字数")
                || trimmed.starts_with("状态")
                || trimmed.starts_with("更新")
                || trimmed.starts_with("第一")
            {
                desc_collecting = false;
            } else {
                desc_lines.push(trimmed.to_string());
            }
        }
    }

    if !desc_lines.is_empty() {
        description = desc_lines.join(" ");
        if description.chars().count() > 300 {
            description = description.chars().take(300).collect::<String>() + "…";
        }
    }

    if title.is_empty() {
        title = "未命名书籍".to_string();
    }
    if author.is_empty() {
        author = "未知作者".to_string();
    }

    (title, author, description)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chapter_heading_detection() {
        assert!(is_chapter_heading("第一章 宴桃园豪杰三结义"));
        assert!(is_chapter_heading("第124章 坐忘道"));
        assert!(is_chapter_heading("第二十一回 曹操煮酒论英雄"));
        assert!(is_chapter_heading("序章"));
        assert!(is_chapter_heading("楔子"));
        assert!(is_chapter_heading("尾声"));
        assert!(is_chapter_heading("后记"));
        assert!(is_chapter_heading("前言"));
        assert!(is_chapter_heading("简介"));
        assert!(is_chapter_heading("001 陨落的天才"));
        assert!(is_chapter_heading("1. 初始的世界"));
        assert!(is_chapter_heading("Chapter 42 The Answer"));

        // All variations of 番外 (side-story / extra chapters)
        assert!(is_chapter_heading("番外"));
        assert!(is_chapter_heading("番外一 赵敏篇"));
        assert!(is_chapter_heading("番外二"));
        assert!(is_chapter_heading("番外1 往事"));
        assert!(is_chapter_heading("番外篇 陆雪琪"));
        assert!(is_chapter_heading("番外：另一个结局"));
        assert!(is_chapter_heading("番外·雪中悍刀行"));
        assert!(is_chapter_heading("【番外】少年游"));
        assert!(is_chapter_heading("【番外一】"));
        assert!(is_chapter_heading("（番外）重逢"));

        // Reject noise, dialog quotes, long sentences, and narrative time transitions
        assert!(!is_chapter_heading(""));
        assert!(!is_chapter_heading("“番外一说得很有道理。”"));
        assert!(!is_chapter_heading("她在第一章中曾经说过这样的一句话，但是没有人相信他。"));
        assert!(!is_chapter_heading("3.5小时后……"));
        assert!(!is_chapter_heading("3.5小时后"));
        assert!(!is_chapter_heading("10分钟后，他站了起来。"));
        assert!(!is_chapter_heading("2天后……"));
        assert!(!is_chapter_heading("1.5万字"));
    }

    #[test]
    fn test_sentence_chunking_with_quotes() {
        let sample = "李火旺摇了摇头。“迷惘，还是清醒？”白灵淼轻声问。篝火噼啪作响！";
        let chunks = split_sentences(sample, "b1:c0");
        assert_eq!(chunks.len(), 4);
        assert_eq!(chunks[0].text, "李火旺摇了摇头。");
        assert_eq!(chunks[1].text, "“迷惘，还是清醒？”");
        assert_eq!(chunks[2].text, "白灵淼轻声问。");
        assert_eq!(chunks[3].text, "篝火噼啪作响！");
    }

    #[test]
    fn test_full_txt_parse_workflow() {
        let text = r#"
作品世界观介绍：这是一个神秘莫测的志怪异仙世界。
第一章 启程
李火旺睁开双眼，只觉天旋地转。
第二章 破庙
破庙里的篝火依旧在燃烧。
"#;
        let novel = parse_txt_str(text, "mock_book_id", Some("道诡异仙")).unwrap();
        assert_eq!(novel.book.title, "道诡异仙");
        assert_eq!(novel.sections.len(), 3);
        assert_eq!(novel.sections[0].title, "前言 / 简介");
        assert_eq!(novel.sections[1].title, "第一章 启程");
        assert_eq!(novel.sections[2].title, "第二章 破庙");
        assert_eq!(novel.sections[1].chunks.len(), 1);
        assert_eq!(novel.sections[1].chunks[0].text, "李火旺睁开双眼，只觉天旋地转。");
    }

    #[test]
    fn test_preface_extra_chapter_partition() {
        let text = r#"
书名：倚天屠龙记
作者：金庸
简介：这是一部经典的武侠巨作。

番外一 张三丰少年游
那一年，张君宝尚在少林寺挑水。

第一章 天涯思君不可忘
春游浩荡，是年年、寒食梨花时节。
"#;
        let novel = parse_txt_str(text, "mock_book_id", Some("倚天屠龙记")).unwrap();
        assert_eq!(novel.sections.len(), 3);
        assert_eq!(novel.sections[0].title, "前言 / 简介");
        assert_eq!(novel.sections[1].title, "番外一 张三丰少年游");
        assert_eq!(novel.sections[2].title, "第一章 天涯思君不可忘");
        assert!(novel.sections[1].body.contains("那一年，张君宝尚在少林寺挑水。"));
    }

    #[test]
    fn test_volume_and_paragraph_tree() {
        let text = r#"
第一卷 幻觉篇
第一章 启程
李火旺睁开双眼，只觉天旋地转。
破庙里的篝火依旧在燃烧。

第二章 迷惘
“迷惘，还是清醒？”
第二卷 真实篇
第三章 觉醒
那是不可名状的恐惧。
"#;
        let novel = parse_txt_str(text, "mock_book_id", Some("道诡异仙")).unwrap();
        for s in &novel.sections {
            println!("SEC: volume='{}' title='{}'", s.volume, s.title);
        }
        assert_eq!(novel.sections.len(), 3);
        assert_eq!(novel.sections[0].volume, "第一卷 幻觉篇");
        assert_eq!(novel.sections[0].title, "第一章 启程");
        // Check paragraph division in chapter 1
        assert_eq!(novel.sections[0].chunks.len(), 2);
        assert_eq!(novel.sections[0].chunks[0].paragraph, 0);
        assert_eq!(novel.sections[0].chunks[1].paragraph, 1);

        assert_eq!(novel.sections[1].volume, "第一卷 幻觉篇");
        assert_eq!(novel.sections[1].title, "第二章 迷惘");

        assert_eq!(novel.sections[2].volume, "第二卷 真实篇");
        assert_eq!(novel.sections[2].title, "第三章 觉醒");
    }

    #[test]
    fn test_gb18030_decoding() {
        // Encode Chinese string in GBK/GB18030
        let (cow, _, _) = encoding_rs::GB18030.encode("第一回 宴桃园豪杰三结义");
        let decoded = decode_bytes(&cow).unwrap();
        assert_eq!(decoded, "第一回 宴桃园豪杰三结义");
    }

    #[test]
    fn reports_progress_for_large_text_with_intact_chapters() {
        let mut text = String::new();
        for index in 1..=2_000 {
            text.push_str(&format!("第{}章 测试\r\n正文第一段。\r\n正文第二段。\r\n", index));
        }
        let mut updates = Vec::new();
        let novel = parse_txt_str_with_progress(&text, "fixture", Some("测试"), |done, total| {
            updates.push((done, total));
        })
        .unwrap();
        assert_eq!(novel.sections.len(), 2_000);
        assert_eq!(novel.sections[1_999].title, "第2000章 测试");
        assert!(novel.sections.iter().all(|section| section.body == "正文第一段。\n正文第二段。"));
        assert_eq!(updates.first(), Some(&(0, text.len())));
        assert_eq!(updates.last(), Some(&(text.len(), text.len())));
        assert!(updates.iter().any(|&(done, _)| done > 0 && done < text.len()));
        assert!(updates.windows(2).all(|pair| pair[0].0 <= pair[1].0));
    }

    #[test]
    fn progress_preserves_empty_and_unterminated_text() {
        for text in ["", "正文。", "正文。\n", "正文。\r\n"] {
            let mut final_count = None;
            let novel = parse_txt_str_with_progress(text, "fixture", None, |done, total| {
                final_count = Some((done, total));
            })
            .unwrap();
            assert_eq!(final_count, Some((text.len(), text.len())));
            if text.is_empty() {
                assert!(novel.sections.is_empty());
            } else {
                assert_eq!(novel.sections.len(), 1);
                assert_eq!(novel.sections[0].body, "正文。");
            }
        }
    }

    #[test]
    fn test_author_and_description_extraction() {
        let text = r#"
《凡人修仙传》
作者：忘语
简介：一个普通的山村穷小子，偶然之下跨入到一个江湖小门派，成了一名记名弟子。
第一章 山村少年
二愣子坐在石头上。
"#;
        let novel = parse_txt_str(text, "mock_id", Some("《凡人修仙传》作者：忘语")).unwrap();
        assert_eq!(novel.book.title, "凡人修仙传");
        assert_eq!(novel.book.author, "忘语");
        assert!(novel.book.description.contains("一个普通的山村穷小子"));
    }
}
