//! Heading candidates and document-level structural selection.

use std::sync::LazyLock;

use regex::Regex;

use crate::validator::{extract_chapter_number, parse_chinese_numeral};

/// Selected source position; rejected candidates remain ordinary body text.
pub(super) struct Boundary {
    pub offset: usize,
    pub volume: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Category {
    Chapter,
    Section,
    Paragraph,
    Number,
    Special,
}

struct Candidate {
    offset: usize,
    category: Category,
    number: Option<u32>,
}

/// Discovers candidates once, then chooses a heading tier independently in each volume.
pub(super) fn scan(text: &str, mut progress: impl FnMut(usize)) -> Vec<Boundary> {
    let mut boundaries = Vec::new();
    let mut candidates = Vec::new();
    let mut offset = 0;
    for (index, segment) in text.split_inclusive('\n').enumerate() {
        let line = segment.trim();
        if is_volume_heading(line) {
            select(&candidates, &mut boundaries);
            candidates.clear();
            boundaries.push(Boundary { offset, volume: true });
        } else if is_chapter_heading(line) {
            let (category, number) = classify(line);
            candidates.push(Candidate { offset, category, number });
        }
        offset += segment.len();
        if index % 256 == 255 {
            progress(offset);
        }
    }
    select(&candidates, &mut boundaries);
    progress(text.len());
    boundaries
}

fn classify(line: &str) -> (Category, Option<u32>) {
    static NUMBERED: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^第[ \t]*([0-9〇零一二三四五六七八九十百千万a-zA-Z]{1,8})([章节回话讲段篇])").expect("valid numbered heading"));
    let title = line.trim_start_matches(['【', '[', '（', '(']).trim_start();
    if let Some(captures) = NUMBERED.captures(title) {
        let category = match &captures[2] {
            "节" => Category::Section,
            "段" => Category::Paragraph,
            _ => Category::Chapter,
        };
        return (category, parse_chinese_numeral(&captures[1]));
    }
    if title.starts_with("Chapter") {
        return (Category::Chapter, extract_chapter_number(title));
    }
    if title.starts_with("Section") {
        return (Category::Section, extract_chapter_number(title));
    }
    let digits = title.bytes().take_while(u8::is_ascii_digit).count();
    if digits > 0 {
        return (Category::Number, title[..digits].parse().ok());
    }
    (Category::Special, None)
}

fn select(candidates: &[Candidate], boundaries: &mut Vec<Boundary>) {
    // Explicit chapters outrank subsections even when a list contains more entries.
    // Do not discard real chapter gaps, duplicates or resets: validation owns those anomalies.
    let selected = if candidates.iter().any(|candidate| candidate.category == Category::Chapter) {
        Some(Category::Chapter)
    } else {
        [Category::Section, Category::Paragraph, Category::Number]
            .into_iter()
            .find(|&category| coherent(candidates, category))
    };
    for candidate in candidates {
        if candidate.category == Category::Special || Some(candidate.category) == selected {
            boundaries.push(Boundary {
                offset: candidate.offset,
                volume: false,
            });
        }
    }
}

fn coherent(candidates: &[Candidate], category: Category) -> bool {
    let mut previous = None;
    for number in candidates
        .iter()
        .filter(|candidate| candidate.category == category)
        .filter_map(|candidate| candidate.number)
    {
        // Missing chapters may still form a real sequence; retain gaps for the validator.
        if previous.is_some_and(|previous| number > previous) {
            return true;
        }
        previous = Some(number);
    }
    false
}

/// Compiled chapter heading regular expression supporting numbers, bracketed formats,
/// structural preambles, and all 番外 (side-story / extra) variations.
static CHAPTER_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^[ \t]{0,4}(?:(?:【|\[|（|\()(?:\s*(?:番外篇?|外传|序章|序言|序幕|序曲|序|楔子|引子|引言|前言|简介|内容简介|背景|设定|题记|写在前面|写在前面的话|后记|尾声|终章|结语|完本感言|附录|第[ \t]{0,4}[\d〇零一二三四五六七八九十百千万a-zA-Z]{1,8}[章节回话讲段篇])[^】\]）\)]*)[】\]）\)](?:.*)?$|(?:(?:番外篇?|外传|序章|序言|序幕|序曲|序|楔子|引子|引言|前言|简介|内容简介|背景|设定|题记|写在前面|写在前面的话|后记|尾声|终章|结语|完本感言|附录)(?:篇)?(?:[ \t:：·、\-–—\.之]?[\d〇零一二三四五六七八九十百千万]{1,6})?(?:[ \t:：·、\-–—\.].*)?$)|(?:第[ \t]{0,4}[\d〇零一二三四五六七八九十百千万a-zA-Z]{1,8}[章节回话讲段篇](?:[ \t:：·、\-–—\.].*)?$)|(?:\d{1,4}(?:[ \t、:：]|\.[ \t]+)[ \t]*[^\n]{1,35}$)|(?:(?:Chapter|Section|Prologue|Epilogue)\s+\d+(?:[ \t:：\-–—\.].*)?$))"
    ).expect("valid chapter regex compilation")
});

/// Compiled volume heading regular expression for hierarchical novel tree structures.
static VOLUME_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^[ \t]{0,4}(?:【?(?:第[ \t]{0,4}[\d〇零一二三四五六七八九十百千万a-zA-Z]{1,8}[卷部集]|卷[ \t]{0,4}[\d〇零一二三四五六七八九十百千万]{1,8}|番外卷|外传卷)】?|(?:Volume|Book)\s+\d+)(?:[ \t:：·、\-–—\.].*)?$"
    ).expect("valid volume regex compilation")
});

#[inline(always)]
fn may_be_volume_prefix(c: char) -> bool {
    matches!(c, '【' | '[' | '第' | '卷' | '番' | '外' | 'V' | 'v' | 'B' | 'b')
}

#[inline(always)]
fn may_be_chapter_prefix(c: char) -> bool {
    matches!(
        c,
        '【' | '[' | '（' | '(' | '第' | '0'
            ..='9'
                | '〇'
                | '零'
                | '一'
                | '二'
                | '三'
                | '四'
                | '五'
                | '六'
                | '七'
                | '八'
                | '九'
                | '十'
                | '百'
                | '千'
                | '万'
                | '番'
                | '外'
                | '序'
                | '楔'
                | '引'
                | '前'
                | '简'
                | '背'
                | '设'
                | '题'
                | '写'
                | '后'
                | '尾'
                | '终'
                | '结'
                | '完'
                | '附'
                | 'C'
                | 'c'
                | 'S'
                | 's'
                | 'P'
                | 'p'
                | 'E'
                | 'e'
    )
}

/// Checks whether a single trimmed line qualifies as a volume heading.
pub fn is_volume_heading(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.len() > 160 {
        return false;
    }
    let Some(first) = trimmed.chars().next() else {
        return false;
    };
    if !may_be_volume_prefix(first) {
        return false;
    }
    if trimmed.chars().count() > 40 {
        return false;
    }
    // Exclude dialogue or quoted lines
    if (trimmed.starts_with('“') && trimmed.ends_with('”'))
        || (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('‘') && trimmed.ends_with('’'))
    {
        return false;
    }
    VOLUME_REGEX.is_match(trimmed)
}

/// Recognizes a lexical candidate; scan selects actual boundaries using the surrounding document.
pub fn is_chapter_heading(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.len() > 160 {
        return false;
    }
    let Some(first) = trimmed.chars().next() else {
        return false;
    };
    if !may_be_chapter_prefix(first) {
        return false;
    }
    // Chapter titles rarely exceed 40 characters; reject long paragraphs matching keywords
    let char_count = trimmed.chars().count();
    if !(1..=40).contains(&char_count) {
        return false;
    }

    // Exclude common novel metadata prefix lines
    if trimmed.starts_with("书名：")
        || trimmed.starts_with("书名:")
        || trimmed.starts_with("作者：")
        || trimmed.starts_with("作者:")
        || trimmed.starts_with("简介：")
        || trimmed.starts_with("简介:")
        || trimmed.starts_with("内容简介：")
        || trimmed.starts_with("内容简介:")
        || trimmed.starts_with("类型：")
        || trimmed.starts_with("字数：")
        || trimmed.starts_with("状态：")
        || trimmed.starts_with("首发：")
        || trimmed.starts_with("更新：")
    {
        return false;
    }

    // Exclude special non-chapter markers like "正文完" or "正文结"
    if trimmed.starts_with("正文完") || trimmed.starts_with("正文结") {
        return false;
    }

    // Exclude dialogue or quoted lines to prevent dialogue containing "第几章" from breaking chapters
    if (trimmed.starts_with('“') && trimmed.ends_with('”'))
        || (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('‘') && trimmed.ends_with('’'))
    {
        return false;
    }

    // Exclude table of contents title line itself
    if trimmed == "目录" || trimmed == "章节目录" || trimmed == "正文目录" {
        return false;
    }

    // A numbered chapter can legitimately end in an ellipsis or a dialogue comma.
    // Narrative punctuation filters below apply only to weaker candidate forms.
    if classify(trimmed).0 == Category::Chapter && CHAPTER_REGEX.is_match(trimmed) {
        return true;
    }

    // Exclude narrative ellipsis and trailing pause markers (e.g. "3.5小时后……")
    if trimmed.ends_with('…') || trimmed.ends_with("...") || trimmed.ends_with("···") {
        return false;
    }

    // Exclude lines ending with sentence-continuing punctuation
    if trimmed.ends_with('，') || trimmed.ends_with(',') || trimmed.ends_with('；') || trimmed.ends_with(';') {
        return false;
    }

    // Exclude leading decimal numbers like "3.5小时", "1.2米"
    if let Some(dot_pos) = trimmed.find('.') {
        let before = &trimmed[..dot_pos];
        let after = &trimmed[dot_pos + 1..];
        if !before.is_empty() && before.chars().all(|c| c.is_ascii_digit()) && after.starts_with(|c: char| c.is_ascii_digit()) {
            return false;
        }
    }

    // Exclude common narrative time passages, durations and measurement sentences (e.g. "3.5小时后", "10分钟后", "2天后")
    static TIME_TRANSITION_REGEX: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^\d+(?:\.\d+)?\s*(?:秒|分|分钟|小时|个?时辰|天|日|月|年|周|刻钟|更|岁|米|厘米|毫米|公里|千米|斤|公斤|克|吨|度|折|倍)(?:之?后|之?前|后|前|内|整|半|许)?(?:！|。|，)?$").expect("valid time regex")
    });
    if TIME_TRANSITION_REGEX.is_match(trimmed) {
        return false;
    }

    CHAPTER_REGEX.is_match(trimmed)
}
