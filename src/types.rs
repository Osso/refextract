use serde::Serialize;

/// A character extracted from a PDF page with position and font info.
#[derive(Debug, Clone)]
pub struct PdfChar {
    pub ch: char,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub font_size: f32,
    pub font_name: String,
}

/// All characters on a single PDF page.
#[derive(Debug)]
pub struct PageChars {
    pub page_num: usize,
    pub width: f32,
    pub height: f32,
    pub chars: Vec<PdfChar>,
}

/// A word: sequence of characters forming a unit.
#[derive(Debug, Clone)]
pub struct Word {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub font_size: f32,
    pub is_superscript: bool,
}

/// A line of text: sequence of words on the same baseline.
#[derive(Debug, Clone)]
pub struct Line {
    pub words: Vec<Word>,
    pub y: f32,
    pub x_start: f32,
    pub x_end: f32,
    pub font_size: f32,
}

impl Line {
    pub fn text(&self) -> String {
        self.words
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// A block: group of consecutive lines forming a paragraph.
#[derive(Debug, Clone)]
pub struct Block {
    pub lines: Vec<Line>,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub font_size: f32,
}

impl Block {
    pub fn text(&self) -> String {
        self.lines
            .iter()
            .map(|l| l.text())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Zone classification for a block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoneKind {
    Header,
    PageNumber,
    Body,
    Footnote,
    ReferenceHeading,
    ReferenceBody,
}

/// A block with its zone classification.
#[derive(Debug, Clone)]
pub struct ZonedBlock {
    pub block: Block,
    pub zone: ZoneKind,
    pub page_num: usize,
}

/// Where a reference was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ReferenceSource {
    ReferenceSection,
    Footnote,
}

/// A raw reference string before parsing.
#[derive(Debug, Clone)]
pub struct RawReference {
    pub text: String,
    pub linemarker: Option<String>,
    pub source: ReferenceSource,
    pub page_num: usize,
}

/// Token kinds for reference tokenization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Doi,
    ArxivId,
    Isbn,
    Url,
    ReportNumber,
    LineMarker,
    Year,
    Number,
    PageRange,
    JournalName,
    Collaboration,
    Word,
    Punctuation,
    Ibid,
}

/// A token in a reference string.
#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub text: String,
    /// The normalized form (for journal names, report numbers).
    pub normalized: Option<String>,
}

/// A parsed reference ready for JSON output.
#[derive(Debug, Clone, Serialize)]
pub struct ParsedReference {
    pub raw_ref: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linemarker: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authors: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub journal_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub journal_volume: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub journal_year: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub journal_page: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doi: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arxiv_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub isbn: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collaboration: Option<String>,
    pub source: ReferenceSource,
}
