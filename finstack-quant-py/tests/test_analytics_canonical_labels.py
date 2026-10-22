"""Canonical frequency and CAGR spellings agree across Rust-owned entry points."""

from datetime import date

import pytest

from finstack_quant.analytics import Performance
from finstack_quant.core.dates import PeriodKind
from finstack_quant.statements import RollupSpec

ALIASES = ["D", "B", "W", "M", "ME", "Q", "QE", "A", "Y", "YE", "semiannual", "DAILY", "monthly ", " daily", ""]
CANONICAL = ["daily", "weekly", "monthly", "quarterly", "semi_annual", "annual"]


def panel(frequency: str = "daily") -> Performance:
    return Performance.from_returns_arrays(
        [date(2025, 1, day) for day in range(1, 5)],
        [[0.01, -0.02, 0.03, 0.005]],
        ["FUND"],
        frequency=frequency,
    )


@pytest.mark.parametrize("label", CANONICAL)
def test_canonical_frequencies_are_accepted(label: str) -> None:
    assert str(PeriodKind.from_name(label)) == label
    assert panel(label).get_frequency() == label
    assert panel().periodic_returns(label)


@pytest.mark.parametrize("label", ALIASES)
def test_noncanonical_frequencies_are_rejected(label: str) -> None:
    with pytest.raises(ValueError, match="period kind"):
        PeriodKind.from_name(label)
    with pytest.raises(ValueError, match="period kind"):
        panel(label)
    with pytest.raises(ValueError, match="period kind"):
        RollupSpec(label)
    with pytest.raises(ValueError, match="period kind"):
        panel().periodic_returns(label)
    with pytest.raises(ValueError, match="period kind"):
        panel().period_stats(0, label)


@pytest.mark.parametrize(
    "label", ["act_365_25", "act/365.25", "Act365_25", " act365_25", "ACT/365F", "act_365_fixed", ""]
)
def test_cagr_aliases_are_rejected(label: str) -> None:
    with pytest.raises(ValueError, match=r"day-count|variant"):
        panel().cagr(day_count=label)


def test_canonical_cagr_is_accepted() -> None:
    assert len(panel().cagr(day_count="act365_25")) == 1


def test_performance_accessors_are_canonical_methods() -> None:
    perf = panel()
    names = (
        "dates",
        "active_dates",
        "active_dates_for_ticker",
        "ticker_names",
        "ticker_index",
        "benchmark_idx",
        "frequency",
        "returns",
        "returns_for_ticker",
        "drawdown_series",
    )
    for name in names:
        assert not hasattr(perf, name)
        assert callable(getattr(perf, f"get_{name}"))
    assert perf.get_ticker_names() == ["FUND"]
    assert perf.get_ticker_index("FUND") == 0
    assert perf.get_benchmark_idx() == 0
    assert perf.get_frequency() == "daily"
    assert perf.get_dates() == perf.get_active_dates()
    assert perf.get_active_dates_for_ticker("FUND") == perf.get_dates()
    assert perf.get_returns_for_ticker("FUND") == perf.get_returns()[0]
    assert len(perf.get_drawdown_series()[0]) == len(perf.get_dates())
    with pytest.raises(KeyError):
        perf.get_ticker_index("fund")
    with pytest.raises(KeyError):
        perf.get_ticker_index("MISSING")
