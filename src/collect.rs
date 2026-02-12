use once_cell::sync::Lazy;
use regex::Regex;

use crate::types::{RawReference, ReferenceSource, ZoneKind, ZonedBlock};
use crate::zones;

/// Line marker patterns: [1], (1), 1., 1)
static LINE_MARKER_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s*(?:\[(\d+)\]|\((\d+)\)|(\d+)[.\)])\s*").unwrap());

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
    let ref_start = find_reference_heading(zoned_pages);
    let Some((page_idx, block_idx)) = ref_start else {
        return Vec::new();
    };

    let ref_blocks = gather_ref_blocks(zoned_pages, page_idx, block_idx);
    split_into_references(&ref_blocks, ReferenceSource::ReferenceSection)
}

fn find_reference_heading(
    zoned_pages: &[Vec<ZonedBlock>],
) -> Option<(usize, usize)> {
    // Search backwards through pages
    for (page_idx, page_blocks) in zoned_pages.iter().enumerate().rev() {
        for (block_idx, zb) in page_blocks.iter().enumerate() {
            if zones::is_reference_heading(&zb.block) {
                return Some((page_idx, block_idx));
            }
        }
    }
    None
}

fn gather_ref_blocks(
    zoned_pages: &[Vec<ZonedBlock>],
    start_page: usize,
    start_block: usize,
) -> Vec<(String, usize)> {
    let mut ref_blocks = Vec::new();

    // Collect blocks after the heading on the same page
    for zb in &zoned_pages[start_page][start_block + 1..] {
        if zb.zone != ZoneKind::Header && zb.zone != ZoneKind::PageNumber {
            ref_blocks.push((zb.block.text(), zb.page_num));
        }
    }

    // Collect from subsequent pages
    for page_blocks in &zoned_pages[start_page + 1..] {
        for zb in page_blocks {
            if zb.zone == ZoneKind::Header || zb.zone == ZoneKind::PageNumber {
                continue;
            }
            // Stop at a new section heading
            if zones::is_reference_heading(&zb.block) {
                break;
            }
            ref_blocks.push((zb.block.text(), zb.page_num));
        }
    }

    ref_blocks
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
