use super::*;
use crate::types::{Block, Line, Word, ZoneKind, ZonedBlock};

fn line(text: &str) -> Line {
    Line {
        words: text
            .split_whitespace()
            .map(|part| Word {
                text: part.to_string(),
                x: 0.0,
                y: 0.0,
                width: 10.0,
                font_size: 10.0,
                is_superscript: false,
            })
            .collect(),
        y: 0.0,
        x_start: 0.0,
        x_end: 100.0,
        font_size: 10.0,
    }
}

fn block(lines: &[&str]) -> Block {
    Block {
        lines: lines.iter().map(|text| line(text)).collect(),
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 12.0 * lines.len() as f32,
        font_size: 10.0,
    }
}

#[test]
fn citation_content_detects_years_journals_and_arxiv() {
    assert!(has_citation_content("Phys. Rev. D 99 (2020)"));
    assert!(has_citation_content("arXiv:2001.12345"));
    assert!(!has_citation_content("plain prose without identifiers"));
}

#[test]
fn marker_counting_scores_blocks() {
    let block = block(&["[1] Phys. Rev. D 99 (2020)", "[2] arXiv:2001.12345"]);

    assert_eq!(count_markers_in_block(&block), 2);
    assert!(has_any_marker(&block));
    assert_eq!(score_citation_block(&block), 4);
}

#[test]
fn count_markers_in_text_handles_multiline_text() {
    let text = "[1] Phys. Rev. D 99\ncontinuation\n(2) Nucl. Phys. B 10";

    assert_eq!(count_markers_in_text(text), 2);
}

#[test]
fn collect_refs_by_markers_splits_numbered_references() {
    let zblock = ZonedBlock {
        block: block(&[
            "[1] A. Author, Phys. Rev. D 99 (2020)",
            "[2] B. Author, Nucl. Phys. B 10 (2021)",
            "[3] C. Author, JHEP 01 (2022) 001",
        ]),
        zone: ZoneKind::Body,
        page_num: 2,
    };

    let refs = collect_refs_by_markers(&[vec![zblock]]);

    assert_eq!(refs.len(), 3);
    assert_eq!(refs[0].linemarker.as_deref(), Some("1"));
    assert_eq!(refs[2].page_num, 2);
}
