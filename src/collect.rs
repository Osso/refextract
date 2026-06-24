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
fn collect_reference_section(zoned_pages: &[Vec<ZonedBlock>]) -> Vec<RawReference> {
    let headings = find_all_reference_headings(zoned_pages);
    if !headings.is_empty() {
        let mut all_blocks = Vec::new();
        for loc in &headings {
            all_blocks.extend(gather_ref_blocks(zoned_pages, loc));
        }
        let heading_refs = split_into_references(&all_blocks, ReferenceSource::ReferenceSection);
        // If heading-based collection yielded few refs, the heading may be
        // a false positive (e.g., TOC entry "References" on page 4 of a 97-page
        // paper). Try the fallback marker scan and use whichever found more.
        if heading_refs.len() < 10 {
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
    let headings = collect_standalone_headings(zoned_pages);
    if !headings.is_empty() {
        return headings;
    }

    collect_embedded_headings(zoned_pages)
}

fn collect_standalone_headings(zoned_pages: &[Vec<ZonedBlock>]) -> Vec<RefHeadingLoc> {
    let mut headings = Vec::new();
    for (page_idx, page_blocks) in zoned_pages.iter().enumerate() {
        for (block_idx, zb) in page_blocks.iter().enumerate() {
            if !zones::is_reference_heading(&zb.block) {
                continue;
            }
            if !has_refs_after(zoned_pages, page_idx, block_idx) {
                continue;
            }
            headings.push(RefHeadingLoc {
                page_idx,
                block_idx,
                line_idx: None,
            });
        }
    }
    headings
}

fn collect_embedded_headings(zoned_pages: &[Vec<ZonedBlock>]) -> Vec<RefHeadingLoc> {
    let mut headings = Vec::new();
    for (page_idx, page_blocks) in zoned_pages.iter().enumerate() {
        for (block_idx, zb) in page_blocks.iter().enumerate() {
            if !has_refs_after(zoned_pages, page_idx, block_idx) {
                continue;
            }
            for (line_idx, line) in zb.block.lines.iter().enumerate() {
                if !zones::is_reference_heading_line(&line.text()) {
                    continue;
                }
                headings.push(RefHeadingLoc {
                    page_idx,
                    block_idx,
                    line_idx: Some(line_idx),
                });
            }
        }
    }
    headings
}

/// Verify a heading by checking if blocks after it contain citation-like content.
fn has_refs_after(zoned_pages: &[Vec<ZonedBlock>], page_idx: usize, block_idx: usize) -> bool {
    let mut citation_score = 0_usize;
    if scan_blocks_for_refs(&zoned_pages[page_idx][block_idx + 1..], &mut citation_score) {
        return true;
    }

    let end = (page_idx + 4).min(zoned_pages.len());
    for next_page in &zoned_pages[page_idx + 1..end] {
        if scan_blocks_for_refs(next_page, &mut citation_score) {
            return true;
        }
    }
    false
}

fn scan_blocks_for_refs(blocks: &[ZonedBlock], citation_score: &mut usize) -> bool {
    let mut checked = 0;
    for zb in blocks {
        if is_header_or_page_number(zb) {
            continue;
        }
        *citation_score += score_citation_block(&zb.block);
        if *citation_score >= 4 {
            return true;
        }
        checked += 1;
        if checked >= 15 {
            break;
        }
    }
    false
}

fn gather_ref_blocks(zoned_pages: &[Vec<ZonedBlock>], loc: &RefHeadingLoc) -> Vec<(String, usize)> {
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
    if ref_blocks
        .iter()
        .any(|(text, _)| count_markers_in_text(text) > 0)
    {
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

struct PageAssessment {
    blocks: Vec<(String, usize)>,
    has_refs: bool,
    saw_heading: bool,
}

fn assess_subsequent_page(page_blocks: &[ZonedBlock], use_markers: bool) -> PageAssessment {
    let mut blocks = Vec::new();
    let mut citation_lines = 0;
    let mut total_lines = 0;
    let mut has_markers = false;
    let mut saw_heading = false;

    for zb in page_blocks {
        if is_header_or_page_number(zb) {
            continue;
        }
        if is_standalone_ref_heading(&zb.block) {
            saw_heading = true;
            continue;
        }
        update_page_ref_signals(
            zb,
            use_markers,
            &mut has_markers,
            &mut citation_lines,
            &mut total_lines,
        );
        blocks.push((zb.block.text(), zb.page_num));
    }

    let has_refs = reference_signals_match(use_markers, has_markers, citation_lines, total_lines);

    PageAssessment {
        blocks,
        has_refs,
        saw_heading,
    }
}

fn is_header_or_page_number(block: &ZonedBlock) -> bool {
    block.zone == ZoneKind::Header || block.zone == ZoneKind::PageNumber
}

fn update_page_ref_signals(
    zb: &ZonedBlock,
    use_markers: bool,
    has_markers: &mut bool,
    citation_lines: &mut i32,
    total_lines: &mut i32,
) {
    if use_markers {
        *has_markers |= has_any_marker(&zb.block);
        return;
    }
    for line in &zb.block.lines {
        *total_lines += 1;
        if has_citation_content(&line.text()) {
            *citation_lines += 1;
        }
    }
}

fn reference_signals_match(
    use_markers: bool,
    has_markers: bool,
    citation_lines: i32,
    total_lines: i32,
) -> bool {
    if use_markers {
        return has_markers;
    }
    citation_lines >= 3 && total_lines > 0 && citation_lines * 2 >= total_lines
}

fn gather_subsequent_pages(
    zoned_pages: &[Vec<ZonedBlock>],
    start_page: usize,
    ref_blocks: &mut Vec<(String, usize)>,
    use_markers: bool,
) {
    let mut pages_without_refs = 0;
    for page_blocks in &zoned_pages[start_page + 1..] {
        let page = assess_subsequent_page(page_blocks, use_markers);

        if page.saw_heading && page.has_refs {
            ref_blocks.extend(page.blocks);
            return;
        }
        if page.has_refs {
            ref_blocks.extend(page.blocks);
            pages_without_refs = 0;
        } else {
            pages_without_refs += 1;
            if pages_without_refs >= 2 {
                return;
            }
            ref_blocks.extend(page.blocks);
        }
    }
}

/// A standalone reference heading (short block, not heading + content).
fn is_standalone_ref_heading(block: &crate::types::Block) -> bool {
    zones::is_reference_heading(block) && block.lines.len() <= 2
}

/// Collect references from footnote zones.
fn collect_footnote_refs(zoned_pages: &[Vec<ZonedBlock>]) -> Vec<RawReference> {
    let mut refs = Vec::new();
    for page_blocks in zoned_pages {
        let footnote_blocks: Vec<(String, usize)> = page_blocks
            .iter()
            .filter(|zb| zb.zone == ZoneKind::Footnote)
            .map(|zb| (zb.block.text(), zb.page_num))
            .collect();
        if !footnote_blocks.is_empty() {
            let page_refs = split_into_references(&footnote_blocks, ReferenceSource::Footnote);
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
    static YEAR_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b(19|20)\d{2}\b").unwrap());
    YEAR_RE.is_match(text)
}

fn dedup_and_merge(section_refs: &mut Vec<RawReference>, footnote_refs: Vec<RawReference>) {
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

#[cfg(test)]
mod tests;
