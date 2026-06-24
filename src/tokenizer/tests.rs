use super::*;

fn kinds(text: &str) -> Vec<TokenKind> {
    tokenize(text).into_iter().map(|token| token.kind).collect()
}

#[test]
fn tokenizes_common_reference_identifiers() {
    let tokens = tokenize(
        "[12] ATLAS Collaboration, Phys. Rev. D 99, 012345 (2020), doi:10.1103/PhysRevD.99.012345, arXiv:2001.12345.",
    );

    assert!(
        tokens
            .iter()
            .any(|t| t.kind == TokenKind::LineMarker && t.text == "12")
    );
    assert!(tokens.iter().any(|t| t.kind == TokenKind::JournalName));
    assert!(tokens.iter().any(|t| t.kind == TokenKind::Doi));
    assert!(tokens.iter().any(|t| t.kind == TokenKind::ArxivId));
    assert!(tokens.iter().any(|t| t.kind == TokenKind::Year));
}

#[test]
fn tokenizes_old_arxiv_url_and_isbn() {
    let tokens = tokenize("See http://arxiv.org/abs/hep-ph/0202089 and ISBN 978-0-521-88068-8.");

    assert!(
        tokens
            .iter()
            .any(|t| t.kind == TokenKind::ArxivId && t.text == "hep-ph/0202089")
    );
    assert!(tokens.iter().any(|t| t.kind == TokenKind::Isbn));
}

#[test]
fn tokenizes_compound_numeration_patterns() {
    assert!(kinds("Phys. Lett. B 417(1994)181").contains(&TokenKind::Year));
    assert!(kinds("Phys. Rev. D 70:094505").contains(&TokenKind::Number));
    assert!(kinds("JHEP 03(2020)001").contains(&TokenKind::Year));
    assert!(kinds("Nucl. Phys. B 72(2):1346-1349").contains(&TokenKind::PageRange));
}

#[test]
fn tokenizes_ibid_and_plain_words() {
    let tokens = tokenize("Ibid. 42, 12 (2020), and followup");

    assert!(tokens.iter().any(|t| t.kind == TokenKind::Ibid));
    assert!(
        tokens
            .iter()
            .any(|t| t.kind == TokenKind::Word && t.text == "followup")
    );
}

#[test]
fn tokenizes_report_numbers_and_bare_old_arxiv() {
    let tokens = tokenize("CERN-TH-2020-001, SLAC-PUB-1234, arXiv:0510213 [hep-ph]");

    assert!(tokens.iter().any(|t| t.kind == TokenKind::ReportNumber));
    assert!(tokens.iter().any(|t| t.kind == TokenKind::ArxivId));
}

#[test]
fn tokenizes_page_ranges_and_article_numbers() {
    let tokens = tokenize("Phys. Rev. D 99, 111301(R), 12-24, 040404/1");

    assert!(tokens.iter().any(|t| t.kind == TokenKind::PageRange));
    assert!(
        tokens
            .iter()
            .filter(|t| t.kind == TokenKind::Number)
            .count()
            >= 2
    );
}

#[test]
fn tokenizes_section_letter_journals() {
    let tokens = tokenize("Nucl. Phys. B253, 15 (1985), Phys. Lett. B 10, 20");

    assert!(tokens.iter().any(|t| {
        t.kind == TokenKind::JournalName && t.normalized.as_deref() == Some("Nucl. Phys. B")
    }));
    assert!(tokens.iter().any(|t| {
        t.kind == TokenKind::JournalName && t.normalized.as_deref() == Some("Phys. Lett. B")
    }));
}

#[test]
fn tokenizes_additional_compact_numeration_forms() {
    for text in [
        "Nucl. Phys. B 301(1993)",
        "JHEP 01(2020)001",
        "Phys. Rev. D 72(2):1346-1349",
        "Phys. Lett. B 10 20-30",
        "Phys. Rev. D 99, 111301(R)",
    ] {
        let tokens = tokenize(text);

        assert!(tokens.iter().any(|t| t.kind == TokenKind::JournalName));
        assert!(tokens.iter().any(|t| matches!(
            t.kind,
            TokenKind::Number | TokenKind::PageRange | TokenKind::Year
        )));
    }
}

#[test]
fn tokenizes_punctuation_and_year_suffixes() {
    let tokens = tokenize("[12] Author et al. (2020a), 12-24.");

    assert!(tokens.iter().any(|t| t.kind == TokenKind::LineMarker));
    assert!(tokens.iter().any(|t| t.kind == TokenKind::Punctuation));
    assert!(tokens.iter().any(|t| t.kind == TokenKind::Year));
    assert!(tokens.iter().any(|t| t.kind == TokenKind::PageRange));
}
