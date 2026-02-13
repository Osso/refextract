use once_cell::sync::Lazy;
use regex::Regex;

use crate::types::{RawReference, ReferenceSource, ZoneKind, ZonedBlock};
use crate::zones;

/// Line marker patterns: [1], (1), 1., 1) — limited to 1-3 digits to avoid matching years.
/// The bare-number variant (N./N)) requires trailing whitespace/EOL to reject decimals like "0.01".
static LINE_MARKER_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s*(?:\[(\d{1,3})\]|\((\d{1,3})\)|(\d{1,3})[.\)](?:\s|$))\s*").unwrap());

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
    if let Some(loc) = find_reference_heading(zoned_pages) {
        let ref_blocks = gather_ref_blocks(zoned_pages, &loc);
        return split_into_references(&ref_blocks, ReferenceSource::ReferenceSection);
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

fn find_reference_heading(zoned_pages: &[Vec<ZonedBlock>]) -> Option<RefHeadingLoc> {
    // First try: standalone heading block, verified by following reference markers.
    // Search forwards to find the first valid heading (avoids picking up
    // running headers near the end of a multi-page reference section).
    for (page_idx, page_blocks) in zoned_pages.iter().enumerate() {
        for (block_idx, zb) in page_blocks.iter().enumerate() {
            if zones::is_reference_heading(&zb.block)
                && has_refs_after(zoned_pages, page_idx, block_idx)
            {
                return Some(RefHeadingLoc {
                    page_idx,
                    block_idx,
                    line_idx: None,
                });
            }
        }
    }
    // Second try: heading line embedded within a block (also verified)
    for (page_idx, page_blocks) in zoned_pages.iter().enumerate() {
        for (block_idx, zb) in page_blocks.iter().enumerate() {
            for (line_idx, line) in zb.block.lines.iter().enumerate() {
                if zones::is_reference_heading_line(&line.text())
                    && has_refs_after(zoned_pages, page_idx, block_idx)
                {
                    return Some(RefHeadingLoc {
                        page_idx,
                        block_idx,
                        line_idx: Some(line_idx),
                    });
                }
            }
        }
    }
    None
}

/// Verify a heading by checking if blocks after it contain citation-like content.
/// Works for both numbered ([1] Author...) and unnumbered (Author, Year, ...) refs.
/// Prevents TOC entries like "1. Introduction" from being mistaken for refs.
fn has_refs_after(
    zoned_pages: &[Vec<ZonedBlock>],
    page_idx: usize,
    block_idx: usize,
) -> bool {
    let mut checked = 0;
    let mut citation_score = 0;
    // Check remaining blocks on the same page
    for zb in &zoned_pages[page_idx][block_idx + 1..] {
        if zb.zone == ZoneKind::Header || zb.zone == ZoneKind::PageNumber {
            continue;
        }
        citation_score += score_citation_block(&zb.block);
        if citation_score >= 4 {
            return true;
        }
        checked += 1;
        if checked >= 5 {
            break;
        }
    }
    // Check blocks on the next page
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
            if checked >= 5 {
                break;
            }
        }
    }
    false
}

/// Score a block for citation content. Lines with markers + citations score 2,
/// lines with just citation content score 1.
fn score_citation_block(block: &crate::types::Block) -> usize {
    block
        .lines
        .iter()
        .map(|l| {
            let text = l.text();
            if let Some(m) = LINE_MARKER_RE.find(&text) {
                if has_citation_content(&text[m.end()..]) { 2 } else { 0 }
            } else if has_citation_content(&text) {
                1
            } else {
                0
            }
        })
        .sum()
}

/// Check if text contains citation-like content (years, journals, arXiv IDs).
fn has_citation_content(text: &str) -> bool {
    static CITATION_RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"(?:(?:19|20)\d{2}|arXiv|hep-|astro-|gr-qc|cond-mat|nucl-|Phys\.|Nucl\.|Lett\.|Rev\.|JHEP|JCAP|doi:|DOI:)").unwrap()
    });
    CITATION_RE.is_match(text)
}

fn gather_ref_blocks(
    zoned_pages: &[Vec<ZonedBlock>],
    loc: &RefHeadingLoc,
) -> Vec<(String, usize)> {
    let mut ref_blocks = Vec::new();

    // If heading is embedded within a block, collect remaining lines from that block
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

    // Collect remaining blocks on the same page
    for zb in &zoned_pages[loc.page_idx][first_full_block..] {
        if zb.zone != ZoneKind::Header && zb.zone != ZoneKind::PageNumber {
            ref_blocks.push((zb.block.text(), zb.page_num));
        }
    }

    // Collect from subsequent pages
    gather_subsequent_pages(zoned_pages, loc.page_idx, &mut ref_blocks);
    ref_blocks
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
) {
    let mut pages_without_markers = 0;
    for page_blocks in &zoned_pages[start_page + 1..] {
        let mut page_has_markers = false;
        let mut page_blocks_buf = Vec::new();
        for zb in page_blocks {
            if zb.zone == ZoneKind::Header || zb.zone == ZoneKind::PageNumber {
                continue;
            }
            if zones::is_reference_heading(&zb.block) {
                ref_blocks.extend(page_blocks_buf);
                return;
            }
            if has_any_marker(&zb.block) {
                page_has_markers = true;
            }
            page_blocks_buf.push((zb.block.text(), zb.page_num));
        }
        if page_has_markers {
            ref_blocks.extend(page_blocks_buf);
            pages_without_markers = 0;
        } else {
            pages_without_markers += 1;
            if pages_without_markers >= 2 {
                return;
            }
            // Allow one page without markers (continuation lines)
            ref_blocks.extend(page_blocks_buf);
        }
    }
}

/// Fallback: find references by scanning blocks that contain numbered markers.
/// Collects lines from blocks that have at least one `[N]` marker, skipping
/// body text blocks between reference columns.
fn collect_refs_by_markers(
    zoned_pages: &[Vec<ZonedBlock>],
) -> Vec<RawReference> {
    let ref_lines = collect_marker_block_lines(zoned_pages);
    if ref_lines.is_empty() {
        return Vec::new();
    }
    split_into_references(&ref_lines, ReferenceSource::ReferenceSection)
}

/// Collect lines from blocks that contain line markers.
/// Strategy 1: blocks with 3+ markers (dense reference blocks).
/// Strategy 2: individual marker blocks from the tail of the document.
fn collect_marker_block_lines(
    zoned_pages: &[Vec<ZonedBlock>],
) -> Vec<(String, usize)> {
    let dense = collect_dense_marker_blocks(zoned_pages);
    if !dense.is_empty() {
        return dense;
    }
    collect_trailing_marker_blocks(zoned_pages)
}

/// Blocks with 3+ markers AND citation content — dense reference lists (e.g., two-column layout).
/// Requires citation content to distinguish from numbered TOC/list entries.
fn collect_dense_marker_blocks(
    zoned_pages: &[Vec<ZonedBlock>],
) -> Vec<(String, usize)> {
    let mut blocks = Vec::new();
    for page_blocks in zoned_pages {
        for zb in page_blocks {
            if zb.zone == ZoneKind::Header || zb.zone == ZoneKind::PageNumber {
                continue;
            }
            let marker_count = count_markers_in_block(&zb.block);
            if marker_count >= 3 && score_citation_block(&zb.block) >= 4 {
                blocks.push((zb.block.text(), zb.page_num));
            }
        }
    }
    blocks
}

/// Scan backwards from the end of the document for blocks with markers.
/// Collects individual marker blocks that form a reference section.
/// Requires 5+ total markers to avoid false positives from numbered lists.
fn collect_trailing_marker_blocks(
    zoned_pages: &[Vec<ZonedBlock>],
) -> Vec<(String, usize)> {
    let mut blocks = Vec::new();
    let mut pages_without_markers = 0;

    for page_blocks in zoned_pages.iter().rev() {
        let mut page_has_markers = false;
        let mut page_blocks_collected = Vec::new();
        for zb in page_blocks {
            if zb.zone == ZoneKind::Header || zb.zone == ZoneKind::PageNumber {
                continue;
            }
            if has_any_marker(&zb.block) {
                page_has_markers = true;
            }
            page_blocks_collected.push((zb.block.text(), zb.page_num));
        }
        if page_has_markers {
            blocks.extend(page_blocks_collected);
            pages_without_markers = 0;
        } else {
            pages_without_markers += 1;
            if !blocks.is_empty() && pages_without_markers >= 2 {
                break;
            }
        }
    }

    let total_markers: usize = blocks
        .iter()
        .map(|(text, _)| count_markers_in_text(text))
        .sum();
    if total_markers < 5 {
        return Vec::new();
    }

    blocks.reverse();
    blocks
}

fn count_markers_in_block(block: &crate::types::Block) -> usize {
    block
        .lines
        .iter()
        .filter(|l| LINE_MARKER_RE.is_match(&l.text()))
        .count()
}

fn has_any_marker(block: &crate::types::Block) -> bool {
    block
        .lines
        .iter()
        .any(|l| LINE_MARKER_RE.is_match(&l.text()))
}

fn count_markers_in_text(text: &str) -> usize {
    text.lines()
        .filter(|l| LINE_MARKER_RE.is_match(l))
        .count()
}

/// Split concatenated text blocks into individual references by line markers.
fn split_into_references(
    blocks: &[(String, usize)],
    source: ReferenceSource,
) -> Vec<RawReference> {
    let mut refs = Vec::new();
    let mut current_text = String::new();
    let mut current_marker: Option<String> = None;
    let mut current_page = 0;

    for (text, page_num) in blocks {
        for line in text.split('\n') {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(caps) = LINE_MARKER_RE.captures(line) {
                flush_reference(
                    &mut refs,
                    &mut current_text,
                    &current_marker,
                    current_page,
                    source,
                );
                current_marker = extract_marker(&caps);
                current_text =
                    LINE_MARKER_RE.replace(line, "").trim().to_string();
                current_page = *page_num;
            } else if !current_text.is_empty() {
                // Continuation line
                current_text.push(' ');
                current_text.push_str(line);
            } else {
                // First line without marker
                current_text = line.to_string();
                current_page = *page_num;
            }
        }
    }
    flush_reference(&mut refs, &mut current_text, &current_marker, current_page, source);
    refs
}

fn extract_marker(caps: &regex::Captures) -> Option<String> {
    caps.get(1)
        .or_else(|| caps.get(2))
        .or_else(|| caps.get(3))
        .map(|m| m.as_str().to_string())
}

fn flush_reference(
    refs: &mut Vec<RawReference>,
    text: &mut String,
    marker: &Option<String>,
    page_num: usize,
    source: ReferenceSource,
) {
    let trimmed = text.trim().to_string();
    if !trimmed.is_empty() {
        refs.push(RawReference {
            text: trimmed,
            linemarker: marker.clone(),
            source,
            page_num,
        });
    }
    text.clear();
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

/// Heuristic: does this look like a citation (has year, journal, arXiv, DOI)?
fn is_citation_like(r: &RawReference) -> bool {
    let t = &r.text;
    has_year_pattern(t) || t.contains("arXiv") || t.contains("doi") || t.contains("DOI")
}

fn has_year_pattern(text: &str) -> bool {
    static YEAR_RE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"\b(19|20)\d{2}\b").unwrap());
    YEAR_RE.is_match(text)
}

/// Remove footnote refs that duplicate ref-section refs.
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

/// Check if two reference texts are substantially similar.
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
