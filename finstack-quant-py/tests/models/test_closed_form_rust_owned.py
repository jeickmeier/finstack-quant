"""Closed-form and SABR behaviour owned by Rust (WASM-audit slice S15).

The same cases are asserted for WASM in
``finstack-quant-wasm/tests/facade/models_rust_owned.test.mjs``.
"""

from __future__ import annotations

import math
import pickle

import pytest

from finstack_quant.models import (
    ForwardGreeks,
    bachelier_greeks,
    bachelier_price,
    black76_greeks,
    black76_implied_vol,
    black76_price,
    black_shifted_price,
    black_shifted_vega,
    heston_price,
)
from finstack_quant.models.volatility import SabrCalibrator, SabrParameters, SabrSmile


@pytest.mark.parametrize("df", [0.0, -0.95])
def test_black76_price_rejects_non_positive_df_like_its_inverse(df: float) -> None:
    with pytest.raises(ValueError, match="Black-76 df must be positive"):
        black76_price(100.0, 100.0, df, 1.0, 0.2, True)
    with pytest.raises(ValueError, match="implied vol requires positive forward, strike, df"):
        black76_implied_vol(100.0, 100.0, df, 1.0, 7.5, True)


def test_forward_kernels_reject_out_of_domain_inputs() -> None:
    with pytest.raises(ValueError, match="Black-76 vol must be non-negative"):
        black76_price(100.0, 100.0, 0.95, 1.0, -0.2, True)
    with pytest.raises(ValueError, match="Bachelier normal_vol must be non-negative"):
        bachelier_price(0.03, 0.03, -0.0075, 1.0, True)
    with pytest.raises(ValueError, match=r"shifted Black strike \+ shift must be positive"):
        black_shifted_price(-0.005, -0.02, 0.25, 1.0, 0.01, False)
    with pytest.raises(ValueError, match=r"shifted Black forward \+ shift must be positive"):
        black_shifted_vega(-0.02, 0.01, 0.25, 1.0, 0.01)


def test_forward_greeks_is_typed_with_pandas_exits() -> None:
    g = black76_greeks(100.0, 100.0, 1.0, 0.2, False)
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
    call = heston_price(100.0, 100.0, 1.0, 0.05, 0.02, 2.0, 0.04, 0.3, -0.7, 0.04)
    put = heston_price(100.0, 100.0, 1.0, 0.05, 0.02, 2.0, 0.04, 0.3, -0.7, 0.04, is_call=False)
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
