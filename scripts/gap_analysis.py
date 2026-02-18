#!/usr/bin/env python3
"""Gap analysis: categorize why INSPIRE references are missed by refextract.

For each unmatched INSPIRE reference, assigns one of:
  no_id             - No arXiv, DOI, or journal+volume in INSPIRE metadata
  doi_only          - INSPIRE has DOI but no arXiv or journal+volume
  journal_no_raw    - INSPIRE has journal+volume but reference has no raw_refs text
  journal_with_raw  - INSPIRE has journal+volume AND raw_refs text (actionable!)
  arxiv_only        - INSPIRE has arXiv but no raw_refs text
  zero_extract      - Paper had 0 extracted refs (extraction failure)
"""

import argparse
import json
import os
import re
import sys
from collections import defaultdict


# ---------------------------------------------------------------------------
# Normalization helpers (identical to compare_refs.py)
# ---------------------------------------------------------------------------

def normalize_arxiv(aid: str) -> str:
    """Normalize arXiv ID: lowercase, strip version suffix."""
    if not aid:
        return ""
    aid = aid.strip().lower()
    aid = re.sub(r"v\d+$", "", aid)
    return aid


def normalize_doi(doi: str) -> str:
    if not doi:
        return ""
    return doi.strip().lower()


def normalize_journal(title: str) -> str:
    """Normalize journal title for comparison: strip non-alpha, lowercase."""
    if not title:
        return ""
    n = re.sub(r"[^a-zA-Z0-9]+", "", title).lower()
    n = n.replace("rept", "rep")
    n = n.replace("annu", "ann")
    n = n.replace("quantum", "quant")
    n = n.replace("gravity", "grav")
    n = n.replace("methods", "meth")
    n = n.replace("annals", "ann")
    n = n.replace("polon", "pol")
    n = n.replace("atom", "at")
    n = n.replace("nuovo", "nuov")
    n = n.replace("cimento", "cim")
    n = n.replace("relativ", "rel")
    n = n.replace("astron", "astr")
    n = n.replace("europhys", "eurphys")
    n = n.replace("royal", "r")
    n = n.replace("roy", "r")
    n = n.replace("spectop", "st")
    n = n.replace("fortschr", "fortsch")
    n = n.replace("london", "lond")
    n = n.replace("scripta", "scr")
    n = n.replace("japan", "jpn")
    n = n.replace("jap", "jpn")
    n = n.replace("czechoslov", "czech")
    n = n.replace("materials", "mater")
    n = n.replace("concepts", "")
    n = n.replace("photonics", "photon")
    n = n.replace("uspekhi", "usp")
    n = n.replace("statistik", "stat")
    n = n.replace("statist", "stat")
    n = n.replace("natl", "nat")
    n = n.replace("national", "nat")
    n = n.replace("frontiers", "front")
    n = n.replace("philos", "phil")
    n = n.replace("theory", "theor")
    n = n.replace("interiors", "inter")
    n = n.replace("molec", "mol")
    n = n.replace("cambridge", "camb")
    n = n.replace("nuclear", "nucl")
    n = n.replace("physics", "phys")
    for suffix in ("usa", "uk"):
        if n.endswith(suffix):
            n = n[: -len(suffix)]
            break
    for suffix in ("series", "ser"):
        if n.endswith(suffix):
            n = n[: -len(suffix)]
            break
    equiv = {
        "jhighenergyphys": "jhep",
        "jcosmolastropartphys": "jcap",
        "nuclinstrummethphysres": "nuclinstrummeth",
        "eurphyslett": "epl",
        "natmater": "naturemater",
        "natphys": "naturephys",
        "nuovcimlett": "lettnuovcim",
        "nuovcimriv": "rivnuovcim",
        "annphysleipzig": "annphys",
        "annphysnewyork": "annphys",
        "highenergyphysnuclphys": "hepnp",
        "highenergyphysnuclphysbeijing": "hepnp",
        "ieeetransinftheor": "ieeetransinfotheor",
        "sovphysjetp": "jexptheorphys",
        "sovphysusp": "physusp",
        "yadfiz": "physatnucl",
        "sovjnuclphys": "physatnucl",
        "zhekspteorfiz": "jexptheorphys",
        "progtheorexpphys": "ptep",
        "procspieintsocopteng": "procspie",
        "jdiffergeom": "jdiffgeom",
        "jmolecspectrosc": "jmolspectrosc",
        "pramanajphys": "pramana",
        "hadronicj": "hadronj",
        "eurphysjdirect": "eurphysj",
        "physscrtopissues": "physscrt",
        "naturwissenschaften": "naturwiss",
        "fortschittederphys": "fortschphys",
        "annalenphys": "annphys",
        "comptesrendusphysique": "crphys",
        "chinjphysc": "chinphysc",
        "gravitcosmol": "gravcosmol",
        "physjc": "eurphysjc",
        "physja": "eurphysja",
        "natphoton": "naturephoton",
        "natnanotech": "naturenanotech",
        "natcommun": "naturecommun",
        "natelectron": "natureelectron",
        "natrevphys": "naturerevphys",
        "natastr": "natureastr",
        "natchem": "naturechem",
    }
    for full, short in equiv.items():
        if n.startswith(full):
            n = short + n[len(full):]
            break
    return n


def volumes_match(v1: str, v2: str) -> bool:
    if v1 == v2:
        return True
    short, long = (v1, v2) if len(v1) <= len(v2) else (v2, v1)
    if len(short) >= 2 and long.endswith(short) and len(long) - len(short) <= 2:
        return True
    for sep in ("-", "\u2013", "\u2014"):
        if sep in v1:
            parts = v1.split(sep)
            if v2 in parts:
                return True
        if sep in v2:
            parts = v2.split(sep)
            if v1 in parts:
                return True
    s1 = v1.lstrip("0") or "0"
    s2 = v2.lstrip("0") or "0"
    if s1 == s2:
        return True
    short2, long2 = (s1, s2) if len(s1) <= len(s2) else (s2, s1)
    if len(short2) >= 1 and long2.endswith(short2) and len(long2) - len(short2) <= 2:
        return True
    if short.isdigit() and not long.isdigit():
        alpha_stripped = long.lstrip("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz")
        if alpha_stripped == short:
            return True
    return False


def journals_match(j1: str, j2: str) -> bool:
    if not j1 or not j2:
        return False
    if j1 == j2:
        return True
    short, long = (j1, j2) if len(j1) <= len(j2) else (j2, j1)
    if long.startswith(short):
        diff = len(long) - len(short)
        if len(short) >= 6 and diff <= 3:
            return True
        tail = long[len(short):]
        if len(short) >= 8 and tail in ("lett", "suppl", "supp", "procsuppl"):
            return True
        if len(short) >= 7 and len(tail) >= 2 and tail[0].isalpha():
            rest = tail[1:]
            if rest in ("procsuppl", "procsup"):
                return True
    if len(j1) == len(j2) and len(j1) >= 8 and j1[:-1] == j2[:-1]:
        if j1[-1].isalpha() and j2[-1].isalpha():
            return True
    return False


# ---------------------------------------------------------------------------
# Data loading
# ---------------------------------------------------------------------------

def load_inspire_refs(meta_path: str) -> list[dict]:
    """Load INSPIRE refs from metadata file, keeping raw fields for categorization."""
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

        raw_refs = ref.get("raw_refs") or []
        raw_text = ""
        if raw_refs:
            for rr in raw_refs:
                val = rr.get("value", rr) if isinstance(rr, dict) else rr
                schema = rr.get("schema", "text") if isinstance(rr, dict) else "text"
                if schema == "text" and val:
                    raw_text = val
                    break

        refs.append({
            "arxiv": normalize_arxiv(ref.get("arxiv_eprint", "")),
            "doi": normalize_doi(doi_val),
            "journal": normalize_journal(pub.get("journal_title", "")),
            "volume": (pub.get("journal_volume") or "").strip(),
            "raw_text": raw_text,
        })
    return refs


def load_extracted_refs(result_path: str) -> list[dict]:
    """Load refextract JSON output."""
    with open(result_path) as f:
        data = json.load(f)

    if isinstance(data, dict):
        data = [data]

    refs = []
    for entry in data:
        refs.append({
            "arxiv": normalize_arxiv(entry.get("arxiv_id", "")),
            "doi": normalize_doi(entry.get("doi", "")),
            "journal": normalize_journal(entry.get("journal_title", "")),
            "volume": (entry.get("journal_volume") or "").strip(),
            "raw_ref": entry.get("raw_ref", ""),
        })
    return refs


# ---------------------------------------------------------------------------
# Matching (identical logic to compare_refs.py)
# ---------------------------------------------------------------------------

def build_ext_lookups(extracted_refs: list[dict]) -> tuple[set, set, list]:
    ext_arxiv = {r["arxiv"] for r in extracted_refs if r["arxiv"]}
    ext_doi = {r["doi"] for r in extracted_refs if r["doi"]}
    ext_jv = [
        (r["journal"], r["volume"])
        for r in extracted_refs
        if r["journal"] and r["volume"]
    ]
    # PoS normalization
    pos_extra = []
    for ej, ev in ext_jv:
        if ej.startswith("pos") and len(ej) > 3:
            suffix = ej[3:].upper()
            pos_extra.append(("pos", suffix + ev))
    ext_jv.extend(pos_extra)
    return ext_arxiv, ext_doi, ext_jv


def classify_unmatched(iref: dict) -> str:
    """Categorize an unmatched INSPIRE ref."""
    has_arxiv = bool(iref["arxiv"])
    has_doi = bool(iref["doi"])
    has_journal = bool(iref["journal"] and iref["volume"])
    has_raw = bool(iref["raw_text"])

    if not has_arxiv and not has_doi and not has_journal:
        return "no_id"
    if has_doi and not has_arxiv and not has_journal:
        return "doi_only"
    if has_journal:
        return "journal_with_raw" if has_raw else "journal_no_raw"
    # has_arxiv only (no journal, no doi)
    return "arxiv_only"


def analyze_paper(
    inspire_refs: list[dict],
    extracted_refs: list[dict],
) -> dict:
    """Match refs and return per-paper stats."""
    zero_extract = len(extracted_refs) == 0
    ext_arxiv, ext_doi, ext_jv = build_ext_lookups(extracted_refs)

    matched_arxiv = 0
    matched_doi = 0
    matched_journal = 0
    unmatched: list[dict] = []

    for iref in inspire_refs:
        if iref["arxiv"] and iref["arxiv"] in ext_arxiv:
            matched_arxiv += 1
            continue

        if iref["doi"] and iref["doi"] in ext_doi:
            matched_doi += 1
            continue

        if iref["journal"] and iref["volume"]:
            for ej, ev in ext_jv:
                if volumes_match(ev, iref["volume"]) and journals_match(iref["journal"], ej):
                    matched_journal += 1
                    break
            else:
                if zero_extract:
                    unmatched.append({**iref, "category": "zero_extract"})
                else:
                    unmatched.append({**iref, "category": classify_unmatched(iref)})
            continue

        if zero_extract:
            unmatched.append({**iref, "category": "zero_extract"})
        else:
            unmatched.append({**iref, "category": classify_unmatched(iref)})

    total_matched = matched_arxiv + matched_doi + matched_journal
    total_inspire = len(inspire_refs)
    recall = total_matched / total_inspire if total_inspire > 0 else 0.0

    cats: dict[str, int] = defaultdict(int)
    for u in unmatched:
        cats[u["category"]] += 1

    return {
        "inspire_count": total_inspire,
        "extracted_count": len(extracted_refs),
        "matched": total_matched,
        "matched_arxiv": matched_arxiv,
        "matched_doi": matched_doi,
        "matched_journal": matched_journal,
        "recall": recall,
        "unmatched": unmatched,
        "categories": dict(cats),
        "extracted_refs": extracted_refs,
    }


# ---------------------------------------------------------------------------
# Output helpers
# ---------------------------------------------------------------------------

CATEGORIES = [
    "no_id",
    "doi_only",
    "journal_no_raw",
    "journal_with_raw",
    "arxiv_only",
    "zero_extract",
]


def print_overall_breakdown(totals: dict[str, int], grand_total_unmatched: int) -> None:
    print("Overall unmatched category breakdown")
    print("=" * 45)
    for cat in CATEGORIES:
        n = totals.get(cat, 0)
        pct = 100 * n / grand_total_unmatched if grand_total_unmatched else 0
        label = cat.ljust(20)
        print(f"  {label} {n:6d}  ({pct:5.1f}%)")
    print(f"  {'TOTAL'.ljust(20)} {grand_total_unmatched:6d}")
    print()


def print_per_paper(
    paper_results: list[tuple[str, dict]],
    min_actionable: int = 5,
) -> list[tuple[str, dict]]:
    """Print per-paper breakdown for papers with >= min_actionable journal_with_raw misses.

    Returns papers sorted by actionable count descending.
    """
    actionable = [
        (paper_id, res)
        for paper_id, res in paper_results
        if res["categories"].get("journal_with_raw", 0) >= min_actionable
    ]
    actionable.sort(key=lambda x: x[1]["categories"]["journal_with_raw"], reverse=True)

    if not actionable:
        print(f"No papers with >= {min_actionable} actionable (journal_with_raw) misses.\n")
        return actionable

    print(f"Papers with >= {min_actionable} actionable misses (sorted by miss count)")
    print("=" * 70)
    header = f"{'Paper':<16} {'Recall':>7} {'Inspire':>8} {'Matched':>8} {'j_w_raw':>8} {'j_no_raw':>9} {'doi_only':>9} {'no_id':>7}"
    print(header)
    print("-" * 70)
    for paper_id, res in actionable:
        cats = res["categories"]
        print(
            f"{paper_id:<16} "
            f"{res['recall']:>7.1%} "
            f"{res['inspire_count']:>8d} "
            f"{res['matched']:>8d} "
            f"{cats.get('journal_with_raw', 0):>8d} "
            f"{cats.get('journal_no_raw', 0):>9d} "
            f"{cats.get('doi_only', 0):>9d} "
            f"{cats.get('no_id', 0):>7d}"
        )
    print()
    return actionable


def print_top_actionable_raw(
    paper_results: list[tuple[str, dict]],
    top_n: int = 20,
) -> None:
    """Print raw_refs text for the top N papers by actionable miss count."""
    actionable = [
        (paper_id, res)
        for paper_id, res in paper_results
        if res["categories"].get("journal_with_raw", 0) > 0
    ]
    actionable.sort(key=lambda x: x[1]["categories"]["journal_with_raw"], reverse=True)

    if not actionable:
        print("No actionable misses (journal_with_raw) found in any paper.\n")
        return

    print(f"Raw ref text for top {top_n} actionable papers")
    print("=" * 70)
    for paper_id, res in actionable[:top_n]:
        missed = [u for u in res["unmatched"] if u["category"] == "journal_with_raw"]
        print(f"\n--- {paper_id} (recall={res['recall']:.1%}, {len(missed)} actionable misses) ---")
        for u in missed:
            journal_raw = u.get("raw_text", "")
            inspire_jv = f"{u['journal']} vol={u['volume']}"
            arxiv_str = f" arxiv={u['arxiv']}" if u["arxiv"] else ""
            doi_str = f" doi={u['doi']}" if u["doi"] else ""
            print(f"  INSPIRE: {inspire_jv}{arxiv_str}{doi_str}")
            if journal_raw:
                print(f"  raw_ref: {journal_raw}")
            else:
                print("  raw_ref: (none)")
    print()


# ---------------------------------------------------------------------------
# Near-miss analysis for journal_no_raw refs
# ---------------------------------------------------------------------------

def analyze_journal_no_raw(paper_results: list[tuple[str, dict]]) -> dict:
    """Break down journal_no_raw unmatched refs into near-miss categories.

    For each journal_no_raw ref, checks extracted refs from the same paper:
      not_extracted   - journal not found at all in extracted refs
      near_miss_journal - same journal found, but no matching volume
      near_miss_volume  - same journal AND volume found (normalization bug — wasn't matched)

    Returns a dict with:
      counts: {category: int}
      near_miss_volume_cases: list of {paper_id, inspire, extracted} for debugging
    """
    counts: dict[str, int] = defaultdict(int)
    near_miss_volume_cases: list[dict] = []

    for paper_id, res in paper_results:
        extracted_refs = res.get("extracted_refs", [])
        ext_with_journal = [r for r in extracted_refs if r["journal"]]

        for iref in res["unmatched"]:
            if iref["category"] != "journal_no_raw":
                continue

            insp_j = iref["journal"]
            insp_v = iref["volume"]

            # Check if any extracted ref has the same journal
            journal_matches = [
                r for r in ext_with_journal
                if journals_match(insp_j, r["journal"])
            ]

            if not journal_matches:
                counts["not_extracted"] += 1
                continue

            # Journal matched — check if any also has the same volume
            volume_matches = [
                r for r in journal_matches
                if insp_v and r["volume"] and volumes_match(insp_v, r["volume"])
            ]

            if volume_matches:
                counts["near_miss_volume"] += 1
                near_miss_volume_cases.append({
                    "paper_id": paper_id,
                    "inspire": iref,
                    "extracted": volume_matches[0],
                })
            else:
                counts["near_miss_journal"] += 1

    return {
        "counts": dict(counts),
        "near_miss_volume_cases": near_miss_volume_cases,
    }


def print_volume_mismatch_details(paper_results: list[tuple[str, dict]]) -> None:
    """Print detailed volume mismatch analysis for near_miss_journal cases."""
    # Collect all near_miss_journal cases with their volume data
    no_extracted_vols = 0
    has_extracted_vols = 0
    letter_prefix_cases = 0

    # Key: (inspire_journal, inspire_volume, tuple(sorted extracted volumes))
    # Value: count
    mismatch_patterns: dict[tuple, int] = defaultdict(int)

    for paper_id, res in paper_results:
        extracted_refs = res.get("extracted_refs", [])
        ext_with_journal = [r for r in extracted_refs if r["journal"]]

        for iref in res["unmatched"]:
            if iref["category"] != "journal_no_raw":
                continue

            insp_j = iref["journal"]
            insp_v = iref["volume"]

            journal_matches = [
                r for r in ext_with_journal
                if journals_match(insp_j, r["journal"])
            ]

            # Only process near_miss_journal (journal matched, volume didn't)
            volume_matches = [
                r for r in journal_matches
                if insp_v and r["volume"] and volumes_match(insp_v, r["volume"])
            ]
            if volume_matches:
                # This is near_miss_volume, not near_miss_journal — skip
                continue
            if not journal_matches:
                # This is not_extracted — skip
                continue

            # near_miss_journal: collect extracted volumes for this journal
            ext_vols = sorted({r["volume"] for r in journal_matches if r["volume"]})

            if not ext_vols:
                no_extracted_vols += 1
            else:
                has_extracted_vols += 1

            # Check if INSPIRE volume starts with a letter (section prefix like D95, A123)
            if insp_v and insp_v[0].isalpha():
                letter_prefix_cases += 1

            key = (insp_j, insp_v, tuple(ext_vols))
            mismatch_patterns[key] += 1

    total = no_extracted_vols + has_extracted_vols
    if total == 0:
        return

    print(f"Volume mismatch analysis ({total} near_miss_journal refs)")
    print("=" * 70)
    print(f"INSPIRE volume present, no matching extraction:")
    print(f"  No extracted volumes for journal:   {no_extracted_vols:5d} cases")
    print(f"  Extracted volumes exist but differ: {has_extracted_vols:5d} cases")
    print(f"  INSPIRE volume starts with letter:  {letter_prefix_cases:5d} cases  (e.g. D95, A123, C80)")
    print()

    # Sort by count descending, then by key for stability
    sorted_patterns = sorted(mismatch_patterns.items(), key=lambda x: -x[1])

    print(f"Top 30 specific mismatches (inspire_journal  INSPIRE_vol -> [extracted_vols]):")
    print("-" * 70)
    for (insp_j, insp_v, ext_vols_tuple), count in sorted_patterns[:30]:
        ext_vols_list = list(ext_vols_tuple) if ext_vols_tuple else []
        ext_display = str(ext_vols_list) if ext_vols_list else "(none)"
        insp_display = f"{insp_v!r}" if insp_v else "(empty)"
        print(f"  {insp_j:<30s} {insp_display:<10s} -> {ext_display:<30s}  ({count} cases)")
    print()


def print_journal_no_raw_breakdown(
    paper_results: list[tuple[str, dict]],
    grand_totals: dict[str, int],
) -> None:
    """Print near-miss breakdown for journal_no_raw refs."""
    total_jnr = grand_totals.get("journal_no_raw", 0)
    if total_jnr == 0:
        return

    result = analyze_journal_no_raw(paper_results)
    counts = result["counts"]
    cases = result["near_miss_volume_cases"]

    # Ordered for display
    order = ["not_extracted", "near_miss_journal", "near_miss_volume"]
    descriptions = {
        "not_extracted": "journal not in extracted output",
        "near_miss_journal": "same journal, different volume",
        "near_miss_volume": "same journal+volume but not matched",
    }

    print(f"journal_no_raw breakdown ({total_jnr} refs)")
    print("=" * 70)
    for cat in order:
        n = counts.get(cat, 0)
        pct = 100 * n / total_jnr if total_jnr else 0
        label = cat.ljust(22)
        desc = descriptions[cat]
        print(f"  {label} {n:5d}  ({pct:5.1f}%)  -- {desc}")
    print()

    if not cases:
        return

    print(f"near_miss_volume cases ({len(cases)} refs — same journal+volume, not matched)")
    print("=" * 70)
    for c in cases:
        paper_id = c["paper_id"]
        ins = c["inspire"]
        ext = c["extracted"]
        print(f"\n  Paper: {paper_id}")
        print(f"  INSPIRE  journal={ins['journal']!r:30s} volume={ins['volume']!r}")
        print(f"  Extracted journal={ext['journal']!r:30s} volume={ext['volume']!r}")
        if ext.get("raw_ref"):
            print(f"  raw_ref: {ext['raw_ref'][:100]}")
    print()


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument(
        "--paper",
        metavar="ARXIV_ID",
        help="Analyze a single paper by arXiv ID (e.g. 0704.1500)",
    )
    p.add_argument(
        "--min-recall",
        type=float,
        default=None,
        metavar="FLOAT",
        help="Only include papers with recall >= this value",
    )
    p.add_argument(
        "--max-recall",
        type=float,
        default=None,
        metavar="FLOAT",
        help="Only include papers with recall <= this value",
    )
    p.add_argument(
        "--results-dir",
        default="tests/fixtures/results",
        metavar="DIR",
        help="Directory containing refextract result JSON files",
    )
    p.add_argument(
        "--metadata-dir",
        default="tests/fixtures/metadata",
        metavar="DIR",
        help="Directory containing INSPIRE metadata JSON files",
    )
    p.add_argument(
        "--min-actionable",
        type=int,
        default=5,
        metavar="N",
        help="Min journal_with_raw misses to include in per-paper table (default: 5)",
    )
    p.add_argument(
        "--top-raw",
        type=int,
        default=20,
        metavar="N",
        help="Number of top papers to show raw refs for (default: 20)",
    )
    return p.parse_args()


def resolve_dir(path: str) -> str:
    """Resolve path relative to script location if not absolute."""
    if os.path.isabs(path):
        return path
    # Try relative to cwd first, then relative to repo root (script is in scripts/)
    if os.path.isdir(path):
        return os.path.abspath(path)
    script_dir = os.path.dirname(os.path.abspath(__file__))
    repo_root = os.path.dirname(script_dir)
    candidate = os.path.join(repo_root, path)
    if os.path.isdir(candidate):
        return candidate
    return os.path.abspath(path)


def find_papers(results_dir: str, metadata_dir: str, paper_id: str | None) -> list[str]:
    """Return list of paper IDs available in both directories."""
    result_files = {
        os.path.splitext(f)[0]
        for f in os.listdir(results_dir)
        if f.endswith(".json")
    }
    meta_files = {
        os.path.splitext(f)[0]
        for f in os.listdir(metadata_dir)
        if f.endswith(".json")
    }
    common = sorted(result_files & meta_files)

    if paper_id:
        if paper_id not in common:
            print(f"Error: paper '{paper_id}' not found in both directories.", file=sys.stderr)
            sys.exit(1)
        return [paper_id]
    return common


def main() -> None:
    args = parse_args()
    results_dir = resolve_dir(args.results_dir)
    metadata_dir = resolve_dir(args.metadata_dir)

    papers = find_papers(results_dir, metadata_dir, args.paper)
    print(f"Analyzing {len(papers)} paper(s)...", file=sys.stderr)

    paper_results: list[tuple[str, dict]] = []
    grand_totals: dict[str, int] = defaultdict(int)
    total_inspire = 0
    total_matched = 0

    for paper_id in papers:
        result_path = os.path.join(results_dir, paper_id + ".json")
        meta_path = os.path.join(metadata_dir, paper_id + ".json")

        try:
            inspire_refs = load_inspire_refs(meta_path)
            extracted_refs = load_extracted_refs(result_path)
        except Exception as e:
            print(f"Warning: skipping {paper_id}: {e}", file=sys.stderr)
            continue

        res = analyze_paper(inspire_refs, extracted_refs)

        # Apply recall filters
        if args.min_recall is not None and res["recall"] < args.min_recall:
            continue
        if args.max_recall is not None and res["recall"] > args.max_recall:
            continue

        paper_results.append((paper_id, res))
        total_inspire += res["inspire_count"]
        total_matched += res["matched"]
        for cat, count in res["categories"].items():
            grand_totals[cat] += count

    grand_total_unmatched = sum(grand_totals.values())
    overall_recall = total_matched / total_inspire if total_inspire else 0.0

    print(f"\nTotal papers analyzed: {len(paper_results)}")
    print(f"Total INSPIRE refs:    {total_inspire}")
    print(f"Total matched:         {total_matched}  ({overall_recall:.1%} recall)")
    print(f"Total unmatched:       {grand_total_unmatched}")
    print()

    print_overall_breakdown(grand_totals, grand_total_unmatched)
    print_journal_no_raw_breakdown(paper_results, grand_totals)
    print_volume_mismatch_details(paper_results)
    print_per_paper(paper_results, min_actionable=args.min_actionable)
    print_top_actionable_raw(paper_results, top_n=args.top_raw)


if __name__ == "__main__":
    main()
