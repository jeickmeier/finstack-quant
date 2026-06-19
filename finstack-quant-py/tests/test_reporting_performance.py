# finstack-quant-py/tests/test_reporting_performance.py
from __future__ import annotations

import datetime as dt

import pandas as pd

from finstack_quant.analytics import Performance
from finstack_quant.reporting import performance_tearsheet
from finstack_quant.reporting.document import TearSheet


def _perf() -> Performance:
    idx = pd.bdate_range("2021-01-01", "2023-12-29")
    # Deterministic, smoothly varying returns (no RNG): small drift + sine wobble.
    import math

    rets = [0.0006 + 0.004 * math.sin(i / 9.0) for i in range(len(idx))]
    df = pd.DataFrame({"STRAT": rets}, index=idx)
    return Performance.from_returns(df)


def test_returns_tearsheet_with_all_sections() -> None:
    ts = performance_tearsheet(_perf(), title="Test Strategy", generated=dt.date(2026, 6, 19))
    assert isinstance(ts, TearSheet)
    html = ts.to_html()
    assert "Test Strategy" in html
    for label in ("Total Return", "CAGR", "Sharpe", "Max Drawdown"):
        assert label in html
    # heatmap + charts present
    assert 'table class="hm"' in html or 'class="hm"' in html
    assert "<svg" in html


def test_sections_can_be_trimmed() -> None:
    ts = performance_tearsheet(_perf(), sections=["summary", "cumulative"], generated=dt.date(2026, 6, 19))
    html = ts.to_html()
    assert "Cumulative Return" in html
    # "drawdown" and "drawdowns" sections are trimmed out — their section titles must be absent.
    # Note: "Max Drawdown" KPI may still appear in the summary KPI strip; we check section titles.
    assert "Worst Drawdowns" not in html  # drawdowns section absent
    assert 'secttl">Drawdown<' not in html  # drawdown chart section title absent


def test_default_title_and_subtitle_derive_from_data() -> None:
    ts = performance_tearsheet(_perf(), generated=dt.date(2026, 6, 19))
    assert ts.subtitle is not None
    assert "2021" in ts.subtitle


def test_unknown_section_raises() -> None:
    import pytest

    with pytest.raises(ValueError, match=r"unknown section"):
        performance_tearsheet(_perf(), sections=["typo"], generated=dt.date(2026, 6, 19))
