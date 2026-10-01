"""Host-level checks for canonical models audit fixes."""

import math

import pytest

from finstack_quant.models import bs_cos_price, bs_price, merton_jump_cos_price, vg_cos_price
from finstack_quant.models.volatility import (
    SabrParameters,
    SabrSmile,
    check_local_vol_density_grid,
    check_surface_grid,
)


@pytest.mark.parametrize("spot", [math.nan, math.inf, -100.0, 0.0])
def test_checked_black_scholes_raises_for_invalid_spot(spot: float) -> None:
    with pytest.raises(ValueError, match="spot"):
        bs_price(spot, 100.0, 0.05, 0.0, 0.2, 1.0, True)


def test_cos_rejects_zero_terms() -> None:
    with pytest.raises(ValueError, match="num_terms"):
        bs_cos_price(100.0, 100.0, 0.05, 0.0, 0.2, 1.0, True, n_terms=0)


@pytest.mark.parametrize("invalid", [math.nan, math.inf, -100.0, 0.0])
@pytest.mark.parametrize("is_call", [True, False])
def test_cos_invalid_prices_raise_value_error(invalid: float, is_call: bool) -> None:
    with pytest.raises(ValueError, match="spot"):
        bs_cos_price(invalid, 100.0, 0.05, 0.0, 0.2, 1.0, is_call)
    with pytest.raises(ValueError, match="strike"):
        bs_cos_price(100.0, invalid, 0.05, 0.0, 0.2, 1.0, is_call)


def test_cos_rejects_invalid_process_parameters() -> None:
    with pytest.raises(ValueError, match="sigma"):
        vg_cos_price(100.0, 100.0, 0.05, 0.0, -0.2, -0.1, 0.2, 1.0, True)
    with pytest.raises(ValueError, match="lambda"):
        merton_jump_cos_price(100.0, 100.0, 0.05, 0.0, 0.2, -0.1, 0.1, -0.1, 1.0, True)


def test_sabr_diagnostics_accept_uneven_strike_spacing() -> None:
    smile = SabrSmile(SabrParameters(0.2, 1.0, 0.0, 0.0), 100.0, 1.0)
    result = smile.arbitrage_diagnostics([99.0, 100.0, 120.0])
    assert result["arbitrage_free"]
    assert result["butterfly_violations"] == []
    with pytest.raises(ValueError, match="ascending"):
        smile.arbitrage_diagnostics([100.0, 99.0, 120.0])


@pytest.mark.parametrize("forward", [0.0, -100.0, math.nan, math.inf])
def test_arbitrage_check_rejects_invalid_forwards(forward: float) -> None:
    with pytest.raises(ValueError, match="forward"):
        check_surface_grid([90.0, 100.0, 110.0], [1.0, 2.0], [[0.3] * 3, [0.1] * 3], [forward])


def test_density_check_requires_one_forward_per_expiry() -> None:
    with pytest.raises(ValueError, match="entries"):
        check_local_vol_density_grid([90.0, 100.0, 110.0], [1.0, 2.0], [[0.2] * 3] * 2, [100.0])
    assert (
        check_local_vol_density_grid(
            [90.0, 100.0, 110.0],
            [1.0, 2.0],
            [[0.2] * 3] * 2,
            [100.0, 100.0],
        )
        == []
    )
