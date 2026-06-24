use super::*;
use crate::types::{Block, Line, RawReference, ReferenceSource, Word, ZoneKind, ZonedBlock};

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
        x_end: 120.0,
        font_size: 10.0,
    }
}

fn block(lines: &[&str], zone: ZoneKind, page_num: usize) -> ZonedBlock {
    ZonedBlock {
        block: Block {
            lines: lines.iter().map(|text| line(text)).collect(),
            x: 0.0,
            y: 0.0,
            width: 120.0,
            height: 12.0 * lines.len() as f32,
            font_size: 10.0,
        },
        zone,
        page_num,
    }
}

#[test]
fn collect_references_extracts_reference_section_after_heading() {
    let pages = vec![vec![
        block(&["Introduction"], ZoneKind::Body, 1),
        block(&["References"], ZoneKind::Body, 1),
        block(
            &[
                "[1] A. Author, Phys. Rev. D 99 (2020)",
                "[2] B. Author, Nucl. Phys. B 10 (2021)",
            ],
            ZoneKind::Body,
            1,
        ),
    ]];

    let refs = collect_references(&pages);

    assert_eq!(refs.len(), 2);
    assert_eq!(refs[0].linemarker.as_deref(), Some("1"));
    assert_eq!(refs[0].page_num, 1);
}

#[test]
fn collect_references_falls_back_to_marker_blocks_without_heading() {
    let pages = vec![vec![block(
        &[
            "[1] A. Author, Phys. Rev. D 99 (2020)",
            "[2] B. Author, Nucl. Phys. B 10 (2021)",
            "[3] C. Author, JHEP 01 (2022) 001",
        ],
        ZoneKind::Body,
        2,
    )]];

    let refs = collect_references(&pages);

    assert_eq!(refs.len(), 3);
    assert_eq!(refs[2].page_num, 2);
}

#[test]
fn dedup_and_merge_removes_overlapping_references() {
    let mut section_refs = vec![RawReference {
        text: "A. Author, Phys. Rev. D 99 (2020)".to_string(),
        linemarker: Some("1".to_string()),
        source: ReferenceSource::ReferenceSection,
        page_num: 1,
    }];
    let footnote_refs = vec![RawReference {
        text: "A. Author, Phys. Rev. D 99 (2020)".to_string(),
        linemarker: Some("1".to_string()),
        source: ReferenceSource::Footnote,
        page_num: 1,
    }];

    dedup_and_merge(&mut section_refs, footnote_refs);

    assert_eq!(section_refs.len(), 1);
    assert_eq!(section_refs[0].source, ReferenceSource::ReferenceSection);
}
