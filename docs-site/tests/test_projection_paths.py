"""Regression tests for isolated learner notebook projections."""

from __future__ import annotations

import os
from pathlib import Path
import subprocess
import sys

import nbformat
from publication_source import notebook_assertion_count
import pytest


def _write_notebook(path: Path, source: str, dependencies: list[str]) -> None:
    notebook = nbformat.v4.new_notebook(
        cells=[nbformat.v4.new_code_cell(source)],
        metadata={"analyst_dependencies": dependencies},
    )
    nbformat.write(notebook, path)


def test_publication_projection_preserves_repo_anchor_and_recursive_dependencies(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    import run_lesson_snippets as runner

    site = tmp_path / "repo" / "docs-site"
    site.mkdir(parents=True)
    monkeypatch.setattr(runner, "SITE", site)
    source_root = tmp_path / "source-notebooks"
    shared = source_root / "_shared"
    shared.mkdir(parents=True)
    (shared / "__init__.py").write_text("")
    (shared / "fixture.py").write_text(
        "from pathlib import Path\nVALUE = (Path(__file__).resolve().parents[4] / 'fixture.txt').read_text()\n"
    )
    (site.parent / "fixture.txt").write_text("projected fixture")
    _write_notebook(source_root / "leaf.ipynb", "assert True\nleaf = 1\n", [])
    _write_notebook(source_root / "middle.ipynb", "middle = 2\n", ["leaf.ipynb"])
    _write_notebook(source_root / "main.ipynb", "main = 3\n", ["middle.ipynb"])
    record = {"labs": ["main.ipynb"], "examples": []}

    with runner.publication_notebook_root(record, source_root) as projected:
        assert projected.parent.parent == runner.SITE
        assert {path.name for path in projected.glob("*.ipynb")} == {
            "leaf.ipynb",
            "middle.ipynb",
            "main.ipynb",
        }
        leaf = nbformat.read(projected / "leaf.ipynb", as_version=4)
        assert notebook_assertion_count(leaf) == 0
        env = os.environ.copy()
        env["PYTHONPATH"] = str(projected)
        completed = subprocess.run(
            [sys.executable, "-c", "from _shared.fixture import VALUE; print(VALUE)"],
            capture_output=True,
            text=True,
            env=env,
            check=False,
        )
        assert completed.returncode == 0, completed.stderr
        assert completed.stdout.strip() == "projected fixture"

    original = nbformat.read(source_root / "leaf.ipynb", as_version=4)
    assert notebook_assertion_count(original) == 1
