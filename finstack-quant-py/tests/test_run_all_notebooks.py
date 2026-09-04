"""Tests for the example notebook runner."""

from __future__ import annotations

import importlib.util
import json
import os
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


def test_configure_pythonpath_promotes_selected_root(
    module: ModuleType, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """A selected copy must outrank every previously configured notebook tree."""
    selected = (tmp_path / "selected").resolve()
    selected.mkdir()
    package = module.NOTEBOOKS_DIR.parents[2] / "finstack-quant-py"
    repository = module.NOTEBOOKS_DIR.parents[2]
    monkeypatch.setenv(
        "PYTHONPATH",
        os.pathsep.join([str(module.NOTEBOOKS_DIR), str(selected), str(package), "/other"]),
    )

    module._configure_pythonpath(selected)

    paths = os.environ["PYTHONPATH"].split(os.pathsep)
    assert paths[:3] == [str(selected), str(package), str(repository)]
    assert paths.count(str(selected)) == 1


def test_cli_executes_copy_outside_original_root(
    module: ModuleType, tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    """A copied tree supplies its own shared inputs while retaining package discovery."""
    source = tmp_path / "authored.ipynb"
    _write_notebook(source, "from _shared import DEMO_AS_OF\nassert DEMO_AS_OF.year == 2031\nprint('copied tree')")
    before = source.read_bytes()
    copy_root = tmp_path / "build"
    copy_root.mkdir()
    shared = copy_root / "_shared"
    shared.mkdir()
    (shared / "__init__.py").write_text("from datetime import date\nDEMO_AS_OF = date(2031, 1, 15)\n")
    copy = copy_root / source.name
    copy.write_bytes(before)
    monkeypatch.setattr("sys.argv", [str(RUNNER_PATH), "--notebook-root", str(copy_root), "--save-outputs"])
    assert module.main() == 0
    assert source.read_bytes() == before
    assert nbformat.read(copy, as_version=4).cells[0].outputs
    assert "authored.ipynb" in capsys.readouterr().out


def test_cli_rejects_notebook_outside_selected_root(
    module: ModuleType, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    _write_notebook(tmp_path / "outside.ipynb", "assert True")
    root = tmp_path / "selected"
    root.mkdir()
    monkeypatch.setattr("sys.argv", [str(RUNNER_PATH), "--notebook-root", str(root), "--directory", "../outside.ipynb"])
    with pytest.raises(SystemExit, match="2"):
        module.main()


def test_run_notebook_reports_success(module: ModuleType, tmp_path: Path) -> None:
    """A successful notebook should report success and elapsed time."""
    notebook = tmp_path / "success.ipynb"
    _write_notebook(notebook, "print('success')")

    ok, message, elapsed = module.run_notebook(notebook, timeout=30)

    assert ok is True
    assert "Executed 1 code cells" in message
    assert elapsed >= 0


def test_run_notebook_records_reachable_assertion_coverage(module: ModuleType, tmp_path: Path) -> None:
    """Saved build copies record every assertion location reached by the kernel."""
    notebook = tmp_path / "covered.ipynb"
    _write_notebook(notebook, "value = 42\nassert value == 42\nprint(value)")

    ok, message, _elapsed = module.run_notebook(notebook, timeout=30, save_outputs=True)

    assert ok is True, message
    executed = nbformat.read(notebook, as_version=4)
    assert executed.metadata["finstack_execution"] == {
        "assertions_total": 1,
        "assertions_executed": 1,
        "assertion_locations": ["1:2:0"],
    }
    assert executed.cells[0].source == "value = 42\nassert value == 42\nprint(value)"


def test_run_notebook_rejects_unreachable_assertion(module: ModuleType, tmp_path: Path) -> None:
    """An assertion that never runs cannot support notebook publication."""
    notebook = tmp_path / "unreachable.ipynb"
    _write_notebook(notebook, "if False:\n    assert False\nprint('no validation')")
    before = notebook.read_bytes()

    ok, message, _elapsed = module.run_notebook(notebook, timeout=30, save_outputs=True)

    assert ok is False
    assert "did not execute every assertion statement" in message
    assert notebook.read_bytes() == before


def test_run_notebook_rejects_swallowed_assertion_failure(module: ModuleType, tmp_path: Path) -> None:
    """A caught AssertionError cannot be reported as a successful validation check."""
    notebook = tmp_path / "swallowed.ipynb"
    _write_notebook(
        notebook,
        "try:\n    assert False, 'failed check'\nexcept AssertionError:\n    pass\nprint('continued')",
    )
    before = notebook.read_bytes()

    ok, message, _elapsed = module.run_notebook(notebook, timeout=30, save_outputs=True)

    assert ok is False
    assert "did not execute every assertion statement" in message
    assert notebook.read_bytes() == before


def test_run_notebook_uses_ipc_transport(module: ModuleType, tmp_path: Path) -> None:
    """Notebook kernels should communicate over local IPC rather than plaintext TCP."""
    notebook = tmp_path / "ipc_transport.ipynb"
    _write_notebook(
        notebook,
        "from ipykernel.connect import get_connection_info\n"
        "assert get_connection_info(unpack=True)['transport'] == 'ipc'",
    )

    ok, message, _elapsed = module.run_notebook(notebook, timeout=30)

    assert ok is True, message


def test_run_notebook_reports_failure(module: ModuleType, tmp_path: Path) -> None:
    """A failing notebook should return a concise error message."""
    notebook = tmp_path / "failure.ipynb"
    _write_notebook(notebook, "raise RuntimeError('boom')")

    ok, message, elapsed = module.run_notebook(notebook, timeout=30)

    assert ok is False
    assert "boom" in message
    assert elapsed >= 0


def test_run_notebook_rejects_python_warning_without_saving_source(module: ModuleType, tmp_path: Path) -> None:
    """A warning is a failed verification run and must not persist outputs."""
    notebook = tmp_path / "warning.ipynb"
    _write_notebook(notebook, "import warnings\nwarnings.warn('visible warning', UserWarning)")
    before = notebook.read_bytes()

    ok, message, _elapsed = module.run_notebook(notebook, timeout=30, save_outputs=True)

    assert ok is False
    assert "visible warning" in message
    assert notebook.read_bytes() == before


def test_run_notebook_rejects_stderr_without_saving_source(module: ModuleType, tmp_path: Path) -> None:
    """Nonempty stderr fails even when the cell raises no Python exception."""
    notebook = tmp_path / "stderr.ipynb"
    _write_notebook(notebook, "import sys\nprint('plain stderr diagnostic', file=sys.stderr)")
    before = notebook.read_bytes()

    ok, message, _elapsed = module.run_notebook(notebook, timeout=30, save_outputs=True)

    assert ok is False
    assert "plain stderr diagnostic" in message
    assert notebook.read_bytes() == before


def test_run_notebook_rejects_missing_cell_id_without_saving_source(module: ModuleType, tmp_path: Path) -> None:
    """A raw v4.5 notebook must retain explicit IDs instead of relying on normalization."""
    notebook = tmp_path / "missing_id.ipynb"
    notebook.write_text(
        json.dumps({
            "cells": [
                {
                    "cell_type": "code",
                    "execution_count": None,
                    "metadata": {},
                    "outputs": [],
                    "source": ["assert True"],
                }
            ],
            "metadata": {},
            "nbformat": 4,
            "nbformat_minor": 5,
        }),
        encoding="utf-8",
    )
    before = notebook.read_bytes()

    ok, message, _elapsed = module.run_notebook(notebook, timeout=30, save_outputs=True)

    assert ok is False
    assert "missing an id field" in message
    assert notebook.read_bytes() == before


def test_run_notebook_rejects_invalid_schema_without_saving_source(module: ModuleType, tmp_path: Path) -> None:
    """Schema-invalid notebooks fail before a kernel starts or outputs are saved."""
    notebook = tmp_path / "invalid_schema.ipynb"
    notebook.write_text(
        json.dumps({
            "cells": [
                {
                    "id": "invalid-metadata",
                    "cell_type": "code",
                    "execution_count": None,
                    "metadata": {},
                    "outputs": [],
                    "source": 42,
                }
            ],
            "metadata": {},
            "nbformat": 4,
            "nbformat_minor": 5,
        }),
        encoding="utf-8",
    )
    before = notebook.read_bytes()

    ok, message, _elapsed = module.run_notebook(notebook, timeout=30, save_outputs=True)

    assert ok is False
    assert "not valid" in message or "not of type" in message
    assert notebook.read_bytes() == before


@pytest.mark.parametrize("tag", ["skip-execution", "raises-exception"])
def test_execution_tags_cannot_hide_failures(module: ModuleType, tmp_path: Path, tag: str) -> None:
    """Every nonempty lesson cell must execute and unexpected errors must fail."""
    path = tmp_path / "tagged.ipynb"
    notebook = new_notebook(cells=[new_code_cell("assert 2 + 2 == 5", metadata={"tags": [tag]})])
    path.write_text(nbformat.writes(notebook))
    ok, _, _ = module.run_notebook(path, timeout=30)
    assert not ok


def test_optimized_environment_keeps_notebook_assertions(
    module: ModuleType, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """The kernel must enforce checks even when its caller is optimized."""
    monkeypatch.setenv("PYTHONOPTIMIZE", "1")
    path = tmp_path / "optimized.ipynb"
    _write_notebook(path, "assert 2 + 2 == 5")
    ok, message, _ = module.run_notebook(path, timeout=30)
    assert not ok
    assert "AssertionError" in message


def test_run_notebook_exposes_shared_helpers(module: ModuleType, tmp_path: Path) -> None:
    """Notebook kernels should be able to import the example-only shared package."""
    notebook = tmp_path / "shared_helpers.ipynb"
    _write_notebook(
        notebook,
        "from _shared import DEMO_AS_OF\nassert DEMO_AS_OF.isoformat() == '2025-01-15'",
    )

    ok, message, _elapsed = module.run_notebook(notebook, timeout=30)

    assert ok is True, message


def test_vol_surfaces_notebook_runs_successfully(module: ModuleType) -> None:
    """The volatility surfaces notebook should execute end-to-end."""
    notebook = NOTEBOOKS_DIR / "01_foundations" / "market_data" / "vol_surfaces.ipynb"

    ok, message, _elapsed = module.run_notebook(notebook, timeout=60)

    assert ok is True, message


def test_liquidity_notebook_runs_successfully(module: ModuleType) -> None:
    """The liquidity risk notebook should execute end-to-end."""
    notebook = NOTEBOOKS_DIR / "05_portfolio" / "liquidity_risk.ipynb"

    ok, message, _elapsed = module.run_notebook(notebook, timeout=60)

    assert ok is True, message


def test_credit_factor_hierarchy_notebook_runs_successfully(module: ModuleType) -> None:
    """The credit factor hierarchy notebook should execute end-to-end."""
    notebook = NOTEBOOKS_DIR / "05_portfolio" / "credit_factor_hierarchy.ipynb"

    ok, message, _elapsed = module.run_notebook(notebook, timeout=60)

    assert ok is True, message
