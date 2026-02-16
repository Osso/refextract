#!/bin/bash
set -euo pipefail

# Evaluate refextract against INSPIRE ground truth.
# Runs refextract on downloaded PDFs, compares output with INSPIRE metadata.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
PDF_DIR="${PROJECT_DIR}/tests/fixtures/pdfs"
META_DIR="${PROJECT_DIR}/tests/fixtures/metadata"
RESULTS_DIR="${PROJECT_DIR}/tests/fixtures/results"
# pdfium path handled by refextract defaults

mkdir -p "$RESULTS_DIR"

LIMIT="${1:-0}"  # Pass a number to limit how many PDFs to process (0 = all)

# Build release binary first
echo "Building refextract..."
cargo build --release --manifest-path="${PROJECT_DIR}/Cargo.toml" 2>&1 | tail -1
REFEXTRACT="${PROJECT_DIR}/target/release/refextract"

echo "Evaluating against INSPIRE ground truth..."
echo ""

total_papers=0
total_inspire_refs=0
total_extracted_refs=0
total_matched_arxiv=0
total_matched_journal=0
total_matched_doi=0
total_errors=0

for pdf in "${PDF_DIR}"/*.pdf; do
    basename=$(basename "$pdf" .pdf)
    meta="${META_DIR}/${basename}.json"

    if [[ ! -f "$meta" ]]; then
        continue
    fi

    if (( LIMIT > 0 && total_papers >= LIMIT )); then
        break
    fi

    # Run refextract (re-run if binary is newer than cached result)
    result_file="${RESULTS_DIR}/${basename}.json"
    if [[ ! -f "$result_file" ]] || [[ "$REFEXTRACT" -nt "$result_file" ]]; then
        if ! "$REFEXTRACT" --no-doi-lookup "$pdf" > "$result_file" 2>/dev/null; then
            echo "ERR  ${basename} (refextract failed)"
            total_errors=$((total_errors + 1))
            rm -f "$result_file"
            continue
        fi
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
