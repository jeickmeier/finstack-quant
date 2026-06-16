"""Tests for the example notebook runner."""

from __future__ import annotations

import importlib.util
from pathlib import Path
from types import ModuleType

import nbformat
from nbformat.v4 import new_code_cell, new_notebook
import pytest

RUNNER_PATH = Path(__file__).resolve().parents[1] / "examples" / "notebooks" / "run_all_notebooks.py"
NOTEBOOKS_DIR = RUNNER_PATH.parent


@pytest.fixture
def module() -> ModuleType:
    """Load the example notebook runner module from disk."""
    assert RUNNER_PATH.exists(), f"Missing runner at {RUNNER_PATH}"

    spec = importlib.util.spec_from_file_location("run_all_notebooks", RUNNER_PATH)
    assert spec is not None
    assert spec.loader is not None

    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _write_notebook(path: Path, source: str) -> None:
    """Create a minimal executable notebook at *path*."""
    notebook = new_notebook(cells=[new_code_cell(source=source)])
    path.write_text(nbformat.writes(notebook), encoding="utf-8")


def test_find_notebooks_skips_checkpoints(module: ModuleType, tmp_path: Path) -> None:
    """Discovery should skip notebook checkpoints."""
    _write_notebook(tmp_path / "01_ok.ipynb", "print('ok')")

    checkpoints = tmp_path / ".ipynb_checkpoints"
    checkpoints.mkdir()
    _write_notebook(checkpoints / "bad.ipynb", "raise RuntimeError('skip')")

    notebooks = module.find_notebooks(tmp_path)

    assert [path.name for path in notebooks] == ["01_ok.ipynb"]


def test_run_notebook_reports_success(module: ModuleType, tmp_path: Path) -> None:
    """A successful notebook should report success and elapsed time."""
    notebook = tmp_path / "success.ipynb"
    _write_notebook(notebook, "print('success')")

    ok, message, elapsed = module.run_notebook(notebook, timeout=30)

    assert ok is True
    assert "Executed 1 code cells" in message
    assert elapsed >= 0


def test_run_notebook_reports_failure(module: ModuleType, tmp_path: Path) -> None:
    """A failing notebook should return a concise error message."""
    notebook = tmp_path / "failure.ipynb"
    _write_notebook(notebook, "raise RuntimeError('boom')")

    ok, message, elapsed = module.run_notebook(notebook, timeout=30)

    assert ok is False
    assert "boom" in message
    assert elapsed >= 0


def test_vol_surfaces_notebook_runs_successfully(module: ModuleType) -> None:
    """The volatility surfaces notebook should execute end-to-end."""
    notebook = NOTEBOOKS_DIR / "01_foundations" / "market_data" / "vol_surfaces.ipynb"

    ok, message, _elapsed = module.run_notebook(notebook, timeout=60)

    assert ok is True, message


def test_liquidity_notebook_runs_successfully(module: ModuleType) -> None:
    """The liquidity risk notebook should execute end-to-end."""
    notebook = NOTEBOOKS_DIR / "05_portfolio_and_scenarios" / "liquidity_risk.ipynb"

    ok, message, _elapsed = module.run_notebook(notebook, timeout=60)

    assert ok is True, message


def test_credit_factor_hierarchy_notebook_runs_successfully(module: ModuleType) -> None:
    """The credit factor hierarchy notebook should execute end-to-end."""
    notebook = NOTEBOOKS_DIR / "05_portfolio_and_scenarios" / "credit_factor_hierarchy.ipynb"

    ok, message, _elapsed = module.run_notebook(notebook, timeout=60)

    assert ok is True, message
