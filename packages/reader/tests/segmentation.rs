use reader::{parse_txt_str, validate_novel_sequence};

fn titles(text: &str) -> Vec<String> {
    parse_txt_str(text, "fixture", Some("Fixture"))
        .unwrap()
        .sections
        .into_iter()
        .map(|section| section.title)
        .collect()
}

#[test]
fn paragraphs_inside_chapter_225_remain_body_text() {
    let body = "这是一封分成两段的信。\n　　第一段：“我去办事，稍后回来。”\n　　第二段：“事情完成后，我会再写信。”\n信的内容到此结束。";
    let text = format!("第225章 来信\n{body}\n第226章 出发\n第二天，他们一起出发了。\n");
    let novel = parse_txt_str(&text, "fixture", None).unwrap();
    assert_eq!(novel.sections.len(), 2);
    assert_eq!(novel.sections[0].title, "第225章 来信");
    assert_eq!(novel.sections[0].body, body);
    assert_eq!(novel.sections[1].title, "第226章 出发");
    assert!(validate_novel_sequence(&novel.sections).anomalies.is_empty());
}

#[test]
fn chapter_structure_beats_more_frequent_subheadings_and_lists() {
    let body = "正文开头。\n第一节 准备\n第一段\n段落甲。\n第二段\n段落乙。\n1. 第一项\n2. 第二项\n第三段\n段落丙。";
    let text = format!("第225章 准备\n{body}\n第226章 出发\n正文结尾。");
    let novel = parse_txt_str(&text, "fixture", None).unwrap();
    assert_eq!(novel.sections.len(), 2);
    assert_eq!(novel.sections[0].body, body);
}

#[test]
fn weak_titles_before_first_chapter_stay_in_preface() {
    let text = "第一段\n说明甲。\n第二段\n说明乙。\n第1章 起点\n正文甲。\n第2章 终点\n正文乙。";
    assert_eq!(titles(text), ["前言 / 简介", "第1章 起点", "第2章 终点"]);
    assert!(parse_txt_str(text, "fixture", None).unwrap().sections[0].body.starts_with("第一段\n"));
}

#[test]
fn coherent_lower_level_formats_work_without_chapter_headings() {
    for (first, second) in [
        ("第1节 起点", "第2节 终点"),
        ("第一段 起点", "第二段 终点"),
        ("1. 起点", "2. 终点"),
        ("001 起点", "002 终点"),
        ("Section 1 Start", "Section 2 End"),
    ] {
        assert_eq!(titles(&format!("{first}\n正文甲。\n{second}\n正文乙。")), [first, second]);
    }
}

#[test]
fn an_isolated_weak_heading_does_not_create_a_chapter() {
    for candidate in ["第一段", "第一节 准备", "1. 提醒"] {
        let text = format!("正文开头。\n{candidate}\n正文结尾。");
        let novel = parse_txt_str(&text, "fixture", None).unwrap();
        assert_eq!(novel.sections.len(), 1);
        assert_eq!(novel.sections[0].title, "正文");
        assert_eq!(novel.sections[0].body, text);
    }
}

#[test]
fn title_conventions_are_selected_per_volume() {
    let text = "第一卷\n第1章 起点\n第一段\n内容甲。\n第二段\n内容乙。\n第2章 终点\n内容丙。\n第二卷\n第一段 起点\n内容丁。\n第二段 终点\n内容戊。";
    let novel = parse_txt_str(text, "fixture", None).unwrap();
    assert_eq!(novel.sections.len(), 4);
    assert_eq!(novel.sections[0].volume, "第一卷");
    assert_eq!(novel.sections[2].volume, "第二卷");
    assert_eq!(novel.sections[2].title, "第一段 起点");
    assert!(novel.sections[0].body.contains("第二段\n内容乙。"));
}

#[test]
fn genuine_numbering_gaps_duplicates_and_resets_are_not_hidden() {
    let text = "第225章 起点\n内容甲。\n第227章 中途\n内容乙。\n第227章 重复\n内容丙。\n第1章 逆序\n内容丁。";
    assert_eq!(titles(text), ["第225章 起点", "第227章 中途", "第227章 重复", "第1章 逆序"]);
    assert!(!validate_novel_sequence(&parse_txt_str(text, "fixture", None).unwrap().sections)
        .anomalies
        .is_empty());
}

#[test]
fn special_and_mixed_explicit_chapter_headings_remain_supported() {
    let text = "序章\n开场。\n第一章 起点\n内容甲。\n第二回 中途\n内容乙。\nChapter 3 End\n内容丙。\n番外一\n内容丁。\n后记\n结语。";
    assert_eq!(titles(text), ["序章", "第一章 起点", "第二回 中途", "Chapter 3 End", "番外一", "后记"]);
}

#[test]
fn brackets_and_crlf_do_not_bypass_hierarchy() {
    let text = "【第225章 起点】\r\n【第一段】\r\n内容甲。\r\n【第二段】\r\n内容乙。\r\n【第226章 终点】\r\n内容丙。";
    let novel = parse_txt_str(text, "fixture", None).unwrap();
    assert_eq!(novel.sections.len(), 2);
    assert_eq!(novel.sections[0].body, "【第一段】\n内容甲。\n【第二段】\n内容乙。");
}

#[test]
fn explicit_chapter_titles_keep_literary_punctuation() {
    let text = "第225章 欲言又止……\n这是一段完整正文。\n第226章 等等，\n这是下一章的完整正文。";
    assert_eq!(titles(text), ["第225章 欲言又止……", "第226章 等等，"]);
}

#[test]
#[ignore = "requires SAGA_SAMPLE pointing to a user-authorized local TXT; read-only"]
fn cached_book_preserves_chapter_225_and_all_explicit_headings() {
    let path = std::env::var_os("SAGA_SAMPLE").expect("SAGA_SAMPLE required");
    let bytes = std::fs::read(path).unwrap();
    let text = reader::decode_bytes(&bytes).unwrap();
    let started = std::time::Instant::now();
    let novel = reader::parse_txt(&bytes, None).unwrap();
    let elapsed = started.elapsed();
    let index = novel.sections.iter().position(|section| section.title.starts_with("第225章 ")).unwrap();
    assert!(novel.sections[index].body.contains("第一段："));
    assert!(novel.sections[index].body.contains("第二段："));
    assert!(novel.sections[index + 1].title.starts_with("第226章 "));
    assert!(!novel.sections.iter().any(|section| section.title.starts_with("第一段：")));

    // Independent oracle: exact, unindented Arabic-numbered chapter lines in the source.
    let headings: Vec<&str> = text
        .lines()
        .filter(|line| {
            let Some(rest) = line.strip_prefix('第') else { return false };
            let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
            digits > 0 && rest[digits..].starts_with("章 ")
        })
        .collect();
    let actual: Vec<&str> = novel.sections.iter().map(|section| section.title.as_str()).collect();
    let retained: Vec<&str> = actual.iter().copied().filter(|title| headings.contains(title)).collect();
    assert_eq!(retained, headings, "explicit headings must retain exact source order and multiplicity");
    println!(
        "bytes={} sections={} retained_explicit_headings={} parse_ms={}",
        bytes.len(),
        novel.sections.len(),
        retained.len(),
        elapsed.as_millis()
    );
    println!(
        "validation={}",
        serde_json::to_string_pretty(&validate_novel_sequence(&novel.sections)).unwrap()
    );
}
