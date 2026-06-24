use crate::types::{ParsedReference, RawReference, Token, TokenKind};

/// Parse a raw reference into one or more structured ParsedReferences.
/// When a single reference string contains multiple journal citations
/// (e.g., "Phys. Rev. D72, 052002. ... Phys. Rev. D72, 052008."),
/// produce a sub-reference for each additional journal citation.
pub fn parse_references(raw: &RawReference, tokens: &[Token]) -> Vec<ParsedReference> {
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
    // A journal name without a volume is almost always a false positive
    // (word like "Science" or "Computing" in a title). Clear it.
    if result.journal_title.is_some() && result.journal_volume.is_none() {
        result.journal_title = None;
    }
    // Standalone ibid ref (from semicolon splitting): extract numeration
    // after the Ibid token. Journal will be resolved later by caller.
    if result.journal_title.is_none() {
        extract_standalone_ibid(tokens, &mut result);
    }
    extract_authors(tokens, &mut result);

    let mut refs = vec![result.clone()];
    refs.extend(extract_sub_references(raw, tokens, &result));
    refs
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
                result.report_number = Some(token.normalized.clone().unwrap_or(token.text.clone()));
            }
            TokenKind::Url if result.url.is_none() => {
                result.url = Some(token.text.clone());
            }
            TokenKind::Collaboration if result.collaboration.is_none() => {
                result.collaboration = Some(token.normalized.clone().unwrap_or(token.text.clone()));
            }
            _ => {}
        }
    }
}

/// Walk tokens to find journal name + numeration (volume, year, page).
fn extract_journal_info(tokens: &[Token], result: &mut ParsedReference) {
    let journal_pos = tokens.iter().position(|t| t.kind == TokenKind::JournalName);

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

/// Handle standalone ibid refs (e.g., "ibid. 94 (1954) 7") from semicolon
/// splitting. Extract numeration after the Ibid token and mark journal as
/// "ibid" placeholder for later resolution.
fn extract_standalone_ibid(tokens: &[Token], result: &mut ParsedReference) {
    let ibid_pos = tokens.iter().position(|t| t.kind == TokenKind::Ibid);
    let Some(ipos) = ibid_pos else { return };
    let window = &tokens[ipos + 1..];
    assign_numeration(window, result);
    if result.journal_volume.is_some() {
        result.journal_title = Some("ibid".to_string());
    }
}

/// Try to extract volume from a Word token (letter-prefixed, old-style, conference).
fn try_word_as_volume(token: &Token, result: &mut ParsedReference) -> bool {
    if let Some(vol) = extract_letter_prefixed_number(&token.text) {
        result.journal_volume = Some(vol);
        return true;
    }
    if let Some((vol, letter)) = extract_old_style_volume(&token.text) {
        result.journal_volume = Some(vol);
        append_section_letter(result, letter);
        return true;
    }
    if let Some((vol, page)) = extract_conference_volume(&token.text) {
        result.journal_volume = Some(vol);
        if let Some(p) = page
            && result.journal_page.is_none()
        {
            result.journal_page = Some(p);
        }
        return true;
    }
    false
}

fn assign_year_token(
    token: &Token,
    next: Option<&Token>,
    volume_found: &mut bool,
    result: &mut ParsedReference,
) {
    let is_bare = !token.text.starts_with('(');
    let next_is_number = next.is_some_and(|t| t.kind == TokenKind::Number);

    if !*volume_found && result.journal_volume.is_none() && is_bare && next_is_number {
        result.journal_year = token.normalized.clone().or(Some(token.text.clone()));
        return;
    }
    if !*volume_found && result.journal_volume.is_none() && is_bare {
        let year_text = token.normalized.as_deref().unwrap_or(&token.text);
        result.journal_volume = Some(year_text.to_string());
        *volume_found = true;
        return;
    }
    if result.journal_year.is_none() {
        result.journal_year = token.normalized.clone().or(Some(token.text.clone()));
    }
}

fn assign_numeration(window: &[Token], result: &mut ParsedReference) {
    let mut volume_found = false;
    let tokens: Vec<&Token> = window.iter().take(8).collect();
    for (i, token) in tokens.iter().enumerate() {
        if should_stop_numeration_scan(token) {
            break;
        }
        if token.kind == TokenKind::Year {
            assign_year_token(token, tokens.get(i + 1).copied(), &mut volume_found, result);
            continue;
        }
        if assign_volume_token(token, result, &mut volume_found) {
            continue;
        }
        assign_page_token(token, result, volume_found);
    }
}

fn should_stop_numeration_scan(token: &Token) -> bool {
    matches!(
        token.kind,
        TokenKind::JournalName | TokenKind::Doi | TokenKind::ArxivId
    )
}

fn assign_volume_token(
    token: &Token,
    result: &mut ParsedReference,
    volume_found: &mut bool,
) -> bool {
    if *volume_found || result.journal_volume.is_some() {
        return false;
    }
    match token.kind {
        TokenKind::Number => {
            let clean = token.text.trim_matches(|c: char| !c.is_ascii_digit());
            result.journal_volume = Some(clean.to_string());
            *volume_found = true;
            true
        }
        TokenKind::PageRange => {
            let clean = token
                .text
                .trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '–');
            result.journal_volume = Some(clean.to_string());
            *volume_found = true;
            true
        }
        TokenKind::Word => {
            *volume_found = try_word_as_volume(token, result);
            *volume_found
        }
        _ => false,
    }
}

fn assign_page_token(token: &Token, result: &mut ParsedReference, volume_found: bool) {
    if result.journal_page.is_some() {
        return;
    }
    match token.kind {
        TokenKind::PageRange => {
            let clean = token
                .text
                .trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '–');
            result.journal_page = Some(clean.to_string());
        }
        TokenKind::Number if volume_found => {
            let clean = token.text.trim_matches(|c: char| !c.is_ascii_digit());
            result.journal_page = Some(clean.to_string());
        }
        TokenKind::Word if volume_found => {
            if let Some(page) = extract_letter_prefixed_number(&token.text) {
                result.journal_page = Some(page);
            }
        }
        _ => {}
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
        if letter_count >= 2
            && conf.len() == letter_count + 4
            && conf[letter_count..].chars().all(|c| c.is_ascii_digit())
            && !page.is_empty()
            && page.chars().all(|c| c.is_ascii_digit())
        {
            return Some((conf.to_string(), Some(page.to_string())));
        }
    }
    let letter_count = clean.bytes().take_while(|b| b.is_ascii_uppercase()).count();
    if letter_count >= 2
        && clean.len() == letter_count + 4
        && clean[letter_count..].chars().all(|c| c.is_ascii_digit())
    {
        Some((clean.to_string(), None))
    } else {
        None
    }
}

/// Old-style volume with trailing section letter: "249B" → ("249", 'B')
/// Used in older citations like "Phys. Lett. 249B (1990) 543".
fn extract_old_style_volume(text: &str) -> Option<(String, char)> {
    let clean = text.trim_matches(|c: char| c == ',' || c == '.' || c == ';' || c == ':');
    // Digits followed by a single uppercase letter (A-D for journal sections)
    if clean.len() >= 2 {
        let last = *clean.as_bytes().last().unwrap();
        if matches!(last, b'A' | b'B' | b'C' | b'D')
            && clean[..clean.len() - 1].chars().all(|c| c.is_ascii_digit())
        {
            let volume = clean[..clean.len() - 1].to_string();
            return Some((volume, last as char));
        }
    }
    None
}

/// Append a section letter to the journal title if it doesn't already have one.
fn append_section_letter(result: &mut ParsedReference, letter: char) {
    if let Some(ref title) = result.journal_title {
        // Only append if journal doesn't already end with a section letter
        let last = title.as_bytes().last().copied().unwrap_or(0);
        if !last.is_ascii_uppercase() {
            result.journal_title = Some(format!("{} {}", title, letter));
        }
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

/// Extract additional ParsedReferences from subsequent JournalName tokens
/// and from arXiv IDs not covered by any journal segment.
///
/// When a single numbered reference contains multiple citations, each journal
/// citation and each standalone arXiv ID becomes its own sub-reference.
/// Identifiers (arXiv, DOI) are assigned by position rather than inherited.
fn extract_sub_references(
    raw: &RawReference,
    tokens: &[Token],
    primary: &ParsedReference,
) -> Vec<ParsedReference> {
    let journal_positions: Vec<usize> = tokens
        .iter()
        .enumerate()
        .filter(|(_, t)| t.kind == TokenKind::JournalName)
        .map(|(i, _)| i)
        .collect();

    let mut used_arxiv_positions: Vec<usize> = Vec::new();
    let mut sub_refs = extract_journal_sub_refs(
        raw,
        tokens,
        primary,
        &journal_positions,
        &mut used_arxiv_positions,
    );

    // Mark the primary's arXiv position as used
    let primary_seg_end = journal_positions.get(1).copied().unwrap_or(tokens.len());
    if let Some(pos) = arxiv_position_in_range(tokens, 0, primary_seg_end) {
        used_arxiv_positions.push(pos);
    }

    sub_refs.extend(extract_ibid_sub_refs(raw, tokens, primary));

    sub_refs.extend(extract_arxiv_only_sub_refs(
        raw,
        tokens,
        primary,
        &used_arxiv_positions,
    ));
    sub_refs
}

/// Create sub-references for each journal citation after the first.
fn extract_journal_sub_refs(
    raw: &RawReference,
    tokens: &[Token],
    primary: &ParsedReference,
    journal_positions: &[usize],
    used_arxiv: &mut Vec<usize>,
) -> Vec<ParsedReference> {
    if journal_positions.len() < 2 {
        return Vec::new();
    }
    let mut sub_refs = Vec::new();
    for &jpos in &journal_positions[1..] {
        let next_journal = journal_positions
            .iter()
            .find(|&&p| p > jpos)
            .copied()
            .unwrap_or(tokens.len());

        if let Some(pos) = arxiv_position_in_range(tokens, jpos, next_journal) {
            used_arxiv.push(pos);
        }

        let mut sub = make_sub_ref(raw, primary, &tokens[jpos]);
        sub.arxiv_id = find_token_in_range(tokens, jpos, next_journal, TokenKind::ArxivId);
        sub.doi = find_token_in_range(tokens, jpos, next_journal, TokenKind::Doi);

        let window_end = next_journal.min(jpos + 9);
        assign_numeration(&tokens[jpos + 1..window_end], &mut sub);

        if sub.journal_volume.is_some() {
            sub_refs.push(sub);
        }
    }
    sub_refs
}

/// Create sub-references for ibid citations (errata, addenda).
/// "Phys. Rev. C 84, 024617 (2011) [Erratum-ibid. 85, 029901 (2012)]"
/// produces a sub-ref with the same journal, different volume/page/year.
fn extract_ibid_sub_refs(
    raw: &RawReference,
    tokens: &[Token],
    primary: &ParsedReference,
) -> Vec<ParsedReference> {
    let Some(ref journal) = primary.journal_title else {
        return Vec::new();
    };
    // Skip placeholder — standalone ibid refs are handled by extract_standalone_ibid
    if journal == "ibid" {
        return Vec::new();
    }

    let mut sub_refs = Vec::new();
    for (i, token) in tokens.iter().enumerate() {
        if token.kind != TokenKind::Ibid {
            continue;
        }
        // Create a sub-ref with the primary's journal
        let mut sub = ParsedReference {
            raw_ref: raw.text.clone(),
            linemarker: raw.linemarker.clone(),
            authors: primary.authors.clone(),
            title: None,
            journal_title: Some(journal.clone()),
            journal_volume: None,
            journal_year: None,
            journal_page: None,
            doi: None,
            arxiv_id: None,
            isbn: None,
            report_number: None,
            url: None,
            collaboration: primary.collaboration.clone(),
            source: raw.source,
        };
        let window_end = (i + 9).min(tokens.len());
        assign_numeration(&tokens[i + 1..window_end], &mut sub);
        if sub.journal_volume.is_some() {
            sub_refs.push(sub);
        }
    }
    sub_refs
}

/// Create sub-references for arXiv IDs not covered by any journal segment.
fn extract_arxiv_only_sub_refs(
    raw: &RawReference,
    tokens: &[Token],
    primary: &ParsedReference,
    used_arxiv: &[usize],
) -> Vec<ParsedReference> {
    tokens
        .iter()
        .enumerate()
        .filter(|(i, t)| t.kind == TokenKind::ArxivId && !used_arxiv.contains(i))
        .map(|(_, t)| {
            let mut sub = make_sub_ref(raw, primary, t);
            sub.journal_title = None;
            sub.arxiv_id = Some(t.text.clone());
            sub.authors = None;
            sub
        })
        .collect()
}

fn make_sub_ref(
    raw: &RawReference,
    primary: &ParsedReference,
    journal_token: &Token,
) -> ParsedReference {
    ParsedReference {
        raw_ref: raw.text.clone(),
        linemarker: raw.linemarker.clone(),
        authors: primary.authors.clone(),
        title: None,
        journal_title: journal_token
            .normalized
            .clone()
            .or_else(|| Some(journal_token.text.clone())),
        journal_volume: None,
        journal_year: None,
        journal_page: None,
        doi: None,
        arxiv_id: None,
        isbn: None,
        report_number: None,
        url: None,
        collaboration: primary.collaboration.clone(),
        source: raw.source,
    }
}

fn find_token_in_range(
    tokens: &[Token],
    start: usize,
    end: usize,
    kind: TokenKind,
) -> Option<String> {
    tokens[start..end]
        .iter()
        .find(|t| t.kind == kind)
        .map(|t| t.text.clone())
}

fn arxiv_position_in_range(tokens: &[Token], start: usize, end: usize) -> Option<usize> {
    tokens[start..end]
        .iter()
        .enumerate()
        .find(|(_, t)| t.kind == TokenKind::ArxivId)
        .map(|(i, _)| start + i)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenizer::tokenize;
    use crate::types::ReferenceSource;

    fn raw(text: &str) -> RawReference {
        RawReference {
            text: text.to_string(),
            linemarker: Some("1".to_string()),
            source: ReferenceSource::ReferenceSection,
            page_num: 3,
        }
    }

    fn parse(text: &str) -> ParsedReference {
        let raw = raw(text);
        let tokens = tokenize(text);
        parse_references(&raw, &tokens).remove(0)
    }

    #[test]
    fn parses_identifiers_and_journal_fields() {
        let parsed = parse(
            "[1] A. Author, Phys. Rev. D 99, 012345 (2020), doi:10.1103/PhysRevD.99.012345, arXiv:2001.12345.",
        );

        assert_eq!(parsed.linemarker.as_deref(), Some("1"));
        assert_eq!(parsed.journal_title.as_deref(), Some("Phys. Rev. D"));
        assert_eq!(parsed.journal_volume.as_deref(), Some("99"));
        assert_eq!(parsed.journal_year.as_deref(), Some("2020"));
        assert_eq!(parsed.doi.as_deref(), Some("10.1103/PhysRevD.99.012345"));
        assert_eq!(parsed.arxiv_id.as_deref(), Some("2001.12345"));
    }

    #[test]
    fn clears_journal_name_without_volume() {
        let parsed = parse("[1] Computing methods and Science notes, arXiv:2001.12345.");

        assert_eq!(parsed.journal_title, None);
        assert_eq!(parsed.arxiv_id.as_deref(), Some("2001.12345"));
    }

    #[test]
    fn creates_sub_references_for_multiple_journals() {
        let raw =
            raw("[1] A. Author, Phys. Rev. D 72, 052002 (2005); Phys. Rev. D 72, 052008 (2005).");
        let tokens = tokenize(&raw.text);

        let parsed = parse_references(&raw, &tokens);

        assert!(parsed.len() >= 2);
        assert!(
            parsed
                .iter()
                .any(|r| r.journal_page.as_deref() == Some("052008"))
        );
    }

    #[test]
    fn parses_report_number_and_isbn() {
        let parsed = parse("[1] CERN-TH-2020-001, ISBN 978-0-521-88068-8.");

        assert!(parsed.report_number.is_some());
        assert!(parsed.isbn.is_some());
    }

    #[test]
    fn parses_url() {
        let parsed = parse("[1] CMS Collaboration, https://example.org/paper.");

        assert_eq!(parsed.url.as_deref(), Some("https://example.org/paper."));
    }

    #[test]
    fn parses_old_arxiv_identifier() {
        let parsed = parse("[1] A. Author, arXiv:hep-ph/0202089.");

        assert_eq!(parsed.arxiv_id.as_deref(), Some("hep-ph/0202089"));
    }

    #[test]
    fn parses_ibid_numeration_without_journal() {
        let parsed = parse("[2] Ibid. 100, 222222 (2021).");

        assert_eq!(parsed.journal_volume.as_deref(), Some("100"));
        assert_eq!(parsed.journal_page.as_deref(), Some("222222"));
        assert_eq!(parsed.journal_year.as_deref(), Some("2021"));
    }

    #[test]
    fn creates_sub_references_for_extra_arxiv_ids() {
        let raw = raw(
            "[1] A. Author, Phys. Rev. D 72, 052002 (2005), arXiv:1111.22222, arXiv:3333.44444.",
        );
        let tokens = tokenize(&raw.text);

        let parsed = parse_references(&raw, &tokens);

        assert!(
            parsed
                .iter()
                .any(|r| r.arxiv_id.as_deref() == Some("3333.44444"))
        );
    }

    #[test]
    fn parses_title_between_quotes() {
        let parsed = parse("[1] A. Author, \"A quoted title\", Phys. Rev. D 10, 20 (1999).");

        assert_eq!(parsed.title.as_deref(), Some("A quoted title"));
        assert_eq!(parsed.authors.as_deref(), Some("A. Author"));
    }

    #[test]
    fn parses_short_jhep_reference() {
        let parsed = parse("[1] ATLAS Collaboration, JHEP 01, 001 (2020).");

        assert_eq!(
            parsed.journal_title.as_deref(),
            Some("J. High Energy Phys.")
        );
        assert_eq!(parsed.journal_volume.as_deref(), Some("01"));
    }

    #[test]
    fn parses_page_range_as_volume_then_page() {
        let parsed = parse("[1] A. Author, Phys. Lett. B 10, 20-30 (1980).");

        assert_eq!(parsed.journal_volume.as_deref(), Some("10"));
        assert_eq!(parsed.journal_page.as_deref(), Some("20-30"));
        assert_eq!(parsed.journal_year.as_deref(), Some("1980"));
    }
}
