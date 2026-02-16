# refextract Brief - 2026-02-15

## Active Tasks
- [ ] Two-column layout support (~1,300 refs from 17 zero-extraction papers) — Several high-impact papers (1204.4325, 2004.03543, 1710.01833) have interleaved text from adjacent columns, garbling extracted references. Biggest structural challenge.
- [ ] Various per-paper layout failures — Unnumbered sections, bare superscript markers, no heading found, chapter-end refs. Each affects 1-5 papers.
- [ ] Image-based PDFs — ~10 old papers. pdfium extracts 0-32 blocks (chart labels only). Unsolvable without OCR.
- [ ] INSPIRE metadata gaps — 2103.01183 (951 missed: DOI-only), 2006.11237 (118: DOI-only), 1905.08669 (112: DOI-only). Comparison methodology issue, not extraction bug.
- [ ] Context-aware journal validation — Words like `Physics`, `Energy`, `Science` in titles matching as journal names from KB. Need volume/year proximity check to filter false positives.

## Progress This Session
- **Author-year marker recognition**: Added `[Author+Year]` pattern to `LINE_MARKER_RE` group 4. Handles [Aal+12], [ABG14], [ATL14a], [CMS15c] style markers. 1507.00966 went from one 13,221-char blob to properly split refs (+236 matched)
- **Leading-zero volume matching**: `volumes_match("04", "4")` now returns true. Handles JCAP/JHEP volumes with/without leading zeros (+121 matched)
- **Comparison normalizations**: cambridge→camb, hadronicj→hadronj equivalence (+11 matched)
- **Column detection**: Increased histogram from 100→200 buckets, lowered minimum gap from 2→1 bucket (+250 matched, previous sub-session)
- **Line-number heading prefix**: Multi-digit prefixes accepted when followed by separator (+~15 matched, previous sub-session)
- **Net gain**: 89.1% → 89.4% (+368 matched refs this sub-session)

## Previous Sessions
- `cc47929` — Author-year markers, leading-zero volumes, KB normalizations (89.1%→89.4%)
- `fd793d1` — Column detection: finer histogram, line-number heading prefix (88.9%→89.1%)
- `63d0e11` — Multi-ref splitting, old-style volumes, KB/normalization (86.1%→88.9%)
- `1a58236` — PageRange-as-volume for combined volume numbers (86%→86.1%)
- `e0315cf` — Add default pdfium library paths
- `9c4d773` — Volume:page tokenizer, false-positive KB cleanup (86%→86.1%)
- `9016e17` — Author-date splitting: no-comma, no-period initials (85%→86%)
- `bac6f19` — KB additions: Chin.Phys.C, Nature Commun., JCAP variants, PTEP (+115 matches)
- `8ccc821` — Fallback marker scan, PoS journal support, bare year as volume (84%→85%)
- `3accd92` — 4-digit markers, marker peek-ahead, standalone heading check (80%→84% recall)
- `6f9339e` — Volume/page parsing, arXiv normalization, journal boundary (79%→80%)
- `2c7a3a2` — Author-date ref boundary, FirstName format, has_refs_after limit (77%→79%)
- `1cbdc98` — Author-date reference splitting, citation-density page continuation (74.6%→77%)
- `1395eb9` — Comma section letter, broken page range, year anchoring, comparison normalization (73%→74.6%)
- `8462fee` — Multi-heading collection, trailing scan cluster-reset, suffix heading numbers (72%→73%)
- `763d0f1` — Heading verification, decimal marker fix, trailing block collection (67%→72%)
- `90aab82` — Compact numeration, journal normalization, semicolon splitting (62%→67%)
- `ead1678` — Fix heading detection, DOI extraction, over-extraction, regex startup
- `a61d21c` — Fix journal matching: normalize full names, word boundaries, section letters
- `a4c5f4f` — Add two-column layout support, evaluation pipeline, download scripts
- `65b1a65` — Initial implementation of layout-aware HEP reference extractor

## Evaluation Results (1000 papers, full run)
```
Papers evaluated:     1,000 (0 errors)
INSPIRE refs total:   136,982
Extracted refs total: 155,862
Matched by arXiv ID:  68,227 (50%)
Matched by journal:   51,780 (38%)
Matched by DOI:        2,421 (2%)
Total recall:         122,428 / 136,982 (89.4%)
```
Previous: 89.1% (122,060). +368 net matched refs this sub-session.

## Top 15 Missed Papers (at 88.9% recall)
```
Rank  Paper            INSPIRE  Matched  Missed  Recall%  Category
1     2103.01183          1036       85     951     8%  INSPIRE DOI-only metadata
2     2105.05208          1923     1156     767    60%  Two-column + author-date
3     0704.3011            575        0     575     0%  No ref heading found
4     hep-ph_9506380       421        0     421     0%  Image-based PDF
5     1406.6311           1783     1458     325    81%  Two-column layout
6     1204.4325            441      138     303    31%  Two-column interleaving
7     1206.2913            950      654     296    68%  Matching failures
8     1909.12524          1636     1342     294    82%  DOI-only in INSPIRE
9     hep-ph_0603175       418      182     236    43%  Chapter-end refs (PYTHIA)
10    2204.03381           635      419     216    65%  Matching failures
11    hep-ph_0503172      1205     1007     198    83%  Matching gap
12    2103.05419          1780     1600     180    89%  Good recall, matching gap
13    1112.2853            483      309     174    63%  Under-extraction
14    hep-ph_9306320       164        0     164     0%  Image-based PDF
15    0802.0007            122        0     122     0%  Zero extraction
```

## Gap Analysis (~14,554 unmatched INSPIRE refs)
- **No identifiers in INSPIRE** (5,951 / 41%): INSPIRE refs with no arXiv, DOI, or journal+volume. Fundamentally unmatchable.
- **DOI-only in INSPIRE** (1,996 / 14%): DOIs not present in PDF text. INSPIRE added editorially.
- **Journal not in extracted** (3,457 / 24%): INSPIRE has journal but we don't have it in extracted refs. Mostly from extraction gaps (two-column, 0-extraction papers), not KB misses.
- **Journal volume mismatch** (2,544 / 17%): Same journal extracted but different volume. Mostly refs not extracted at all (the journal appears in OTHER extracted refs). True volume format mismatches are a small fraction.
- **ArXiv not in extracted** (613 / 4%): INSPIRE arXiv IDs not found in extracted data.

## 0-Recall Paper Categories (~28 papers)
1. **Image-based PDFs** (~10 papers): Old (1993-1999) papers where pdfium extracts 0-32 blocks. Unsolvable without OCR.
2. **Unnumbered author-date format** (~5 papers): "References" heading found but refs use "Author (Year)" format with no numbered markers. Need author-name splitting.
3. **Bare superscript markers** (~3 papers): Refs use bare numbers in small font (Nature/Science style). Need new marker pattern.
4. **No reference heading** (~3 papers): 0704.3011, 0802.0007, 1003.3928. No "References" or "Bibliography" heading in PDF.
5. **INSPIRE metadata gaps** (~4 papers): Extraction works but INSPIRE has DOIs only. Comparison issue.
6. **Other** (~3 papers): Non-standard formats.

## Key Decisions
- **4-digit markers**: `[N]` and `(N)` allow `\d{1,4}` for review papers. Bare `N.` stays at `\d{1,3}` — 4-digit bare numbers like `2024.` would match years.
- **Marker format peek-ahead**: When heading page has no content blocks, peek at next page to detect markers. Prevents false author-date classification.
- **Standalone heading stop**: Only stop page gathering at ≤2-line heading blocks. Large blocks starting with "References" followed by ref text should be collected, not treated as section boundaries.
- **Volume(issue) handling**: Strip issue number from `82(25)` format, emit only volume. Issue numbers are not used for reference matching.
- **Article number suffix stripping**: `111301(R)` → `111301`. The `(R)` Rapid Communication suffix is not part of the page/article number for matching.
- **Letter-prefixed numeration**: In `assign_numeration` context only. `D60` as a Word after a JournalName → volume=60. Same for pages. Not applied globally.
- **Journal period boundary**: A period at the end of a journal abbreviation is a word boundary, even when immediately followed by digits. Safe because journal names are abbreviations ending in periods.
- **arXiv space normalization**: Spaces between category parts (`hep ph`) normalized to hyphens (`hep-ph`). Common PDF text extraction artifact.
- **Comma section letter**: Skip `, ` before section letter in `extend_section_letter`. Safe because it requires `[A-Z]\d` after comma.
- **Broken page range**: Join words where word-ending dash meets digit-starting next word. Handles line-break splits in page ranges.
- **Year anchoring**: Full-word match for years prevents article numbers from being misclassified. `[a-z]?` suffix allows astronomy-style `1999a`.
- **Comparison normalization**: Strip all non-alphanumeric rather than just dots/spaces. Comprehensive abbreviation normalization bridge between KB abbreviations and INSPIRE forms.
- **Citation-based heading verification**: `has_refs_after()` uses citation content scoring instead of marker detection. Marker + citation = 2pts, citation only = 1pt, need >= 4 to accept.
- **Multi-heading collection**: Collect from ALL verified headings, not just the first. Handles multi-chapter documents.
- **Trailing cluster validation**: Mid-scan clusters need 5+ markers AND 3+ citation lines. Final cluster needs 5+ markers only.
- **Decimal protection**: `N.` marker variant requires `(?:\s|$)` after the dot to avoid matching "0.01" etc.
- **Dense block validation**: Both marker count >= 3 AND citation score >= 4 required for dense block detection.
- **Running header rejection**: `is_heading_text` rejects numeric prefix/suffix with 2+ digits (page numbers) but accepts 0-1 digits (section numbers).
- **Marker-based stop**: `gather_subsequent_pages` stops after 2 consecutive markerless pages.
- **JCAP volume**: Keep faithful extraction ("0904"); normalize only in comparison script.
- **Dots as word separators**: In `normalize_abbrev` and `find_original_byte_len`, dots are treated identically to spaces for journal name matching.
- **Semicolon split guard**: Only split when 2+ sub-parts look like citations (have years/arXiv IDs/DOIs).
- **No-period initials**: `AUTHOR_START_RE` accepts `[A-Z]` followed by comma/space (not just `[A-Z]\.`). Matches "Abe, T," style used by Rev. Mod. Phys. and similar journals.
- **No-comma author pattern**: `AUTHOR_START_NOCOMMA_RE` matches "Surname I." format (e.g., "Abrahams E."). Requires 3+ lowercase chars in surname to avoid matching journal abbreviations.
- **Lower splitting thresholds**: Blob splitting triggers at 200 chars (was 500) and 2 parts (was 3). Safe because `is_ref_boundary` check prevents false splits.

## Performance Profile
Per-paper timing (1303.4571, 104 pages):
- pdfium char extraction: 402ms (17%)
- layout + zones: 21ms (1%)
- collect + parse (incl. KB init): 1,988ms (82%)
- **Bottleneck**: Report number KB regex compilation (~500ms after optimization, was ~1.5s)
- Each eval invocation re-initializes KB (Lazy static per process). Batch mode would amortize.

## Commits
- `cc47929` — Author-year markers, leading-zero volumes, KB normalizations (89.1%→89.4%)
- `fd793d1` — Column detection: finer histogram, line-number heading prefix (88.9%→89.1%)
- `63d0e11` — Multi-ref splitting, old-style volumes, KB/normalization (86.1%→88.9%)
- `1a58236` — PageRange-as-volume for combined volume numbers (86%→86.1%)
- `e0315cf` — Add default pdfium library paths
- `9c4d773` — Volume:page tokenizer, false-positive KB cleanup (86%→86.1%)
- `9016e17` — Author-date splitting: no-comma, no-period initials (85%→86%)
- `bac6f19` — KB additions: Chin.Phys.C, Nature Commun., JCAP variants, PTEP (+115 matches)
- `8ccc821` — Fallback marker scan, PoS journal support, bare year as volume (84%→85%)
- `3accd92` — 4-digit markers, marker peek-ahead, standalone heading check (80%→84%)
- `6f9339e` — Volume/page parsing, arXiv normalization, journal boundary (79%→80%)
- `2c7a3a2` — Author-date ref boundary, FirstName format, has_refs_after limit (77%→79%)
- `1cbdc98` — Author-date reference splitting, citation-density page continuation (74.6%→77%)
- `1395eb9` — Comma section letter, broken page range, year anchoring, comparison normalization (73%→74.6%)
- `8462fee` — Multi-heading collection, trailing scan cluster-reset, suffix heading numbers (72%→73%)
- `763d0f1` — Heading verification, decimal marker fix, trailing block collection (67%→72%)
- `90aab82` — Compact numeration, journal normalization, semicolon splitting (62%→67%)
- `ead1678` — Fix heading detection, DOI extraction, over-extraction, regex startup
- `a61d21c` — Fix journal matching: normalize full names, word boundaries, section letters
- `a4c5f4f` — Add two-column layout support, evaluation pipeline, download scripts
- `65b1a65` — Initial implementation of layout-aware HEP reference extractor

## Next Steps (by estimated impact)
1. **Two-column layout support** (~1,300 refs from 17 zero-extraction papers) — biggest structural challenge, requires column deinterleaving in layout.rs
2. **Per-paper layout failures** — unnumbered sections, bare superscript markers, no heading, chapter-end refs. Each affects 1-5 papers.
3. **Context-aware journal validation** (require volume/year near journal match to filter false positives)
4. **Batch mode** to amortize KB init cost
5. **Prefix trie** for report number matching (skip most patterns without regex)

## Technical Context

### Project Location
`~/Projects/cli/refextract/` — Rust CLI, edition 2024

### Key Source Files
- `src/kb.rs` — KB loading, journal matching (dots-as-spaces normalization, `is_journal_boundary`), report number patterns
- `src/tokenizer.rs` — Reference tokenization, compound numeration (volume:page, volume(year)page, volume(issue), article(suffix)), journal/arXiv/DOI span detection, section letter extension (incl. comma-separated), broken page range re-join, anchored year detection, arXiv old-style normalization
- `src/layout.rs` — Column detection: `split_columns()`, `detect_column_boundary()`
- `src/collect.rs` — `RefHeadingLoc`, `find_all_reference_headings`, `has_refs_after` (citation scoring, 15-block limit), `gather_subsequent_pages`, `detect_marker_format` (peek-ahead), `is_standalone_ref_heading`, fallback marker collection (dense + trailing), `score_citation_block`, `has_citation_content`, `is_valid_trailing_cluster`
- `src/zones.rs` — `is_heading_text` (running header rejection, prefix/suffix number handling, colon stripping)
- `src/parse.rs` — Token-based parser, multi-journal sub-ref extraction with position-based arXiv/DOI assignment, arXiv-only sub-refs, old-style volume splitting ("249B"), conference volume parsing
- `src/main.rs` — CLI, pipeline orchestration, `split_semicolon_subrefs`
- `scripts/compare_refs.py` — Comparison (flexible journal/volume matching, abbreviation normalization, DOI matching)
- `scripts/evaluate.sh` — Evaluation orchestrator (caches results in `tests/fixtures/results/`)

### pdfium
- Using bblanchon/pdfium-binaries (chromium/7678) at `/usr/local/lib/libpdfium.so`
- AUR pdfium (7428) incompatible — missing `FPDFPageObjMark_GetParamFloatValue`
- Must pass `--pdfium-path /usr/local/lib/libpdfium.so` (default search finds AUR version first)

### Test Data (all in .gitignore)
- PDFs: `tests/fixtures/pdfs/` (1000 downloaded)
- Metadata: `tests/fixtures/metadata/` (INSPIRE JSON ground truth)
- Results: `tests/fixtures/results/` (cached refextract output)
- Download: `scripts/download-papers.sh` via nohup
