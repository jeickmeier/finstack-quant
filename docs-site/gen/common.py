"""Shared paths and content parsing for the documentation build."""

from __future__ import annotations

import hashlib
import importlib
from importlib import metadata
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tomllib

import yaml

SITE = Path(__file__).resolve().parents[1]
REPO = SITE.parent
NOTEBOOKS = REPO / "finstack-quant-py" / "examples" / "notebooks"
CONTENT = SITE / "content" / "learn"
BUILD = SITE / ".build"
ID_PATTERN = re.compile(r"(?:[1-4]\.[1-8]|C[1-5]|V[1-3]|capstone)\Z")
LAB_EXECUTION_POLICY = {
    "python_warnings": "error",
    "stderr": "forbidden",
    "cell_errors": "forbidden",
    "skipped_nonempty_cells": "forbidden",
    "canonical_assertions": "all_reached_and_succeeded",
    "published_assertions": "removed",
    "published_code": "executed",
}
SNIPPET_EXECUTION_POLICY = {
    "python_warnings": "error",
    "stderr": "forbidden",
    "exceptions": "forbidden",
    "canonical_assertions": "all_reached_and_succeeded",
    "published_assertions": "removed",
    "published_code": "executed",
}
PUBLICATION_EXECUTION_INPUTS = {
    "FINSTACK_RUN_XL_NOTEBOOK_BENCH": "0",
    "FINSTACK_RUN_EXTREME_ATTRIBUTION_NOTEBOOK_BENCH": "0",
}


def curriculum(site: Path = SITE) -> list[dict]:
    """Read ordered curriculum records from the sole mapping manifest."""
    with (site / "curriculum.toml").open("rb") as handle:
        return tomllib.load(handle).get("lesson", [])


def lesson_labs(record: dict) -> list[str]:
    """Return common and variant-specific lab mappings in manifest order."""
    return [
        *record.get("labs", []),
        *(lab for labs in record.get("variant_labs", {}).values() for lab in labs),
    ]


def lesson_notebook_names(record: dict) -> list[str]:
    """Return every lab and supporting example mapped to one lesson."""
    return [*lesson_labs(record), *record.get("examples", [])]


def frontmatter(path: Path) -> tuple[dict, str]:
    """Read YAML frontmatter without interpreting MDX body expressions."""
    text = path.read_text()
    match = re.match(r"\A---\r?\n(.*?)\r?\n---(?:\r?\n|\Z)", text, re.S)
    if not match:
        raise ValueError(f"{path}: missing YAML frontmatter")
    metadata = yaml.safe_load(match[1])
    if not isinstance(metadata, dict):
        raise TypeError(f"{path}: frontmatter must be a mapping")
    return metadata, text[match.end() :]


def contained(root: Path, relative: str) -> Path:
    """Resolve a relative content path and reject traversal outside its root."""
    path = (root / relative).resolve()
    if not path.is_relative_to(root.resolve()) or Path(relative).is_absolute():
        raise ValueError(f"Path must remain inside {root}: {relative}")
    return path


def digest(path: Path) -> str:
    """Return the SHA-256 of one source file."""
    return hashlib.sha256(path.read_bytes()).hexdigest()


def runtime_identity() -> dict[str, object]:
    """Identify the loaded extension and Python interpreter for build evidence."""
    extension = importlib.import_module("finstack_quant.finstack_quant")
    binary = Path(extension.__file__)
    python_sources = hashlib.sha256()
    package = REPO / "finstack-quant-py" / "finstack_quant"
    for path in sorted(package.rglob("*.py")):
        python_sources.update(str(path.relative_to(package)).encode())
        python_sources.update(path.read_bytes())
    execution_packages = {
        name: metadata.version(name)
        for name in ("ipykernel", "matplotlib", "nbclient", "nbconvert", "nbformat", "numpy", "pandas")
    }
    return {
        "python": sys.version.split()[0],
        "python_executable": str(Path(sys.executable).resolve()),
        "package": extension.__version__,
        "extension_sha256": digest(binary),
        "python_sources_sha256": python_sources.hexdigest(),
        "uv_lock_sha256": digest(REPO / "uv.lock"),
        "execution_packages": execution_packages,
        "publication_execution_inputs": dict(PUBLICATION_EXECUTION_INPUTS),
    }


def execution_environment(build_root: Path = BUILD) -> dict[str, str]:
    """Return the strict worker environment after preparing writable plot caches."""
    env = os.environ.copy()
    matplotlib_cache = build_root / "cache" / "matplotlib"
    xdg_cache = build_root / "cache" / "xdg"
    matplotlib_cache.mkdir(parents=True, exist_ok=True)
    xdg_cache.mkdir(parents=True, exist_ok=True)
    env["MPLBACKEND"] = "Agg"
    env["MPLCONFIGDIR"] = str(matplotlib_cache)
    env["XDG_CACHE_HOME"] = str(xdg_cache)
    env.update(PUBLICATION_EXECUTION_INPUTS)
    if not any(matplotlib_cache.glob("fontlist-v*.json")):
        completed = subprocess.run(
            [
                sys.executable,
                "-c",
                "from matplotlib import font_manager; font_manager._load_fontmanager()",
            ],
            capture_output=True,
            text=True,
            env=env,
            check=False,
        )
        allowed = "Matplotlib is building the font cache; this may take a moment."
        diagnostics = [line for line in completed.stderr.splitlines() if line.strip() != allowed]
        if completed.returncode or diagnostics:
            detail = "\n".join([completed.stdout, *diagnostics]).strip()
            raise RuntimeError(f"Could not prepare the Matplotlib cache: {detail}")
    return env


def fixture_digest() -> str:
    """Fingerprint shared factories and notebook data, excluding generated outputs."""
    value = hashlib.sha256()
    for path in sorted(NOTEBOOKS.rglob("*")):
        if (
            path.is_file()
            and not {"__pycache__", ".ipynb_checkpoints"}.intersection(path.parts)
            and path.suffix in {".py", ".json", ".csv", ".toml", ".yaml", ".yml", ".parquet"}
        ):
            value.update(str(path.relative_to(NOTEBOOKS)).encode())
            value.update(path.read_bytes())
    # Notebooks read these repository inputs directly rather than copying them
    # into the notebook tree. Keep every such input in the same freshness gate.
    external_inputs = [
        *sorted((REPO / "finstack-quant" / "calibration" / "examples").rglob("*.json")),
        REPO / "finstack-quant" / "models" / "data" / "defaults" / "pricer_defaults.v1.json",
    ]
    for path in external_inputs:
        value.update(str(path.relative_to(REPO)).encode())
        value.update(path.read_bytes())
    return value.hexdigest()


def notebook_closure(names: list[str], notebook_root: Path = NOTEBOOKS) -> set[str]:
    """Return mapped notebooks and their recursively declared source dependencies."""
    pending = list(names)
    resolved: set[str] = set()
    while pending:
        name = pending.pop()
        if name in resolved:
            continue
        path = contained(notebook_root, name)
        notebook = json.loads(path.read_text())
        dependencies = notebook.get("metadata", {}).get("analyst_dependencies", [])
        if not isinstance(dependencies, list) or not all(isinstance(item, str) for item in dependencies):
            raise ValueError(f"{name}: analyst_dependencies must list notebook-relative paths")
        resolved.add(name)
        pending.extend(dependencies)
    return resolved


def lesson_notebooks(record: dict, notebook_root: Path = NOTEBOOKS) -> dict[str, str]:
    """Fingerprint every mapped notebook and recursively declared source dependency."""
    names = notebook_closure(lesson_notebook_names(record), notebook_root)
    return {name: digest(contained(notebook_root, name)) for name in sorted(names)}


def write_json(path: Path, value: object) -> None:
    """Write a readable UTF-8 build artifact using a replace operation."""
    path.parent.mkdir(parents=True, exist_ok=True)
    temp = path.with_suffix(path.suffix + ".tmp")
    temp.write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n")
    temp.replace(path)
