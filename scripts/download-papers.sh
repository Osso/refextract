#!/bin/bash
set -euo pipefail

# Download ~1000 HEP PDFs from arXiv for testing refextract.
# Uses INSPIRE API to find papers with known references (ground truth).
# Respects arXiv rate limits (3s between requests).

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
PDF_DIR="${PROJECT_DIR}/tests/fixtures/pdfs"
META_DIR="${PROJECT_DIR}/tests/fixtures/metadata"
PROGRESS_FILE="${PROJECT_DIR}/tests/fixtures/.download-progress"

mkdir -p "$PDF_DIR" "$META_DIR"

# INSPIRE query categories — spread across HEP subfields
QUERIES=(
    # Core HEP theory and phenomenology
    'inspire_categories.term:Phenomenology-HEP&sort=mostcited'
    'inspire_categories.term:Theory-HEP&sort=mostcited'
    'inspire_categories.term:Experiment-HEP&sort=mostcited'
    'inspire_categories.term:Lattice&sort=mostcited'
    'inspire_categories.term:Gravitation and Cosmology&sort=mostcited'
    'inspire_categories.term:Astrophysics&sort=mostcited'
    # Recent papers (more diverse styles)
    'inspire_categories.term:Phenomenology-HEP&sort=mostrecent'
    'inspire_categories.term:Theory-HEP&sort=mostrecent'
    'inspire_categories.term:Experiment-HEP&sort=mostrecent'
    'inspire_categories.term:Gravitation and Cosmology&sort=mostrecent'
)

PAPERS_PER_QUERY=100
TOTAL_TARGET=1000

downloaded_count() {
    find "$PDF_DIR" -name '*.pdf' 2>/dev/null | wc -l
}

already_have() {
    local arxiv_id="$1"
    local safe_name
    safe_name=$(echo "$arxiv_id" | tr '/' '_')
    [[ -f "${PDF_DIR}/${safe_name}.pdf" ]]
}

fetch_inspire_page() {
    local query="$1"
    local page="$2"
    local size="$3"
    curl -sf "https://inspirehep.net/api/literature?q=${query}&size=${size}&page=${page}" 2>/dev/null
}

download_pdf() {
    local arxiv_id="$1"
    local safe_name
    safe_name=$(echo "$arxiv_id" | tr '/' '_')
    local pdf_path="${PDF_DIR}/${safe_name}.pdf"

    if [[ -f "$pdf_path" ]]; then
        return 0
    fi

    # arXiv rate limit: wait 3 seconds between requests
    sleep 3

    if curl -sfL "https://arxiv.org/pdf/${arxiv_id}" -o "$pdf_path" 2>/dev/null; then
        # Verify it's actually a PDF (not an error page)
        if file "$pdf_path" | grep -q PDF; then
            return 0
        else
            rm -f "$pdf_path"
            return 1
        fi
    else
        rm -f "$pdf_path"
        return 1
    fi
}

save_metadata() {
    local arxiv_id="$1"
    local inspire_id="$2"
    local safe_name
    safe_name=$(echo "$arxiv_id" | tr '/' '_')
    local meta_path="${META_DIR}/${safe_name}.json"

    if [[ -f "$meta_path" ]]; then
        return 0
    fi

    # Fetch full record with references for ground truth
    curl -sf "https://inspirehep.net/api/literature/${inspire_id}?fields=arxiv_eprints,titles,references,publication_info" \
        -o "$meta_path" 2>/dev/null || true
}

echo "=== refextract PDF downloader ==="
echo "Target: ${TOTAL_TARGET} papers"
echo "PDF dir: ${PDF_DIR}"
echo "Already have: $(downloaded_count) PDFs"
echo ""

for query in "${QUERIES[@]}"; do
    current=$(downloaded_count)
    if (( current >= TOTAL_TARGET )); then
        echo "Reached target of ${TOTAL_TARGET} PDFs."
        break
    fi

    category=$(echo "$query" | grep -oP 'term:\K[^&]+' || echo "$query")
    sort_order=$(echo "$query" | grep -oP 'sort=\K\w+' || echo "unknown")
    echo "--- Fetching: ${category} (${sort_order}) ---"

    page=1
    fetched_in_query=0

    while (( fetched_in_query < PAPERS_PER_QUERY )); do
        current=$(downloaded_count)
        if (( current >= TOTAL_TARGET )); then
            break
        fi

        response=$(fetch_inspire_page "$query" "$page" 25)
        if [[ -z "$response" ]]; then
            echo "  Failed to fetch page ${page}, moving on"
            break
        fi

        # Extract arxiv IDs and inspire IDs
        papers=$(echo "$response" | jq -r '.hits.hits[] | select(.metadata.arxiv_eprints != null) | "\(.id) \(.metadata.arxiv_eprints[0].value)"' 2>/dev/null)

        if [[ -z "$papers" ]]; then
            break
        fi

        while IFS=' ' read -r inspire_id arxiv_id; do
            if [[ -z "$arxiv_id" || "$arxiv_id" == "null" ]]; then
                continue
            fi

            current=$(downloaded_count)
            if (( current >= TOTAL_TARGET )); then
                break 2
            fi

            if already_have "$arxiv_id"; then
                continue
            fi

            if download_pdf "$arxiv_id"; then
                save_metadata "$arxiv_id" "$inspire_id"
                fetched_in_query=$((fetched_in_query + 1))
                current=$((current + 1))
                echo "  [${current}/${TOTAL_TARGET}] ${arxiv_id}"
            else
                echo "  [skip] ${arxiv_id} (download failed)"
            fi
        done <<< "$papers"

        page=$((page + 1))

        # Small delay between INSPIRE API calls
        sleep 1
    done

    echo "  Got ${fetched_in_query} from this query (total: $(downloaded_count))"
done

echo ""
echo "=== Done ==="
echo "Total PDFs: $(downloaded_count)"
echo "Location: ${PDF_DIR}"
