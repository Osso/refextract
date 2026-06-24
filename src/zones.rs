use once_cell::sync::Lazy;
use regex::Regex;

use crate::types::{Block, ZoneKind, ZonedBlock};

static TRAILING_PAREN_RANGE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\s*\(\d+\)(?:-\(\d+\))?\s*$").unwrap());

/// Classify blocks on a page into zones based on position and font.
pub fn classify_page(
    blocks: &[Block],
    page_num: usize,
    page_height: f32,
    body_font_size: f32,
) -> Vec<ZonedBlock> {
    blocks
        .iter()
        .map(|block| {
            let zone = classify_block(block, page_height, body_font_size);
            ZonedBlock {
                block: block.clone(),
                zone,
                page_num,
            }
        })
        .collect()
}

fn classify_block(block: &Block, page_height: f32, body_font_size: f32) -> ZoneKind {
    let relative_y = block.y / page_height;
    let block_bottom = (block.y - block.height) / page_height;

    // Header: top ~5%
    if relative_y > 0.95 {
        return ZoneKind::Header;
    }

    // Page number: bottom ~3%, only digits
    if block_bottom < 0.03 && is_page_number(block) {
        return ZoneKind::PageNumber;
    }

    // Footnote: bottom ~25%, smaller font, starts with superscript marker
    if block_bottom < 0.25 && block.font_size < body_font_size * 0.9 && has_superscript_start(block)
    {
        return ZoneKind::Footnote;
    }

    ZoneKind::Body
}

fn is_page_number(block: &Block) -> bool {
    let text = block.text();
    let trimmed = text.trim();
    !trimmed.is_empty() && trimmed.chars().all(|c| c.is_ascii_digit() || c == '-')
}

fn has_superscript_start(block: &Block) -> bool {
    block
        .lines
        .first()
        .and_then(|line| line.words.first())
        .is_some_and(|word| word.is_superscript)
}

/// Detect if a block is a "References" / "Bibliography" heading.
pub fn is_reference_heading(block: &Block) -> bool {
    let text = block.text().to_uppercase();
    let trimmed = text.trim();
    is_heading_text(trimmed)
}

/// Check if a single line's text is a reference heading.
pub fn is_reference_heading_line(line_text: &str) -> bool {
    let trimmed = line_text.trim().to_uppercase();
    is_heading_text(&trimmed)
}

/// Strip trailing parenthesized number ranges: "(36)-(84)", "(1)-(35)"
fn strip_trailing_paren_range(text: &str) -> &str {
    let trimmed = text.trim_end();
    let Some(m) = TRAILING_PAREN_RANGE_RE.find(trimmed) else {
        return trimmed;
    };
    if m.end() != trimmed.len() {
        return trimmed;
    }
    trimmed[..m.start()].trim_end()
}

/// Detect dot-leader patterns used in Tables of Contents, e.g.:
///   "References . . . . . . . ."  (space-separated dots)
///   "References..........."       (consecutive dots)
///   "References … … …"           (ellipsis characters, Unicode U+2026)
/// Three or more dots (consecutive or space-separated) signals a TOC entry.
fn has_dot_leaders(text: &str) -> bool {
    // Check for 3+ consecutive ASCII dots
    if text.contains("...") {
        return true;
    }
    // Check for 3+ consecutive Unicode ellipsis characters (…)
    if text.contains("\u{2026}\u{2026}\u{2026}") {
        return true;
    }
    // Check for space-separated dots: ". . ." (dot, space, dot, space, dot)
    // Count how many isolated dots appear in a row
    let chars: Vec<char> = text.chars().collect();
    let mut dot_run = 0usize;
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '.' || chars[i] == '\u{2026}' {
            dot_run += 1;
            if dot_run >= 3 {
                return true;
            }
            i += 1;
        } else if chars[i] == ' '
            && i + 1 < chars.len()
            && (chars[i + 1] == '.' || chars[i + 1] == '\u{2026}')
        {
            // Space before another dot: keep the run going
            i += 1;
        } else {
            dot_run = 0;
            i += 1;
        }
    }
    false
}

fn is_heading_text(text: &str) -> bool {
    if has_dot_leaders(text) {
        return false;
    }
    let text = text.trim_end_matches([':', '.']);
    let text = strip_trailing_paren_range(text);
    if is_exact_heading(text) {
        return true;
    }
    if text.len() >= 30 {
        return false;
    }
    is_prefixed_heading(text) || is_suffix_number_heading(text)
}

fn is_exact_heading(text: &str) -> bool {
    matches!(
        text,
        "REFERENCES" | "BIBLIOGRAPHY" | "REFERENCES AND NOTES" | "LITERATURE CITED"
    )
}

fn is_prefixed_heading(text: &str) -> bool {
    let prefix = text
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == ' ')
        .collect::<String>();
    let stripped = &text[prefix.len()..];
    if stripped != "REFERENCES" && stripped != "BIBLIOGRAPHY" {
        return false;
    }
    let has_separator = prefix.ends_with(' ') || prefix.ends_with('.');
    let digit_count = prefix.chars().filter(|c| c.is_ascii_digit()).count();
    digit_count <= 1 || has_separator
}

fn is_suffix_number_heading(text: &str) -> bool {
    let suffix = text
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit() || *c == ' ')
        .collect::<String>();
    let suffix_len = suffix.len();
    let stripped = text[..text.len() - suffix_len].trim_end();
    if stripped != "REFERENCES" && stripped != "BIBLIOGRAPHY" {
        return false;
    }
    let digit_count = suffix.chars().filter(|c| c.is_ascii_digit()).count();
    digit_count <= 1
}

/// Compute the dominant (most common) font size across all pages.
pub fn compute_body_font_size(all_blocks: &[Vec<Block>]) -> f32 {
    let mut size_counts: Vec<(i32, usize)> = Vec::new();
    for blocks in all_blocks {
        for block in blocks {
            for line in &block.lines {
                let key = (line.font_size * 10.0) as i32;
                if let Some(entry) = size_counts.iter_mut().find(|(k, _)| *k == key) {
                    entry.1 += line.words.len();
                } else {
                    size_counts.push((key, line.words.len()));
                }
            }
        }
    }
    size_counts
        .iter()
        .max_by_key(|(_, count)| *count)
        .map(|(key, _)| *key as f32 / 10.0)
        .unwrap_or(10.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Block, Line, Word, ZoneKind};

    fn word(text: &str, superscript: bool) -> Word {
        Word {
            text: text.to_string(),
            x: 10.0,
            y: 10.0,
            width: 20.0,
            font_size: if superscript { 7.0 } else { 10.0 },
            is_superscript: superscript,
        }
    }

    fn block(text: &str, y: f32, font_size: f32) -> Block {
        Block {
            lines: vec![Line {
                words: text
                    .split_whitespace()
                    .map(|part| word(part, false))
                    .collect(),
                y,
                x_start: 10.0,
                x_end: 100.0,
                font_size,
            }],
            x: 10.0,
            y,
            width: 90.0,
            height: 12.0,
            font_size,
        }
    }

    #[test]
    fn reference_heading_accepts_common_forms() {
        assert!(is_reference_heading_line("References"));
        assert!(is_reference_heading_line("Bibliography"));
        assert!(is_reference_heading_line("8. References"));
        assert!(is_reference_heading_line("REFERENCES (36)-(84)"));
    }

    #[test]
    fn reference_heading_rejects_toc_dot_leaders() {
        assert!(!is_reference_heading_line("References . . . . . . 42"));
        assert!(!is_reference_heading_line("References........42"));
    }

    #[test]
    fn classify_page_marks_header_page_number_footnote_and_body() {
        let mut footnote = block("1 Footnote citation Phys. Rev. 2020", 120.0, 7.0);
        footnote.lines[0].words[0].is_superscript = true;
        let blocks = vec![
            block("Journal header", 780.0, 10.0),
            block("12", 20.0, 10.0),
            block("Main body paragraph", 400.0, 10.0),
            footnote,
        ];

        let zones = classify_page(&blocks, 1, 800.0, 10.0);

        assert_eq!(zones[0].zone, ZoneKind::Header);
        assert_eq!(zones[1].zone, ZoneKind::PageNumber);
        assert_eq!(zones[2].zone, ZoneKind::Body);
        assert_eq!(zones[3].zone, ZoneKind::Footnote);
    }

    #[test]
    fn compute_body_font_size_uses_most_common_font() {
        let pages = vec![vec![
            block("small", 20.0, 8.0),
            block("body one", 100.0, 10.0),
            block("body two", 120.0, 10.0),
        ]];

        assert_eq!(compute_body_font_size(&pages), 10.0);
    }
}
