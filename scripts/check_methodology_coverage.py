"""Check the source ledger for the audited financial and numerical crates.

This checks source enumeration, record ownership and that every document in a
scope is a record, an ADR or linked from the scope, not mathematical correctness
or methodology approval. Use --verify-source to check the dated source snapshot,
and --write to regenerate the readable source map from the reviewed ledgers.
"""

from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
import os
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
AUDIT = ROOT / "docs/decisions/financial"
CLASSIFICATIONS = {"algorithm", "support", "test", "generated"}
ADR_NAME = re.compile(r"^\d{4}-.+\.md$")
MARKDOWN_LINK = re.compile(r"\]\(([^)#\s]+\.md)")
AUDITED_CRATES = (
    "core",
    "cashflows",
    "models",
    "calibration",
    "analytics",
    "attribution",
    "statements",
    "statements-analytics",
    "scenarios",
    "portfolio",
    "margin",
    "factor",
    "covenants",
    "features",
    "arrow-interchange",
)


def _source_files() -> set[str]:
    """Enumerate the audited production trees and their Rust build generators."""
    crates = [ROOT / "finstack-quant" / name for name in AUDITED_CRATES]
    valuations = ROOT / "finstack-quant/valuations"
    crates.append(valuations)
    crates.extend(
        child
        for child in valuations.iterdir()
        if child.is_dir() and child.name != "test-support" and (child / "src").is_dir()
    )
    files = {path.relative_to(ROOT).as_posix() for crate in crates for path in (crate / "src").rglob("*.rs")}
    for crate in crates:
        build = crate / "build.rs"
        if build.is_file():
            files.add(build.relative_to(ROOT).as_posix())
        files.update(path.relative_to(ROOT).as_posix() for path in (crate / "build").rglob("*.rs"))
    return files


def _orphan_documents(scope: Path, record_paths: set[str]) -> list[str]:
    """List scope documents that are not a record, an ADR, or linked from the scope."""
    linked: set[str] = set()
    roots = {"README.md", *record_paths}
    roots.update(document.name for document in scope.glob("*.md") if ADR_NAME.match(document.name))
    for name in roots:
        document = scope / name
        if document.is_file():
            linked.update(Path(link).name for link in MARKDOWN_LINK.findall(document.read_text()) if "/" not in link)
    return [
        document.name
        for document in sorted(scope.glob("*.md"))
        if document.name != "README.md"
        and not ADR_NAME.match(document.name)
        and document.name not in record_paths
        and document.name not in linked
    ]


def _check(verify_source: bool) -> tuple[list[str], list[tuple[Path, dict]]]:
    """Validate each source classification and its links to distinct records."""
    errors: list[str] = []
    ledgers: list[tuple[Path, dict]] = []
    seen: set[str] = set()
    ids: set[str] = set()
    expected = _source_files()
    for path in sorted(AUDIT.glob("*/coverage.json")):
        data = json.loads(path.read_text())
        ledgers.append((path, data))
        if data["scope"] != path.parent.name:
            errors.append(f"{path}: scope does not match directory")
        records = {}
        for record in data["records"]:
            record_id = record["id"]
            if record_id in ids:
                errors.append(f"Duplicate record ID: {record_id}")
            ids.add(record_id)
            target = (path.parent / record["path"]).resolve()
            if not target.is_relative_to(path.parent.resolve()) or not target.is_file():
                errors.append(f"{record_id}: record file missing or outside its scope")
            records[record_id] = record
        errors.extend(
            f"{path.parent.name}/{name}: document is neither a record, an ADR, nor linked from the scope"
            for name in _orphan_documents(path.parent, {record["path"] for record in data["records"]})
        )
        used: set[str] = set()
        for entry in data["files"]:
            source = entry["path"]
            if source in seen:
                errors.append(f"Duplicate source ownership: {source}")
            seen.add(source)
            kind = entry["classification"]
            if kind not in CLASSIFICATIONS:
                errors.append(f"{source}: unknown classification {kind}")
            if not entry["reason"].strip():
                errors.append(f"{source}: missing classification explanation")
            if kind == "algorithm" and not entry["records"]:
                errors.append(f"{source}: algorithm has no methodology record")
            for record_id in entry["records"]:
                if record_id not in records:
                    errors.append(f"{source}: unknown record {record_id}")
                used.add(record_id)
            if verify_source and source in expected:
                digest = hashlib.sha256((ROOT / source).read_bytes()).hexdigest()
                if digest != entry.get("sha256"):
                    errors.append(f"{source}: content differs from the audited snapshot")
        for unused in records.keys() - used:
            errors.append(f"{unused}: no source mapped to record")
    errors.extend(f"Unclassified source: {source}" for source in sorted(expected - seen))
    errors.extend(f"Out-of-scope or removed source: {source}" for source in sorted(seen - expected))
    return errors, ledgers


def _source_map(ledgers: list[tuple[Path, dict]]) -> str:
    """Render the readable map without changing audit evidence or source hashes."""
    lines = [
        "# Methodology source coverage",
        "",
        "This map is generated from the per-scope `coverage.json` ledgers by",
        "`uv run --no-sync python scripts/check_methodology_coverage.py --write`.",
        "Edit the reviewed ledger first. Hashes identify inspected source content;",
        "they do not certify correctness or approve a method. See the",
        "[audit scope and evidence limits](README.md).",
        "",
    ]
    for _path, data in ledgers:
        scope = data["scope"]
        records = {record["id"]: record for record in data["records"]}
        lines.extend([
            f"## {scope}",
            "",
            f"[Algorithm inventory]({scope}/README.md) · [Source ledger]({scope}/coverage.json)",
            "",
            "| Source | Classification | Methodology records | Inspected responsibility |",
            "| --- | --- | --- | --- |",
        ])
        for entry in sorted(data["files"], key=lambda item: item["path"]):
            relative = os.path.relpath(ROOT / entry["path"], AUDIT)
            refs = (
                ", ".join(f"[{record_id}]({scope}/{records[record_id]['path']})" for record_id in entry["records"])
                or "No independent methodology"
            )
            reason = entry["reason"].replace("|", "\\|").replace("\n", " ")
            lines.append(f"| [{entry['path']}]({relative}) | {entry['classification']} | {refs} | {reason} |")
        lines.append("")
    return "\n".join(lines)


def main() -> int:
    """Run the audit-ledger checks and return zero on success, one on failure."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--verify-source", action="store_true", help="Verify inspected source hashes")
    parser.add_argument("--write", action="store_true", help="Regenerate SOURCE_COVERAGE.md")
    args = parser.parse_args()
    errors, ledgers = _check(args.verify_source)
    if errors:
        print("\n".join(errors))
        return 1
    rendered = _source_map(ledgers)
    output = AUDIT / "SOURCE_COVERAGE.md"
    if args.write:
        output.write_text(rendered)
    elif not output.is_file() or output.read_text() != rendered:
        print("SOURCE_COVERAGE.md is stale; review the ledgers, then run with --write")
        return 1
    classifications = Counter(entry["classification"] for _, data in ledgers for entry in data["files"])
    records = sum(len(data["records"]) for _, data in ledgers)
    print(f"Checked {sum(classifications.values())} source files and {records} records in {len(ledgers)} scopes")
    print(", ".join(f"{kind}: {classifications[kind]}" for kind in sorted(classifications)))
    if args.verify_source:
        print("All inspected source hashes match")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
