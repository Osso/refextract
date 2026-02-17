# refextract Brief - 2026-02-16

## Active Tasks
- [ ] Various per-paper layout failures — Unnumbered sections, no heading found, chapter-end refs. Each affects 1-5 papers.
- [ ] Image-based PDFs — ~10 old papers. pdfium extracts 0-32 blocks (chart labels only). Unsolvable without OCR.
- [ ] INSPIRE metadata gaps — 2103.01183 (951 missed: DOI-only), 2006.11237 (118: DOI-only), 1905.08669 (112: DOI-only), 1003.3928 (121 refs: empty metadata). Comparison methodology issue, not extraction bug.
- [ ] Context-aware journal validation — Words like `Physics`, `Energy`, `Science` in titles matching as journal names from KB. Need volume/year proximity check to filter false positives.

## Progress This Session (90.5%)
- **Marker scan strategy optimization**: Compare dense vs trailing scan results by marker count instead of short-circuiting on first success. The fallback pipeline previously stopped at first non-empty result; now it evaluates both strategies and picks the one with more markers. Fixes cases like hep-ex_0602035 (5→62 refs). +57 matches.
- **Fallback threshold raised**: Increased TOC false-positive heading threshold from 5 to 10. Reduces spurious references from table-of-contents sections with dense but low-value entries.
- **Bare arXiv format parsing**: Added support for "arXiv:0510213 [hep-ph]" format (colon prefix, category in brackets). Converts to "hep-ph/0510213". Extends arXiv ID extraction.
- **Lowercase journal name matching**: Fixed case-sensitivity in journal matching. KB now matches "npj Quantum Inf." and similar mixed-case journal names. Added UTF-8 char boundary fix during journal span detection.
- **Quantum journal KB entry**: Added "Quantum" journal to KB (common in quantum computing papers).
- **Comparison script improvements**: Fixed annphysleipzig equivalence chain, added Fortschritte journal equivalence for normalization.
- **Investigation: worst-recall papers**: Papers with 2006.11237 (98 refs, 14% recall), 1905.08669 (112 refs, 6%), 1905.08255 (115 refs, 2%), 1911.11977 (127 refs, 6%) are limited by INSPIRE metadata — their refs have DOIs only, no arXiv IDs or journal+volume data in INSPIRE. Not fixable from extraction side.
- **Net gain**: 123,935 → 123,992 (+57 matches, 90.5%)

## Previous Session (90.1% → 90.4%)
- **Ibid/erratum sub-reference extraction**: Recognize `[Erratum-ibid, V, P (Y)]` and `ibid., V, P (Y)` patterns as sub-references inheriting the primary's journal title. Extended tokenizer to match `Erratum-ibid`, `Addendum-ibid`, `Erratum:ibid` as Ibid tokens. +43 matches.
- **Bracket trimming in tokenizer**: Added `[` and `]` to `classify_word` trim set, fixing year detection in `(2012)].` patterns where trailing `]` broke year regex matching.
- **Nature journal KB fixes**: Changed `Nature Nanotechnology` → `Nature Nanotech.` to match INSPIRE abbreviation. Fixed duplicate `NATURE PHOTONICS` entry (had both `Nature Photonics` and `Nature Photon.` outputs).
- **New journal KB entries**: Phys. Rev. Applied, Phys. Rev. Research, Galaxies, additional Nature Rev. Phys. variants.
- **Comparison equivalences**: Added Nat↔Nature prefix mapping for sub-journals (photon, nanotech, commun, electron, revphys, astr, chem). +8 matches.
- **DOI lookup via CrossRef** (user-added): Query CrossRef API to enrich parsed refs with DOIs. SQLite cache at `~/.cache/refextract/doi_cache.db`. Skip with `--no-doi-lookup`.
- **Evaluate script**: Added `--no-doi-lookup` flag to avoid CrossRef latency during evaluation.
- **Deep gap analysis** (proper normalization): Of 13,187 unmatched refs: 45% no_id (5,935), 25% journal_no_raw (3,347), 15% doi_only (1,995), 8% zero_extract (1,091), 3% journal_with_raw (424, actionable), 3% arxiv_only (395, metadata-only).
- **Net gain**: 123,702 → 123,795 (+93 matches, 90.4%)

## Previous Sessions
- `57abff8` — Skip DOI lookup in evaluation
- `209cd30` — DOI lookup via CrossRef with SQLite cache
- `19602da` — Journal KB entries, Nature abbreviation fixes, Erratum:ibid
- `bec7c54` — Ibid/erratum sub-reference extraction, Nature journal KB variants (+43)
- `267a5da` — Old arXiv categories: q-alg, alg-geom, solv-int (+9)
- `99fca27` — arXiv ID extraction from URLs (+20)
- `71311c2` — KB false positive cleanup: ASTRO/ASTRON, SCIEN
- `2f6261e` — Comparison normalization: journal equivalences from arXiv cross-matching (+7)
- `bba4a2b` — Colon separator in journal name normalization (+35 matches, 90.3%→90.4%)
- `33280bd` — Soviet/Russian journal equivalences (+17 journal matches)
- `b650256` — KB cleanup, comparison normalization, journal-requires-volume (90.0%→90.3%)
- `1d81a30` — Tokenizer refactor: section-letter volume:page, try_compound_numeration extraction
- `3be9f6b` — Biblio label splitting, running header tolerance, extended heading verification (90.0%→90.1%)
- `fb24734` — Superscript marker gap tolerance (89.9%→90.0%)
- `c2ae243` — Citation density for dense blocks, eval cache busting (89.4%→89.9%)
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

## Evaluation Results (1000 papers, full run)
```
Papers evaluated:     1,000 (0 errors)
INSPIRE refs total:   136,982
Extracted refs total: 162,592
Matched by arXiv ID:  69,613 (51%)
Matched by journal:   52,029 (38%)
Matched by DOI:        2,421 (2%)
Total matched:        123,992 / 136,982 (90.5%)
```

## Top 15 Missed Papers (at 90.1% recall)
```
Rank  Paper            INSPIRE  Matched  Missed  Recall%  Category
1     2103.01183          1036       85     951     8%  INSPIRE DOI-only metadata
2     0704.3011            575        0     575     0%  pdfium extracts only spaces
3     hep-ph_9506380       421        0     421     0%  Image-based PDF
4     1206.2913            950      654     296    68%  No identifiers (292/296)
5     1909.12524          1636     1342     294    82%  DOI-only in INSPIRE
6     1204.4325            441      179     262    40%  Parsing gaps (layout OK)
7     2204.03381           635      419     216    65%  No identifiers + DOI-only
8     hep-ph_0503172      1205     1008     197    83%  Matching gap (184 no_id)
9     2103.05419          1780     1600     180    89%  Matching gap (165 no_id)
10    1112.2853            483      309     174    63%  No identifiers (165/174)
11    hep-ph_9306320       164        0     164     0%  Image-based PDF
12    1406.6311           1783     1630     153    91%  Extraction gap (improved)
13    0802.0007            122        0     122     0%  pdfium extracts only spaces
14    1003.3928            121        0     121     0%  INSPIRE metadata empty
15    1902.00134          1051      932     119    88%  Matching gap (118 no_id)
```

## Gap Analysis (~13,187 unmatched INSPIRE refs)
- **No identifiers in INSPIRE** (5,935 / 45%): Refs with no arXiv, DOI, or journal+volume. Fundamentally unmatchable.
- **Journal without raw text** (3,347 / 25%): INSPIRE has journal+vol metadata but no raw_ref text — published-version-only entries not in arXiv PDF.
- **DOI-only in INSPIRE** (1,995 / 15%): INSPIRE has only DOI, no journal or arXiv. Mostly editorial additions.
- **Zero-extraction papers** (1,091 / 8%): Image-based or Type3 font PDFs where pdfium extracts no usable text.
- **Journal with raw text** (424 / 3%): **Only actionable category.** INSPIRE has journal+vol and raw text. Spread thinly: Phys.Rev.D (20), Phys.Lett.B (20), Nucl.Phys.B (16), PoS (15), Eur.Phys.J.C (14). Edge cases in formatting.
- **ArXiv-only in INSPIRE** (395 / 3%): INSPIRE has arXiv ID but no raw text — record-linking metadata, not in PDF.
- **Theoretical ceiling** (~94.7%): Only 424 refs are genuinely fixable through extraction improvements.

## 0-Recall Paper Categories (14 papers with >10 INSPIRE refs)
1. **pdfium text extraction failure** (3 papers): 0704.3011 (575 refs), 0802.0007 (122), 0711.3596 (55). pdfium extracts only spaces from reference pages (Type3 fonts or outlined text). Characters are NOT interleaved — they arrive per-column. The layout pipeline handles two-column correctly via `split_columns`. Unsolvable without alternative text extraction (e.g., poppler/pdftotext).
2. **Image-based/Type3 font PDFs** (~7 papers): hep-ph_9506380, hep-ph_9306320, hep-th_9411108, hep-ph_9903282, hep-ph_9507378, hep-lat_9605038, hep-lat_9609035, hep-lat_9309005, hep-lat_9310022, hep-lat_9308011. pdfium extracts <10 blocks from 30-50 page papers. Unsolvable without OCR.
3. **INSPIRE metadata empty** (2 papers): 1003.3928 (121 refs), 1310.7534 (35 refs). Extraction works, nothing to match against.
4. **Other** (2 papers): hep-ex_0012035 (12 refs), 1102.1523 (4 refs).

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
- **Dense block validation**: Standard: marker count >= 3 AND citation score >= 4. OR citation density: ≥20 citation lines AND ≥60% of lines are citations.
- **Running header rejection**: `is_heading_text` rejects numeric prefix/suffix with 2+ digits (page numbers) but accepts 0-1 digits (section numbers).
- **Marker-based stop**: `gather_subsequent_pages` stops after 2 consecutive markerless pages.
- **JCAP volume**: Keep faithful extraction ("0904"); normalize only in comparison script.
- **Dots as word separators**: In `normalize_abbrev` and `find_original_byte_len`, dots are treated identically to spaces for journal name matching.
- **Semicolon split guard**: Only split when 2+ sub-parts look like citations (have years/arXiv IDs/DOIs).
- **No-period initials**: `AUTHOR_START_RE` accepts `[A-Z]` followed by comma/space (not just `[A-Z]\.`). Matches "Abe, T," style used by Rev. Mod. Phys. and similar journals.
- **No-comma author pattern**: `AUTHOR_START_NOCOMMA_RE` matches "Surname I." format (e.g., "Abrahams E."). Requires 3+ lowercase chars in surname to avoid matching journal abbreviations.
- **Lower splitting thresholds**: Blob splitting triggers at 200 chars (was 500) and 2 parts (was 3). Safe because `is_ref_boundary` check prevents false splits.
- **Superscript gap tolerance**: `find_superscript_pairs` allows 30 consecutive non-ref/non-citation blocks before breaking. Extended notes in references no longer prematurely stop the backward scan.
- **Bibliography label splitting**: `find_biblio_label_positions` detects "Surname et al. YYYY:" patterns as split positions in author-year blobs. Scans backward from year-colon to find label start, validates with `is_ref_boundary`. Handles hyphenated names, "et al.", connectors (and/de/von).
- **Running header tolerance**: `gather_subsequent_pages` no longer stops at standalone "References" headings. Instead sets `saw_heading` flag and only stops if the page also has ref content (marker-based or citation density). Prevents false stop on running headers at top of appendix pages.
- **Extended heading verification**: `has_refs_after` checks up to 3 subsequent pages (not just 1) with per-page 15-block limit. Handles appendix pages between heading and ref continuation.
- **No character-level column reorder**: pdfium delivers chars per-column (left then right, 1-2 switches per page), NOT interleaved. Character reordering was tested and caused -12 net regressions by disrupting reference section block boundaries. The existing `split_columns` at line level is sufficient.
- **Backward-jump word break**: Break word when `(ch.x + ch.width) < acc.x` — the new char's right edge is left of the word start. Protects against occasional backward x-jumps without false positives on subscripts.
- **Marker scan comparison**: Dense vs trailing scan fallback now compares final marker counts instead of stopping at first non-empty result. Evaluates both strategies before picking best.
- **Fallback threshold increased to 10**: Prevents table-of-contents sections with dense but low-quality ref-like entries from being misclassified.
- **Bare arXiv format support**: "arXiv:0510213 [hep-ph]" → "hep-ph/0510213" conversion for papers using colon-prefixed format with category suffix.
- **Lowercase journal matching**: Fixed case-sensitivity in journal KB lookup; now matches mixed-case names like "npj Quantum Inf.". Added UTF-8 char boundary safety in journal span detection.
- **Quantum journal KB entry**: Added standalone "Quantum" journal entry for quantum computing papers.

## Performance Profile
Per-paper timing (1303.4571, 104 pages):
- pdfium char extraction: 402ms (17%)
- layout + zones: 21ms (1%)
- collect + parse (incl. KB init): 1,988ms (82%)
- **Bottleneck**: Report number KB regex compilation (~500ms after optimization, was ~1.5s)
- Each eval invocation re-initializes KB (Lazy static per process). Batch mode would amortize.

## Commits
- `5fab80f` — Parse bare arXiv format, fix lowercase journal matching, add Quantum KB (+71)
- `06e176c` — Compare marker scan strategies by count instead of short-circuiting (+57)
- `81d6d76` — Detect reference headings with parenthesized number ranges (+37)
- `e2b47c3` — Suppress false-positive journal matches for common English words, add Sci. Adv. KB entry
- `b92c177` — Resolve ibid journal references from semicolon-split sub-refs (+98)
- `c56fdf8` — Update README with two-column layout detection
- `6cf38fb` — Add backward-jump word break in layout (+5)
- `57abff8` — Skip DOI lookup in evaluation (--no-doi-lookup)
- `209cd30` — DOI lookup via CrossRef with SQLite cache
- `19602da` — Journal KB entries, Nature abbreviation fixes, Erratum:ibid support (+8)
- `bec7c54` — Ibid/erratum sub-reference extraction, Nature journal KB variants (+43)
- `267a5da` — Old arXiv categories (+9)
- `99fca27` — arXiv ID extraction from URLs (+20)
- `71311c2` — KB false positive cleanup: ASTRO/ASTRON, SCIEN
- `2f6261e` — Journal equivalences in comparison (+7)
- `bba4a2b` — Colon separator in journal name normalization (+35)
- `33280bd` — Soviet/Russian journal equivalences (+17)
- `b650256` — KB cleanup, comparison normalization, journal-requires-volume (90.0%→90.3%)
- `1d81a30` — Tokenizer refactor: section-letter volume:page, try_compound_numeration extraction (+74 journal matches)
- `3be9f6b` — Biblio label splitting, running header tolerance, extended heading verification (90.0%→90.1%)
- `fb24734` — Superscript marker gap tolerance (89.9%→90.0%)
- `c2ae243` — Citation density for dense blocks, eval cache busting (89.4%→89.9%)
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
1. **Investigate medium-recall papers (60-80%)** — Diminishing returns but potentially fixable. Gap analysis shows only 424 of 13,187 unmatched refs are genuinely fixable (journal+volume in INSPIRE with raw text). Most low-recall papers are INSPIRE metadata limited.
2. **Context-aware journal validation** — Words like `Physics`, `Energy`, `Science` in titles match as journal names from KB. Need volume/year proximity check to filter false positives.
3. **Prefix trie** for report number matching (skip most patterns without regex)
4. **Note**: OCR task deferred. Estimated 750 refs from Type3/outlined PDFs but adds tesseract runtime dependency.

## Technical Context

### Project Location
`~/Projects/cli/refextract/` — Rust CLI, edition 2024

### Key Source Files
- `src/kb.rs` — KB loading, journal matching (dots-as-spaces normalization, `is_journal_boundary`), report number patterns
- `src/tokenizer.rs` — Reference tokenization, compound numeration (volume:page, volume(year)page, volume(issue), article(suffix)), journal/arXiv/DOI span detection, section letter extension (incl. comma-separated), broken page range re-join, anchored year detection, arXiv old-style normalization
- `src/layout.rs` — Column detection: `split_columns()`, `detect_column_boundary()`
- `src/collect.rs` — `RefHeadingLoc`, `find_all_reference_headings`, `has_refs_after` (citation scoring, 15-block limit), `gather_subsequent_pages`, `detect_marker_format` (peek-ahead), `is_standalone_ref_heading`, fallback marker collection (dense + trailing), `score_citation_block`, `has_citation_content`, `is_valid_trailing_cluster`
- `src/markers.rs` — `collect_refs_by_markers` (3-strategy fallback: dense blocks → trailing scan → superscript pairs), `is_dense_ref_block` (marker count OR citation density), `find_superscript_pairs` (gap-tolerant backward scan), author-date blob splitting
- `src/zones.rs` — `is_heading_text` (running header rejection, prefix/suffix number handling, colon stripping)
- `src/parse.rs` — Token-based parser, multi-journal sub-ref extraction with position-based arXiv/DOI assignment, arXiv-only sub-refs, ibid/erratum sub-refs, old-style volume splitting ("249B"), conference volume parsing
- `src/doi.rs` — DOI lookup via CrossRef bibliographic API, SQLite cache
- `src/main.rs` — CLI, pipeline orchestration, `split_semicolon_subrefs`
- `scripts/compare_refs.py` — Comparison (flexible journal/volume matching, abbreviation normalization, DOI matching)
- `scripts/evaluate.sh` — Evaluation orchestrator (caches results in `tests/fixtures/results/`, invalidates on binary change)

### pdfium
- Using bblanchon/pdfium-binaries (chromium/7678) at `/usr/local/lib/libpdfium.so`
- AUR pdfium (7428) incompatible — missing `FPDFPageObjMark_GetParamFloatValue`
- Must pass `--pdfium-path /usr/local/lib/libpdfium.so` (default search finds AUR version first)

### Test Data (all in .gitignore)
- PDFs: `tests/fixtures/pdfs/` (1000 downloaded)
- Metadata: `tests/fixtures/metadata/` (INSPIRE JSON ground truth)
- Results: `tests/fixtures/results/` (cached refextract output)
- Download: `scripts/download-papers.sh` via nohup
