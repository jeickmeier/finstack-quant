"""Fixture-driven numerical parity tests for the `Performance` binding.

Loads the shared `api_invariants_data.json` fixture and confirms that
Python `Performance` methods reproduce the regression-style expected
values for `rolling_greeks` and `multi_factor_greeks`. Methods whose
expected values come from freestanding-function helpers with explicit
`CagrBasis` / `ann_factor` are covered by the Rust parity test instead;
this file focuses on the Performance-mediated path.
"""

from __future__ import annotations

from datetime import date
import json
from pathlib import Path
from typing import Any

import pytest

from finstack_quant.analytics import Performance


def _fixture() -> dict[str, Any]:
    path = Path(__file__).resolve().parents[2] / "finstack-quant" / "analytics" / "src" / "api_invariants_data.json"
    return json.loads(path.read_text())


def _dates(values: list[str]) -> list[date]:
    return [date.fromisoformat(value) for value in values]


def _assert_sequence_close(actual: list[float], expected: list[float]) -> None:
    assert actual == pytest.approx(expected, abs=1e-12)


def _build_performance(fixture: dict[str, Any]) -> Performance:
    """Build a two-ticker Performance from the fixture (target + benchmark)."""
    dates = _dates(fixture["dates"])
    target = fixture["returns"]
    benchmark = fixture["benchmark"]
    return Performance.from_returns_arrays(
        dates,
        [target, benchmark],
        ["TARGET", "BENCH"],
        benchmark_ticker="BENCH",
        freq="daily",
    )


def test_performance_rolling_greeks_matches_fixture() -> None:
    fixture = _fixture()
    expected = fixture["expected"]["rolling_greeks"]

    perf = _build_performance(fixture)
    rolling = perf.rolling_greeks(0, window=5)

    _assert_sequence_close(rolling.alphas, expected["alphas"])
    _assert_sequence_close(rolling.betas, expected["betas"])


def test_performance_multi_factor_greeks_matches_fixture() -> None:
    fixture = _fixture()
    expected = fixture["expected"]["multi_factor_greeks"]

    perf = _build_performance(fixture)
    multi = perf.multi_factor_greeks(0, fixture["factors"])

    assert multi.alpha == pytest.approx(expected["alpha"], abs=1e-12)
    _assert_sequence_close(multi.betas, expected["betas"])
    assert multi.r_squared == pytest.approx(expected["r_squared"], abs=1e-12)
    assert multi.adjusted_r_squared == pytest.approx(expected["adjusted_r_squared"], abs=1e-12)
    assert multi.residual_vol == pytest.approx(expected["residual_vol"], abs=1e-12)
