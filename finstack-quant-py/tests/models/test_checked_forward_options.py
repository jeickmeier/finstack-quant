"""Checked host APIs must reject corrupt quotes before deterministic limits."""

import math

import pytest

from finstack_quant.models import (
    bachelier_greeks,
    bachelier_price,
    black76_greeks,
    black76_price,
    black_shifted_price,
    black_shifted_vega,
)


@pytest.mark.parametrize("invalid", [math.nan, math.inf, -math.inf])
@pytest.mark.parametrize("expiry", [0.0, 1.0])
def test_invalid_quotes_raise_before_intrinsic(invalid: float, expiry: float) -> None:
    with pytest.raises(ValueError, match="forward must be finite"):
        black76_price(invalid, 100.0, 0.95, expiry, 0.2, True)
    with pytest.raises(ValueError, match="forward must be finite"):
        black76_greeks(invalid, 100.0, expiry, 0.2, True)
    with pytest.raises(ValueError, match="forward must be finite"):
        bachelier_price(invalid, 100.0, 0.2, expiry, True)
    with pytest.raises(ValueError, match="forward must be finite"):
        bachelier_greeks(invalid, 100.0, 0.2, expiry, True)
    with pytest.raises(ValueError, match="shift must be finite"):
        black_shifted_price(-0.01, -0.01, 0.2, expiry, invalid, True)
    with pytest.raises(ValueError, match="shift must be finite"):
        black_shifted_vega(-0.01, -0.01, 0.2, expiry, invalid)


@pytest.mark.parametrize("df", [0.0, -0.95, math.nan, math.inf])
def test_black76_rejects_invalid_discount_factor(df: float) -> None:
    with pytest.raises(ValueError, match="discount factor"):
        black76_price(100.0, 100.0, df, 1.0, 0.2, True)


@pytest.mark.parametrize(("vol", "expiry"), [(-0.2, 1.0), (0.2, -1.0)])
def test_negative_volatility_or_expiry_raises(vol: float, expiry: float) -> None:
    with pytest.raises(ValueError, match="volatility and expiry must be non-negative"):
        black76_price(100.0, 100.0, 0.95, expiry, vol, True)
    with pytest.raises(ValueError, match="volatility and expiry must be non-negative"):
        black76_greeks(100.0, 100.0, expiry, vol, True)
    with pytest.raises(ValueError, match="volatility and expiry must be non-negative"):
        bachelier_price(100.0, 100.0, vol, expiry, True)
    with pytest.raises(ValueError, match="volatility and expiry must be non-negative"):
        bachelier_greeks(100.0, 100.0, vol, expiry, True)
    with pytest.raises(ValueError, match="volatility and expiry must be non-negative"):
        black_shifted_price(-0.01, -0.01, vol, expiry, 0.02, True)
    with pytest.raises(ValueError, match="volatility and expiry must be non-negative"):
        black_shifted_vega(-0.01, -0.01, vol, expiry, 0.02)


def test_valid_deterministic_limits_and_negative_normal_rates() -> None:
    assert black76_price(110.0, 100.0, 0.95, 1.0, 0.0, True) == pytest.approx(9.5)
    assert bachelier_price(-0.01, -0.02, 0.0, 1.0, True) == pytest.approx(0.01)
    assert black_shifted_price(-0.01, -0.02, 0.0, 1.0, 0.03, True) == pytest.approx(0.01)
    assert bachelier_price(-0.01, -0.01, 0.005, 1.0, True) > 0.0
