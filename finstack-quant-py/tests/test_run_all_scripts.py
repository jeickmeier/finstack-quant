"""Tests for the example script runner."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import sys
from types import ModuleType

import pytest

RUNNER_PATH = Path(__file__).resolve().parents[1] / "examples" / "scripts" / "run_all_scripts.py"


@pytest.fixture
def module() -> ModuleType:
    """Load the example script runner module from disk."""
    assert RUNNER_PATH.exists(), f"Missing runner at {RUNNER_PATH}"

    spec = importlib.util.spec_from_file_location("run_all_scripts", RUNNER_PATH)
    assert spec is not None
    assert spec.loader is not None

    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_find_scripts_skips_runner_and_checkpoints(module: ModuleType, tmp_path: Path) -> None:
    """Discovery should skip the runner file and notebook checkpoints."""
    (tmp_path / "01_ok.py").write_text("print('ok')\n", encoding="utf-8")
    (tmp_path / "run_all_scripts.py").write_text("# runner\n", encoding="utf-8")

    checkpoints = tmp_path / ".ipynb_checkpoints"
    checkpoints.mkdir()
    (checkpoints / "bad.py").write_text("raise RuntimeError('skip')\n", encoding="utf-8")

    scripts = module.find_scripts(tmp_path)

    assert [path.name for path in scripts] == ["01_ok.py"]


def test_run_script_reports_success(module: ModuleType, tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """A successful script should return stdout and a zero-style status."""
    script = tmp_path / "success.py"
    script.write_text("print('success')\n", encoding="utf-8")
    monkeypatch.setattr(module, "PYTHON_RUNNER", (sys.executable,))

    ok, message, elapsed = module.run_script(script, timeout=5)

    assert ok is True
    assert "success" in message
    assert elapsed >= 0


def test_run_script_reports_failure(module: ModuleType, tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """A failing script should return stderr and a non-zero-style status."""
    script = tmp_path / "failure.py"
    script.write_text("raise RuntimeError('boom')\n", encoding="utf-8")
    monkeypatch.setattr(module, "PYTHON_RUNNER", (sys.executable,))

    ok, message, elapsed = module.run_script(script, timeout=5)

    assert ok is False
    assert "boom" in message
    assert elapsed >= 0


def test_example_modules_export_symbols_used_by_scripts(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Top-level Python shims should re-export symbols used by example scripts."""
    package_root = RUNNER_PATH.parents[1]
    monkeypatch.syspath_prepend(str(package_root))

    from finstack_quant import analytics, portfolio, statements_analytics, valuations

    analytics_exports = {
        "Performance",
        "BetaResult",
        "GreeksResult",
        "MultiFactorResult",
        "LookbackReturns",
        "PeriodStats",
        "RollingGreeks",
        "DatedSeries",
    }
    statements_analytics_exports = {
        "compute_multiple",
        "peer_stats",
        "percentile_rank",
        "regression_fair_value",
        "score_relative_value",
        "z_score",
    }
    valuation_exports = {
        "bs_cos_price",
        "merton_jump_cos_price",
        "vg_cos_price",
    }
    portfolio_exports = {
        "almgren_chriss_impact",
        "amihud_illiquidity",
        "days_to_liquidate",
        "kyle_lambda",
        "liquidity_tier",
        "lvar_bangia",
        "roll_effective_spread",
    }

    assert analytics_exports.issubset(set(analytics.__all__))
    assert statements_analytics_exports.issubset(set(statements_analytics.__all__))
    assert valuation_exports.issubset(set(valuations.__all__))
    assert portfolio_exports.issubset(set(portfolio.__all__))
