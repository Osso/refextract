use crate::types::{ParsedReference, RawReference, Token, TokenKind};

/// Parse a raw reference into a structured ParsedReference.
pub fn parse_reference(raw: &RawReference, tokens: &[Token]) -> ParsedReference {
    let mut result = ParsedReference {
        raw_ref: raw.text.clone(),
        linemarker: raw.linemarker.clone(),
        authors: None,
        title: None,
        journal_title: None,
        journal_volume: None,
        journal_year: None,
        journal_page: None,
        doi: None,
        arxiv_id: None,
        isbn: None,
        report_number: None,
        url: None,
        collaboration: None,
        source: raw.source,
    };

    extract_identifiers(tokens, &mut result);
    extract_journal_info(tokens, &mut result);
    extract_authors(tokens, &mut result);

    result
}

fn extract_identifiers(tokens: &[Token], result: &mut ParsedReference) {
    for token in tokens {
        match &token.kind {
            TokenKind::Doi if result.doi.is_none() => {
                result.doi = Some(token.text.clone());
            }
            TokenKind::ArxivId if result.arxiv_id.is_none() => {
                result.arxiv_id = Some(token.text.clone());
            }
            TokenKind::Isbn if result.isbn.is_none() => {
                result.isbn = Some(token.text.clone());
            }
            TokenKind::ReportNumber if result.report_number.is_none() => {
                result.report_number =
                    Some(token.normalized.clone().unwrap_or(token.text.clone()));
            }
            TokenKind::Url if result.url.is_none() => {
                result.url = Some(token.text.clone());
            }
            TokenKind::Collaboration if result.collaboration.is_none() => {
                result.collaboration =
                    Some(token.normalized.clone().unwrap_or(token.text.clone()));
            }
            _ => {}
        }
    }
}

/// Walk tokens to find journal name + numeration (volume, year, page).
fn extract_journal_info(tokens: &[Token], result: &mut ParsedReference) {
    let journal_pos = tokens
        .iter()
        .position(|t| t.kind == TokenKind::JournalName);

    let Some(jpos) = journal_pos else {
        extract_standalone_year(tokens, result);
        return;
    };

    result.journal_title = tokens[jpos]
        .normalized
        .clone()
        .or_else(|| Some(tokens[jpos].text.clone()));

    // Scan tokens after journal name for volume, year, page
    let window = &tokens[jpos + 1..];
    assign_numeration(window, result);

    if result.journal_year.is_none() {
        extract_standalone_year(tokens, result);
    }
}

fn assign_numeration(window: &[Token], result: &mut ParsedReference) {
    let mut volume_found = false;
    for token in window.iter().take(8) {
        match &token.kind {
            TokenKind::Number if !volume_found && result.journal_volume.is_none() => {
                let clean = token.text.trim_matches(|c: char| !c.is_ascii_digit());
                result.journal_volume = Some(clean.to_string());
                volume_found = true;
            }
            // Bare year (no parens) as first token: treat as volume.
            // Standard format is "Journal Vol, Page (Year)" — the first number
            // after a journal name is always the volume. JHEP/JCAP use year-based
            // volumes like "2006" that look like years but are volumes.
            // Parenthesized years like "(2006)" are clearly year indicators.
            TokenKind::Year if !volume_found && result.journal_volume.is_none()
                && !token.text.starts_with('(') =>
            {
                let year_text = token.normalized.as_deref().unwrap_or(&token.text);
                result.journal_volume = Some(year_text.to_string());
                volume_found = true;
            }
            TokenKind::Year if result.journal_year.is_none() => {
                result.journal_year =
                    token.normalized.clone().or(Some(token.text.clone()));
            }
            TokenKind::PageRange if result.journal_page.is_none() => {
                let clean = token.text.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '–');
                result.journal_page = Some(clean.to_string());
            }
            TokenKind::Number if volume_found && result.journal_page.is_none() => {
                let clean = token.text.trim_matches(|c: char| !c.is_ascii_digit());
                result.journal_page = Some(clean.to_string());
            }
            // Section-letter + digits as volume: "D60", "A534", "B272"
            // Also conference identifiers: "LAT2005", "LATTICE2019", "HEP2005"
            TokenKind::Word if !volume_found && result.journal_volume.is_none() => {
                if let Some(vol) = extract_letter_prefixed_number(&token.text) {
                    result.journal_volume = Some(vol);
                    volume_found = true;
                } else if let Some((vol, page)) = extract_conference_volume(&token.text) {
                    result.journal_volume = Some(vol);
                    volume_found = true;
                    if let Some(p) = page {
                        if result.journal_page.is_none() {
                            result.journal_page = Some(p);
                        }
                    }
                }
            }
            // Letter-prefixed page: "B962", "L85", "R183"
            TokenKind::Word if volume_found && result.journal_page.is_none() => {
                if let Some(page) = extract_letter_prefixed_number(&token.text) {
                    result.journal_page = Some(page);
                }
            }
            TokenKind::JournalName | TokenKind::Doi | TokenKind::ArxivId => break,
            _ => {}
        }
    }
}

/// Extract conference identifier as volume: "LAT2005" → ("LAT2005", None)
/// Also handles compound "LAT2006:022" → ("LAT2006", Some("022"))
/// Requires 2+ uppercase letters followed by 4 digits (year).
fn extract_conference_volume(text: &str) -> Option<(String, Option<String>)> {
    let clean = text.trim_matches(|c: char| c == ',' || c == '.' || c == ';');
    // Check for conference:page compound (e.g., "LAT2006:022")
    if let Some((conf, page)) = clean.split_once(':') {
        let letter_count = conf.bytes().take_while(|b| b.is_ascii_uppercase()).count();
        if letter_count >= 2 && conf.len() == letter_count + 4
            && conf[letter_count..].chars().all(|c| c.is_ascii_digit())
            && !page.is_empty() && page.chars().all(|c| c.is_ascii_digit())
        {
            return Some((conf.to_string(), Some(page.to_string())));
        }
    }
    let letter_count = clean.bytes().take_while(|b| b.is_ascii_uppercase()).count();
    if letter_count >= 2 && clean.len() == letter_count + 4
        && clean[letter_count..].chars().all(|c| c.is_ascii_digit())
    {
        Some((clean.to_string(), None))
    } else {
        None
    }
}

/// Extract digits from letter-prefixed number: "D60" → "60", "B962" → "962", "L85" → "85"
fn extract_letter_prefixed_number(text: &str) -> Option<String> {
    let clean = text.trim_matches(|c: char| c == ',' || c == '.' || c == ';' || c == ':');
    if clean.len() >= 2
        && clean.as_bytes()[0].is_ascii_uppercase()
        && clean[1..].chars().all(|c| c.is_ascii_digit())
    {
        Some(clean[1..].to_string())
    } else {
        None
    }
}

fn extract_standalone_year(tokens: &[Token], result: &mut ParsedReference) {
    if result.journal_year.is_some() {
        return;
    }
    if let Some(yt) = tokens.iter().find(|t| t.kind == TokenKind::Year) {
        result.journal_year = yt.normalized.clone().or(Some(yt.text.clone()));
    }
}

/// Extract authors and title from the raw reference text.
/// Authors are text before the first quoted title or journal/identifier.
/// Title is text within quotes.
fn extract_authors(tokens: &[Token], result: &mut ParsedReference) {
    // Use raw_ref to extract quoted title and author text before it
    extract_title_from_raw(&result.raw_ref.clone(), result);

    let mut author_words = Vec::new();
    for token in tokens {
        if is_author_terminator(token) {
            break;
        }
        if token.kind == TokenKind::LineMarker {
            continue;
        }
        // Stop at opening quote (smart or ASCII or right-quote used as open)
        if token.text.contains('\u{201c}')
            || token.text.contains('\u{201d}')
            || token.text.contains('"')
        {
            break;
        }
        author_words.push(token.text.as_str());
    }
    let author_text = author_words.join(" ");
    let author_text = author_text.trim().trim_end_matches(',').trim();
    if !author_text.is_empty() && author_text.len() > 2 {
        result.authors = Some(author_text.to_string());
    }
}

fn is_author_terminator(token: &Token) -> bool {
    matches!(
        token.kind,
        TokenKind::JournalName
            | TokenKind::Doi
            | TokenKind::ArxivId
            | TokenKind::ReportNumber
            | TokenKind::Year
            | TokenKind::Number
            | TokenKind::PageRange
            | TokenKind::Ibid
    )
}

fn extract_title_from_raw(raw: &str, result: &mut ParsedReference) {
    // Try various quote patterns (PDFs use inconsistent quoting)
    let title = extract_between_quotes(raw, '\u{201c}', '\u{201d}')
        .or_else(|| extract_between_quotes(raw, '\u{201d}', '\u{201d}'))
        .or_else(|| extract_between_quotes(raw, '"', '"'));
    if let Some(t) = title {
        let t = t.trim().trim_end_matches(',').trim();
        if !t.is_empty() {
            result.title = Some(t.to_string());
        }
    }
}

fn extract_between_quotes(text: &str, open: char, close: char) -> Option<String> {
    let start = text.find(open)? + open.len_utf8();
    let end = text[start..].find(close)? + start;
    Some(text[start..end].to_string())
}
