"""Interval forward queries use Rust validation in every host."""

import math

import pytest

from finstack_quant.core.market_data import ForwardCurve


def _curve() -> ForwardCurve:
    return ForwardCurve.flat("USD-SOFR-3M", "2025-01-01", 0.04, "act_360")


@pytest.mark.parametrize(
    ("start", "end"),
    [(1.0, 0.5), (1.0, 1.0), (math.nan, 1.0), (0.0, math.nan), (-math.inf, 1.0), (0.0, math.inf)],
)
def test_rate_between_rejects_invalid_interval(start: float, end: float) -> None:
    with pytest.raises(ValueError, match=r"requires finite t1 < t2"):
        _curve().rate_between(start, end, 0.25)


@pytest.mark.parametrize("accrual", [0.0, -0.25, math.nan, math.inf])
def test_rate_between_rejects_invalid_accrual(accrual: float) -> None:
    with pytest.raises(ValueError, match="accrual_year_fraction"):
        _curve().rate_between(0.0, 0.25, accrual)


@pytest.mark.parametrize(("start", "end"), [(0.0, 1.0), (-1.0, -0.5), (2.0, 2.25)])
def test_rate_between_on_flat_curve_includes_pre_base_intervals(start: float, end: float) -> None:
    tau = end - start
    assert _curve().rate_between(start, end, tau) == pytest.approx(math.expm1(0.04 * tau) / tau, rel=1e-13)
