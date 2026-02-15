# refextract

Layout-aware reference extractor for HEP (High Energy Physics) papers. Rust replacement for the Python `refextract` library used by INSPIRE.

## Architecture

Pipeline: PDF → chars → words/lines/blocks → zone classification → reference collection → tokenization → parsing → JSON

```
src/
  main.rs       -- CLI (clap), pipeline orchestration
  pdf.rs        -- PDF loading via pdfium-render, char extraction with positions
  layout.rs     -- Char → word → line → block grouping
  zones.rs      -- Page zone classification (header, body, footnote, refs)
  collect.rs    -- Reference collection from ref-section + footnotes
  tokenizer.rs  -- Tokenize reference string into semantic tokens
  parse.rs      -- Token-based parser: assign roles (author, title, journal, etc.)
  kb.rs         -- Knowledge base loading (compile-time embedded via include_str!)
  types.rs      -- Data structures
kbs/            -- Knowledge bases copied from Python refextract
```

## Dependencies

- **pdfium-render** — PDF text extraction. Requires `libpdfium.so` at runtime.
- System pdfium from AUR (`pdfium-binaries-bin`) is incompatible — use bblanchon/pdfium-binaries.
- Compatible binary installed at `/usr/local/lib/libpdfium.so`.

## Running

```bash
cargo run --bin refextract -- <file.pdf> --pretty --pdfium-path /usr/local/lib/libpdfium.so
# Or set PDFIUM_LIB_PATH=/usr/local/lib/libpdfium.so
```

## Testing

Test PDFs go in `tests/fixtures/` (gitignored). Download from arXiv:
```bash
curl -sL -o tests/fixtures/test.pdf "https://arxiv.org/pdf/1001.0785"
```

Verify against INSPIRE API ground truth:
```
https://inspirehep.net/api/literature?q=arxiv:1001.0785&fields=references
```

## Known Limitations

- Journal abbreviation matching: some false positives on short journal names (e.g., "Physics")
- False positives are NOT acceptable — every false-positive KB entry must be investigated and removed
- Two-column layout support not yet implemented
- Footnote citation extraction is basic (heuristic-based)
