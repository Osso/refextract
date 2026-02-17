mod collect;
mod doi;
mod kb;
mod layout;
mod markers;
mod parse;
mod pdf;
mod tokenizer;
mod types;
mod zones;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Parser;
use pdfium_render::prelude::*;
use serde::Serialize;

use types::ParsedReference;

#[derive(Parser)]
#[command(name = "refextract", about = "Extract references from HEP papers")]
struct Cli {
    /// PDF file(s) to process
    files: Vec<PathBuf>,

    /// Pretty-print JSON output
    #[arg(long)]
    pretty: bool,

    /// Show zone classification per page (debug)
    #[arg(long)]
    debug_layout: bool,

    /// Skip footnote extraction
    #[arg(long)]
    no_footnotes: bool,

    /// Skip DOI lookup via CrossRef
    #[arg(long)]
    no_doi_lookup: bool,

    /// Override pdfium library path
    #[arg(long, env = "PDFIUM_LIB_PATH")]
    pdfium_path: Option<String>,
}

#[derive(Serialize)]
struct BatchResult {
    file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    references: Option<Vec<ParsedReference>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if cli.files.is_empty() {
        anyhow::bail!("No input files specified");
    }
    let pdfium = bind_pdfium(&cli.pdfium_path)?;
    let batch = cli.files.len() > 1;

    // Force KB initialization upfront (amortize ~500ms regex compilation).
    let _ = (&*kb::JOURNAL_TITLES, &*kb::JOURNAL_ABBREVS, &*kb::REPORT_NUMBERS);

    let doi_cache = if !cli.no_doi_lookup {
        Some(doi::DoiCache::open()?)
    } else {
        None
    };

    if batch {
        run_batch(&pdfium, &cli, &doi_cache)
    } else {
        run_single(&pdfium, &cli, &doi_cache)
    }
}

fn run_single(pdfium: &Pdfium, cli: &Cli, doi_cache: &Option<doi::DoiCache>) -> Result<()> {
    if cli.debug_layout {
        let page_chars = pdf::extract_chars(pdfium, &cli.files[0])?;
        let all_blocks = build_page_blocks(&page_chars);
        let body_font_size = zones::compute_body_font_size(&all_blocks);
        let zoned_pages = classify_all_pages(&page_chars, &all_blocks, body_font_size);
        print_debug_layout(&zoned_pages);
        return Ok(());
    }

    let parsed = process_pdf(pdfium, &cli.files[0], doi_cache)?;
    print_output(&parsed, cli.pretty)
}

fn run_batch(pdfium: &Pdfium, cli: &Cli, doi_cache: &Option<doi::DoiCache>) -> Result<()> {
    let total = cli.files.len();
    for (i, file) in cli.files.iter().enumerate() {
        eprint!("\r[{}/{}] {}", i + 1, total, file.display());

        let result = match process_pdf(pdfium, file, doi_cache) {
            Ok(refs) => BatchResult {
                file: file.display().to_string(),
                references: Some(refs),
                error: None,
            },
            Err(e) => BatchResult {
                file: file.display().to_string(),
                references: None,
                error: Some(format!("{e:#}")),
            },
        };
        println!("{}", serde_json::to_string(&result)?);
    }
    eprintln!();
    Ok(())
}

fn process_pdf(
    pdfium: &Pdfium,
    file: &Path,
    doi_cache: &Option<doi::DoiCache>,
) -> Result<Vec<ParsedReference>> {
    let page_chars = pdf::extract_chars(pdfium, file)?;
    let all_blocks = build_page_blocks(&page_chars);
    let body_font_size = zones::compute_body_font_size(&all_blocks);
    let zoned_pages = classify_all_pages(&page_chars, &all_blocks, body_font_size);
    let raw_refs = collect::collect_references(&zoned_pages);
    let raw_refs = split_semicolon_subrefs(raw_refs);
    let mut parsed = parse_all_references(&raw_refs);
    resolve_ibid_journals(&mut parsed);
    if let Some(cache) = doi_cache {
        doi::enrich_dois(&mut parsed, cache);
    }
    Ok(parsed)
}

const DEFAULT_PDFIUM_PATHS: &[&str] = &[
    "/usr/local/lib/libpdfium.so",
    "/usr/lib/libpdfium.so",
    "/usr/local/lib/libpdfium.dylib",
    "/usr/lib/libpdfium.dylib",
];

fn bind_pdfium(pdfium_path: &Option<String>) -> Result<Pdfium> {
    let bindings = if let Some(path) = pdfium_path {
        Pdfium::bind_to_library(path)
            .with_context(|| format!("Failed to load pdfium from: {path}"))?
    } else if let Ok(bindings) = Pdfium::bind_to_system_library() {
        bindings
    } else {
        try_default_pdfium_paths()?
    };
    Ok(Pdfium::new(bindings))
}

fn try_default_pdfium_paths() -> Result<Box<dyn PdfiumLibraryBindings>> {
    for path in DEFAULT_PDFIUM_PATHS {
        if let Ok(bindings) = Pdfium::bind_to_library(path) {
            return Ok(bindings);
        }
    }
    anyhow::bail!(
        "Failed to find pdfium. Searched system library path and {:?}. \
         Use --pdfium-path or set PDFIUM_LIB_PATH.",
        DEFAULT_PDFIUM_PATHS
    )
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
        .flat_map(|raw| {
            let tokens = tokenizer::tokenize(&raw.text);
            parse::parse_references(raw, &tokens)
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

/// Resolve ibid placeholders from semicolon-split references.
/// When parse.rs finds a standalone "ibid. V, P (Y)" ref, it sets
/// journal_title to "ibid". Here we replace that with the actual journal
/// from the nearest prior ref with the same linemarker.
fn resolve_ibid_journals(refs: &mut [ParsedReference]) {
    for i in 1..refs.len() {
        if refs[i].journal_title.as_deref() != Some("ibid") {
            continue;
        }
        let linemarker = &refs[i].linemarker;
        for j in (0..i).rev() {
            if refs[j].linemarker != *linemarker {
                continue;
            }
            match refs[j].journal_title.as_deref() {
                Some("ibid") | None => continue,
                Some(_) => {
                    refs[i].journal_title = refs[j].journal_title.clone();
                    break;
                }
            }
        }
    }
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
