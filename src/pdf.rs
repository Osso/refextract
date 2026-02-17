use std::path::Path;

use anyhow::{Context, Result};
use pdfium_render::prelude::*;

use crate::types::{PageChars, PdfChar};

/// Load a PDF and extract characters with positions from every page.
pub fn extract_chars(
    pdfium: &Pdfium,
    path: &Path,
    ocr_fallback: bool,
) -> Result<Vec<PageChars>> {
    let document = pdfium
        .load_pdf_from_file(path, None)
        .with_context(|| format!("Failed to load PDF: {}", path.display()))?;

    document
        .pages()
        .iter()
        .enumerate()
        .map(|(idx, page)| extract_page_chars(idx, &page, ocr_fallback))
        .collect()
}

fn extract_page_chars(
    page_idx: usize,
    page: &PdfPage,
    ocr_fallback: bool,
) -> Result<PageChars> {
    let text_page = page
        .text()
        .with_context(|| format!("Failed to load text for page {}", page_idx + 1))?;

    let mut chars: Vec<PdfChar> = text_page
        .chars()
        .iter()
        .filter_map(|ch| convert_text_char(&ch))
        .collect();

    let meaningful_chars = chars.iter().filter(|c| !c.ch.is_whitespace()).count();
    if meaningful_chars < 10 && ocr_fallback {
        match crate::ocr::ocr_page(page, page_idx) {
            Ok(ocr_chars) if ocr_chars.len() > chars.len() => {
                eprintln!("OCR fallback: page {} ({} chars)", page_idx + 1, ocr_chars.len());
                chars = ocr_chars;
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("OCR failed on page {}: {e:#}", page_idx + 1);
            }
        }
    }

    Ok(PageChars {
        page_num: page_idx + 1,
        width: page.width().value,
        height: page.height().value,
        chars,
    })
}

fn convert_text_char(ch: &PdfPageTextChar) -> Option<PdfChar> {
    let unicode = ch.unicode_char()?;
    if unicode.is_control() && unicode != ' ' {
        return None;
    }

    // Skip zero-size font characters (watermarks, hidden text)
    let font_size = ch.scaled_font_size().value;
    if font_size < 0.5 {
        return None;
    }

    let (x, y, width, height) = char_bounds(ch)?;

    Some(PdfChar {
        ch: unicode,
        x,
        y,
        width,
        height,
        font_size,
        font_name: ch.font_name(),
    })
}

fn char_bounds(ch: &PdfPageTextChar) -> Option<(f32, f32, f32, f32)> {
    let rect = ch.loose_bounds().or_else(|_| ch.tight_bounds()).ok()?;
    Some((
        rect.left().value,
        rect.bottom().value,
        (rect.right().value - rect.left().value).abs(),
        (rect.top().value - rect.bottom().value).abs(),
    ))
}
