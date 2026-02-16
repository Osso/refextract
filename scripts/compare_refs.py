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
    """Normalize journal title for comparison: strip non-alpha, lowercase."""
    if not title:
        return ""
    n = re.sub(r"[^a-zA-Z0-9]+", "", title).lower()
    # Known equivalent abbreviations
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
    # Strip trailing location/country suffixes
    for suffix in ("usa", "uk"):
        if n.endswith(suffix):
            n = n[:-len(suffix)]
            break
    # Strip trailing "ser" / "series" (supplement series)
    for suffix in ("series", "ser"):
        if n.endswith(suffix):
            n = n[:-len(suffix)]
            break
    # Map full forms to INSPIRE short forms
    equiv = {
        "jhighenergyphys": "jhep",
        "jcosmolastropartphys": "jcap",
        "nuclinstrummethphysres": "nuclinstrummeth",
        "eurphyslett": "epl",
        "natmater": "naturemater",
        "natphys": "naturephys",
        "nuovcimlett": "lettnuovcim",
        "nuovcimriv": "rivnuovcim",
        "annphysleipzig": "annalenphys",
        "annphysnewyork": "annphys",
        "highenergyphysnuclphys": "hepnp",
        "highenergyphysnuclphysbeijing": "hepnp",
        "ieeetransinftheor": "ieeetransinfotheor",
        # Soviet → modern journal equivalences
        "sovphysjetp": "jexptheorphys",
        "sovphysusp": "physusp",
        # Full name → abbreviation
        "progtheorexpphys": "ptep",
        "procspieintsocopteng": "procspie",
        # Minor abbreviation differences
        "jdiffergeom": "jdiffgeom",
        "jmolecspectrosc": "jmolspectrosc",
        "pramanajphys": "pramana",
        "hadronicj": "hadronj",
    }
    for full, short in equiv.items():
        if n.startswith(full):
            n = short + n[len(full):]
            break
    return n


def volumes_match(v1: str, v2: str) -> bool:
    """Flexible volume matching. Handles JCAP/JHEP year-month encoding:
    extracted "0904" matches INSPIRE "04" (year prefix stripped by INSPIRE).
    Also handles combined volumes: "904-905" matches "904" or "905"."""
    if v1 == v2:
        return True
    # One may have a year prefix: "0904" ends with "04"
    short, long = (v1, v2) if len(v1) <= len(v2) else (v2, v1)
    if len(short) >= 2 and long.endswith(short) and len(long) - len(short) <= 2:
        return True
    # Combined volume: "904-905" matches "904" or "905"
    for sep in ("-", "–", "—"):
        if sep in v1:
            parts = v1.split(sep)
            if v2 in parts:
                return True
        if sep in v2:
            parts = v2.split(sep)
            if v1 in parts:
                return True
    # Leading-zero normalization: "04" matches "4" (JCAP/JHEP volumes)
    s1 = v1.lstrip("0") or "0"
    s2 = v2.lstrip("0") or "0"
    if s1 == s2:
        return True
    # Year-prefix + leading zero: "1509" vs "9" — strip year, then leading zeros
    short2, long2 = (s1, s2) if len(s1) <= len(s2) else (s2, s1)
    if len(short2) >= 1 and long2.endswith(short2) and len(long2) - len(short2) <= 2:
        return True
    # Alpha-prefixed volume: "LAT2006" matches "2006" (PoS conference codes)
    if short.isdigit() and not long.isdigit():
        alpha_stripped = long.lstrip("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz")
        if alpha_stripped == short:
            return True
    return False


def journals_match(j1: str, j2: str) -> bool:
    """Flexible journal name matching for INSPIRE vs extracted comparison.

    Handles section letters (Phys.Rev.D → physrevd vs physrev)
    and minor abbreviation differences.
    """
    if not j1 or not j2:
        return False
    if j1 == j2:
        return True
    # Prefix match: shorter is prefix of longer
    short, long = (j1, j2) if len(j1) <= len(j2) else (j2, j1)
    if long.startswith(short):
        diff = len(long) - len(short)
        # Max 3-char diff, minimum 6-char match to avoid "phys" matching "physrev"
        if len(short) >= 6 and diff <= 3:
            return True
        # Also match sub-journal suffixes: "lett", "suppl"
        # e.g., "astrophysj" matches "astrophysjlett"
        tail = long[len(short):]
        if len(short) >= 8 and tail in ("lett", "suppl", "procsuppl"):
            return True
    # Section letter mismatch: physreva vs physrevd (same base, different trailing letter)
    # INSPIRE sometimes has wrong section letter for Phys.Rev., Int.J.Mod.Phys., etc.
    if len(j1) == len(j2) and len(j1) >= 8 and j1[:-1] == j2[:-1]:
        if j1[-1].isalpha() and j2[-1].isalpha():
            return True
    return False


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
    ext_jv = [
        (r["journal"], r["volume"])
        for r in extracted_refs
        if r["journal"] and r["volume"]
    ]

    for iref in inspire_refs:
        # Try arXiv match first
        if iref["arxiv"] and iref["arxiv"] in ext_arxiv:
            matched_arxiv += 1
            continue

        # Try DOI match
        if iref["doi"] and iref["doi"] in ext_doi:
            matched_doi += 1
            continue

        # Try journal + volume match (flexible journal name matching)
        if iref["journal"] and iref["volume"]:
            for ej, ev in ext_jv:
                if volumes_match(ev, iref["volume"]) and journals_match(iref["journal"], ej):
                    matched_journal += 1
                    break
            else:
                continue
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
