#!/bin/bash
set -euo pipefail

# Evaluate refextract against INSPIRE ground truth.
# Runs refextract on downloaded PDFs, compares output with INSPIRE metadata.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
PDF_DIR="${PROJECT_DIR}/tests/fixtures/pdfs"
META_DIR="${PROJECT_DIR}/tests/fixtures/metadata"
RESULTS_DIR="${PROJECT_DIR}/tests/fixtures/results"

mkdir -p "$RESULTS_DIR"

LIMIT="${1:-0}"  # Pass a number to limit how many PDFs to process (0 = all)

# Build release binary first
echo "Building refextract..."
cargo build --release --manifest-path="${PROJECT_DIR}/Cargo.toml" 2>&1 | tail -1
REFEXTRACT="${PROJECT_DIR}/target/release/refextract"

# Collect PDFs that need (re-)extraction and the full list for comparison
stale_pdfs=()
all_basenames=()
for pdf in "$PDF_DIR"/*.pdf; do
    basename=$(basename "$pdf" .pdf)
    meta="${META_DIR}/${basename}.json"
    [[ ! -f "$meta" ]] && continue

    all_basenames+=("$basename")
    if (( LIMIT > 0 && ${#all_basenames[@]} > LIMIT )); then
        unset 'all_basenames[-1]'
        break
    fi

    result_file="${RESULTS_DIR}/${basename}.json"
    if [[ ! -f "$result_file" ]] || [[ "$REFEXTRACT" -nt "$result_file" ]]; then
        stale_pdfs+=("$pdf")
    fi
done

# Batch extract stale PDFs in one invocation
extract_errors=0
total_stale=${#stale_pdfs[@]}
if (( total_stale > 0 )); then
    echo "Extracting ${total_stale} papers..."
    extract_errors=$("$REFEXTRACT" --no-doi-lookup "${stale_pdfs[@]}" 2>/dev/null | python3 -c "
import json, sys, os

results_dir = sys.argv[1]
errors = 0
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    d = json.loads(line)
    basename = os.path.splitext(os.path.basename(d['file']))[0]
    result_file = os.path.join(results_dir, basename + '.json')
    if d.get('error'):
        print(f'ERR  {basename} (refextract failed)', file=sys.stderr)
        errors += 1
        try:
            os.unlink(result_file)
        except FileNotFoundError:
            pass
    else:
        with open(result_file, 'w') as f:
            json.dump(d['references'], f)
print(errors)
" "$RESULTS_DIR")
    echo ""
fi

echo "Evaluating against INSPIRE ground truth..."
echo ""

total_papers=0
total_inspire_refs=0
total_extracted_refs=0
total_matched_arxiv=0
total_matched_journal=0
total_matched_doi=0
total_errors=$extract_errors

for basename in "${all_basenames[@]}"; do
    result_file="${RESULTS_DIR}/${basename}.json"
    meta="${META_DIR}/${basename}.json"

    if [[ ! -f "$result_file" ]]; then
        total_errors=$((total_errors + 1))
        continue
    fi

    # Compare with INSPIRE
    eval_line=$(python3 "${SCRIPT_DIR}/compare_refs.py" "$result_file" "$meta" 2>/dev/null) || {
        echo "ERR  ${basename} (compare failed)"
        total_errors=$((total_errors + 1))
        continue
    }

    # Parse: inspire_count extracted_count matched_arxiv matched_journal matched_doi
    read -r i_count e_count m_arxiv m_journal m_doi <<< "$eval_line"

    total_papers=$((total_papers + 1))
    total_inspire_refs=$((total_inspire_refs + i_count))
    total_extracted_refs=$((total_extracted_refs + e_count))
    total_matched_arxiv=$((total_matched_arxiv + m_arxiv))
    total_matched_journal=$((total_matched_journal + m_journal))
    total_matched_doi=$((total_matched_doi + m_doi))

    # Per-paper summary
    if (( i_count > 0 )); then
        recall=$(( (m_arxiv + m_journal + m_doi) * 100 / i_count ))
    else
        recall=0
    fi
    printf "%-30s inspire=%3d  extracted=%3d  matched(a=%d j=%d d=%d)  recall=%d%%\n" \
        "$basename" "$i_count" "$e_count" "$m_arxiv" "$m_journal" "$m_doi" "$recall"
done

echo ""
echo "=== Summary ==="
echo "Papers evaluated:     ${total_papers}"
echo "Errors:               ${total_errors}"
echo "INSPIRE refs total:   ${total_inspire_refs}"
echo "Extracted refs total: ${total_extracted_refs}"
echo ""

if (( total_inspire_refs > 0 )); then
    total_matched=$((total_matched_arxiv + total_matched_journal + total_matched_doi))
    echo "Matched by arXiv ID:  ${total_matched_arxiv}"
    echo "Matched by journal:   ${total_matched_journal}"
    echo "Matched by DOI:       ${total_matched_doi}"
    echo "Total matched:        ${total_matched} / ${total_inspire_refs} ($(( total_matched * 100 / total_inspire_refs ))%)"
fi
