"""Period-average forward queries use Rust validation in every host."""

import pytest

from finstack_quant.core.market_data import ForwardCurve


@pytest.mark.parametrize(
    ("start", "end"),
    [(1.0, 0.5), (float("nan"), 1.0), (0.0, float("nan")), (float("-inf"), 1.0), (0.0, float("inf")), (-1e308, 1e308)],
)
def test_forward_period_rejects_invalid_interval(start: float, end: float) -> None:
    curve = ForwardCurve.flat("USD-SOFR-3M", 0.25, "2025-01-01", 0.04)
    with pytest.raises(ValueError, match=r"requires finite t1 <= t2|interval length must be finite"):
        curve.rate_period(start, end)


def test_forward_period_preserves_short_and_negative_interval_semantics() -> None:
    curve = ForwardCurve.flat("USD-SOFR-3M", 0.25, "2025-01-01", 0.04)
    for time in [-1.0, 0.0, 1.0]:
        assert curve.rate_period(time, time) == curve.rate(time)
        assert curve.rate_period(time, time + 1e-13) == curve.rate(time)
    assert curve.rate_period(-1.0, -0.5) == pytest.approx(0.04)
    assert curve.rate_period(0.0, 1.0) == pytest.approx(0.04)
