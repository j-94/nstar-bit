#!/usr/bin/env python3
"""
fetch_retraction_data.py
Fetches the live Retraction Watch database (Crossref-hosted CSV, ~55k records)
and produces a filtered subset of medical/clinical retractions with DOIs.

Data source:
  https://gitlab.com/crossref/retraction-watch-data
  Updated daily. Publicly available.

Output:
  data/retractions.json  — filtered medical retractions with metadata
  data/fetch_stats.json  — provenance and row counts
"""

import csv
import io
import json
import time
import urllib.request
from pathlib import Path

RW_CSV_URL = (
    "https://gitlab.com/crossref/retraction-watch-data/-/raw/main/"
    "retraction_watch.csv?ref_type=heads&inline=false"
)

# Medical subject areas to include
MEDICAL_SUBJECTS = {
    "medicine", "clinical", "health", "nursing", "pharmacology",
    "cardiology", "oncology", "psychiatry", "neurology", "surgery",
    "pediatrics", "obstetrics", "gynecology", "pathology", "radiology",
    "immunology", "infectious", "endocrinology", "gastroenterology",
    "pulmonology", "nephrology", "dermatology", "ophthalmology",
    "orthopedics", "anesthesiology", "emergency", "urology", "hematology",
}

# Reason codes that indicate the evidence is truly invalidated
HIGH_SEVERITY_REASONS = {
    "Falsified data", "Fabricated data", "Fraud", "Misconduct",
    "Data falsification", "Unreliable data", "Data manipulation",
    "Fake peer review", "Concerns about data", "Duplication of data",
}


def fetch_csv(url: str) -> list[dict]:
    print(f"Fetching {url} ...")
    req = urllib.request.Request(url, headers={"User-Agent": "episteme-audit/1.0"})
    with urllib.request.urlopen(req, timeout=120) as resp:
        raw = resp.read().decode("utf-8", errors="replace")
    reader = csv.DictReader(io.StringIO(raw))
    return list(reader)


def is_medical(row: dict) -> bool:
    subject = (row.get("Subject", "") or "").lower()
    journal = (row.get("Journal", "") or "").lower()
    return any(
        term in subject or term in journal
        for term in MEDICAL_SUBJECTS
    )


def severity(row: dict) -> str:
    reasons_raw = row.get("Reason", "") or ""
    reasons = {r.strip() for r in reasons_raw.split(";")}
    if reasons & HIGH_SEVERITY_REASONS:
        return "high"
    if any(r for r in reasons if r):
        return "medium"
    return "low"


def parse_year(row: dict) -> int | None:
    # Try RetractionDate first, then OriginalPaperDate
    for field in ("RetractionDate", "OriginalPaperDate"):
        val = (row.get(field) or "").strip()
        if val and len(val) >= 4:
            try:
                return int(val[:4])
            except ValueError:
                pass
    return None


def main():
    out_dir = Path(__file__).parent.parent / "data"
    out_dir.mkdir(exist_ok=True)

    t0 = time.time()
    rows = fetch_csv(RW_CSV_URL)
    fetch_elapsed = time.time() - t0
    print(f"Fetched {len(rows):,} rows in {fetch_elapsed:.1f}s")

    # Filter to medical retractions with a DOI
    medical = []
    for row in rows:
        doi = (row.get("RetractionDOI") or row.get("OriginalPaperDOI") or "").strip()
        if not doi:
            continue
        if not is_medical(row):
            continue
        year = parse_year(row)
        medical.append({
            "doi": doi,
            "title": (row.get("Title") or "").strip(),
            "journal": (row.get("Journal") or "").strip(),
            "subject": (row.get("Subject") or "").strip(),
            "retraction_year": year,
            "reasons": [r.strip() for r in (row.get("Reason") or "").split(";") if r.strip()],
            "severity": severity(row),
            "country": (row.get("Country") or "").strip(),
            "author_count": len((row.get("Author") or "").split(";")) if row.get("Author") else 0,
        })

    # Sort: high severity first, then by most recent retraction
    medical.sort(key=lambda r: (
        0 if r["severity"] == "high" else 1 if r["severity"] == "medium" else 2,
        -(r["retraction_year"] or 0),
    ))

    stats = {
        "fetched_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "source_url": RW_CSV_URL,
        "total_records": len(rows),
        "medical_with_doi": len(medical),
        "high_severity": sum(1 for r in medical if r["severity"] == "high"),
        "medium_severity": sum(1 for r in medical if r["severity"] == "medium"),
        "low_severity": sum(1 for r in medical if r["severity"] == "low"),
        "fetch_seconds": round(fetch_elapsed, 2),
    }

    (out_dir / "retractions.json").write_text(
        json.dumps(medical, indent=2, ensure_ascii=False)
    )
    (out_dir / "fetch_stats.json").write_text(
        json.dumps(stats, indent=2)
    )

    print()
    print("── Retraction Watch Fetch Complete ─────────────────────")
    print(f"  Total records:        {stats['total_records']:,}")
    print(f"  Medical (with DOI):   {stats['medical_with_doi']:,}")
    print(f"  High severity:        {stats['high_severity']:,}  (fraud/falsification)")
    print(f"  Medium severity:      {stats['medium_severity']:,}")
    print(f"  Written to:           data/retractions.json")
    print("─────────────────────────────────────────────────────────")


if __name__ == "__main__":
    main()
