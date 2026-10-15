"""Tests for periodic-return exits on ``Performance``."""

from __future__ import annotations

from datetime import date

import pandas as pd
import pytest

from finstack_quant.analytics import Performance


def _two_month_perf() -> Performance:
    idx = pd.bdate_range("2021-01-01", "2021-02-26")
    rets = pd.DataFrame({"STRAT": [0.001] * len(idx)}, index=idx)
    return Performance.from_returns(rets)


def test_periodic_monthly_shape_and_columns() -> None:
    panel = _two_month_perf().periodic_returns("monthly")
    assert len(panel) == 1
    assert len(panel[0]) == 2  # Jan + Feb 2021


def test_raw_periodic_returns_are_ticker_major_dated_points() -> None:
    idx = pd.bdate_range("2021-01-01", "2021-02-26")
    rets = pd.DataFrame(
        {
            "STRAT": [0.001] * len(idx),
            "BENCH": [0.002] * len(idx),
        },
        index=idx,
    )
    panel = Performance.from_returns(rets).periodic_returns()

    assert len(panel) == 2
    assert [len(series) for series in panel] == [2, 2]
    for series in panel:
        assert all(isinstance(point[0], date) and isinstance(point[1], float) for point in series)
        assert [point[0] for point in series] == sorted(point[0] for point in series)


def test_periodic_annual_single_year() -> None:
    panel = _two_month_perf().periodic_returns("annual")
    assert len(panel[0]) == 1  # all observations are in 2021


def test_periodic_rejects_unknown_frequency() -> None:
    perf = _two_month_perf()
    with pytest.raises(ValueError, match=r"frequency|monthly|annual"):
        perf.periodic_returns("hourly")


def test_periodic_monthly_reconciles_with_cumulative() -> None:
    perf = _two_month_perf()
    monthly = perf.periodic_returns("monthly")[0]
    total = perf.cumulative_returns()[0][-1]
    chained = (1.0 + monthly[0][1]) * (1.0 + monthly[1][1]) - 1.0
    assert abs(chained - total) < 1e-9
