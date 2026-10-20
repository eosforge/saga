//! Saga reader sequence validation and anomaly detection engine.
//! Validates volume/chapter monotonic ordering, per-volume resets, and false positive body headings.

use protocol::{AnomalyType, NovelValidation, Section, ValidationAnomaly};
use regex::Regex;
use std::sync::LazyLock;

static CHINESE_CHAPTER_NUM_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"第[ \t]*([0-9〇零一二三四五六七八九十百千万a-zA-Z]+)[ \t]*[章节卷集部篇回话讲段]").expect("valid chapter num regex"));

static PREFIX_DIGIT_CHAPTER_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d{1,5})(?:[ \t、:：]|\.[ \t]+)").expect("valid prefix digit regex"));

static ENGLISH_CHAPTER_NUM_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?:Chapter|Section)[ \t]+(\d+)").expect("valid english chapter regex"));

static VOLUME_NUM_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:第[ \t]*([0-9〇零一二三四五六七八九十百千万]+)[ \t]*[卷部集]|卷[ \t]*([0-9〇零一二三四五六七八九十百千万]+)|(?:Volume|Book)[ \t]+(\d+))")
        .expect("valid volume num regex")
});

/// Parses Chinese or Arabic numerals into a standard integer `u32`.
pub fn parse_chinese_numeral(s: &str) -> Option<u32> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return None;
    }

    // 1. Direct ASCII integer parsing (e.g. "124", "001")
    if let Ok(val) = trimmed.parse::<u32>() {
        return Some(val);
    }

    // Overflowing ASCII values must not fall through to the Chinese parser.
    if trimmed.bytes().any(|byte| byte.is_ascii_digit()) {
        return None;
    }

    let positional = !trimmed
        .chars()
        .any(|character| matches!(character, '十' | '拾' | '百' | '佰' | '千' | '仟' | '万'));

    // 3. Traditional / Simplified Chinese numeral accumulation
    let mut total: u64 = 0;
    let mut current_unit: u64 = 0;
    let mut has_digit = false;

    for c in trimmed.chars() {
        let val = match c {
            '零' | '〇' => 0,
            '一' | '壹' => 1,
            '二' | '贰' | '两' => 2,
            '三' | '叁' => 3,
            '四' | '肆' => 4,
            '五' | '伍' => 5,
            '六' | '陆' => 6,
            '七' | '柒' => 7,
            '八' | '捌' => 8,
            '九' | '玖' => 9,
            '十' | '拾' => {
                let unit = if current_unit == 0 { 1 } else { current_unit };
                total = total.checked_add(unit.checked_mul(10)?)?;
                current_unit = 0;
                has_digit = true;
                continue;
            }
            '百' | '佰' => {
                let unit = if current_unit == 0 { 1 } else { current_unit };
                total = total.checked_add(unit.checked_mul(100)?)?;
                current_unit = 0;
                has_digit = true;
                continue;
            }
            '千' | '仟' => {
                let unit = if current_unit == 0 { 1 } else { current_unit };
                total = total.checked_add(unit.checked_mul(1000)?)?;
                current_unit = 0;
                has_digit = true;
                continue;
            }
            '万' => {
                let unit = if current_unit == 0 && total == 0 { 1 } else { current_unit };
                total = total.checked_add(unit)?.checked_mul(10000)?;
                current_unit = 0;
                has_digit = true;
                continue;
            }
            _ => return None,
        };
        current_unit = if positional { current_unit.checked_mul(10)?.checked_add(val)? } else { val };
        has_digit = true;
    }

    total = total.checked_add(current_unit)?;
    if has_digit {
        u32::try_from(total).ok()
    } else {
        None
    }
}

/// Extracts chapter number from a chapter heading string if numbered.
pub fn extract_chapter_number(title: &str) -> Option<u32> {
    if let Some(caps) = CHINESE_CHAPTER_NUM_REGEX.captures(title) {
        if let Some(m) = caps.get(1) {
            return parse_chinese_numeral(m.as_str());
        }
    }
    if let Some(caps) = PREFIX_DIGIT_CHAPTER_REGEX.captures(title) {
        if let Some(m) = caps.get(1) {
            return m.as_str().parse::<u32>().ok();
        }
    }
    if let Some(caps) = ENGLISH_CHAPTER_NUM_REGEX.captures(title) {
        if let Some(m) = caps.get(1) {
            return m.as_str().parse::<u32>().ok();
        }
    }
    None
}

/// Extracts volume number from a volume heading string if numbered.
pub fn extract_volume_number(volume: &str) -> Option<u32> {
    if let Some(caps) = VOLUME_NUM_REGEX.captures(volume) {
        for i in 1..=3 {
            if let Some(m) = caps.get(i) {
                return parse_chinese_numeral(m.as_str());
            }
        }
    }
    None
}

/// Checks whether a section is a recognized non-numbered structural component (e.g. 序章, 楔子, 番外).
pub fn is_special_unnumberable(title: &str) -> bool {
    let t = title.trim();
    t.contains("序章")
        || t.contains("楔子")
        || t.contains("引子")
        || t.contains("前言")
        || t.contains("后记")
        || t.contains("尾声")
        || t.contains("番外")
        || t.contains("感言")
        || t.contains("简介")
        || t.contains("大结局")
}

/// Validates volume and chapter sequences, detecting ordering anomalies and false positives.
pub fn validate_novel_sequence(sections: &[Section]) -> NovelValidation {
    let mut warnings = Vec::new();
    let mut anomalies = Vec::new();

    if sections.is_empty() {
        return NovelValidation {
            is_valid: true,
            total_volumes: 0,
            total_chapters: 0,
            volume_style: "空书籍".to_string(),
            chapter_style: "无章节".to_string(),
            warnings,
            anomalies,
        };
    }

    // 1. Group sections by volume and check volume ordering
    let mut volume_groups: Vec<(String, Vec<&Section>)> = Vec::new();
    let mut seen_volumes = std::collections::HashSet::new();
    let mut last_vol_name = String::new();

    for sec in sections {
        let vol_name = if sec.volume.trim().is_empty() {
            "正文".to_string()
        } else {
            sec.volume.trim().to_string()
        };

        if vol_name != last_vol_name {
            if seen_volumes.contains(&vol_name) {
                anomalies.push(ValidationAnomaly {
                    key: String::new(),
                    retained: false,
                    anomaly_type: AnomalyType::VolumeDisorder,
                    section_index: sec.index,
                    title: sec.title.clone(),
                    volume: vol_name.clone(),
                    description: format!("卷名《{}》再次出现但不连续，疑似正文中出现卷名关键词导致误切", vol_name),
                });
            }
            seen_volumes.insert(vol_name.clone());
            volume_groups.push((vol_name.clone(), Vec::new()));
            last_vol_name = vol_name;
        }

        volume_groups.last_mut().unwrap().1.push(sec);
    }

    let total_volumes = volume_groups.len() as u32;
    let total_chapters = sections.len() as u32;

    // Check volume number progression
    let mut last_vol_num: Option<u32> = None;
    let mut volume_has_numbers = false;

    for (vol_name, vol_sections) in &volume_groups {
        if let Some(v_num) = extract_volume_number(vol_name) {
            volume_has_numbers = true;
            if let Some(prev) = last_vol_num {
                if v_num < prev {
                    anomalies.push(ValidationAnomaly {
                        key: String::new(),
                        retained: false,
                        anomaly_type: AnomalyType::VolumeDisorder,
                        section_index: vol_sections[0].index,
                        title: vol_name.clone(),
                        volume: vol_name.clone(),
                        description: format!("卷序号逆序：由 第{}卷 倒退回 第{}卷", prev, v_num),
                    });
                } else if v_num.saturating_sub(prev) > 1 {
                    let diff = v_num - prev - 1;
                    warnings.push(format!("卷序号跳跃：由 第{}卷 跳至 第{}卷 (可能缺失 {} 卷)", prev, v_num, diff));
                }
            }
            last_vol_num = Some(v_num);
        }
    }

    let volume_style = if total_volumes <= 1 {
        "单卷正文".to_string()
    } else if volume_has_numbers {
        "多卷序号连续".to_string()
    } else {
        "多卷命名分组".to_string()
    };

    // 2. Detect chapter numbering style (Continuous vs. Per-Volume Reset)
    let mut is_per_volume_reset = false;
    if volume_groups.len() > 1 {
        // Inspect second volume's first numbered chapter
        let first_vol_last_num = volume_groups[0].1.iter().filter_map(|s| extract_chapter_number(&s.title)).next_back();
        let second_vol_first_num = volume_groups[1].1.iter().filter_map(|s| extract_chapter_number(&s.title)).next();

        if let (Some(last1), Some(first2)) = (first_vol_last_num, second_vol_first_num) {
            if first2 <= 3 && first2 < last1 {
                is_per_volume_reset = true;
            }
        }
    }

    let chapter_style = if is_per_volume_reset {
        "每卷重置从第1章开始".to_string()
    } else {
        "全书全局连续递增".to_string()
    };

    // 3. Inspect chapter sequences
    let mut global_last_num: Option<u32> = None;
    let mut consecutive_short_body_count = 0u32;

    for (vol_name, vol_sections) in &volume_groups {
        let mut vol_last_num: Option<u32> = None;
        let mut seen_titles_in_vol = std::collections::HashSet::new();

        for sec in vol_sections {
            let body_len = sec.body.chars().count();

            // Check for suspicious short body (< 10 chars)
            if body_len < 10 && !is_special_unnumberable(&sec.title) {
                consecutive_short_body_count += 1;
                anomalies.push(ValidationAnomaly {
                    key: String::new(),
                    retained: false,
                    anomaly_type: AnomalyType::SuspiciousShortSection,
                    section_index: sec.index,
                    title: sec.title.clone(),
                    volume: vol_name.clone(),
                    description: format!("章节《{}》正文仅 {} 个字符，疑似目录条目或正文对话被误判为章节标题", sec.title, body_len),
                });
            } else {
                consecutive_short_body_count = 0;
            }

            // Check duplicate titles in same scope
            if seen_titles_in_vol.contains(&sec.title) && !is_special_unnumberable(&sec.title) {
                anomalies.push(ValidationAnomaly {
                    key: String::new(),
                    retained: false,
                    anomaly_type: AnomalyType::DuplicateHeading,
                    section_index: sec.index,
                    title: sec.title.clone(),
                    volume: vol_name.clone(),
                    description: format!("在卷《{}》内发现完全同名的重复章节标题", vol_name),
                });
            }
            seen_titles_in_vol.insert(sec.title.clone());

            // Sequence comparison
            if let Some(ch_num) = extract_chapter_number(&sec.title) {
                let reference_last = if is_per_volume_reset { vol_last_num } else { global_last_num };

                if let Some(prev) = reference_last {
                    if ch_num == prev {
                        anomalies.push(ValidationAnomaly {
                            key: String::new(),
                            retained: false,
                            anomaly_type: AnomalyType::DuplicateHeading,
                            section_index: sec.index,
                            title: sec.title.clone(),
                            volume: vol_name.clone(),
                            description: format!("发现连续重复章节序号：第{}章", ch_num),
                        });
                    } else if ch_num < prev {
                        anomalies.push(ValidationAnomaly {
                            key: String::new(),
                            retained: false,
                            anomaly_type: AnomalyType::SequenceRegression,
                            section_index: sec.index,
                            title: sec.title.clone(),
                            volume: vol_name.clone(),
                            description: format!("章节序号逆序：由 第{}章 倒退回 第{}章，疑似正文文本被误切", prev, ch_num),
                        });
                    } else if ch_num > prev + 1 {
                        let gap = ch_num - prev - 1;
                        if gap <= 3 {
                            warnings.push(format!(
                                "卷《{}》章节序号跳跃：由 第{}章 跳至 第{}章 (可能缺失 {} 章)",
                                vol_name, prev, ch_num, gap
                            ));
                        } else {
                            anomalies.push(ValidationAnomaly {
                                key: String::new(),
                                retained: false,
                                anomaly_type: AnomalyType::SequenceGap,
                                section_index: sec.index,
                                title: sec.title.clone(),
                                volume: vol_name.clone(),
                                description: format!(
                                    "章节序号大幅突增跳跃：由 第{}章 突跃至 第{}章 (跳跃 {} 章)，疑似序号混乱或误切",
                                    prev, ch_num, gap
                                ),
                            });
                        }
                    }
                }

                vol_last_num = Some(ch_num);
                global_last_num = Some(ch_num);
            }
        }
    }

    if consecutive_short_body_count >= 3 {
        warnings.push("检测到开头存在连续超短章节，极可能是未被剔除的目录列表 (TOC)".to_string());
    }

    let is_valid = anomalies.is_empty();

    NovelValidation {
        is_valid,
        total_volumes,
        total_chapters,
        volume_style,
        chapter_style,
        warnings,
        anomalies,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chinese_numeral_parsing() {
        assert_eq!(parse_chinese_numeral("一"), Some(1));
        assert_eq!(parse_chinese_numeral("十"), Some(10));
        assert_eq!(parse_chinese_numeral("十二"), Some(12));
        assert_eq!(parse_chinese_numeral("二十五"), Some(25));
        assert_eq!(parse_chinese_numeral("一百零五"), Some(105));
        assert_eq!(parse_chinese_numeral("一千零二十四"), Some(1024));
        assert_eq!(parse_chinese_numeral("124"), Some(124));
        assert_eq!(parse_chinese_numeral("007"), Some(7));
        assert_eq!(parse_chinese_numeral("二〇二六"), Some(2026));
        assert_eq!(parse_chinese_numeral("一x二"), None);
        assert_eq!(parse_chinese_numeral("4294967296"), None);
        assert_eq!(parse_chinese_numeral("一万万万万万万"), None);
    }

    #[test]
    fn test_extract_chapter_number() {
        assert_eq!(extract_chapter_number("第一章 启程"), Some(1));
        assert_eq!(extract_chapter_number("第124章 坐忘道"), Some(124));
        assert_eq!(extract_chapter_number("第二十一回 煮酒论英雄"), Some(21));
        assert_eq!(extract_chapter_number("003 天才陨落"), Some(3));
        assert_eq!(extract_chapter_number("Chapter 42 The Answer"), Some(42));
        assert_eq!(extract_chapter_number("序章"), None);
    }

    #[test]
    fn test_sequence_gap_and_regression_detection() {
        let sec1 = Section {
            id: "1".into(),
            index: 0,
            title: "第一章 启程".into(),
            volume: "第一卷".into(),
            body: "正文内容十分丰富，没有任何短段落问题。".into(),
            chunks: vec![],
        };
        let sec2 = Section {
            id: "2".into(),
            index: 1,
            title: "第二章 历险".into(),
            volume: "第一卷".into(),
            body: "正文内容十分丰富，没有任何短段落问题。".into(),
            chunks: vec![],
        };
        let sec_jump = Section {
            id: "3".into(),
            index: 2,
            title: "第十章 决战".into(),
            volume: "第一卷".into(),
            body: "正文内容十分丰富，没有任何短段落问题。".into(),
            chunks: vec![],
        };

        let report = validate_novel_sequence(&[sec1, sec2, sec_jump]);
        assert!(!report.is_valid);
        assert_eq!(report.anomalies.len(), 1);
        assert_eq!(report.anomalies[0].anomaly_type, AnomalyType::SequenceGap);
    }

    #[test]
    fn volume_regression_points_to_the_actual_first_section_of_that_volume() {
        let novel = crate::parse_txt_str(
            "第二卷\n第1章 开始\n这是一段完整的正文内容。\n第2章 继续\n这是另一段完整的正文内容。\n第一卷\n第3章 转折\n这里是卷序逆转之后的正文内容。",
            "fixture",
            None,
        )
        .unwrap();
        let report = validate_novel_sequence(&novel.sections);
        let anomaly = report.anomalies.iter().find(|item| item.anomaly_type == AnomalyType::VolumeDisorder).unwrap();
        assert_eq!(anomaly.section_index, 2);
        assert_eq!(novel.sections[anomaly.section_index as usize].title, "第3章 转折");
    }

    #[test]
    fn test_per_volume_reset_convention() {
        let v1_c1 = Section {
            id: "1".into(),
            index: 0,
            title: "第一章 始动".into(),
            volume: "第一卷".into(),
            body: "正文内容十分丰富，正常阅读篇幅。".into(),
            chunks: vec![],
        };
        let v1_c2 = Section {
            id: "2".into(),
            index: 1,
            title: "第二章 归途".into(),
            volume: "第一卷".into(),
            body: "正文内容十分丰富，正常阅读篇幅。".into(),
            chunks: vec![],
        };
        // Volume 2 resets from Chapter 1
        let v2_c1 = Section {
            id: "3".into(),
            index: 2,
            title: "第一章 风起".into(),
            volume: "第二卷".into(),
            body: "正文内容十分丰富，正常阅读篇幅。".into(),
            chunks: vec![],
        };
        let v2_c2 = Section {
            id: "4".into(),
            index: 3,
            title: "第二章 云涌".into(),
            volume: "第二卷".into(),
            body: "正文内容十分丰富，正常阅读篇幅。".into(),
            chunks: vec![],
        };

        let report = validate_novel_sequence(&[v1_c1, v1_c2, v2_c1, v2_c2]);
        assert_eq!(report.chapter_style, "每卷重置从第1章开始");
        assert!(report.is_valid);
    }
}
