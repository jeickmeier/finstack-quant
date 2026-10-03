"""Host-level checks for canonical models audit fixes."""

import copy
import math
import pickle

import pytest

from finstack_quant.models import (
    bachelier_price,
    black76_implied_vol,
    black76_price,
    bs_cos_price,
    bs_price,
    merton_jump_cos_price,
    vg_cos_price,
)
from finstack_quant.models.volatility import (
    SabrCalibrator,
    SabrParameters,
    SabrSmile,
    check_local_vol_density_grid,
    check_surface_grid,
    implied_vol_bachelier,
    implied_vol_black,
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
    # Rust `SabrSmile::validate_no_arbitrage(strikes, r)`, the twin of WASM `validateNoArbitrage`.
    result = smile.validate_no_arbitrage([99.0, 100.0, 120.0], 0.0)
    assert result["arbitrage_free"]
    assert result["butterfly_violations"] == []
    with pytest.raises(ValueError, match="ascending"):
        smile.validate_no_arbitrage([100.0, 99.0, 120.0], 0.0)


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


# MODB-019: a normal-model price inverts to its normal vol, including the
# negative forward/strike case the lognormal solvers reject.
@pytest.mark.parametrize(
    ("forward", "strike", "normal_vol", "is_call"),
    [(0.03, 0.025, 0.005, True), (-0.002, -0.001, 0.006, True), (-0.002, -0.001, 0.006, False)],
)
def test_implied_vol_bachelier_inverts_bachelier_price(
    forward: float, strike: float, normal_vol: float, is_call: bool
) -> None:
    price = bachelier_price(forward, strike, normal_vol, 1.0, is_call)
    assert implied_vol_bachelier(price, forward, strike, 1.0, is_call) == pytest.approx(normal_vol, abs=1e-12)


def test_implied_vol_black_matches_undiscounted_black76() -> None:
    price = black76_price(100.0, 105.0, 1.0, 1.0, 0.25, True)
    assert implied_vol_black(price, 100.0, 105.0, 1.0, True) == pytest.approx(0.25, abs=1e-12)
    assert implied_vol_black(price, 100.0, 105.0, 1.0, True) == pytest.approx(
        black76_implied_vol(100.0, 105.0, 1.0, 1.0, price, True), abs=1e-14
    )
    with pytest.raises(ValueError, match="positive"):
        implied_vol_black(0.001, -0.002, -0.001, 1.0, True)


# MODB-020: the SABR typed surface binds the Rust members.
def test_sabr_parameters_round_trip_compare_and_pickle() -> None:
    strikes = [0.02, 0.025, 0.03, 0.035, 0.04]
    vols = SabrSmile(SabrParameters(0.05, 0.5, 0.4, -0.2), 0.03, 1.0).generate_smile(strikes)
    fitted = SabrCalibrator().calibrate(forward=0.03, strikes=strikes, market_vols=vols, t=1.0, beta=0.5)
    assert SabrParameters.from_json(fitted.to_json()) == fitted
    assert pickle.loads(pickle.dumps(fitted)) == fitted  # noqa: S301 - round-trips our own bytes
    assert copy.deepcopy(fitted) == fitted
    assert SabrParameters(0.2, 1.0, 0.3, -0.3) == SabrParameters(0.2, 1.0, 0.3, -0.3)
    assert SabrParameters(0.2, 1.0, 0.3, -0.3) != SabrParameters(0.2, 1.0, 0.3, -0.3, shift=0.01)
    with pytest.raises(ValueError, match="greater than zero"):
        SabrParameters.from_json('{"alpha": -0.2, "beta": 1.0, "nu": 0.3, "rho": -0.3}')


def test_sabr_parameter_factories_fix_beta_and_shift() -> None:
    assert SabrParameters.equity_standard(0.2, 0.3, -0.2).beta == 1.0
    assert SabrParameters.rates_standard(0.2, 0.3, -0.2).beta == 0.5
    assert SabrParameters.normal(0.01, 0.3, -0.2).beta == 0.0
    assert SabrParameters.lognormal(0.2, 0.3, -0.2).beta == 1.0
    shifted = SabrParameters.shifted_normal(0.01, 0.3, -0.2, 0.02)
    assert (shifted.beta, shifted.shift) == (0.0, 0.02)
    assert SabrParameters.shifted_lognormal(0.2, 0.3, -0.2, 0.02).shift == 0.02
    with pytest.raises(ValueError, match="shift"):
        SabrParameters.shifted_lognormal(0.2, 0.3, -0.2, -0.01)


def test_sabr_smile_delta_strike_and_arbitrage_methods() -> None:
    smile = SabrSmile(SabrParameters.equity_default(), 100.0, 1.0)
    call_strike = smile.strike_from_delta(0.25, True)
    put_strike = smile.strike_from_delta(0.25, False)
    assert put_strike < 100.0 < call_strike
    with pytest.raises(ValueError, match="between zero and one"):
        smile.strike_from_delta(1.0, True)
    strikes = [80.0, 90.0, 100.0, 110.0, 120.0]
    assert smile.check_no_arbitrage(strikes, 0.0) is None
    with pytest.raises(ValueError, match="strictly ascending"):
        smile.check_no_arbitrage([120.0, 100.0, 80.0], 0.0)
    assert smile.repair_arbitrage(strikes, 0.0, 10) == smile.generate_smile(strikes)


def test_sabr_calibrator_getters_read_back_settings() -> None:
    default = SabrCalibrator()
    assert (default.shift, default.atm_pinning) == (None, False)
    cal = default.with_tolerance(1e-8).with_max_iterations(77).with_shift(0.01).with_atm_pinning(True)
    assert (cal.tolerance, cal.max_iterations, cal.shift, cal.atm_pinning) == (1e-8, 77, 0.01, True)
    assert default.with_shift("auto").shift == "auto"


def test_sabr_smile_dataframe_log_moneyness_is_shift_aware() -> None:
    """MODB-006: ``log_moneyness`` is ``ln((K+s)/(F+s))``, computed in Rust."""
    shifted = SabrParameters(alpha=0.01, beta=0.5, nu=0.3, rho=-0.2, shift=0.02)
    frame = SabrSmile(shifted, -0.001, 1.0).to_dataframe(strikes=[-0.005, 0.0, 0.005])
    expected = [math.log((k + 0.02) / (-0.001 + 0.02)) for k in (-0.005, 0.0, 0.005)]
    assert list(frame["log_moneyness"]) == pytest.approx(expected, abs=1e-14)
    positive = SabrSmile(shifted, 0.01, 1.0).to_dataframe(strikes=[0.005, 0.01, 0.02])
    expected = [math.log((k + 0.02) / 0.03) for k in (0.005, 0.01, 0.02)]
    assert list(positive["log_moneyness"]) == pytest.approx(expected, abs=1e-14)
