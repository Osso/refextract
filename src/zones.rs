use crate::types::{Block, ZoneKind, ZonedBlock};

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

fn classify_block(
    block: &Block,
    page_height: f32,
    body_font_size: f32,
) -> ZoneKind {
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
    if block_bottom < 0.25
        && block.font_size < body_font_size * 0.9
        && has_superscript_start(block)
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

fn is_heading_text(text: &str) -> bool {
    // Strip trailing punctuation (colon, period)
    let text = text.trim_end_matches([':', '.']);
    // Exact matches
    if matches!(
        text,
        "REFERENCES"
            | "BIBLIOGRAPHY"
            | "REFERENCES AND NOTES"
            | "LITERATURE CITED"
    ) {
        return true;
    }
    if text.len() >= 30 {
        return false;
    }
    // Accept section-numbered headings: "IX. REFERENCES", "5. REFERENCES"
    // Accept line-numbered headings: "1204 REFERENCES" (line numbers in
    // papers like 0810.4930 and 1104.1607 have multi-digit prefixes)
    // Reject running headers: "REFERENCES" with a page number suffix
    let prefix = text
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == ' ')
        .collect::<String>();
    let stripped = &text[prefix.len()..];
    if stripped == "REFERENCES" || stripped == "BIBLIOGRAPHY" {
        // Prefix must end with space/dot before heading (line numbers always do)
        let has_separator = prefix.ends_with(' ') || prefix.ends_with('.');
        let digit_count = prefix.chars().filter(|c| c.is_ascii_digit()).count();
        return digit_count <= 1 || has_separator;
    }
    // Reject suffix numbers: "REFERENCES 835" — likely running headers
    let suffix = text
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit() || *c == ' ')
        .collect::<String>();
    let suffix_len = suffix.len();
    let stripped = text[..text.len() - suffix_len].trim_end();
    if stripped == "REFERENCES" || stripped == "BIBLIOGRAPHY" {
        let digit_count = suffix.chars().filter(|c| c.is_ascii_digit()).count();
        return digit_count <= 1;
    }
    false
}

/// Compute the dominant (most common) font size across all pages.
pub fn compute_body_font_size(all_blocks: &[Vec<Block>]) -> f32 {
    let mut size_counts: Vec<(i32, usize)> = Vec::new();
    for blocks in all_blocks {
        for block in blocks {
            for line in &block.lines {
                let key = (line.font_size * 10.0) as i32;
                if let Some(entry) =
                    size_counts.iter_mut().find(|(k, _)| *k == key)
                {
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
