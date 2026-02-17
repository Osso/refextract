use std::io::Cursor;

use anyhow::{Context, Result};
use image::ImageFormat;
use leptess::LepTess;
use pdfium_render::prelude::*;

use crate::types::PdfChar;

const DPI: f32 = 300.0;
const MIN_CONFIDENCE: i32 = 40;

/// Check if tesseract is available (eng traineddata exists).
pub fn tesseract_available() -> bool {
    LepTess::new(None, "eng").is_ok()
}

/// OCR a single PDF page: render to bitmap, run tesseract, return PdfChars.
pub fn ocr_page(page: &PdfPage, page_idx: usize) -> Result<Vec<PdfChar>> {
    let bitmap = render_page(page, page_idx)?;
    let dynamic_image = bitmap.as_image();
    let gray = dynamic_image.to_luma8();
    let tiff_bytes = encode_tiff(&gray)?;
    let words = run_tesseract(&tiff_bytes)?;
    let page_height_px = bitmap.height() as f32;
    let page_height_pt = page.height().value;
    Ok(words_to_chars(&words, page_height_px, page_height_pt))
}

fn render_page<'a>(page: &'a PdfPage, page_idx: usize) -> Result<PdfBitmap<'a>> {
    let scale = DPI / 72.0;
    let config = PdfRenderConfig::new().scale_page_by_factor(scale);
    page.render_with_config(&config)
        .map_err(|e| anyhow::anyhow!("Failed to render page {} for OCR: {e}", page_idx + 1))
}

fn encode_tiff(gray: &image::GrayImage) -> Result<Vec<u8>> {
    let mut buf = Cursor::new(Vec::new());
    gray.write_to(&mut buf, ImageFormat::Tiff)
        .context("Failed to encode page as TIFF for OCR")?;
    Ok(buf.into_inner())
}

struct OcrWord {
    text: String,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

fn run_tesseract(tiff_bytes: &[u8]) -> Result<Vec<OcrWord>> {
    let mut lt = LepTess::new(None, "eng").context("Failed to init tesseract")?;
    lt.set_image_from_mem(tiff_bytes)
        .map_err(|_| anyhow::anyhow!("Failed to load image into tesseract"))?;

    let boxes = lt
        .get_component_boxes(leptess::capi::TessPageIteratorLevel_RIL_WORD, true)
        .ok_or_else(|| anyhow::anyhow!("Tesseract returned no component boxes"))?;

    let mut words = Vec::new();
    for b in &boxes {
        let geo = b.get_geometry();
        lt.set_rectangle(geo.x, geo.y, geo.w, geo.h);
        let conf = lt.mean_text_conf();
        if conf < MIN_CONFIDENCE {
            continue;
        }
        let text = match lt.get_utf8_text() {
            Ok(t) => t.trim().to_string(),
            Err(_) => continue,
        };
        if text.is_empty() {
            continue;
        }
        words.push(OcrWord {
            text,
            x: geo.x,
            y: geo.y,
            w: geo.w,
            h: geo.h,
        });
    }
    Ok(words)
}

/// Convert OCR words (pixel coords) to PdfChar entries (PDF points).
/// PDF coordinate system: origin at bottom-left, y increases upward.
/// Tesseract: origin at top-left, y increases downward.
fn words_to_chars(
    words: &[OcrWord],
    page_height_px: f32,
    page_height_pt: f32,
) -> Vec<PdfChar> {
    let scale = page_height_pt / page_height_px;
    let mut chars = Vec::new();

    for word in words {
        let char_count = word.text.chars().count();
        if char_count == 0 {
            continue;
        }
        let char_w_px = word.w as f32 / char_count as f32;
        let h_pt = word.h as f32 * scale;
        let font_size = h_pt; // approximate

        for (i, ch) in word.text.chars().enumerate() {
            let px_x = word.x as f32 + i as f32 * char_w_px;
            let px_y = word.y as f32;
            let x_pt = px_x * scale;
            // Flip y: PDF origin is bottom-left
            let y_pt = page_height_pt - (px_y + word.h as f32) * scale;
            let w_pt = char_w_px * scale;

            chars.push(PdfChar {
                ch,
                x: x_pt,
                y: y_pt,
                width: w_pt,
                height: h_pt,
                font_size,
                font_name: "OCR".to_string(),
            });
        }

        // Add space after each word
        let last_x_px = word.x as f32 + word.w as f32;
        chars.push(PdfChar {
            ch: ' ',
            x: last_x_px * scale,
            y: page_height_pt - (word.y as f32 + word.h as f32) * scale,
            width: char_w_px * scale,
            height: h_pt,
            font_size,
            font_name: "OCR".to_string(),
        });
    }

    chars
}
