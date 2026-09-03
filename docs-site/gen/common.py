"""Shared paths and content parsing for the documentation build."""

from __future__ import annotations

import hashlib
import importlib
import json
from pathlib import Path
import re
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
}


def curriculum(site: Path = SITE) -> list[dict]:
    """Read ordered curriculum records from the sole mapping manifest."""
    with (site / "curriculum.toml").open("rb") as handle:
        return tomllib.load(handle).get("lesson", [])


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


def runtime_identity() -> dict[str, str]:
    """Identify the loaded extension and Python interpreter for build evidence."""
    extension = importlib.import_module("finstack_quant.finstack_quant")
    binary = Path(extension.__file__)
    python_sources = hashlib.sha256()
    package = REPO / "finstack-quant-py" / "finstack_quant"
    for path in sorted(package.rglob("*.py")):
        python_sources.update(str(path.relative_to(package)).encode())
        python_sources.update(path.read_bytes())
    return {
        "python": sys.version.split()[0],
        "package": extension.__version__,
        "extension_sha256": digest(binary),
        "python_sources_sha256": python_sources.hexdigest(),
    }


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
    # Shared analyst markets and the bootstrap tour consume these canonical inputs.
    for path in sorted((REPO / "finstack-quant" / "calibration" / "examples").rglob("*.json")):
        value.update(str(path.relative_to(REPO)).encode())
        value.update(path.read_bytes())
    return value.hexdigest()


def lesson_notebooks(record: dict, notebook_root: Path = NOTEBOOKS) -> dict[str, str]:
    """Fingerprint every mapped notebook used by a lesson or its visible baseline."""
    return {
        name: digest(contained(notebook_root, name))
        for name in sorted(set(record.get("labs", []) + record.get("examples", [])))
    }


def write_json(path: Path, value: object) -> None:
    """Write a readable UTF-8 build artifact using a replace operation."""
    path.parent.mkdir(parents=True, exist_ok=True)
    temp = path.with_suffix(path.suffix + ".tmp")
    temp.write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n")
    temp.replace(path)
