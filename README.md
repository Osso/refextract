# refextract

Extract structured references from HEP (High Energy Physics) PDF papers.

Uses pdfium for layout-aware PDF parsing — extracting text with character positions and font info — then classifies page zones to collect references from both end-of-document sections and per-page footnotes. Reference parsing uses a token-based approach rather than regex.

## Usage

```bash
refextract paper.pdf                    # JSON output
refextract paper.pdf --pretty           # Pretty-printed JSON
refextract paper.pdf --debug-layout     # Show zone classification per page
refextract paper.pdf --no-footnotes     # Skip footnote extraction
refextract --pdfium-path /path/to/libpdfium.so paper.pdf
```

## Output

```json
[
  {
    "raw_ref": "J. D. Bekenstein, \u201cBlack holes and entropy,\u201d Phys. Rev. D 7, 2333 (1973).",
    "linemarker": "1",
    "authors": "J. D. Bekenstein",
    "title": "Black holes and entropy",
    "journal_title": "Phys. Rev. D",
    "journal_volume": "7",
    "journal_year": "1973",
    "journal_page": "2333",
    "source": "ReferenceSection"
  }
]
```

## Requirements

Requires `libpdfium.so` at runtime. Install via:

- **Arch Linux**: Download from [bblanchon/pdfium-binaries](https://github.com/bblanchon/pdfium-binaries/releases) and place in `/usr/local/lib/`
- **Docker**: Use the included Dockerfile (downloads pdfium automatically)
- Set `PDFIUM_LIB_PATH` environment variable or use `--pdfium-path`

## Building

```bash
cargo build --release
```

### Docker

```bash
docker build -t refextract .
docker run --rm -v /path/to/papers:/data refextract /data/paper.pdf --pretty
```

## Knowledge Bases

Includes knowledge bases from the [Python refextract](https://github.com/inspirehep/refextract) project:

- **journal-titles.kb** — 7648 journal name mappings (full name → abbreviation)
- **report-numbers.kb** — 471 report number patterns (CERN, Fermilab, SLAC, etc.)
- **collaborations.kb** — 32 HEP collaboration names (ATLAS, CMS, ALICE, etc.)
- **special-journals.kb** — JHEP/JCAP (year-in-volume handling)

## How It Works

1. **PDF extraction** (`pdf.rs`): Load PDF via pdfium, extract every character with bounding box and font size
2. **Layout grouping** (`layout.rs`): Group characters into words, words into lines, lines into blocks based on spatial proximity. Detects two-column layouts and reorders into reading order
3. **Zone classification** (`zones.rs`): Classify blocks as header, body, footnote, or page number based on position and font size
4. **Reference collection** (`collect.rs`): Find "References" heading, split following text by line markers (`[1]`, `1.`, etc.)
5. **Tokenization** (`tokenizer.rs`): Classify tokens as DOI, arXiv ID, journal name, year, page range, etc.
6. **Parsing** (`parse.rs`): Assign semantic roles (author, title, journal numeration) based on token sequence

## License

MIT
