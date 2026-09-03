"""Execute copied notebooks with the canonical runner and preserve rich outputs."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import shutil
import subprocess
import sys
from tempfile import TemporaryDirectory
import time
import warnings

from common import (
    BUILD,
    LAB_EXECUTION_POLICY,
    NOTEBOOKS,
    SITE,
    contained,
    curriculum,
    digest,
    fixture_digest,
    runtime_identity,
    write_json,
)
from nbconvert import HTMLExporter, MarkdownExporter
import nbformat


def source_notebooks() -> dict[str, str]:
    """Fingerprint authored notebooks, ignoring checkpoint and cache directories."""
    return {
        str(path.relative_to(NOTEBOOKS)): digest(path)
        for path in NOTEBOOKS.rglob("*.ipynb")
        if ".ipynb_checkpoints" not in path.parts
    }


def lab_provenance() -> dict:
    """Bind every result to the runner, builder and strict execution policy."""
    return {
        "execution_policy": LAB_EXECUTION_POLICY,
        "runner_sha256": digest(NOTEBOOKS / "run_all_notebooks.py"),
        "builder_sha256": digest(Path(__file__).resolve()),
    }


def lab_report_errors(report: dict, require_complete: bool = True) -> list[str]:
    """Return reasons that a lab report cannot support a publication claim."""
    errors = []
    provenance = lab_provenance()
    if any(report.get(key) != value for key, value in provenance.items()):
        errors.append("Lab evidence provenance does not match the current strict runner and builder")
    failures = report.get("failed", [])
    if failures:
        errors.append(f"Lab evidence records failed notebooks: {sorted(failures)}")
    entries = report.get("labs", [])
    names = [entry.get("notebook") for entry in entries]
    if not all(isinstance(name, str) for name in names):
        errors.append("Lab evidence contains a record without a notebook path")
    if len(names) != len(set(names)):
        errors.append("Lab evidence contains duplicate notebook records")
    expected = source_notebooks()
    if require_complete and set(names) != set(expected):
        errors.append(
            "Lab evidence does not cover the complete source notebook set: "
            f"missing={sorted(set(expected) - set(names))}, "
            f"extra={sorted(str(name) for name in set(names) - set(expected))}"
        )
    fixtures = fixture_digest()
    runtime = runtime_identity()
    for entry in entries:
        name = entry.get("notebook")
        if name not in expected:
            continue
        if (
            entry.get("source_sha256") != expected[name]
            or entry.get("fixtures_sha256") != fixtures
            or entry.get("runtime") != runtime
            or entry.get("dependencies_sha256", {}) != notebook_dependencies(name)
            or any(entry.get(key) != value for key, value in provenance.items())
        ):
            errors.append(f"Stale lab evidence: {name}")
        executed = BUILD / "notebooks" / name
        if not executed.is_file() or entry.get("executed_sha256") != digest(executed):
            errors.append(f"Missing or stale executed notebook copy: {name}")
    return errors


def preserved_lab_entries(evidence_path: Path, selected: list[str], all_sources: dict[str, str]) -> list[dict]:
    """Load unrelated current entries for a focused rebuild."""
    selected_names = set(selected)
    if not evidence_path.exists() or selected_names == set(all_sources):
        return []
    report = json.loads(evidence_path.read_text())
    preserved = [entry for entry in report.get("labs", []) if entry.get("notebook") not in selected_names]
    preserved_report = {
        **report,
        "labs": preserved,
        "failed": [name for name in report.get("failed", []) if name not in selected_names],
    }
    errors = lab_report_errors(preserved_report, require_complete=False)
    expected = set(all_sources) - selected_names
    actual = {entry.get("notebook") for entry in preserved}
    if actual != expected:
        errors.append(
            "Preserved lab evidence does not cover every unselected notebook: "
            f"missing={sorted(expected - actual)}, extra={sorted(str(name) for name in actual - expected)}"
        )
    if errors:
        raise ValueError(
            "Cannot preserve unrelated lab evidence; run a complete lab build first:\n" + "\n".join(errors)
        )
    return preserved


def notebook_dependencies(relative: str) -> dict[str, str]:
    """Fingerprint declared notebook source dependencies used by reference labs."""
    source = json.loads(contained(NOTEBOOKS, relative).read_text())
    names = source.get("metadata", {}).get("analyst_dependencies", [])
    if not isinstance(names, list) or not all(isinstance(name, str) for name in names):
        raise ValueError(f"{relative}: analyst_dependencies must list notebook-relative paths")
    return {name: digest(contained(NOTEBOOKS, name)) for name in sorted(set(names))}


def require_clean_source(relative: str) -> None:
    """Reject checked-in execution state; only build copies may retain outputs."""
    source = json.loads(contained(NOTEBOOKS, relative).read_text())
    if any(
        cell.get("outputs") or cell.get("execution_count") is not None
        for cell in source.get("cells", [])
        if cell.get("cell_type") == "code"
    ):
        raise ValueError(f"{relative}: clear source outputs and execution counts before building")


def export_notebook(path: Path, relative: str, provenance: dict) -> dict:
    """Export computed HTML and downloadable Markdown without parsing HTML as MDX."""
    with warnings.catch_warnings():
        warnings.simplefilter("error")
        notebook = nbformat.read(path, as_version=4)
    if any(
        cell.execution_count is None or any(output.output_type == "error" for output in cell.outputs)
        for cell in notebook.cells
        if cell.cell_type == "code" and cell.source.strip()
    ):
        raise ValueError(f"{relative}: exported notebook contains unexecuted or failed code")
    for cell_number, cell in enumerate(notebook.cells, start=1):
        for output in cell.get("outputs", []):
            if (
                output.output_type == "stream"
                and output.get("name") == "stderr"
                and str(output.get("text", "")).strip()
            ):
                raise ValueError(f"{relative}: exported notebook contains stderr in cell {cell_number}")
    slug = str(Path(relative).with_suffix(""))
    asset = SITE / "public" / "lab-assets" / slug
    asset.parent.mkdir(parents=True, exist_ok=True)
    title = next(
        (
            cell.source.splitlines()[0].lstrip("# ")
            for cell in notebook.cells
            if cell.cell_type == "markdown" and cell.source.startswith("# ")
        ),
        path.stem.replace("_", " "),
    )
    with warnings.catch_warnings():
        warnings.simplefilter("error")
        html, _ = HTMLExporter(template_name="lab").from_notebook_node(notebook)
        markdown, resources = MarkdownExporter().from_notebook_node(
            notebook, resources={"unique_key": path.stem, "output_files_dir": path.stem + "_files"}
        )
    asset.with_suffix(".html").write_text(html)
    asset.with_suffix(".md").write_text(markdown)
    for name, data in resources.get("outputs", {}).items():
        output = contained(asset.parent, name)
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_bytes(data)
    shutil.copy2(path, asset.with_suffix(".ipynb"))
    target = SITE / "content" / "labs" / (slug + ".mdx")
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(
        "---\ntitle: "
        + json.dumps(title)
        + "\ndescription: "
        + json.dumps("Executed Python lab with complete source and rich results.")
        + "\n---\n\n"
        + f'<LabHtml src="/lab-assets/{slug}.html" />\n\n'
        + f"[Download executed notebook](/lab-assets/{slug}.ipynb) · [Download Markdown](/lab-assets/{slug}.md)\n"
    )
    return {
        "notebook": relative,
        "html": f"/lab-assets/{slug}.html",
        "code_cells": sum(c.cell_type == "code" for c in notebook.cells),
        "source_sha256": digest(NOTEBOOKS / relative),
        "executed_sha256": digest(path),
        "fixtures_sha256": fixture_digest(),
        "dependencies_sha256": notebook_dependencies(relative),
        "runtime": runtime_identity(),
        **provenance,
    }


def main() -> int:
    """Build selected labs, or the whole source notebook collection by default."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--notebook", action="append", help="Notebook-relative path; repeat to select several")
    parser.add_argument(
        "--curriculum-only", action="store_true", help="Execute the manifest's lab and example mappings"
    )
    parser.add_argument("--timeout", type=int, default=600, help="Per-cell timeout passed to the original runner")
    args = parser.parse_args()
    before = source_notebooks()
    selected = args.notebook
    if not selected and args.curriculum_only:
        selected = sorted({
            name for lesson in curriculum() for name in [*lesson.get("labs", []), *lesson.get("examples", [])]
        })
    selected = selected or sorted(before)
    for relative in selected:
        if not contained(NOTEBOOKS, relative).is_file():
            parser.error(f"Missing notebook: {relative}")
        require_clean_source(relative)
    selected_before = {name: before[name] for name in selected}
    fixture_before = fixture_digest()
    runtime_before = runtime_identity()
    dependencies_before = {name: notebook_dependencies(name) for name in selected}
    provenance_before = lab_provenance()
    # Focused runs use a fresh working tree and preserve unrelated executed copies.
    with TemporaryDirectory(prefix=".lab-run-", dir=SITE) as directory:
        copies = Path(directory) / "notebooks"
        # Copy the full tree: notebooks may load sibling data and shared helpers.
        shutil.copytree(
            NOTEBOOKS,
            copies,
            dirs_exist_ok=True,
            ignore=shutil.ignore_patterns("__pycache__", ".ipynb_checkpoints", "*.html", "*.pdf"),
        )
        evidence_path = BUILD / "labs.json"
        existing = preserved_lab_entries(evidence_path, selected, before)
        evidence = {entry["notebook"]: entry for entry in existing if entry["notebook"] not in selected}
        failures = []
        try:
            for relative in selected:
                started = time.monotonic()
                result = subprocess.run(
                    [
                        sys.executable,
                        str(NOTEBOOKS / "run_all_notebooks.py"),
                        "--notebook-root",
                        str(copies),
                        "--directory",
                        relative,
                        "--save-outputs",
                        "--timeout",
                        str(args.timeout),
                    ],
                    capture_output=True,
                    text=True,
                    check=False,
                )
                if result.returncode or result.stderr.strip():
                    failures.append(relative)
                    if result.stderr.strip() and not result.returncode:
                        print(f"{relative}: notebook runner emitted stderr", file=sys.stderr, flush=True)
                    print(result.stdout + result.stderr, file=sys.stderr, flush=True)
                else:
                    evidence[relative] = export_notebook(copies / relative, relative, provenance_before)
                    published_copy = BUILD / "notebooks" / relative
                    published_copy.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copy2(copies / relative, published_copy)
                    evidence[relative]["execution_and_export_seconds"] = time.monotonic() - started
                    print(f"Lab passed: {relative}", flush=True)
        finally:
            after = source_notebooks()
            if (
                selected_before != {name: after.get(name) for name in selected}
                or fixture_before != fixture_digest()
                or runtime_before != runtime_identity()
                or dependencies_before != {name: notebook_dependencies(name) for name in selected}
                or provenance_before != lab_provenance()
            ):
                # Also detects concurrent author edits: never claim a stable source run.
                raise RuntimeError(
                    "Selected source notebooks or shared fixtures changed during execution; repeat against a stable checkout"
                )
            write_json(
                evidence_path,
                {
                    **provenance_before,
                    "labs": list(evidence.values()),
                    "failed": failures,
                },
            )
    return int(bool(failures))


if __name__ == "__main__":
    raise SystemExit(main())
