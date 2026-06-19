"""Tests for the additive Performance.periodic_returns_to_dataframe export."""

from __future__ import annotations

import pandas as pd
import pytest

from finstack_quant.analytics import Performance


def _two_month_perf() -> Performance:
    idx = pd.bdate_range("2021-01-01", "2021-02-26")
    rets = pd.DataFrame({"STRAT": [0.001] * len(idx)}, index=idx)
    return Performance.from_returns(rets)


def test_periodic_monthly_shape_and_columns() -> None:
    df = _two_month_perf().periodic_returns_to_dataframe("monthly")
    assert isinstance(df, pd.DataFrame)
    assert list(df.columns) == ["STRAT"]
    assert len(df) == 2  # Jan + Feb 2021


def test_periodic_annual_single_year() -> None:
    df = _two_month_perf().periodic_returns_to_dataframe("annual")
    assert len(df) == 1  # all observations are in 2021


def test_periodic_rejects_unknown_freq() -> None:
    with pytest.raises(Exception, match=r"freq|monthly|annual"):
        _two_month_perf().periodic_returns_to_dataframe("hourly")


def test_periodic_monthly_reconciles_with_cumulative() -> None:
    perf = _two_month_perf()
    monthly = perf.periodic_returns_to_dataframe("monthly")["STRAT"]
    total = perf.cumulative_returns_to_dataframe()["STRAT"].iloc[-1]
    chained = (1.0 + monthly.iloc[0]) * (1.0 + monthly.iloc[1]) - 1.0
    assert abs(chained - total) < 1e-9
