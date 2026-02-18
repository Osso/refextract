use std::collections::HashMap;

use once_cell::sync::Lazy;
use regex::Regex;

// ── Trie for report-number prefix dispatch ─────────────────────────────────

struct TrieNode {
    /// Children indexed by lowercase ASCII byte.
    children: HashMap<u8, Box<TrieNode>>,
    /// Patterns whose prefix ends at this node.
    leaves: Vec<TrieLeaf>,
}

struct TrieLeaf {
    standardized: String,
    /// Matches the numeration part that follows the prefix: `[\s\-/]*(?:alt1|alt2|…)`.
    numeration_re: Regex,
}

pub struct ReportNumberTrie {
    root: TrieNode,
}

pub struct ReportNumberMatch {
    pub matched: String,
    pub standardized: String,
}

impl TrieNode {
    fn new() -> Self {
        TrieNode { children: HashMap::new(), leaves: Vec::new() }
    }
}

impl ReportNumberTrie {
    /// Find the first report number match anywhere in `text`.
    pub fn find_match(&self, text: &str) -> Option<ReportNumberMatch> {
        let bytes = text.as_bytes();
        for start in 0..bytes.len() {
            // Require word boundary: start of string or previous char is not alphanumeric.
            if start > 0 && bytes[start - 1].is_ascii_alphanumeric() {
                continue;
            }
            if let Some(m) = self.try_match_at(text, start) {
                return Some(m);
            }
        }
        None
    }

    fn try_match_at(&self, text: &str, start: usize) -> Option<ReportNumberMatch> {
        let bytes = text.as_bytes();
        let mut node = &self.root;
        let mut pos = start;
        let mut best: Option<ReportNumberMatch> = None;

        // Walk trie edges, matching text case-insensitively.
        // A space edge in the trie consumes 1+ separator chars (space/tab/dash/slash).
        loop {
            // At every node with leaves, try numeration regex on remaining text.
            if !node.leaves.is_empty()
                && let Some(m) = try_leaves(&node.leaves, text, pos, start)
            {
                best = Some(m);
            }
            if pos >= bytes.len() {
                break;
            }
            let ch = bytes[pos].to_ascii_lowercase();
            // Separators always route through the space edge, consuming all consecutive ones.
            if ch == b' ' || ch == b'\t' || ch == b'-' || ch == b'/' {
                if let Some(child) = node.children.get(&b' ') {
                    while pos < bytes.len()
                        && matches!(bytes[pos], b' ' | b'\t' | b'-' | b'/')
                    {
                        pos += 1;
                    }
                    node = child;
                } else {
                    break;
                }
            } else if let Some(child) = node.children.get(&ch) {
                node = child;
                pos += 1;
            } else {
                break;
            }
        }
        best
    }
}

/// Try all leaves at the current trie node against remaining text.
fn try_leaves(
    leaves: &[TrieLeaf],
    text: &str,
    pos: usize,
    start: usize,
) -> Option<ReportNumberMatch> {
    let suffix = &text[pos..];
    for leaf in leaves {
        if let Some(m) = leaf.numeration_re.find(suffix) {
            // Only accept match anchored at position 0 in suffix.
            if m.start() == 0 {
                let matched = text[start..pos + m.end()].to_string();
                return Some(ReportNumberMatch {
                    matched,
                    standardized: leaf.standardized.clone(),
                });
            }
        }
    }
    None
}

/// Build the report-number trie from KB text.
pub fn build_report_trie(kb_text: &str) -> ReportNumberTrie {
    let mut root = TrieNode::new();
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
            insert_into_trie(
                &mut root,
                prefix.trim(),
                standardized.trim(),
                &current_numerations,
            );
        }
    }
    ReportNumberTrie { root }
}

fn insert_into_trie(
    root: &mut TrieNode,
    prefix: &str,
    standardized: &str,
    numerations: &[String],
) {
    if numerations.is_empty() {
        return;
    }
    // Normalize: collapse tabs/double-spaces to single space, then lowercase.
    let normalized = prefix
        .replace('\t', " ")
        .replace("  ", " ")
        .to_ascii_lowercase();

    // Walk/create trie nodes for each character of the normalized prefix.
    // Spaces in the prefix represent flexible separators (space/tab/dash/slash).
    let mut node = root;
    for byte in normalized.bytes() {
        node = node.children.entry(byte).or_insert_with(|| Box::new(TrieNode::new()));
    }

    // Build numeration regex anchored to start of remaining text.
    let num_alt = numerations.join("|");
    let pattern = format!(r"(?i)^[\s\-/]*(?:{num_alt})");
    if let Ok(re) = Regex::new(&pattern) {
        node.leaves.push(TrieLeaf {
            standardized: standardized.to_string(),
            numeration_re: re,
        });
    }
}

/// Compiled report-number trie (replaces sequential REPORT_NUMBERS scan).
pub static REPORT_NUMBER_TRIE: Lazy<ReportNumberTrie> =
    Lazy::new(|| build_report_trie(REPORT_NUMBERS_KB));

// Force recompilation when KB files change (hash set by build.rs).
#[allow(dead_code)]
const _KB_HASH: &str = env!("KB_HASH");

static JOURNAL_TITLES_KB: &str = include_str!("../kbs/journal-titles.kb");
static REPORT_NUMBERS_KB: &str = include_str!("../kbs/report-numbers.kb");
static COLLABORATIONS_KB: &str = include_str!("../kbs/collaborations.kb");

/// Journal title mapping: normalized full name → abbreviated name.
/// Keys are normalized (dots stripped, whitespace collapsed, uppercased)
/// so that text like "Astrophys. J. Suppl." can match KB entry "ASTROPHYS J SUPPL".
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
            Some((normalize_abbrev(full.trim()), abbrev.trim().to_string()))
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
            // Skip short abbreviations — too many false positives
            // e.g., "EN" matches "Witten,", "PR" matches "er," in author names
            // Require at least 3 chars (e.g., "PoS", "JHEP", "JCAP" ok, "EN" "PR" not)
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
/// "Phys.Rev.D" → "PHYS REV D"  (dots act as word separators)
fn normalize_abbrev(s: &str) -> String {
    s.chars()
        .map(|c| if c == '.' || c == ':' { ' ' } else { c })
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
#[allow(dead_code)]
pub struct ReportNumberPattern {
    pub prefix: String,
    pub standardized: String,
    pub numeration_re: Regex,
}

/// Compiled report number patterns (kept for reference; replaced by REPORT_NUMBER_TRIE).
#[allow(dead_code)]
pub static REPORT_NUMBERS: Lazy<Vec<ReportNumberPattern>> =
    Lazy::new(|| parse_report_numbers(REPORT_NUMBERS_KB));

#[allow(dead_code)]
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

#[allow(dead_code)]
fn add_prefix_patterns(
    patterns: &mut Vec<ReportNumberPattern>,
    prefix: &str,
    standardized: &str,
    numerations: &[String],
) {
    if numerations.is_empty() {
        return;
    }
    let escaped = regex::escape(&prefix.replace('\t', " ").replace("  ", " "));
    let flex_prefix = escaped.replace(r"\ ", r"[\s\-/]+");
    // Combine all numerations into a single regex with alternation
    let num_alt = numerations.join("|");
    let full_pattern = format!(r"(?i)\b{flex_prefix}[\s\-/]*(?:{num_alt})");
    if let Ok(re) = Regex::new(&full_pattern) {
        patterns.push(ReportNumberPattern {
            prefix: prefix.to_string(),
            standardized: standardized.to_string(),
            numeration_re: re,
        });
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
    // Must be at a word boundary: position 0 or preceded by non-alphanumeric.
    // Prevents matching "AP" inside "WMAP" or "EN" inside "Witten".
    if pos > 0 && text.as_bytes()[pos - 1].is_ascii_alphanumeric() {
        return None;
    }
    let suffix = &text[pos..];
    match_full_journal(suffix)
        .or_else(|| match_abbrev_journal(suffix))
}

fn match_full_journal(suffix: &str) -> Option<(usize, String)> {
    // Must start with a letter (some journals like "npj Quantum Inf." start lowercase)
    if !suffix.as_bytes().first().is_some_and(|b| b.is_ascii_alphabetic()) {
        return None;
    }
    let normalized = normalize_abbrev(suffix);
    for (full_name, abbrev) in JOURNAL_TITLES.iter() {
        if !normalized.starts_with(full_name.as_str()) {
            continue;
        }
        // Map normalized match length back to original byte position
        let match_len = find_original_byte_len(suffix, full_name.len());
        if is_journal_boundary(suffix, match_len) {
            return Some((match_len, abbrev.clone()));
        }
    }
    None
}

fn match_abbrev_journal(suffix: &str) -> Option<(usize, String)> {
    // First char must be a letter (some abbreviations like "npj" start lowercase)
    let first = suffix.as_bytes().first()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }

    let normalized = normalize_abbrev(suffix);
    for (norm_key, abbrev) in JOURNAL_ABBREVS.iter() {
        if !normalized.starts_with(norm_key.as_str()) {
            continue;
        }
        // Find how many original bytes correspond to the matched normalized key
        let byte_len = find_original_byte_len(suffix, norm_key.len());
        if is_journal_boundary(suffix, byte_len) {
            return Some((byte_len, abbrev.clone()));
        }
    }
    None
}

/// Check if the match ends at a word boundary.
/// A boundary exists when: end of string, next char is non-alphanumeric,
/// a trailing period was consumed (abbreviation end like "Lett.74"),
/// or the match ends with a section letter directly followed by a digit
/// (e.g., "Chin. Phys. C40" — section letter "C" + volume "40").
fn is_journal_boundary(suffix: &str, match_len: usize) -> bool {
    if match_len >= suffix.len() {
        return true;
    }
    let next = suffix.as_bytes()[match_len];
    if !next.is_ascii_alphanumeric() {
        return true;
    }
    // Period before match_len means the abbreviation ended with a dot,
    // which is a natural word boundary (e.g., "Lett.74")
    if match_len > 0 && suffix.as_bytes()[match_len - 1] == b'.' {
        return true;
    }
    // Section letter followed by digit: "...C40", "...D72", "...A562"
    // The last matched char is an uppercase letter and next char is a digit.
    if match_len > 0 && next.is_ascii_digit() {
        let last = suffix.as_bytes()[match_len - 1];
        if last.is_ascii_uppercase() {
            return true;
        }
    }
    false
}

/// Find how many bytes in the original string correspond to N normalized chars.
/// Normalized form treats dots, colons, and whitespace as word separators,
/// collapsing consecutive separators to a single space.
fn find_original_byte_len(original: &str, norm_len: usize) -> usize {
    let mut norm_pos = 0;
    let mut orig_pos = 0;
    let bytes = original.as_bytes();

    while orig_pos < bytes.len() && norm_pos < norm_len {
        let ch = bytes[orig_pos];
        // Dots, colons, and whitespace are word separators; normalize to single space
        if ch == b'.' || ch == b':' || ch == b' ' || ch == b'\t' {
            if norm_pos > 0 {
                norm_pos += 1;
            }
            while orig_pos < bytes.len()
                && matches!(bytes[orig_pos], b'.' | b':' | b' ' | b'\t')
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
    REPORT_NUMBER_TRIE
        .find_match(text)
        .map(|m| (m.matched, m.standardized))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trie() -> ReportNumberTrie {
        build_report_trie(REPORT_NUMBERS_KB)
    }

    #[test]
    fn fermilab_pub_hyphen_separator() {
        let t = trie();
        let m = t.find_match("see FERMILAB-PUB-93-123 for details");
        let m = m.expect("should match FERMILAB-PUB");
        assert_eq!(m.standardized, "FERMILAB-Pub");
        assert!(m.matched.to_uppercase().starts_with("FERMILAB"));
    }

    #[test]
    fn fermilab_pub_space_separator() {
        let t = trie();
        let m = t.find_match("see FERMILAB PUB 93-123 for details");
        let m = m.expect("should match FERMILAB PUB");
        assert_eq!(m.standardized, "FERMILAB-Pub");
    }

    #[test]
    fn slac_pub_match() {
        let t = trie();
        let m = t.find_match("B. Richter, SLAC-PUB-8587 (hep-ph/0008222)");
        let m = m.expect("should match SLAC-PUB");
        assert!(m.standardized.to_uppercase().contains("SLAC"));
    }

    #[test]
    fn cern_match() {
        let t = trie();
        let m = t.find_match("CERN 96-01 Vol. 2");
        let m = m.expect("should match CERN");
        assert!(m.standardized.contains("CERN"));
    }

    #[test]
    fn no_match_plain_text() {
        let t = trie();
        let m = t.find_match("No report number here just text");
        assert!(m.is_none());
    }

    #[test]
    fn double_space_separator() {
        // "FERMILAB  PUB" (double space) should still match via separator collapse
        let t = trie();
        let m = t.find_match("FERMILAB  PUB 93-123");
        let m = m.expect("should match FERMILAB  PUB with double space");
        assert_eq!(m.standardized, "FERMILAB-Pub");
    }
}
