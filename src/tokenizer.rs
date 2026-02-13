use once_cell::sync::Lazy;
use regex::Regex;

use crate::kb;
use crate::types::{Token, TokenKind};

static DOI_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"10\.\d{4,}/[^\s,;]+").unwrap());

static ARXIV_NEW_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\d{4}\.\d{4,5}(?:v\d+)?").unwrap());

static ARXIV_OLD_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?:hep|astro|cond|gr|math|nucl|physics|quant|cs|nlin|q-bio|q-fin|stat)(?:[\s-][a-z]{2,3})?[\s/]+\d{7}(?:v\d+)?").unwrap()
});

static URL_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"https?://[^\s,;]+").unwrap());

static ISBN_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?:978|979)[-\s]?\d[-\s]?\d{2,5}[-\s]?\d{2,5}[-\s]?\d").unwrap());

static YEAR_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\(?((?:19|20)\d{2})[a-z]?\)?$").unwrap());

static PAGE_RANGE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\d+\s*[-–—]\s*\d+").unwrap());

static NUMBER_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\d+").unwrap());

/// Compact volume(year)page: "417(1994)181" or "417(1994)181-193"
static VOLUME_YEAR_PAGE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(\d+)\(((?:19|20)\d{2})\)(\d+(?:\s*[-–—]\s*\d+)?)$").unwrap()
});

/// Volume:page: "70:094505" or "95:122002"
static VOLUME_COLON_PAGE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(\d+):(\d+(?:\s*[-–—]\s*\d+)?)$").unwrap());

/// Compact volume(year) without page: "301(1993)"
static VOLUME_YEAR_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(\d+)\(((?:19|20)\d{2})\)$").unwrap());

/// Volume with issue number: "82(25)" or "82(2-3)" — extract volume, discard issue
static VOLUME_ISSUE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(\d+)\(\d+(?:[-–—]\d+)?\)$").unwrap());

/// Article number with letter suffix: "111301(R)", "040404/1" — extract digits
static ARTICLE_NUMBER_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(\d+)(?:\([A-Za-z]+\)|/\d+)$").unwrap());

static LINE_MARKER_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s*(?:\[(\d+)\]|\((\d+)\)|(\d+)[.\)])\s*").unwrap());

/// Tokenize a reference string into a sequence of typed tokens.
pub fn tokenize(text: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let work = strip_line_marker(text, &mut tokens);
    let spans = find_identifier_spans(work);
    fill_tokens(work, &spans, &mut tokens);
    tokens
}

fn strip_line_marker<'a>(text: &'a str, tokens: &mut Vec<Token>) -> &'a str {
    if let Some(caps) = LINE_MARKER_RE.captures(text) {
        let marker = caps
            .get(1)
            .or_else(|| caps.get(2))
            .or_else(|| caps.get(3))
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();
        tokens.push(Token {
            kind: TokenKind::LineMarker,
            text: marker,
            normalized: None,
        });
        let end = caps.get(0).unwrap().end();
        return &text[end..];
    }
    text
}

struct Span {
    start: usize,
    end: usize,
    kind: TokenKind,
    text: String,
    normalized: Option<String>,
}

fn find_identifier_spans(text: &str) -> Vec<Span> {
    let mut spans = Vec::new();
    add_doi_spans(&mut spans, text);
    add_regex_spans(&mut spans, text, &URL_RE, TokenKind::Url);
    add_arxiv_old_spans(&mut spans, text);
    add_regex_spans(&mut spans, text, &ARXIV_NEW_RE, TokenKind::ArxivId);
    add_regex_spans(&mut spans, text, &ISBN_RE, TokenKind::Isbn);
    add_report_number_spans(&mut spans, text);
    add_journal_name_spans(&mut spans, text);
    spans.sort_by_key(|s| s.start);
    remove_overlapping_spans(&mut spans);
    spans
}

fn add_doi_spans(spans: &mut Vec<Span>, text: &str) {
    for m in DOI_RE.find_iter(text) {
        let matched = m.as_str().trim_end_matches(|c: char| ".)]}>".contains(c));
        let end = m.start() + matched.len();
        if !overlaps_existing(spans, m.start(), end) {
            spans.push(Span {
                start: m.start(),
                end,
                kind: TokenKind::Doi,
                text: matched.to_string(),
                normalized: None,
            });
        }
    }
}

/// Add old-style arXiv ID spans with normalization: "hep ph/0202058" → "hep-ph/0202058"
fn add_arxiv_old_spans(spans: &mut Vec<Span>, text: &str) {
    for m in ARXIV_OLD_RE.find_iter(text) {
        if !overlaps_existing(spans, m.start(), m.end()) {
            let raw = m.as_str().to_string();
            // Normalize: replace whitespace between category parts with hyphens,
            // and ensure single slash separator before digits
            let normalized = normalize_arxiv_old(&raw);
            spans.push(Span {
                start: m.start(),
                end: m.end(),
                kind: TokenKind::ArxivId,
                text: normalized,
                normalized: None,
            });
        }
    }
}

/// Normalize old-style arXiv ID: "hep ph/0202058" → "hep-ph/0202058"
fn normalize_arxiv_old(raw: &str) -> String {
    // Replace spaces between letters with hyphens, collapse multiple separators
    let mut result = String::new();
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c == ' ' || c == '\t' {
            // Check if this space is between letter parts (not before digits)
            if chars.peek().is_some_and(|&next| next.is_ascii_alphabetic()) {
                result.push('-');
            } else {
                // Space before slash or digits — skip
            }
        } else {
            result.push(c);
        }
    }
    result
}

fn add_regex_spans(
    spans: &mut Vec<Span>,
    text: &str,
    re: &Regex,
    kind: TokenKind,
) {
    for m in re.find_iter(text) {
        if !overlaps_existing(spans, m.start(), m.end()) {
            spans.push(Span {
                start: m.start(),
                end: m.end(),
                kind: kind.clone(),
                text: m.as_str().to_string(),
                normalized: None,
            });
        }
    }
}

fn add_report_number_spans(spans: &mut Vec<Span>, text: &str) {
    if let Some((matched, standardized)) = kb::match_report_number(text)
        && let Some(pos) = text.find(&matched)
            && !overlaps_existing(spans, pos, pos + matched.len()) {
                spans.push(Span {
                    start: pos,
                    end: pos + matched.len(),
                    kind: TokenKind::ReportNumber,
                    text: matched,
                    normalized: Some(standardized),
                });
            }
}

fn add_journal_name_spans(spans: &mut Vec<Span>, text: &str) {
    let quoted_regions = find_quoted_regions(text);
    let mut pos = 0;
    while pos < text.len() {
        if !text.is_char_boundary(pos) || in_quoted_region(pos, &quoted_regions) {
            pos += 1;
            continue;
        }
        if overlaps_existing(spans, pos, pos + 1) {
            pos += 1;
            continue;
        }
        if let Some((len, abbrev)) = kb::match_journal_name(text, pos) {
            let (len, abbrev) = extend_section_letter(text, pos, len, abbrev);
            spans.push(Span {
                start: pos,
                end: pos + len,
                kind: TokenKind::JournalName,
                text: text[pos..pos + len].to_string(),
                normalized: Some(abbrev),
            });
            pos += len;
        } else {
            pos += 1;
        }
    }
}

/// Extend a journal match to include a section letter if present.
/// "Phys. Rev." + " D31" → "Phys. Rev. D" (volume "31" becomes a separate token).
/// "Nucl. Phys." + " B253" → "Nucl. Phys. B"
fn extend_section_letter(
    text: &str,
    pos: usize,
    len: usize,
    abbrev: String,
) -> (usize, String) {
    let remaining = &text[pos + len..].as_bytes();
    let mut i = 0;
    // Skip optional comma + whitespace (for "Journal, D7:1888" format)
    if i < remaining.len() && remaining[i] == b',' {
        i += 1;
    }
    while i < remaining.len() && remaining[i] == b' ' {
        i += 1;
    }
    // Single uppercase letter followed immediately by a digit
    if i < remaining.len()
        && remaining[i].is_ascii_uppercase()
        && i + 1 < remaining.len()
        && remaining[i + 1].is_ascii_digit()
    {
        let letter = remaining[i] as char;
        let new_len = len + i + 1;
        let new_abbrev = format!("{} {}", abbrev, letter);
        return (new_len, new_abbrev);
    }
    (len, abbrev)
}

/// Find byte ranges of quoted text (both smart quotes and ASCII quotes).
fn find_quoted_regions(text: &str) -> Vec<(usize, usize)> {
    let mut regions = Vec::new();
    find_quote_pairs(text, '\u{201c}', '\u{201d}', &mut regions);
    find_quote_pairs(text, '\u{201d}', '\u{201d}', &mut regions);
    find_quote_pairs(text, '"', '"', &mut regions);
    regions
}

fn find_quote_pairs(text: &str, open: char, close: char, regions: &mut Vec<(usize, usize)>) {
    let mut search_from = 0;
    while let Some(start) = text[search_from..].find(open) {
        let abs_start = search_from + start;
        let after_open = abs_start + open.len_utf8();
        if let Some(end) = text[after_open..].find(close) {
            let abs_end = after_open + end + close.len_utf8();
            regions.push((abs_start, abs_end));
            search_from = abs_end;
        } else {
            break;
        }
    }
}

fn in_quoted_region(pos: usize, regions: &[(usize, usize)]) -> bool {
    regions.iter().any(|(start, end)| pos >= *start && pos < *end)
}

fn overlaps_existing(spans: &[Span], start: usize, end: usize) -> bool {
    spans
        .iter()
        .any(|s| start < s.end && end > s.start)
}

fn remove_overlapping_spans(spans: &mut Vec<Span>) {
    let mut keep = vec![true; spans.len()];
    for i in 0..spans.len() {
        for j in (i + 1)..spans.len() {
            if spans[i].end > spans[j].start && spans[i].start < spans[j].end {
                // Keep the earlier/longer one
                if spans[i].end - spans[i].start >= spans[j].end - spans[j].start {
                    keep[j] = false;
                } else {
                    keep[i] = false;
                }
            }
        }
    }
    let mut idx = 0;
    spans.retain(|_| {
        let k = keep[idx];
        idx += 1;
        k
    });
}

/// Fill tokens between identifier spans with classified remaining text.
fn fill_tokens(text: &str, spans: &[Span], tokens: &mut Vec<Token>) {
    let mut pos = 0;
    for span in spans {
        if pos < span.start {
            classify_gap(&text[pos..span.start], tokens);
        }
        tokens.push(Token {
            kind: span.kind.clone(),
            text: span.text.clone(),
            normalized: span.normalized.clone(),
        });
        pos = span.end;
    }
    if pos < text.len() {
        classify_gap(&text[pos..], tokens);
    }
}

/// Classify remaining text fragments into words, years, numbers, etc.
fn classify_gap(text: &str, tokens: &mut Vec<Token>) {
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut i = 0;
    while i < words.len() {
        // Re-join broken page ranges: "1547–" + "1553" → "1547–1553"
        // Common in two-column PDFs where "179:1547– 1553" spans a line break
        if i + 1 < words.len()
            && ends_with_dash(words[i])
            && words[i + 1].as_bytes().first().is_some_and(|b| b.is_ascii_digit())
        {
            let joined = format!("{}{}", words[i], words[i + 1]);
            classify_word(&joined, tokens);
            i += 2;
        } else {
            classify_word(words[i], tokens);
            i += 1;
        }
    }
}

fn ends_with_dash(word: &str) -> bool {
    let trimmed = word.trim_end_matches(|c: char| c == ',' || c == '.' || c == ';' || c == ':');
    trimmed.ends_with('-') || trimmed.ends_with('–') || trimmed.ends_with('—')
}

fn classify_word(word: &str, tokens: &mut Vec<Token>) {
    let clean = word.trim_matches(|c: char| c == ',' || c == '.' || c == ';' || c == ':');

    // Compact volume(year)page: "417(1994)181"
    if let Some(caps) = VOLUME_YEAR_PAGE_RE.captures(clean) {
        push_number(tokens, &caps[1]);
        push_year(tokens, &caps[2]);
        push_page_or_number(tokens, &caps[3]);
        return;
    }
    // Volume:page: "70:094505"
    if let Some(caps) = VOLUME_COLON_PAGE_RE.captures(clean) {
        push_number(tokens, &caps[1]);
        push_page_or_number(tokens, &caps[2]);
        return;
    }
    // Compact volume(year): "301(1993)"
    if let Some(caps) = VOLUME_YEAR_RE.captures(clean) {
        push_number(tokens, &caps[1]);
        push_year(tokens, &caps[2]);
        return;
    }
    // Volume with issue number: "82(25)" → emit volume only
    if let Some(caps) = VOLUME_ISSUE_RE.captures(clean) {
        push_number(tokens, &caps[1]);
        return;
    }
    // Article number with suffix: "111301(R)", "040404/1" → emit digits
    if let Some(caps) = ARTICLE_NUMBER_RE.captures(clean) {
        push_number(tokens, &caps[1]);
        return;
    }

    if clean.eq_ignore_ascii_case("ibid") || clean.eq_ignore_ascii_case("ibid.") {
        tokens.push(Token { kind: TokenKind::Ibid, text: word.to_string(), normalized: None });
        return;
    }
    if is_punctuation(word) {
        tokens.push(Token { kind: TokenKind::Punctuation, text: word.to_string(), normalized: None });
        return;
    }
    if let Some(caps) = YEAR_RE.captures(clean) {
        let year: u32 = caps[1].parse().unwrap_or(0);
        if (1900..=2030).contains(&year) {
            tokens.push(Token { kind: TokenKind::Year, text: word.to_string(), normalized: Some(caps[1].to_string()) });
            return;
        }
    }
    if PAGE_RANGE_RE.is_match(clean) {
        tokens.push(Token { kind: TokenKind::PageRange, text: word.to_string(), normalized: None });
        return;
    }
    if NUMBER_RE.is_match(clean) && clean.chars().all(|c| c.is_ascii_digit()) {
        tokens.push(Token { kind: TokenKind::Number, text: word.to_string(), normalized: None });
        return;
    }
    if let Some(collab) = kb::match_collaboration(clean) {
        tokens.push(Token { kind: TokenKind::Collaboration, text: word.to_string(), normalized: Some(collab) });
        return;
    }
    tokens.push(Token { kind: TokenKind::Word, text: word.to_string(), normalized: None });
}

fn push_number(tokens: &mut Vec<Token>, num: &str) {
    tokens.push(Token {
        kind: TokenKind::Number,
        text: num.to_string(),
        normalized: None,
    });
}

fn push_year(tokens: &mut Vec<Token>, year: &str) {
    tokens.push(Token {
        kind: TokenKind::Year,
        text: format!("({year})"),
        normalized: Some(year.to_string()),
    });
}

fn push_page_or_number(tokens: &mut Vec<Token>, page: &str) {
    let kind = if page.contains('-') || page.contains('–') || page.contains('—') {
        TokenKind::PageRange
    } else {
        TokenKind::Number
    };
    tokens.push(Token {
        kind,
        text: page.to_string(),
        normalized: None,
    });
}

fn is_punctuation(word: &str) -> bool {
    let trimmed = word.trim();
    matches!(trimmed, "," | "." | ";" | ":" | "and" | "et" | "al." | "al" | "&" | "-" | "–" | "—")
}
