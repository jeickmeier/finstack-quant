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

import common
from common import (
    BUILD,
    LAB_EXECUTION_POLICY,
    NOTEBOOKS,
    SITE,
    contained,
    curriculum,
    digest,
    execution_environment,
    fixture_digest,
    lesson_notebook_names,
    notebook_closure,
    notebook_dependencies,
    runtime_identity,
    write_json,
)
from nbconvert import HTMLExporter, MarkdownExporter
import nbformat
import publication_source
from publication_source import (
    notebook_assertion_count,
    notebook_assertion_locations,
    notebook_learner_assertion_count,
    remove_notebook_assertions,
)


def source_notebooks() -> dict[str, str]:
    """Fingerprint authored notebooks, ignoring checkpoint and cache directories."""
    return {
        str(path.relative_to(NOTEBOOKS)): digest(path)
        for path in NOTEBOOKS.rglob("*.ipynb")
        if ".ipynb_checkpoints" not in path.parts
    }


def published_notebooks() -> set[str]:
    """Return notebooks the curriculum exposes as labs or supporting examples."""
    return {name for lesson in curriculum(SITE) for name in lesson_notebook_names(lesson)}


def lab_provenance() -> dict:
    """Bind every result to the runner, builder and strict execution policy."""
    return {
        "execution_policy": LAB_EXECUTION_POLICY,
        "runner_sha256": digest(NOTEBOOKS / "run_all_notebooks.py"),
        "builder_sha256": digest(Path(__file__).resolve()),
        "publication_filter_sha256": digest(Path(publication_source.__file__).resolve()),
        "common_sha256": digest(Path(common.__file__).resolve()),
    }


def lab_artifact_paths(relative: str) -> tuple[list[Path], Path]:
    """Return required publication files and the extracted-resource directory."""
    slug = str(Path(relative).with_suffix(""))
    asset = SITE / "public" / "lab-assets" / slug
    required = [
        Path(str(asset) + ".html"),
        Path(str(asset) + ".md"),
        Path(str(asset) + ".ipynb"),
        SITE / "content" / "labs" / (slug + ".mdx"),
    ]
    return required, asset.parent / f"{asset.name}_files"


def current_lab_artifacts(relative: str) -> dict[str, str]:
    """Fingerprint the exact learner-facing artifact set for one notebook."""
    required, resource_dir = lab_artifact_paths(relative)
    if any(path.is_symlink() for path in [*required, resource_dir]):
        raise ValueError(f"{relative}: generated lab artifacts must not contain symlinks")
    paths = [path for path in required if path.is_file()]
    if resource_dir.is_dir():
        resources = list(resource_dir.rglob("*"))
        if any(path.is_symlink() for path in resources):
            raise ValueError(f"{relative}: generated lab artifacts must not contain symlinks")
        paths.extend(path for path in resources if path.is_file())
    return {str(path.relative_to(SITE)): digest(path) for path in sorted(paths)}


def current_lab_artifact_tree() -> dict[str, str]:
    """Fingerprint every generated lab file across both publication roots."""
    paths = []
    for root in (SITE / "public" / "lab-assets", SITE / "content" / "labs"):
        if root.is_symlink():
            raise ValueError(f"Generated lab root must not be a symlink: {root}")
        if root.is_dir():
            entries = list(root.rglob("*"))
            symlinks = [str(path.relative_to(SITE)) for path in entries if path.is_symlink()]
            if symlinks:
                raise ValueError(f"Generated lab artifacts must not contain symlinks: {sorted(symlinks)}")
            paths.extend(path for path in entries if path.is_file())
    return {str(path.relative_to(SITE)): digest(path) for path in sorted(paths)}


def recorded_lab_artifact_tree(entries: list[dict]) -> dict[str, str]:
    """Return the exact artifact manifest represented by lab evidence entries."""
    recorded = {}
    for entry in entries:
        artifacts = entry.get("artifacts_sha256", {})
        if isinstance(artifacts, dict):
            recorded.update(artifacts)
    return recorded


def remove_lab_artifacts(relative: str) -> None:
    """Remove one successfully validated execution-only notebook's public files."""
    required, resource_dir = lab_artifact_paths(relative)
    for path in [*required, resource_dir]:
        if path.is_symlink() or path.is_file():
            path.unlink()
        elif path.is_dir():
            shutil.rmtree(path)


def prune_unrecorded_lab_artifacts(entries: list[dict]) -> None:
    """Remove files outside a successful full build's exact artifact manifest."""
    expected = set(recorded_lab_artifact_tree(entries))
    roots = (SITE / "public" / "lab-assets", SITE / "content" / "labs")
    for root in roots:
        if root.is_symlink():
            raise ValueError(f"Generated lab root must not be a symlink: {root}")
        if not root.is_dir():
            continue
        for path in sorted(root.rglob("*"), reverse=True):
            if path.is_symlink() or (path.is_file() and str(path.relative_to(SITE)) not in expected):
                path.unlink()
            elif path.is_dir() and not any(path.iterdir()):
                path.rmdir()


def _lab_artifact_errors(entry: dict, name: str, publication_execution: dict) -> list[str]:
    """Validate the exact published artifact set recorded for one lab."""
    errors = []
    required_artifacts, _ = lab_artifact_paths(name)
    required_keys = {str(path.relative_to(SITE)) for path in required_artifacts}
    recorded_artifacts = entry.get("artifacts_sha256")
    try:
        actual_artifacts = current_lab_artifacts(name)
    except ValueError as error:
        return [str(error)]
    if (
        not isinstance(recorded_artifacts, dict)
        or not required_keys.issubset(recorded_artifacts)
        or recorded_artifacts != actual_artifacts
    ):
        errors.append(f"Missing or stale learner-facing lab artifacts: {name}")
    download = required_artifacts[2]
    if not download.is_file() or digest(download) != publication_execution.get("sha256"):
        errors.append(f"Missing or stale learner-facing notebook download: {name}")
    return errors


def _executed_lab_errors(entry: dict, name: str, should_publish: bool) -> list[str]:
    """Validate canonical and assertion-free execution copies for one lab."""
    errors = []
    canonical_execution = entry.get("canonical_execution", {})
    publication_execution = entry.get("publication_execution", {})
    canonical_copy = BUILD / "canonical-notebooks" / name
    executed = BUILD / "notebooks" / name
    canonical_ok = (
        canonical_copy.is_file()
        and canonical_execution.get("status") == "passed"
        and canonical_execution.get("sha256") == digest(canonical_copy)
    )
    publication_ok = (
        executed.is_file()
        and publication_execution.get("status") == "passed"
        and publication_execution.get("sha256") == digest(executed)
        and publication_execution.get("assertions") == 0
        and publication_execution.get("assertions_executed") == 0
    )
    if not canonical_ok:
        errors.append(f"Missing or stale canonical executed notebook copy: {name}")
    if not publication_ok:
        errors.append(f"Missing or stale executed notebook copy: {name}")
    if entry.get("published") is not should_publish:
        errors.append(f"Stale lab publication status: {name}")
    if not canonical_ok or not publication_ok:
        return [*errors, *_lab_artifact_errors(entry, name, publication_execution)] if should_publish else errors

    try:
        with warnings.catch_warnings():
            warnings.simplefilter("error")
            published = nbformat.read(executed, as_version=4)
            canonical = nbformat.read(contained(NOTEBOOKS, name), as_version=4)
            canonical_executed = _read_executed_notebook(canonical_copy, name)
    except (OSError, ValueError) as error:
        errors.append(f"Invalid executed notebook copy: {name}: {error}")
        return [*errors, *_lab_artifact_errors(entry, name, publication_execution)]

    if notebook_learner_assertion_count(published, name):
        errors.append(f"Learner-facing executed notebook contains assertions or explicit raise AssertionError: {name}")
    try:
        _validate_assertion_coverage(canonical_executed, name)
        _validate_assertion_coverage(published, name)
    except ValueError as error:
        errors.append(f"Invalid executed notebook copy: {name}: {error}")

    canonical_sources = [(cell.cell_type, cell.source) for cell in canonical.cells]
    executed_sources = [(cell.cell_type, cell.source) for cell in canonical_executed.cells]
    canonical_assertions = notebook_assertion_count(canonical, name)
    if (
        canonical_execution.get("assertions") != canonical_assertions
        or canonical_execution.get("assertions_executed") != canonical_assertions
        or canonical_sources != executed_sources
    ):
        errors.append(f"Stale canonical execution evidence: {name}")
    try:
        validate_execution_equivalence(canonical_executed, published, name)
    except ValueError as error:
        errors.append(str(error))
    if should_publish:
        return [*errors, *_lab_artifact_errors(entry, name, publication_execution)]
    if entry.get("artifacts_sha256") != {} or "html" in entry:
        errors.append(f"Execution-only notebook records learner-facing artifacts: {name}")
    return errors


def _lab_entry_errors(
    entry: dict,
    name: str,
    source_sha256: str,
    fixtures_sha256: str,
    runtime: dict[str, object],
    provenance: dict,
    should_publish: bool,
) -> list[str]:
    """Validate source, dependency, runtime and execution evidence for one lab."""
    errors = []
    if (
        entry.get("source_sha256") != source_sha256
        or entry.get("fixtures_sha256") != fixtures_sha256
        or entry.get("runtime") != runtime
        or entry.get("dependencies_sha256", {}) != notebook_dependencies(name)
        or any(entry.get(key) != value for key, value in provenance.items())
    ):
        errors.append(f"Stale lab evidence: {name}")
    return [*errors, *_executed_lab_errors(entry, name, should_publish)]


def lab_report_errors(report: dict, require_complete: bool = True) -> list[str]:
    """Return reasons that a lab report cannot support a publication claim."""
    errors = []
    provenance = lab_provenance()
    try:
        actual_artifact_tree = current_lab_artifact_tree()
    except ValueError as error:
        actual_artifact_tree = None
        errors.append(str(error))
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
    publishable = published_notebooks()
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
        errors.extend(
            _lab_entry_errors(
                entry,
                name,
                expected[name],
                fixtures,
                runtime,
                provenance,
                name in publishable,
            )
        )
    if (
        require_complete
        and actual_artifact_tree is not None
        and recorded_lab_artifact_tree(entries) != actual_artifact_tree
    ):
        errors.append("Generated lab artifact tree differs from the complete evidence manifest")
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


def require_clean_source(relative: str) -> None:
    """Reject malformed notebooks and checked-in execution state before running."""
    path = contained(NOTEBOOKS, relative)
    with warnings.catch_warnings():
        warnings.simplefilter("error")
        source = nbformat.read(path, as_version=4)
        nbformat.validate(source)
    if any(
        cell.get("outputs") or cell.get("execution_count") is not None
        for cell in source.get("cells", [])
        if cell.get("cell_type") == "code"
    ):
        raise ValueError(f"{relative}: clear source outputs and execution counts before building")


def _read_executed_notebook(path: Path, relative: str) -> nbformat.NotebookNode:
    """Read one strict execution result and reject skipped, failed or noisy cells."""
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
    return notebook


def _observable_outputs(cell: nbformat.NotebookNode) -> list[dict]:
    """Return stable JSON output data, excluding execution counters."""
    outputs = json.loads(json.dumps(cell.get("outputs", [])))
    normalized = []
    for output in outputs:
        output.pop("execution_count", None)
        if output.get("output_type") == "stream":
            text = output.get("text", "")
            output["text"] = "".join(text) if isinstance(text, list) else str(text)
            if (
                normalized
                and normalized[-1].get("output_type") == "stream"
                and normalized[-1].get("name") == output.get("name")
            ):
                normalized[-1]["text"] += output["text"]
                continue
        normalized.append(output)
    return normalized


def _expected_assertion_coverage(notebook: nbformat.NotebookNode, relative: str) -> dict:
    """Return the exact assertion coverage required for an executed notebook."""
    locations = notebook_assertion_locations(notebook, relative)
    return {
        "assertions_total": len(locations),
        "assertions_executed": len(locations),
        "assertion_locations": locations,
    }


def _validate_assertion_coverage(notebook: nbformat.NotebookNode, relative: str) -> None:
    """Require runner metadata proving every assertion location was reached."""
    expected = _expected_assertion_coverage(notebook, relative)
    if notebook.metadata.get("finstack_execution") != expected:
        raise ValueError(f"{relative}: notebook assertion execution coverage is missing or stale")


def validate_execution_equivalence(
    canonical: nbformat.NotebookNode,
    published: nbformat.NotebookNode,
    relative: str,
) -> None:
    """Require hidden checks to leave learner-visible cell results unchanged."""
    if len(canonical.cells) != len(published.cells):
        raise ValueError(f"{relative}: canonical and learner notebooks have different cell counts")
    for number, (canonical_cell, published_cell) in enumerate(
        zip(canonical.cells, published.cells, strict=True), start=1
    ):
        if canonical_cell.cell_type != "code":
            continue
        if _observable_outputs(canonical_cell) != _observable_outputs(published_cell):
            raise ValueError(f"{relative}: canonical checks change learner-visible output in cell {number}")


def prepare_publication_notebook(path: Path, relative: str, canonical_path: Path | None = None) -> dict:
    """Retain canonical execution, then strip assertions and captured outputs."""
    notebook = _read_executed_notebook(path, relative)
    with warnings.catch_warnings():
        warnings.simplefilter("error")
        canonical = nbformat.read(contained(NOTEBOOKS, relative), as_version=4)
    canonical_sources = [(cell.cell_type, cell.source) for cell in canonical.cells]
    executed_sources = [(cell.cell_type, cell.source) for cell in notebook.cells]
    if executed_sources != canonical_sources:
        raise ValueError(f"{relative}: canonical execution changed the authored notebook source")
    _validate_assertion_coverage(notebook, relative)
    canonical_assertions = notebook_assertion_count(canonical, relative)
    executed_assertions = notebook_assertion_count(notebook, relative)
    if executed_assertions != canonical_assertions:
        raise ValueError(f"{relative}: executed notebook source differs from its canonical assertion inventory")
    canonical_path = path.with_suffix(".canonical.ipynb") if canonical_path is None else canonical_path
    canonical_path.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(path, canonical_path)
    removed = remove_notebook_assertions(notebook, relative)
    if removed != canonical_assertions or notebook_assertion_count(notebook, relative):
        raise ValueError(f"{relative}: learner-facing notebook assertion removal is incomplete")
    for cell in notebook.cells:
        if cell.cell_type == "code":
            cell.execution_count = None
            cell.outputs = []
    canonical_execution = {
        "status": "passed",
        "sha256": digest(canonical_path),
        "assertions": canonical_assertions,
        "assertions_executed": canonical_assertions,
    }
    with warnings.catch_warnings():
        warnings.simplefilter("error")
        nbformat.write(notebook, path)
    return canonical_execution


def _install_lab_artifacts(
    path: Path,
    relative: str,
    html: str,
    markdown: str,
    resources: dict,
    lab_page: str,
) -> None:
    """Replace one lab's complete publication artifact set from staged files."""
    required_artifacts, resource_dir = lab_artifact_paths(relative)
    asset = Path(str(required_artifacts[0])[: -len(".html")])
    asset.parent.mkdir(parents=True, exist_ok=True)
    with TemporaryDirectory(prefix=f".{asset.name}-export-", dir=asset.parent) as directory:
        staging = Path(directory)
        staged_artifacts = [
            staging / f"{asset.name}.html",
            staging / f"{asset.name}.md",
            staging / f"{asset.name}.ipynb",
            staging / f"{asset.name}.mdx",
        ]
        staged_artifacts[0].write_text(html)
        staged_artifacts[1].write_text(markdown)
        shutil.copy2(path, staged_artifacts[2])
        staged_artifacts[3].write_text(lab_page)
        staged_resource_dir = staging / resource_dir.name
        staged_resource_dir.mkdir()
        for name, data in resources.get("outputs", {}).items():
            final_output = contained(asset.parent, name)
            if not final_output.is_relative_to(resource_dir.resolve()):
                raise ValueError(f"{relative}: exported resource escapes {resource_dir.name}: {name}")
            output = contained(staging, name)
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_bytes(data)

        destinations = [*required_artifacts, resource_dir]
        staged = [*staged_artifacts, staged_resource_dir]
        backups: list[tuple[Path, Path]] = []
        installed: list[Path] = []
        try:
            for number, destination in enumerate(destinations):
                destination.parent.mkdir(parents=True, exist_ok=True)
                if destination.exists():
                    backup = staging / f"backup-{number}"
                    destination.replace(backup)
                    backups.append((backup, destination))
            for source, destination in zip(staged, destinations, strict=True):
                source.replace(destination)
                installed.append(destination)
        except OSError:
            for destination in reversed(installed):
                if destination.is_dir():
                    shutil.rmtree(destination)
                elif destination.exists():
                    destination.unlink()
            for backup, destination in reversed(backups):
                backup.replace(destination)
            raise


def export_notebook(
    path: Path,
    relative: str,
    provenance: dict,
    canonical_execution: dict,
    *,
    publish: bool = True,
) -> dict:
    """Record a strict assertion-free execution and optionally publish its rich outputs."""
    notebook = _read_executed_notebook(path, relative)
    if notebook_learner_assertion_count(notebook, relative):
        raise ValueError(
            f"{relative}: learner-facing executed notebook contains assertions or explicit raise AssertionError"
        )
    _validate_assertion_coverage(notebook, relative)
    with warnings.catch_warnings():
        warnings.simplefilter("error")
        expected = nbformat.read(contained(NOTEBOOKS, relative), as_version=4)
    remove_notebook_assertions(expected, relative)
    expected_sources = [(cell.cell_type, cell.source) for cell in expected.cells]
    published_sources = [(cell.cell_type, cell.source) for cell in notebook.cells]
    if published_sources != expected_sources:
        raise ValueError(f"{relative}: learner-facing notebook source differs from the canonical publication source")
    evidence = {
        "notebook": relative,
        "code_cells": sum(c.cell_type == "code" for c in notebook.cells),
        "source_sha256": digest(NOTEBOOKS / relative),
        "canonical_execution": canonical_execution,
        "publication_execution": {
            "status": "passed",
            "sha256": digest(path),
            "assertions": 0,
            "assertions_executed": 0,
        },
        "fixtures_sha256": fixture_digest(),
        "dependencies_sha256": notebook_dependencies(relative),
        "runtime": runtime_identity(),
        "published": publish,
        **provenance,
    }
    if not publish:
        remove_lab_artifacts(relative)
        return {**evidence, "artifacts_sha256": {}}

    slug = str(Path(relative).with_suffix(""))
    asset = SITE / "public" / "lab-assets" / slug
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
            notebook, resources={"unique_key": asset.name, "output_files_dir": asset.name + "_files"}
        )
    lab_page = (
        "---\ntitle: "
        + json.dumps(title)
        + "\ndescription: "
        + json.dumps("Executed Python lab with complete source and rich results.")
        + "\n---\n\n"
        + f'<LabHtml src="/lab-assets/{slug}.html" />\n\n'
        + f"[Download executed notebook](/lab-assets/{slug}.ipynb) · [Download Markdown](/lab-assets/{slug}.md)\n"
    )
    _install_lab_artifacts(path, relative, html, markdown, resources, lab_page)
    return {
        **evidence,
        "html": f"/lab-assets/{slug}.html",
        "artifacts_sha256": current_lab_artifacts(relative),
    }


def execute_notebook_copy(copies: Path, relative: str, timeout: int) -> subprocess.CompletedProcess[str]:
    """Run one copied notebook through the original strict notebook runner."""
    env = execution_environment(BUILD)
    return subprocess.run(
        [
            sys.executable,
            str(NOTEBOOKS / "run_all_notebooks.py"),
            "--notebook-root",
            str(copies),
            "--directory",
            relative,
            "--save-outputs",
            "--timeout",
            str(timeout),
        ],
        capture_output=True,
        text=True,
        env=env,
        check=False,
    )


def report_execution_failure(relative: str, result: subprocess.CompletedProcess[str]) -> None:
    """Print a strict runner failure without losing captured diagnostics."""
    if result.stderr.strip() and not result.returncode:
        print(f"{relative}: notebook runner emitted stderr", file=sys.stderr, flush=True)
    print(result.stdout + result.stderr, file=sys.stderr, flush=True)


def restore_notebook_sources(copies: Path, names: set[str]) -> None:
    """Restore selected notebooks and dependencies to canonical authored source."""
    for name in names:
        target = contained(copies, name)
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(contained(NOTEBOOKS, name), target)


def remove_notebook_sources(copies: Path) -> None:
    """Remove notebooks so a run can see only its declared source closure."""
    for path in copies.rglob("*.ipynb"):
        path.unlink()


def strip_notebook_dependencies(copies: Path, names: set[str], selected: str) -> None:
    """Project assertion-free dependency sources for learner execution."""
    for name in names - {selected}:
        path = contained(copies, name)
        with warnings.catch_warnings():
            warnings.simplefilter("error")
            notebook = nbformat.read(path, as_version=4)
        remove_notebook_assertions(notebook, name)
        with warnings.catch_warnings():
            warnings.simplefilter("error")
            nbformat.write(notebook, path)


def build_notebook_copy(
    copies: Path,
    canonical_root: Path,
    publication_root: Path,
    relative: str,
    timeout: int,
    provenance: dict,
    *,
    publish: bool = True,
) -> tuple[dict | None, subprocess.CompletedProcess[str] | None]:
    """Execute canonical and learner sources, returning evidence or the failed run."""
    visible = notebook_closure([relative], NOTEBOOKS)
    remove_notebook_sources(copies)
    restore_notebook_sources(copies, visible)
    try:
        result = execute_notebook_copy(copies, relative, timeout)
        if result.returncode or result.stderr.strip():
            return None, result
        canonical_path = contained(canonical_root, relative)
        canonical_execution = prepare_publication_notebook(copies / relative, relative, canonical_path)
        strip_notebook_dependencies(copies, visible, relative)
        result = execute_notebook_copy(copies, relative, timeout)
        if result.returncode or result.stderr.strip():
            return None, result
        publication_path = contained(publication_root, relative)
        publication_path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(copies / relative, publication_path)
        canonical_notebook = _read_executed_notebook(canonical_path, relative)
        publication_notebook = _read_executed_notebook(publication_path, relative)
        validate_execution_equivalence(canonical_notebook, publication_notebook, relative)
        return (
            export_notebook(
                publication_path,
                relative,
                provenance,
                canonical_execution,
                publish=publish,
            ),
            None,
        )
    finally:
        restore_notebook_sources(copies, visible)


def _finish_lab_build(
    snapshot: dict,
    selected: list[str],
    evidence: dict[str, dict],
    failures: list[str],
    completed_all: bool,
    full_build: bool,
) -> None:
    """Verify source stability, prune on full success, and write evidence."""
    after = source_notebooks()
    if (
        snapshot["selected_sources"] != {name: after.get(name) for name in selected}
        or snapshot["fixtures"] != fixture_digest()
        or snapshot["runtime"] != runtime_identity()
        or snapshot["dependencies"] != {name: notebook_dependencies(name) for name in selected}
        or snapshot["provenance"] != lab_provenance()
        or snapshot["published_notebooks"] != published_notebooks()
    ):
        raise RuntimeError(
            "Selected source notebooks or shared fixtures changed during execution; repeat against a stable checkout"
        )
    if completed_all and not failures and full_build:
        prune_unrecorded_lab_artifacts(list(evidence.values()))
    write_json(
        BUILD / "labs.json",
        {
            **snapshot["provenance"],
            "labs": list(evidence.values()),
            "failed": failures,
        },
    )


def main() -> int:
    """Build selected labs, or the whole source notebook collection by default."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--notebook", action="append", help="Notebook-relative path; repeat to select several")
    parser.add_argument(
        "--check-sources",
        action="store_true",
        help="Validate selected notebook structure and cleanliness without executing them",
    )
    parser.add_argument(
        "--curriculum-only", action="store_true", help="Execute the manifest's lab and example mappings"
    )
    parser.add_argument("--timeout", type=int, default=600, help="Per-cell timeout passed to the original runner")
    args = parser.parse_args()
    before = source_notebooks()
    selected = args.notebook
    if not selected and args.curriculum_only:
        selected = sorted({name for lesson in curriculum() for name in lesson_notebook_names(lesson)})
    selected = selected or sorted(before)
    full_build = set(selected) == set(before)
    for relative in selected:
        if not contained(NOTEBOOKS, relative).is_file():
            parser.error(f"Missing notebook: {relative}")
        require_clean_source(relative)
    if args.check_sources:
        print(f"Notebook source preflight passed: {len(selected)} notebooks", flush=True)
        return 0
    current_lab_artifact_tree()
    snapshot = {
        "selected_sources": {name: before[name] for name in selected},
        "fixtures": fixture_digest(),
        "runtime": runtime_identity(),
        "dependencies": {name: notebook_dependencies(name) for name in selected},
        "provenance": lab_provenance(),
        "published_notebooks": published_notebooks(),
    }
    # Focused runs use a fresh working tree and preserve unrelated executed copies.
    with TemporaryDirectory(prefix=".lab-run-", dir=SITE) as directory:
        copies = Path(directory) / "notebooks"
        canonical_root = Path(directory) / "canonical-notebooks"
        publication_root = Path(directory) / "publication-notebooks"
        # Copy the full tree: notebooks may load sibling data and shared helpers.
        shutil.copytree(
            NOTEBOOKS,
            copies,
            dirs_exist_ok=True,
            ignore=shutil.ignore_patterns("__pycache__", ".ipynb_checkpoints", "*.html", "*.pdf", "*.ipynb"),
        )
        evidence_path = BUILD / "labs.json"
        existing = preserved_lab_entries(evidence_path, selected, before)
        evidence = {entry["notebook"]: entry for entry in existing if entry["notebook"] not in selected}
        failures = []
        completed_all = False
        try:
            for relative in selected:
                started = time.monotonic()
                entry, failure = build_notebook_copy(
                    copies,
                    canonical_root,
                    publication_root,
                    relative,
                    args.timeout,
                    snapshot["provenance"],
                    publish=relative in snapshot["published_notebooks"],
                )
                if failure is not None:
                    failures.append(relative)
                    report_execution_failure(relative, failure)
                    continue
                evidence[relative] = entry
                published_copy = BUILD / "notebooks" / relative
                published_copy.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(publication_root / relative, published_copy)
                canonical_copy = BUILD / "canonical-notebooks" / relative
                canonical_copy.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(canonical_root / relative, canonical_copy)
                evidence[relative]["execution_and_export_seconds"] = time.monotonic() - started
                print(f"Lab passed: {relative}", flush=True)
            completed_all = True
        finally:
            _finish_lab_build(snapshot, selected, evidence, failures, completed_all, full_build)
    return int(bool(failures))


if __name__ == "__main__":
    raise SystemExit(main())
