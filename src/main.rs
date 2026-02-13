mod collect;
mod kb;
mod layout;
mod parse;
mod pdf;
mod tokenizer;
mod types;
mod zones;

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use pdfium_render::prelude::*;

use types::ParsedReference;

#[derive(Parser)]
#[command(name = "refextract", about = "Extract references from HEP papers")]
struct Cli {
    /// PDF file to process
    file: PathBuf,

    /// Pretty-print JSON output
    #[arg(long)]
    pretty: bool,

    /// Show zone classification per page (debug)
    #[arg(long)]
    debug_layout: bool,

    /// Skip footnote extraction
    #[arg(long)]
    no_footnotes: bool,

    /// Override pdfium library path
    #[arg(long, env = "PDFIUM_LIB_PATH")]
    pdfium_path: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let pdfium = bind_pdfium(&cli.pdfium_path)?;
    let page_chars = pdf::extract_chars(&pdfium, &cli.file)?;

    let all_blocks = build_page_blocks(&page_chars);
    let body_font_size = zones::compute_body_font_size(&all_blocks);
    let zoned_pages = classify_all_pages(&page_chars, &all_blocks, body_font_size);

    if cli.debug_layout {
        print_debug_layout(&zoned_pages);
        return Ok(());
    }

    let raw_refs = collect::collect_references(&zoned_pages);
    let raw_refs = split_semicolon_subrefs(raw_refs);
    let parsed = parse_all_references(&raw_refs);
    print_output(&parsed, cli.pretty)
}

fn bind_pdfium(pdfium_path: &Option<String>) -> Result<Pdfium> {
    let bindings = if let Some(path) = pdfium_path {
        Pdfium::bind_to_library(path)
            .with_context(|| format!("Failed to load pdfium from: {path}"))?
    } else {
        Pdfium::bind_to_system_library()
            .context("Failed to find pdfium. Install pdfium-binaries or use --pdfium-path")?
    };
    Ok(Pdfium::new(bindings))
}

fn build_page_blocks(page_chars: &[types::PageChars]) -> Vec<Vec<types::Block>> {
    page_chars.iter().map(layout::group_page).collect()
}

fn classify_all_pages(
    page_chars: &[types::PageChars],
    all_blocks: &[Vec<types::Block>],
    body_font_size: f32,
) -> Vec<Vec<types::ZonedBlock>> {
    page_chars
        .iter()
        .zip(all_blocks.iter())
        .map(|(pc, blocks)| {
            zones::classify_page(blocks, pc.page_num, pc.height, body_font_size)
        })
        .collect()
}

fn parse_all_references(
    raw_refs: &[types::RawReference],
) -> Vec<ParsedReference> {
    raw_refs
        .iter()
        .map(|raw| {
            let tokens = tokenizer::tokenize(&raw.text);
            parse::parse_reference(raw, &tokens)
        })
        .collect()
}

/// Split reference entries that contain semicolons into sub-references.
/// In HEP papers, semicolons within a single numbered reference entry
/// typically separate distinct citations (e.g., "[1] Author1; Author2").
fn split_semicolon_subrefs(
    refs: Vec<types::RawReference>,
) -> Vec<types::RawReference> {
    let mut result = Vec::new();
    for raw in refs {
        if !raw.text.contains(';') {
            result.push(raw);
            continue;
        }
        let parts: Vec<&str> = raw.text.split(';').collect();
        if parts.len() <= 1 {
            result.push(raw);
            continue;
        }
        // Only split if sub-parts look like citations
        let subrefs: Vec<&str> = parts
            .iter()
            .map(|p| p.trim())
            .filter(|p| !p.is_empty())
            .collect();
        if subrefs.len() <= 1 {
            result.push(raw);
            continue;
        }
        // Check: at least 2 sub-parts should look like citations
        let citation_count = subrefs.iter().filter(|s| looks_like_citation(s)).count();
        if citation_count < 2 {
            result.push(raw);
            continue;
        }
        for subref in &subrefs {
            result.push(types::RawReference {
                text: subref.to_string(),
                linemarker: raw.linemarker.clone(),
                source: raw.source,
                page_num: raw.page_num,
            });
        }
    }
    result
}

/// Heuristic: does this text fragment look like a citation?
/// Checks for patterns common in HEP references.
fn looks_like_citation(text: &str) -> bool {
    use once_cell::sync::Lazy;
    use regex::Regex;
    static YEAR_RE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"(?:19|20)\d{2}").unwrap());
    static ARXIV_RE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"(?:arXiv|hep-|astro-|gr-qc|cond-mat|nucl-|math-|quant-ph|physics/)").unwrap());

    YEAR_RE.is_match(text)
        || ARXIV_RE.is_match(text)
        || text.contains("doi")
        || text.contains("DOI")
        || text.contains("Preprint")
        || text.contains("preprint")
}

fn print_output(parsed: &[ParsedReference], pretty: bool) -> Result<()> {
    let json = if pretty {
        serde_json::to_string_pretty(parsed)?
    } else {
        serde_json::to_string(parsed)?
    };
    println!("{json}");
    Ok(())
}

fn print_debug_layout(zoned_pages: &[Vec<types::ZonedBlock>]) {
    for page_blocks in zoned_pages {
        for zb in page_blocks {
            let zone_label = format!("{:?}", zb.zone);
            let text = zb.block.text();
            let preview: String = text.chars().take(80).collect();
            println!(
                "p{} [{:<18}] y={:6.1} fs={:4.1} | {}",
                zb.page_num, zone_label, zb.block.y, zb.block.font_size, preview
            );
        }
    }
}
