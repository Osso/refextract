use std::collections::HashMap;

use once_cell::sync::Lazy;
use regex::Regex;

static JOURNAL_TITLES_KB: &str = include_str!("../kbs/journal-titles.kb");
static REPORT_NUMBERS_KB: &str = include_str!("../kbs/report-numbers.kb");
static COLLABORATIONS_KB: &str = include_str!("../kbs/collaborations.kb");

/// Journal title mapping: uppercase full name → abbreviated name.
/// Sorted by key length descending for longest-match-first lookup.
pub static JOURNAL_TITLES: Lazy<Vec<(String, String)>> = Lazy::new(|| {
    let mut entries: Vec<(String, String)> = JOURNAL_TITLES_KB
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (full, abbrev) = line.split_once("---")?;
            Some((full.trim().to_uppercase(), abbrev.trim().to_string()))
        })
        .collect();
    entries.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
    entries
});

/// Abbreviated journal forms for matching in references.
/// Maps normalized abbreviation (dots/spaces removed, uppercased) → canonical abbreviation.
pub static JOURNAL_ABBREVS: Lazy<Vec<(String, String)>> = Lazy::new(|| {
    let mut seen = std::collections::HashSet::new();
    let mut entries: Vec<(String, String)> = JOURNAL_TITLES_KB
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (_, abbrev) = line.split_once("---")?;
            let abbrev = abbrev.trim();
            // Normalize: "Phys. Rev. D" → "PHYS REV D" for matching
            let normalized = normalize_abbrev(abbrev);
            if normalized.len() < 3 || !seen.insert(normalized.clone()) {
                return None;
            }
            Some((normalized, abbrev.to_string()))
        })
        .collect();
    entries.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
    entries
});

/// Normalize an abbreviated journal name for matching.
/// "Phys. Rev. D" → "PHYS REV D"
fn normalize_abbrev(s: &str) -> String {
    s.chars()
        .filter(|c| *c != '.')
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_uppercase()
}

/// Collaboration name mapping: uppercase name → standardized name.
pub static COLLABORATIONS: Lazy<HashMap<String, String>> = Lazy::new(|| {
    COLLABORATIONS_KB
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (name, standardized) = line.split_once("---")?;
            Some((name.trim().to_uppercase(), standardized.trim().to_string()))
        })
        .collect()
});

/// A report number pattern: institute prefix + compiled regex for numeration.
pub struct ReportNumberPattern {
    pub prefix: String,
    pub standardized: String,
    pub numeration_re: Regex,
}

/// Compiled report number patterns.
pub static REPORT_NUMBERS: Lazy<Vec<ReportNumberPattern>> =
    Lazy::new(|| parse_report_numbers(REPORT_NUMBERS_KB));

fn parse_report_numbers(kb_text: &str) -> Vec<ReportNumberPattern> {
    let mut patterns = Vec::new();
    let mut current_numerations: Vec<String> = Vec::new();

    for line in kb_text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("*****") {
            continue;
        }
        if line.starts_with('<') && line.ends_with('>') {
            let inner = &line[1..line.len() - 1];
            if let Some(regex_str) = numeration_to_regex(inner) {
                current_numerations.push(regex_str);
            }
            continue;
        }
        if let Some((prefix, standardized)) = line.split_once("---") {
            add_prefix_patterns(
                &mut patterns,
                prefix.trim(),
                standardized.trim(),
                &current_numerations,
            );
        }
    }
    patterns
}

fn add_prefix_patterns(
    patterns: &mut Vec<ReportNumberPattern>,
    prefix: &str,
    standardized: &str,
    numerations: &[String],
) {
    let escaped = regex::escape(&prefix.replace('\t', " ").replace("  ", " "));
    let flex_prefix = escaped.replace(r"\ ", r"[\s\-/]+");
    for num_re in numerations {
        let full_pattern = format!(r"(?i)\b{flex_prefix}[\s\-/]*{num_re}");
        if let Ok(re) = Regex::new(&full_pattern) {
            patterns.push(ReportNumberPattern {
                prefix: prefix.to_string(),
                standardized: standardized.to_string(),
                numeration_re: re,
            });
        }
    }
}

/// Convert the KB numeration DSL to a regex string.
///
/// DSL: `9`→`\d`, `9?`→`\d?`, `s`→separator, `yyyy`→year,
/// `yy`→2-digit year, `mm`→month, `a`→letter.
/// Regex constructs pass through verbatim.
fn numeration_to_regex(dsl: &str) -> Option<String> {
    let mut result = String::new();
    let chars: Vec<char> = dsl.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let consumed = try_emit_regex_construct(&chars, i, &mut result)
            .or_else(|| try_emit_dsl_token(&chars, i, &mut result))
            .unwrap_or_else(|| emit_literal(chars[i], &mut result));
        i += consumed;
    }

    Some(result)
}

/// Try to emit a pass-through regex construct (escape, char class, group).
/// Returns number of chars consumed, or None if not a regex construct.
fn try_emit_regex_construct(
    chars: &[char],
    i: usize,
    out: &mut String,
) -> Option<usize> {
    match chars[i] {
        '\\' if i + 1 < chars.len() => {
            out.push(chars[i]);
            out.push(chars[i + 1]);
            Some(2)
        }
        '[' => Some(emit_char_class(chars, i, out)),
        '(' => Some(emit_group(chars, i, out)),
        ')' | '|' | '+' | '*' | '?' => {
            out.push(chars[i]);
            Some(1)
        }
        _ => None,
    }
}

fn emit_char_class(chars: &[char], start: usize, out: &mut String) -> usize {
    let mut i = start;
    while i < chars.len() {
        out.push(chars[i]);
        if chars[i] == ']' && i > start {
            return i - start + 1;
        }
        i += 1;
    }
    i - start
}

fn emit_group(chars: &[char], start: usize, out: &mut String) -> usize {
    let mut i = start;
    let mut depth = 0;
    while i < chars.len() {
        if chars[i] == '(' {
            depth += 1;
        }
        if chars[i] == ')' {
            depth -= 1;
        }
        out.push(chars[i]);
        i += 1;
        if depth == 0 {
            // Consume trailing quantifier
            if i < chars.len() && matches!(chars[i], '?' | '+' | '*') {
                out.push(chars[i]);
                i += 1;
            }
            break;
        }
    }
    i - start
}

/// Try to emit a DSL token (yyyy, yy, mm, 9?, 9, s, a).
/// Returns number of chars consumed, or None.
fn try_emit_dsl_token(
    chars: &[char],
    i: usize,
    out: &mut String,
) -> Option<usize> {
    let remaining: String = chars[i..].iter().collect();

    if remaining.starts_with("yyyy") {
        out.push_str(r"[12]\d{3}");
        return Some(4);
    }
    if remaining.starts_with("yy") {
        out.push_str(r"\d{2}");
        return Some(2);
    }
    if remaining.starts_with("mm") {
        out.push_str(r"[01]\d");
        return Some(2);
    }
    if chars[i] == '9' && i + 1 < chars.len() && chars[i + 1] == '?' {
        out.push_str(r"\d?");
        return Some(2);
    }
    if chars[i] == '9' {
        out.push_str(r"\d");
        return Some(1);
    }
    if chars[i] == 's' {
        out.push_str(r"[\s\-/]+");
        return Some(1);
    }
    if chars[i] == 'a' {
        out.push_str(r"[A-Za-z]");
        let extra = if i + 1 < chars.len() && chars[i + 1] == '?' {
            out.push('?');
            1
        } else {
            0
        };
        return Some(1 + extra);
    }
    if chars[i] == ' ' {
        out.push_str(r"[\s\-/]+");
        return Some(1);
    }
    None
}

fn emit_literal(ch: char, out: &mut String) -> usize {
    out.push(ch);
    1
}

/// Try to match a journal name at the given byte position in text.
/// Returns (matched_byte_length, abbreviated_name) if found.
/// Tries both full names and abbreviated forms.
pub fn match_journal_name(text: &str, pos: usize) -> Option<(usize, String)> {
    if !text.is_char_boundary(pos) {
        return None;
    }
    let suffix = &text[pos..];
    match_full_journal(suffix)
        .or_else(|| match_abbrev_journal(suffix))
}

fn match_full_journal(suffix: &str) -> Option<(usize, String)> {
    let upper = suffix.to_uppercase();
    for (full_name, abbrev) in JOURNAL_TITLES.iter() {
        if !upper.starts_with(full_name.as_str()) {
            continue;
        }
        let match_len = full_name.len();
        if match_len >= suffix.len()
            || !suffix.as_bytes()[match_len].is_ascii_alphanumeric()
        {
            return Some((match_len, abbrev.clone()));
        }
    }
    None
}

fn match_abbrev_journal(suffix: &str) -> Option<(usize, String)> {
    let normalized = normalize_abbrev(suffix);
    for (norm_key, abbrev) in JOURNAL_ABBREVS.iter() {
        if !normalized.starts_with(norm_key.as_str()) {
            continue;
        }
        // Find how many original bytes correspond to the matched normalized key
        let byte_len = find_original_byte_len(suffix, norm_key.len());
        if byte_len >= suffix.len()
            || !suffix.as_bytes()[byte_len].is_ascii_alphanumeric()
        {
            return Some((byte_len, abbrev.clone()));
        }
    }
    None
}

/// Find how many bytes in the original string correspond to N normalized chars.
/// Normalized form strips dots and collapses whitespace.
fn find_original_byte_len(original: &str, norm_len: usize) -> usize {
    let mut norm_pos = 0;
    let mut orig_pos = 0;
    let bytes = original.as_bytes();

    while orig_pos < bytes.len() && norm_pos < norm_len {
        let ch = bytes[orig_pos];
        if ch == b'.' {
            orig_pos += 1;
            continue;
        }
        if ch == b' ' || ch == b'\t' {
            // In normalized form, whitespace becomes single space
            if norm_pos > 0 {
                norm_pos += 1; // Count the space
            }
            while orig_pos < bytes.len()
                && (bytes[orig_pos] == b' ' || bytes[orig_pos] == b'\t')
            {
                orig_pos += 1;
            }
            continue;
        }
        norm_pos += 1;
        orig_pos += 1;
    }
    // Skip trailing dots only (not spaces — space is the word boundary)
    while orig_pos < bytes.len() && bytes[orig_pos] == b'.' {
        orig_pos += 1;
    }
    orig_pos
}

/// Try to match a collaboration name in the text.
pub fn match_collaboration(text: &str) -> Option<String> {
    let upper = text.to_uppercase();
    COLLABORATIONS
        .iter()
        .find(|(name, _)| upper.contains(name.as_str()))
        .map(|(_, standardized)| standardized.clone())
}

/// Try to match a report number in the text.
/// Returns (matched_text, standardized_prefix).
pub fn match_report_number(text: &str) -> Option<(String, String)> {
    REPORT_NUMBERS.iter().find_map(|pattern| {
        pattern
            .numeration_re
            .find(text)
            .map(|m| (m.as_str().to_string(), pattern.standardized.clone()))
    })
}
