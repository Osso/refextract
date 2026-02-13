#!/usr/bin/env python3
"""Compare refextract output against INSPIRE ground truth.

Outputs a single line:
    inspire_count extracted_count matched_arxiv matched_journal matched_doi

Matching rules (each INSPIRE ref matched at most once, by priority):
  1. arXiv ID (normalized: strip version, lowercase)
  2. DOI (lowercase)
  3. Journal title + volume (normalized abbreviation)
"""

import json
import re
import sys


def normalize_arxiv(aid: str) -> str:
    """Normalize arXiv ID: lowercase, strip version suffix."""
    if not aid:
        return ""
    aid = aid.strip().lower()
    # Remove version: 1234.5678v2 -> 1234.5678
    aid = re.sub(r"v\d+$", "", aid)
    return aid


def normalize_doi(doi: str) -> str:
    if not doi:
        return ""
    return doi.strip().lower()


def normalize_journal(title: str) -> str:
    """Normalize journal title for comparison: strip dots, spaces, lowercase."""
    if not title:
        return ""
    return re.sub(r"[\s.]+", "", title).lower()


def load_inspire_refs(meta_path: str) -> list[dict]:
    """Extract reference identifiers from INSPIRE metadata."""
    with open(meta_path) as f:
        data = json.load(f)

    refs = []
    for entry in data.get("metadata", {}).get("references", []):
        ref = entry.get("reference", {})
        pub = ref.get("publication_info", {})

        dois = ref.get("dois") or []
        doi_val = ""
        if dois:
            first = dois[0]
            doi_val = first.get("value", first) if isinstance(first, dict) else first

        refs.append({
            "arxiv": normalize_arxiv(ref.get("arxiv_eprint", "")),
            "doi": normalize_doi(doi_val),
            "journal": normalize_journal(pub.get("journal_title", "")),
            "volume": (pub.get("journal_volume") or "").strip(),
        })
    return refs


def load_extracted_refs(result_path: str) -> list[dict]:
    """Load refextract JSON output."""
    with open(result_path) as f:
        data = json.load(f)

    # Handle both array and single-object output
    if isinstance(data, dict):
        data = [data]

    refs = []
    for entry in data:
        refs.append({
            "arxiv": normalize_arxiv(entry.get("arxiv_id", "")),
            "doi": normalize_doi(entry.get("doi", "")),
            "journal": normalize_journal(entry.get("journal_title", "")),
            "volume": (entry.get("journal_volume") or "").strip(),
        })
    return refs


def match_refs(inspire_refs: list[dict], extracted_refs: list[dict]) -> tuple[int, int, int]:
    """Match extracted refs against INSPIRE ground truth.

    Returns (matched_arxiv, matched_journal, matched_doi).
    Each INSPIRE ref is matched at most once, by priority: arxiv > doi > journal.
    """
    matched_arxiv = 0
    matched_journal = 0
    matched_doi = 0

    # Build lookup sets from extracted refs
    ext_arxiv = {r["arxiv"] for r in extracted_refs if r["arxiv"]}
    ext_doi = {r["doi"] for r in extracted_refs if r["doi"]}
    ext_jv = {
        (r["journal"], r["volume"])
        for r in extracted_refs
        if r["journal"] and r["volume"]
    }

    for iref in inspire_refs:
        # Try arXiv match first
        if iref["arxiv"] and iref["arxiv"] in ext_arxiv:
            matched_arxiv += 1
            continue

        # Try DOI match
        if iref["doi"] and iref["doi"] in ext_doi:
            matched_doi += 1
            continue

        # Try journal + volume match
        if iref["journal"] and iref["volume"]:
            if (iref["journal"], iref["volume"]) in ext_jv:
                matched_journal += 1
                continue

    return matched_arxiv, matched_journal, matched_doi


def main():
    if len(sys.argv) != 3:
        print(f"Usage: {sys.argv[0]} <result.json> <metadata.json>", file=sys.stderr)
        sys.exit(1)

    result_path = sys.argv[1]
    meta_path = sys.argv[2]

    inspire_refs = load_inspire_refs(meta_path)
    extracted_refs = load_extracted_refs(result_path)

    m_arxiv, m_journal, m_doi = match_refs(inspire_refs, extracted_refs)

    print(f"{len(inspire_refs)} {len(extracted_refs)} {m_arxiv} {m_journal} {m_doi}")


if __name__ == "__main__":
    main()
