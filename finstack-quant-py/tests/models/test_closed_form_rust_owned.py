"""Closed-form and SABR behaviour owned by Rust (WASM-audit slice S15).

The same cases are asserted for WASM in
``finstack-quant-wasm/tests/facade/models_rust_owned.test.mjs``.
"""

from __future__ import annotations

import math
import pickle

import pytest

from finstack_quant.models import (
    BsGreeks,
    ForwardGreeks,
    OptionCalculationResult,
    bachelier_greeks,
    bachelier_price,
    black76_greeks,
    black76_implied_vol,
    black76_price,
    black_shifted_price,
    black_shifted_vega,
    bs_greeks,
    bs_price,
    heston_price,
    option_calculation,
)
from finstack_quant.models.volatility import SabrCalibrator, SabrParameters, SabrSmile


@pytest.mark.parametrize("df", [0.0, -0.95])
def test_black76_price_rejects_non_positive_df_like_its_inverse(df: float) -> None:
    with pytest.raises(ValueError, match="Black-76 df must be positive"):
        black76_price(100.0, 100.0, df, 0.2, 1.0, True)
    with pytest.raises(ValueError, match="implied vol requires positive forward, strike, df"):
        black76_implied_vol(100.0, 100.0, df, 1.0, 7.5, True)


def test_forward_kernels_reject_out_of_domain_inputs() -> None:
    with pytest.raises(ValueError, match="Black-76 vol must be non-negative"):
        black76_price(100.0, 100.0, 0.95, -0.2, 1.0, True)
    with pytest.raises(ValueError, match="Bachelier normal_vol must be non-negative"):
        bachelier_price(0.03, 0.03, -0.0075, 1.0, True)
    with pytest.raises(ValueError, match=r"shifted Black strike \+ shift must be positive"):
        black_shifted_price(-0.005, -0.02, 0.25, 1.0, 0.01, False)
    with pytest.raises(ValueError, match=r"shifted Black forward \+ shift must be positive"):
        black_shifted_vega(-0.02, 0.01, 0.25, 1.0, 0.01)


def test_forward_greeks_is_typed_with_pandas_exits() -> None:
    g = black76_greeks(100.0, 100.0, 0.2, 1.0, False)
    assert isinstance(g, ForwardGreeks)
    assert -0.5 < g.delta < -0.4
    assert g.to_series().index.tolist() == ["delta", "gamma", "vega"]
    assert list(g.to_dataframe().columns) == ["delta", "gamma", "vega"]
    assert ForwardGreeks.from_json(g.to_json()) == g
    assert pickle.loads(pickle.dumps(g)) == g  # noqa: S301
    n = bachelier_greeks(0.03, 0.03, 0.0075, 1.0, True)
    assert n.delta == pytest.approx(0.5)
    with pytest.raises(ValueError, match="vega"):
        ForwardGreeks.from_json('{"delta": 1.0, "gamma": 0.0}')


def test_heston_price_dispatches_put_through_rust() -> None:
    call = heston_price(
        100.0, 100.0, 1.0, {"r": 0.05, "q": 0.02, "v0": 0.04, "kappa": 2.0, "theta": 0.04, "sigma_v": 0.3, "rho": -0.7}
    )
    put = heston_price(
        100.0,
        100.0,
        1.0,
        {"r": 0.05, "q": 0.02, "v0": 0.04, "kappa": 2.0, "theta": 0.04, "sigma_v": 0.3, "rho": -0.7},
        is_call=False,
    )
    parity = call - put - (100.0 * math.exp(-0.02) - 100.0 * math.exp(-0.05))
    assert abs(parity) < 1e-8


def test_sabr_smile_single_strike_and_arbitrage_verdict_come_from_rust() -> None:
    smile = SabrSmile(SabrParameters(0.2, 0.5, 0.3, -0.2), 100.0, 1.0)
    strikes = [80.0, 90.0, 100.0, 110.0, 120.0]
    assert [smile.implied_vol(k) for k in strikes] == smile.generate_smile(strikes)
    report = smile.validate_no_arbitrage(strikes, 0.0)
    assert report == {"arbitrage_free": True, "butterfly_violations": [], "monotonicity_violations": []}
    with pytest.raises(TypeError):
        smile.validate_no_arbitrage(strikes)  # type: ignore[call-arg]


def test_sabr_with_shift_keyword_is_parsed_by_rust_and_bool_is_rejected() -> None:
    assert "shift='auto'" in repr(SabrCalibrator().with_shift("auto"))
    with pytest.raises(ValueError, match="got bool"):
        SabrCalibrator().with_shift(True)
    with pytest.raises(ValueError, match='SABR shift must be null/None, a number, or "auto"'):
        SabrCalibrator().with_shift("AUTO")


BLACK_SCHOLES = {
    "kind": "black_scholes",
    "spot": 100.0,
    "strike": 105.0,
    "rate": 0.03,
    "div_yield": 0.01,
    "expiry": 0.75,
    "is_call": False,
}


def test_option_calculation_price_solve_matches_the_kernels() -> None:
    result = option_calculation({"model": BLACK_SCHOLES, "solve": {"kind": "price", "vol": 0.25}})
    assert result.price == bs_price(100.0, 105.0, 0.03, 0.01, 0.25, 0.75, False)
    assert result.implied_vol == 0.25
    assert isinstance(result.greeks, BsGreeks)
    assert result.greeks == bs_greeks({
        "spot": 100.0,
        "strike": 105.0,
        "rate": 0.03,
        "dividend_yield": 0.01,
        "volatility": 0.25,
        "time_to_expiry": 0.75,
        "option_type": "put",
    })


@pytest.mark.parametrize(
    ("model", "vol"),
    [
        (BLACK_SCHOLES, 0.25),
        ({"kind": "black76", "forward": 0.04, "strike": 0.035, "df": 0.97, "expiry": 2.0, "is_call": True}, 0.3),
        ({"kind": "bachelier", "forward": -0.002, "strike": 0.001, "expiry": 1.5, "is_call": True}, 0.006),
    ],
)
def test_option_calculation_implied_vol_inverts_each_model(model: dict[str, object], vol: float) -> None:
    priced = option_calculation({"model": model, "solve": {"kind": "price", "vol": vol}})
    solved = option_calculation({"model": model, "solve": {"kind": "implied_vol", "price": priced.price}})
    assert solved.implied_vol == pytest.approx(vol, abs=1e-7)
    assert solved.price == priced.price
    if model["kind"] != "black_scholes":
        assert isinstance(solved.greeks, ForwardGreeks)


def test_option_calculation_result_round_trips_and_rejects_bad_requests() -> None:
    result = option_calculation({"model": BLACK_SCHOLES, "solve": {"kind": "price", "vol": 0.25}})
    assert OptionCalculationResult.from_json(result.to_json()) == result
    assert pickle.loads(pickle.dumps(result)) == result  # noqa: S301 - trusted in-process round trip
    with pytest.raises(ValueError, match="unknown field"):
        option_calculation({"model": {**BLACK_SCHOLES, "forward": 1.0}, "solve": {"kind": "price", "vol": 0.25}})
    with pytest.raises(ValueError, match="intrinsic"):
        option_calculation({
            "model": {**BLACK_SCHOLES, "spot": 120.0, "is_call": True},
            "solve": {"kind": "implied_vol", "price": 1.0},
        })
