use once_cell::sync::Lazy;
use regex::Regex;

use crate::markers::{
    collect_refs_by_markers, count_markers_in_block, count_markers_in_text, has_any_marker,
    has_citation_content, score_citation_block, split_into_references,
};
use crate::types::{RawReference, ReferenceSource, ZoneKind, ZonedBlock};
use crate::zones;

/// Collect all references from zoned blocks across all pages.
pub fn collect_references(zoned_pages: &[Vec<ZonedBlock>]) -> Vec<RawReference> {
    let mut refs = collect_reference_section(zoned_pages);
    let footnote_refs = collect_footnote_refs(zoned_pages);
    dedup_and_merge(&mut refs, footnote_refs);
    refs
}

/// Find the reference section and extract individual references.
fn collect_reference_section(
    zoned_pages: &[Vec<ZonedBlock>],
) -> Vec<RawReference> {
    let headings = find_all_reference_headings(zoned_pages);
    if !headings.is_empty() {
        let mut all_blocks = Vec::new();
        for loc in &headings {
            all_blocks.extend(gather_ref_blocks(zoned_pages, loc));
        }
        let heading_refs = split_into_references(&all_blocks, ReferenceSource::ReferenceSection);
        // If heading-based collection yielded very few refs, the heading may be
        // a false positive (e.g., TOC entry). Try the fallback marker scan and
        // use whichever found more references.
        if heading_refs.len() < 5 {
            let fallback = collect_refs_by_markers(zoned_pages);
            if fallback.len() > heading_refs.len() {
                return fallback;
            }
        }
        return heading_refs;
    }
    // Fallback: no heading found. Scan all blocks for numbered reference lines.
    collect_refs_by_markers(zoned_pages)
}

/// Location of a reference heading: page index, block index, and optionally
/// the line index within the block (if the heading is inside a larger block).
struct RefHeadingLoc {
    page_idx: usize,
    block_idx: usize,
    line_idx: Option<usize>,
}

fn find_all_reference_headings(zoned_pages: &[Vec<ZonedBlock>]) -> Vec<RefHeadingLoc> {
    let mut headings = Vec::new();
    // First try: standalone heading blocks, verified by following reference markers.
    for (page_idx, page_blocks) in zoned_pages.iter().enumerate() {
        for (block_idx, zb) in page_blocks.iter().enumerate() {
            if zones::is_reference_heading(&zb.block)
                && has_refs_after(zoned_pages, page_idx, block_idx)
            {
                headings.push(RefHeadingLoc {
                    page_idx,
                    block_idx,
                    line_idx: None,
                });
            }
        }
    }
    if !headings.is_empty() {
        return headings;
    }
    // Second try: heading lines embedded within blocks (also verified)
    for (page_idx, page_blocks) in zoned_pages.iter().enumerate() {
        for (block_idx, zb) in page_blocks.iter().enumerate() {
            for (line_idx, line) in zb.block.lines.iter().enumerate() {
                if zones::is_reference_heading_line(&line.text())
                    && has_refs_after(zoned_pages, page_idx, block_idx)
                {
                    headings.push(RefHeadingLoc {
                        page_idx,
                        block_idx,
                        line_idx: Some(line_idx),
                    });
                }
            }
        }
    }
    headings
}

/// Verify a heading by checking if blocks after it contain citation-like content.
fn has_refs_after(
    zoned_pages: &[Vec<ZonedBlock>],
    page_idx: usize,
    block_idx: usize,
) -> bool {
    let mut checked = 0;
    let mut citation_score = 0;
    for zb in &zoned_pages[page_idx][block_idx + 1..] {
        if zb.zone == ZoneKind::Header || zb.zone == ZoneKind::PageNumber {
            continue;
        }
        citation_score += score_citation_block(&zb.block);
        if citation_score >= 4 {
            return true;
        }
        checked += 1;
        if checked >= 15 {
            break;
        }
    }
    if page_idx + 1 < zoned_pages.len() {
        for zb in &zoned_pages[page_idx + 1] {
            if zb.zone == ZoneKind::Header || zb.zone == ZoneKind::PageNumber {
                continue;
            }
            citation_score += score_citation_block(&zb.block);
            if citation_score >= 4 {
                return true;
            }
            checked += 1;
            if checked >= 15 {
                break;
            }
        }
    }
    false
}

fn gather_ref_blocks(
    zoned_pages: &[Vec<ZonedBlock>],
    loc: &RefHeadingLoc,
) -> Vec<(String, usize)> {
    let mut ref_blocks = Vec::new();

    let first_full_block = if let Some(line_idx) = loc.line_idx {
        let zb = &zoned_pages[loc.page_idx][loc.block_idx];
        let remaining = collect_lines_after(zb, line_idx);
        if !remaining.is_empty() {
            ref_blocks.push((remaining, zb.page_num));
        }
        loc.block_idx + 1
    } else {
        loc.block_idx + 1
    };

    for zb in &zoned_pages[loc.page_idx][first_full_block..] {
        if zb.zone != ZoneKind::Header && zb.zone != ZoneKind::PageNumber {
            ref_blocks.push((zb.block.text(), zb.page_num));
        }
    }

    let has_markers = detect_marker_format(&ref_blocks, zoned_pages, loc.page_idx);
    gather_subsequent_pages(zoned_pages, loc.page_idx, &mut ref_blocks, has_markers);
    ref_blocks
}

/// Determine if the reference section uses numbered markers.
fn detect_marker_format(
    ref_blocks: &[(String, usize)],
    zoned_pages: &[Vec<ZonedBlock>],
    heading_page: usize,
) -> bool {
    if ref_blocks.iter().any(|(text, _)| count_markers_in_text(text) > 0) {
        return true;
    }
    if heading_page + 1 < zoned_pages.len() {
        for zb in &zoned_pages[heading_page + 1] {
            if zb.zone == ZoneKind::Header || zb.zone == ZoneKind::PageNumber {
                continue;
            }
            if count_markers_in_block(&zb.block) > 0 {
                return true;
            }
        }
    }
    false
}

fn collect_lines_after(zb: &ZonedBlock, heading_line_idx: usize) -> String {
    zb.block.lines[heading_line_idx + 1..]
        .iter()
        .map(|l| l.text())
        .collect::<Vec<_>>()
        .join(" ")
}

fn gather_subsequent_pages(
    zoned_pages: &[Vec<ZonedBlock>],
    start_page: usize,
    ref_blocks: &mut Vec<(String, usize)>,
    use_markers: bool,
) {
    let mut pages_without_refs = 0;
    for page_blocks in &zoned_pages[start_page + 1..] {
        let mut page_has_refs = false;
        let mut page_blocks_buf = Vec::new();
        let mut page_citation_lines = 0;
        let mut page_total_lines = 0;
        for zb in page_blocks {
            if zb.zone == ZoneKind::Header || zb.zone == ZoneKind::PageNumber {
                continue;
            }
            if is_standalone_ref_heading(&zb.block) {
                ref_blocks.extend(page_blocks_buf);
                return;
            }
            if use_markers {
                if has_any_marker(&zb.block) {
                    page_has_refs = true;
                }
            } else {
                for line in &zb.block.lines {
                    page_total_lines += 1;
                    if has_citation_content(&line.text()) {
                        page_citation_lines += 1;
                    }
                }
            }
            page_blocks_buf.push((zb.block.text(), zb.page_num));
        }
        if !use_markers && page_citation_lines >= 3
            && page_total_lines > 0
            && page_citation_lines * 2 >= page_total_lines
        {
            page_has_refs = true;
        }
        if page_has_refs {
            ref_blocks.extend(page_blocks_buf);
            pages_without_refs = 0;
        } else {
            pages_without_refs += 1;
            if pages_without_refs >= 2 {
                return;
            }
            ref_blocks.extend(page_blocks_buf);
        }
    }
}

/// A standalone reference heading (short block, not heading + content).
fn is_standalone_ref_heading(block: &crate::types::Block) -> bool {
    zones::is_reference_heading(block) && block.lines.len() <= 2
}

/// Collect references from footnote zones.
fn collect_footnote_refs(
    zoned_pages: &[Vec<ZonedBlock>],
) -> Vec<RawReference> {
    let mut refs = Vec::new();
    for page_blocks in zoned_pages {
        let footnote_blocks: Vec<(String, usize)> = page_blocks
            .iter()
            .filter(|zb| zb.zone == ZoneKind::Footnote)
            .map(|zb| (zb.block.text(), zb.page_num))
            .collect();
        if !footnote_blocks.is_empty() {
            let page_refs =
                split_into_references(&footnote_blocks, ReferenceSource::Footnote);
            refs.extend(page_refs.into_iter().filter(is_citation_like));
        }
    }
    refs
}

fn is_citation_like(r: &RawReference) -> bool {
    let t = &r.text;
    has_year_pattern(t) || t.contains("arXiv") || t.contains("doi") || t.contains("DOI")
}

fn has_year_pattern(text: &str) -> bool {
    static YEAR_RE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"\b(19|20)\d{2}\b").unwrap());
    YEAR_RE.is_match(text)
}

fn dedup_and_merge(
    section_refs: &mut Vec<RawReference>,
    footnote_refs: Vec<RawReference>,
) {
    for fref in footnote_refs {
        let is_dup = section_refs
            .iter()
            .any(|sr| refs_overlap(&sr.text, &fref.text));
        if !is_dup {
            section_refs.push(fref);
        }
    }
}

fn refs_overlap(a: &str, b: &str) -> bool {
    let a_norm = normalize_for_dedup(a);
    let b_norm = normalize_for_dedup(b);
    a_norm == b_norm
}

fn normalize_for_dedup(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}
