# finstack-quant-py/tests/test_reporting_benchmark.py
from __future__ import annotations

import datetime as dt
import random

import pandas as pd
import pytest

from finstack_quant.analytics import Performance
from finstack_quant.reporting import benchmark_tearsheet
from finstack_quant.reporting.document import TearSheet


def _perf() -> Performance:
    random.seed(42)
    dates = pd.bdate_range("2024-01-02", periods=160)
    bench = [random.gauss(0.0004, 0.01) for _ in range(160)]
    fund = [0.00025 + 0.9 * bench[i] + random.gauss(0.0, 0.004) for i in range(160)]
    df = pd.DataFrame({"FUND": fund, "BENCH": bench}, index=dates)
    return Performance.from_returns(df, benchmark_ticker="BENCH", freq="daily")


def test_benchmark_tearsheet_renders_sections_and_kpis() -> None:
    ts = benchmark_tearsheet(_perf(), generated=dt.date(2026, 6, 23))
    assert isinstance(ts, TearSheet)
    html = ts.to_html()
    assert "Benchmark-Relative Statistics" in html
    assert "Alpha (ann.)" in html
    assert "Information Ratio" in html
    assert "Capture Ratio" in html
    assert "Tracking Error" in html
    assert "Up Capture" in html
    assert "Relative to Benchmark" in html
    assert "Rolling" in html  # rolling section title
    assert "FUND" in html  # default title = non-benchmark ticker
    assert "vs BENCH" in html  # subtitle


def test_benchmark_tearsheet_default_ticker_is_non_benchmark() -> None:
    # BENCH is benchmark_idx=1; default ticker must be the FUND (idx 0).
    ts = benchmark_tearsheet(_perf(), generated=dt.date(2026, 6, 23))
    assert ts.title == "FUND"


def test_benchmark_tearsheet_multifactor_section() -> None:
    mf = {
        "alpha": 0.12,
        "betas": [0.9, -0.01, -0.02],
        "r_squared": 0.80,
        "adjusted_r_squared": 0.79,
        "residual_vol": 0.07,
    }
    html = benchmark_tearsheet(
        _perf(),
        multi_factor=mf,
        factor_names=["Market", "Size", "Value"],
        sections=["multifactor"],
        generated=dt.date(2026, 6, 23),
    ).to_html()
    assert "Multi-Factor Attribution" in html
    assert "Market Beta" in html
    assert "Residual Vol" in html


def test_benchmark_tearsheet_multifactor_absent_omitted() -> None:
    html = benchmark_tearsheet(_perf(), sections=["multifactor"], generated=dt.date(2026, 6, 23)).to_html()
    assert "Multi-Factor Attribution" not in html


def test_benchmark_tearsheet_section_selection() -> None:
    ts = benchmark_tearsheet(_perf(), sections=["summary"], generated=dt.date(2026, 6, 23))
    assert [s.title for s in ts.sections] == ["Benchmark-Relative Statistics"]


def test_benchmark_tearsheet_rejects_unknown_section() -> None:
    with pytest.raises(ValueError, match="unknown section"):
        benchmark_tearsheet(_perf(), sections=["summary", "nope"])


def test_benchmark_tearsheet_deterministic() -> None:
    a = benchmark_tearsheet(_perf(), generated=dt.date(2026, 6, 23)).to_html()
    b = benchmark_tearsheet(_perf(), generated=dt.date(2026, 6, 23)).to_html()
    assert a == b
