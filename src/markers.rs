use once_cell::sync::Lazy;
use regex::Regex;

use crate::types::{RawReference, ReferenceSource, ZoneKind, ZonedBlock};

/// Line marker patterns: [1], (1), 1., 1), [Author+Year] at the start of a line.
/// Bracketed/paren forms allow up to 4 digits (review papers with 2000+ refs).
/// Bare-number variants (N./N)) limited to 1-3 digits to avoid matching years like "2024.".
/// Bare variants also require trailing whitespace/EOL to reject decimals like "0.01".
/// Author-year markers: [Aal+12], [ABG14], [Kim+15a], [ATL14a], [CMS15c].
pub(crate) static LINE_MARKER_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"^\s*(?:\[(\d{1,4})\]|\((\d{1,4})\)|(\d{1,3})[.\)](?:\s|$)|\[([A-Z][\p{L}+]{0,7}\d{2}[a-z]?)\])\s*",
    )
    .unwrap()
});

/// Check if text contains citation-like content (years, journals, arXiv IDs).
pub(crate) fn has_citation_content(text: &str) -> bool {
    static CITATION_RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"(?:(?:19|20)\d{2}|arXiv|hep-|astro-|gr-qc|cond-mat|nucl-|Phys\.|Nucl\.|Lett\.|Rev\.|JHEP|JCAP|doi:|DOI:)").unwrap()
    });
    CITATION_RE.is_match(text)
}

/// Score a block for citation content. Lines with markers + citations score 2,
/// lines with just citation content score 1.
pub(crate) fn score_citation_block(block: &crate::types::Block) -> usize {
    block
        .lines
        .iter()
        .map(|l| {
            let text = l.text();
            if let Some(m) = LINE_MARKER_RE.find(&text) {
                if has_citation_content(&text[m.end()..]) {
                    2
                } else {
                    0
                }
            } else if has_citation_content(&text) {
                1
            } else {
                0
            }
        })
        .sum()
}

pub(crate) fn count_markers_in_block(block: &crate::types::Block) -> usize {
    block
        .lines
        .iter()
        .filter(|l| LINE_MARKER_RE.is_match(&l.text()))
        .count()
}

pub(crate) fn has_any_marker(block: &crate::types::Block) -> bool {
    block
        .lines
        .iter()
        .any(|l| LINE_MARKER_RE.is_match(&l.text()))
}

pub(crate) fn count_markers_in_text(text: &str) -> usize {
    text.lines().filter(|l| LINE_MARKER_RE.is_match(l)).count()
}

/// Fallback: find references by scanning blocks that contain numbered markers.
pub(crate) fn collect_refs_by_markers(zoned_pages: &[Vec<ZonedBlock>]) -> Vec<RawReference> {
    let ref_lines = collect_marker_block_lines(zoned_pages);
    if ref_lines.is_empty() {
        return Vec::new();
    }
    split_into_references(&ref_lines, ReferenceSource::ReferenceSection)
}

/// Collect lines from blocks that contain line markers.
/// Strategy 1: blocks with 3+ markers (dense reference blocks).
/// Strategy 2: individual marker blocks from the tail of the document.
/// Strategy 3: superscript bare-number markers on their own lines/blocks.
fn collect_marker_block_lines(zoned_pages: &[Vec<ZonedBlock>]) -> Vec<(String, usize)> {
    let dense = collect_dense_marker_blocks(zoned_pages);
    let dense_markers: usize = dense
        .iter()
        .map(|(text, _)| count_markers_in_text(text))
        .sum();

    // If the dense strategy found a substantial number of markers, use it
    // directly. Otherwise, also try the trailing scan and compare — a small
    // dense result often means a single multi-line block was found while
    // the real reference section spans many single-line blocks.
    if dense_markers >= 10 {
        return dense;
    }

    let trailing = collect_trailing_marker_blocks(zoned_pages);
    let trailing_markers: usize = trailing
        .iter()
        .map(|(text, _)| count_markers_in_text(text))
        .sum();

    if trailing_markers > dense_markers {
        return trailing;
    }
    if !dense.is_empty() {
        return dense;
    }
    if !trailing.is_empty() {
        return trailing;
    }
    collect_superscript_marker_refs(zoned_pages)
}

/// Blocks with 3+ markers AND citation content — dense reference lists.
fn collect_dense_marker_blocks(zoned_pages: &[Vec<ZonedBlock>]) -> Vec<(String, usize)> {
    let mut blocks = Vec::new();
    for page_blocks in zoned_pages {
        for zb in page_blocks {
            if zb.zone == ZoneKind::Header || zb.zone == ZoneKind::PageNumber {
                continue;
            }
            if is_dense_ref_block(&zb.block) {
                blocks.push((zb.block.text(), zb.page_num));
            }
        }
    }
    blocks
}

/// A block qualifies as a dense reference block if it has enough markers
/// and citation content. For author-date papers, lines rarely start with
/// `(year)` so we also accept blocks with very high citation density.
fn is_dense_ref_block(block: &crate::types::Block) -> bool {
    let marker_count = count_markers_in_block(block);
    let cite_score = score_citation_block(block);
    // Standard: 3+ markers with citation content
    if marker_count >= 3 && cite_score >= 4 {
        return true;
    }
    // High citation density: accept blocks where >= 60% of lines
    // are citations and at least 20 citation lines present.
    // This captures author-date reference blocks that lack line markers.
    let total_lines = block.lines.len();
    let cite_lines = block
        .lines
        .iter()
        .filter(|l| has_citation_content(&l.text()))
        .count();
    cite_lines >= 20 && total_lines > 0 && cite_lines * 5 >= total_lines * 3
    // 60% threshold: cite_lines / total >= 3/5
}

struct TrailingPageResult {
    blocks: Vec<(String, usize)>,
    has_refs: bool,
}

fn assess_trailing_page(page_blocks: &[ZonedBlock], found_so_far: bool) -> TrailingPageResult {
    let mut collected = Vec::new();
    let mut has_markers = false;
    let mut citation_lines = 0;
    let mut total_lines = 0;

    for zb in page_blocks {
        if zb.zone == ZoneKind::Header || zb.zone == ZoneKind::PageNumber {
            continue;
        }
        if has_any_marker(&zb.block) {
            has_markers = true;
        }
        for line in &zb.block.lines {
            total_lines += 1;
            if has_citation_content(&line.text()) {
                citation_lines += 1;
            }
        }
        collected.push((zb.block.text(), zb.page_num));
    }

    let has_citation_density =
        citation_lines >= 3 && total_lines > 0 && citation_lines * 2 >= total_lines;
    let has_refs = has_markers || (found_so_far && has_citation_density);

    TrailingPageResult {
        blocks: collected,
        has_refs,
    }
}

/// Scan backwards from the end of the document for blocks with markers.
/// Requires 5+ total markers to avoid false positives from numbered lists.
/// Also uses citation content density to fill gaps between marker pages
/// (handles author-date papers where lines rarely start with `(year)`).
fn collect_trailing_marker_blocks(zoned_pages: &[Vec<ZonedBlock>]) -> Vec<(String, usize)> {
    let mut blocks = Vec::new();
    let mut pages_without_refs = 0;

    for page_blocks in zoned_pages.iter().rev() {
        let page = assess_trailing_page(page_blocks, !blocks.is_empty());
        if page.has_refs {
            blocks.extend(page.blocks);
            pages_without_refs = 0;
        } else {
            pages_without_refs += 1;
            if !blocks.is_empty() && pages_without_refs >= 2 {
                if is_valid_trailing_cluster(&blocks) {
                    break;
                }
                blocks.clear();
                pages_without_refs = 0;
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

/// Strategy 3: Detect superscript-style bare-number markers.
/// Some papers (e.g., PRL format) use small-font numbers as reference markers
/// on separate lines/blocks, followed by regular-font citation text.
fn collect_superscript_marker_refs(zoned_pages: &[Vec<ZonedBlock>]) -> Vec<(String, usize)> {
    static BARE_NUM_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\s*(\d{1,4})\s*$").unwrap());

    let all_blocks: Vec<&ZonedBlock> = zoned_pages
        .iter()
        .flat_map(|page| page.iter())
        .filter(|zb| zb.zone != ZoneKind::Header && zb.zone != ZoneKind::PageNumber)
        .collect();

    let pairs = find_superscript_pairs(&all_blocks, &BARE_NUM_RE);
    if pairs.len() < 5 {
        return Vec::new();
    }

    pairs
        .into_iter()
        .map(|(marker, text, page)| (format!("{marker}. {text}"), page))
        .collect()
}

/// Find pairs of (bare_number, citation_text) from the tail of the document.
fn find_superscript_pairs(
    all_blocks: &[&ZonedBlock],
    bare_num_re: &Regex,
) -> Vec<(String, String, usize)> {
    let mut pairs = Vec::new();
    let mut i = all_blocks.len();
    let mut gap = 0_usize;

    while i > 0 {
        i -= 1;
        let text = all_blocks[i].block.text();
        let trimmed = text.trim();

        if trimmed.is_empty() {
            continue;
        }

        match classify_superscript_block(trimmed, all_blocks, i, bare_num_re) {
            SuperscriptBlock::Pair(pair) => {
                pairs.push(pair);
                gap = 0;
            }
            SuperscriptBlock::Citation => {
                gap = 0;
            }
            SuperscriptBlock::Gap => {
                gap += 1;
                if !pairs.is_empty() && gap >= 30 {
                    break;
                }
            }
        }
    }

    pairs.reverse();
    pairs
}

enum SuperscriptBlock {
    Pair((String, String, usize)),
    Citation,
    Gap,
}

fn classify_superscript_block(
    trimmed: &str,
    all_blocks: &[&ZonedBlock],
    index: usize,
    bare_num_re: &Regex,
) -> SuperscriptBlock {
    if let Some(caps) = bare_num_re.captures(trimmed) {
        let num: u32 = caps[1].parse().unwrap_or(0);
        if (1900..2100).contains(&num) {
            return SuperscriptBlock::Gap;
        }
        let citation = collect_citation_after(all_blocks, index + 1);
        if citation.is_empty() {
            return SuperscriptBlock::Gap;
        }
        return SuperscriptBlock::Pair((caps[1].to_string(), citation, all_blocks[index].page_num));
    }
    if has_citation_content(trimmed) {
        return SuperscriptBlock::Citation;
    }
    SuperscriptBlock::Gap
}

/// Collect citation text from blocks following a bare-number marker.
fn collect_citation_after(all_blocks: &[&ZonedBlock], start: usize) -> String {
    static BARE_NUM: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\s*(\d{1,4})\s*$").unwrap());

    let mut parts = Vec::new();
    for zb in all_blocks.iter().skip(start) {
        let text = zb.block.text();
        let trimmed = text.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Stop at bare numbers that aren't years (next reference marker)
        if let Some(caps) = BARE_NUM.captures(trimmed) {
            let num: u32 = caps[1].parse().unwrap_or(0);
            if !(1900..2100).contains(&num) {
                break;
            }
        }
        parts.push(trimmed.to_string());
        if parts.len() >= 4 {
            break;
        }
    }
    parts.join(" ")
}

fn is_valid_trailing_cluster(blocks: &[(String, usize)]) -> bool {
    let mut total_markers = 0;
    let mut citation_lines = 0;
    for (text, _) in blocks {
        for line in text.lines() {
            if LINE_MARKER_RE.is_match(line) {
                total_markers += 1;
                let after = LINE_MARKER_RE.replace(line, "");
                if has_citation_content(after.trim()) {
                    citation_lines += 1;
                }
            }
        }
    }
    total_markers >= 5 && citation_lines >= 3
}

/// Split concatenated text blocks into individual references by line markers.
pub(crate) fn split_into_references(
    blocks: &[(String, usize)],
    source: ReferenceSource,
) -> Vec<RawReference> {
    let mut refs = Vec::new();
    let mut state = ReferenceBuildState::default();

    for (text, page_num) in blocks {
        for line in text.split('\n') {
            append_reference_line(&mut refs, &mut state, line.trim(), *page_num, source);
        }
    }
    flush_reference(
        &mut refs,
        &mut state.current_text,
        &state.current_marker,
        state.current_page,
        source,
    );
    split_author_date_blobs(&mut refs);
    refs
}

#[derive(Default)]
struct ReferenceBuildState {
    current_text: String,
    current_marker: Option<String>,
    current_page: usize,
}

fn append_reference_line(
    refs: &mut Vec<RawReference>,
    state: &mut ReferenceBuildState,
    line: &str,
    page_num: usize,
    source: ReferenceSource,
) {
    if line.is_empty() {
        return;
    }
    if let Some(caps) = LINE_MARKER_RE.captures(line) {
        if is_year_continuation(&caps, line) && !state.current_text.is_empty() {
            append_text_line(&mut state.current_text, line);
            return;
        }
        flush_reference(
            refs,
            &mut state.current_text,
            &state.current_marker,
            state.current_page,
            source,
        );
        state.current_marker = extract_marker(&caps);
        state.current_text = LINE_MARKER_RE.replace(line, "").trim().to_string();
        state.current_page = page_num;
        return;
    }
    if state.current_text.is_empty() {
        state.current_text = line.to_string();
        state.current_page = page_num;
    } else {
        append_text_line(&mut state.current_text, line);
    }
}

fn append_text_line(current_text: &mut String, line: &str) {
    current_text.push(' ');
    current_text.push_str(line);
}

fn split_author_date_blobs(refs: &mut Vec<RawReference>) {
    let mut i = 0;
    while i < refs.len() {
        if refs[i].text.len() > 200 {
            let splits = split_author_date_text(&refs[i].text);
            if splits.len() >= 2 {
                let source = refs[i].source;
                let page = refs[i].page_num;
                let new_refs: Vec<RawReference> = splits
                    .into_iter()
                    .map(|t| RawReference {
                        text: t,
                        linemarker: None,
                        source,
                        page_num: page,
                    })
                    .collect();
                refs.splice(i..i + 1, new_refs);
                continue;
            }
        }
        i += 1;
    }
}

/// Match "Surname, I." or "Surname, FirstName" pattern that starts an
/// author-date reference.
static AUTHOR_START_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"[A-Z][^\s,.:;\[\]()]+(?:\s[A-Z][^\s,.:;\[\]()]+){0,2}, (?:[^A-Za-z0-9\s]? ?[A-Z](?:\.|\s|,)|[A-Z][a-z]{2,})",
    )
    .unwrap()
});

/// Match "Surname I." pattern (no comma between surname and initial).
static AUTHOR_START_NOCOMMA_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"[A-Z][a-z]{2,}(?:[\s-][A-Z][a-z]+)* [A-Z]\.").unwrap());

/// Bibliography label year-colon ending: "2005:" or "2013a:" at the end
/// of a cite key label like "Aaij et al. 2013c:".
static BIBLIO_YEAR_COLON_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?:19|20)\d{2}[a-z]?\s*:").unwrap());

fn split_author_date_text(text: &str) -> Vec<String> {
    let split_positions = find_author_split_positions(text);

    if split_positions.is_empty() {
        return vec![text.to_string()];
    }

    let mut refs = Vec::new();
    let mut last = 0;
    for &pos in &split_positions {
        let ref_text = text[last..pos].trim().to_string();
        if !ref_text.is_empty() {
            refs.push(ref_text);
        }
        last = pos;
    }
    if last < text.len() {
        let ref_text = text[last..].trim().to_string();
        if !ref_text.is_empty() {
            refs.push(ref_text);
        }
    }
    refs
}

fn find_author_split_positions(text: &str) -> Vec<usize> {
    let mut seen = std::collections::HashSet::new();
    let mut positions: Vec<usize> = Vec::new();

    for m in AUTHOR_START_RE.find_iter(text) {
        if let Some(pos) = validate_split_position(text, m.start())
            && seen.insert(pos)
        {
            positions.push(pos);
        }
    }

    for m in AUTHOR_START_NOCOMMA_RE.find_iter(text) {
        if let Some(pos) = validate_split_position(text, m.start())
            && seen.insert(pos)
        {
            positions.push(pos);
        }
    }

    for pos in find_biblio_label_positions(text) {
        if seen.insert(pos) {
            positions.push(pos);
        }
    }

    positions.sort_unstable();
    positions
}

/// Find split positions for bibliography labels ending with "YYYY[a-z]:".
/// Scans backward from each year-colon to find the start of the label
/// (first uppercase word after a reference boundary).
fn find_biblio_label_positions(text: &str) -> Vec<usize> {
    let mut positions = Vec::new();
    for m in BIBLIO_YEAR_COLON_RE.find_iter(text) {
        if let Some(pos) = find_label_start(text, m.start())
            && pos > 0
        {
            let before = text[..pos].trim_end();
            if !before.is_empty() && is_ref_boundary(before) {
                positions.push(pos);
            }
        }
    }
    positions
}

/// Scan backward from `year_pos` to find the start of a bibliography label.
/// Labels look like: "Surname et al." / "Surname and Foo" / "Surname, Foo, and Bar"
/// followed by the year. Returns the byte offset of the first surname character.
fn find_label_start(text: &str, year_pos: usize) -> Option<usize> {
    let before = &text[..year_pos];
    let trimmed = before.trim_end();
    if trimmed.is_empty() {
        return None;
    }

    let bytes = trimmed.as_bytes();
    let mut pos = bytes.len();

    loop {
        pos = skip_label_separators(bytes, pos);
        if pos == 0 {
            break;
        }
        let word_end = pos;
        pos = scan_word_start(bytes, pos);
        let word = &trimmed[pos..word_end];

        if is_label_connector(word) || is_label_name(word) {
            continue;
        }
        pos = word_end;
        break;
    }

    while pos < trimmed.len() && is_label_separator(trimmed.as_bytes()[pos]) {
        pos += 1;
    }
    if pos < trimmed.len() { Some(pos) } else { None }
}

fn scan_word_start(bytes: &[u8], mut pos: usize) -> usize {
    while pos > 0 && !is_label_separator(bytes[pos - 1]) {
        pos -= 1;
    }
    pos
}

fn skip_label_separators(bytes: &[u8], mut pos: usize) -> usize {
    while pos > 0 && is_label_separator(bytes[pos - 1]) {
        pos -= 1;
    }
    pos
}

fn is_label_separator(byte: u8) -> bool {
    matches!(byte, b' ' | b',' | b'\t')
}

fn is_label_connector(word: &str) -> bool {
    let word_lower = word.to_lowercase();
    matches!(
        word_lower.as_str(),
        "et" | "al." | "and" | "de" | "di" | "du" | "von" | "van" | "le" | "la"
    )
}

fn is_label_name(word: &str) -> bool {
    word.split('-').all(|part| {
        let mut chars = part.chars();
        chars.next().is_some_and(|c| c.is_ascii_uppercase())
            && chars.all(|c| c.is_alphanumeric() || c == '\'')
    })
}

fn validate_split_position(text: &str, author_pos: usize) -> Option<usize> {
    if author_pos == 0 {
        return None;
    }
    let before = text[..author_pos].trim_end();
    if before.is_empty() {
        return None;
    }
    if is_ref_boundary(before) {
        Some(author_pos)
    } else {
        None
    }
}

fn is_ref_boundary(before: &str) -> bool {
    let last = match before.chars().last() {
        Some(c) => c,
        None => return false,
    };
    match last {
        '.' => is_ref_ending_period(before),
        ']' | ')' => true,
        c if c.is_ascii_digit() => true,
        _ => false,
    }
}

fn is_ref_ending_period(before: &str) -> bool {
    let without_period = before[..before.len() - 1].trim_end();
    if without_period.is_empty() {
        return false;
    }
    let last_char = match without_period.chars().last() {
        Some(c) => c,
        None => return false,
    };
    if matches!(last_char, ']' | ')') || last_char.is_ascii_digit() {
        return true;
    }
    let last_token = without_period.split_whitespace().last().unwrap_or("");
    let clean = last_token.trim_end_matches(',');
    !is_initial_token(clean)
}

fn is_initial_token(token: &str) -> bool {
    if token.is_empty() {
        return false;
    }
    token.split('-').all(|part| {
        let trimmed = part.trim_end_matches('.');
        trimmed.len() == 1 && trimmed.chars().all(|c| c.is_ascii_uppercase())
    })
}

/// Detect paren-form year markers like "(2011)." that are actually years,
/// not reference markers. Treats (YYYY) as a continuation when the remaining
/// text is short or doesn't start with an author name (new reference).
fn is_year_continuation(caps: &regex::Captures, line: &str) -> bool {
    if let Some(m) = caps.get(2) {
        let num: u32 = m.as_str().parse().unwrap_or(0);
        if (1900..2100).contains(&num) {
            let rest = LINE_MARKER_RE.replace(line, "");
            let trimmed = rest.trim();
            if trimmed.len() < 40 {
                return true;
            }
            // Longer text: only treat as continuation if it doesn't
            // start with an uppercase letter (author name = new ref).
            return !trimmed.starts_with(|c: char| c.is_ascii_uppercase());
        }
    }
    false
}

fn extract_marker(caps: &regex::Captures) -> Option<String> {
    caps.get(1)
        .or_else(|| caps.get(2))
        .or_else(|| caps.get(3))
        .or_else(|| caps.get(4))
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

#[cfg(test)]
mod tests;
