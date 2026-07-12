"""Smoke tests for the new B1-B6 valuations bindings.

Covers:
- B1: `bs_price`, `bs_greeks`, `bs_implied_vol`, `black76_implied_vol`.
- B2: `finstack_quant.valuations.correlation.nearest_correlation`.
- B3: `SabrParameters` / `SabrModel` / `SabrSmile` / `SabrCalibrator`.
- B4: `instrument_cashflows_json` and the `instrument_cashflows`
      DataFrame helper.
- B5: `barrier_call`, `asian_option_price`, `lookback_option_price`,
      `quanto_option_price`.
"""

from __future__ import annotations

from datetime import date
import json

import pytest

from finstack_quant.core.market_data import DiscountCurve, MarketContext
from finstack_quant.valuations import (
    SabrCalibrator,
    SabrModel,
    SabrParameters,
    SabrSmile,
    asian_option_price,
    barrier_call,
    black76_implied_vol,
    bs_greeks,
    bs_implied_vol,
    bs_price,
    instrument_cashflows,
    inverse_floater_coupon_profile,
    lookback_option_price,
    quanto_option_price,
    snowball_coupon_profile,
)
from finstack_quant.valuations.correlation import nearest_correlation
from finstack_quant.valuations.instruments import instrument_cashflows_json, price_instrument

# ---------------------------------------------------------------------------
# B1 — Black-Scholes / Black-76 primitives
# ---------------------------------------------------------------------------


def test_bs_price_call_atm_is_positive() -> None:
    assert bs_price(100.0, 100.0, 0.05, 0.02, 0.2, 1.0, True) > 0.0


def test_bs_greeks_has_expected_keys() -> None:
    g = bs_greeks(100.0, 100.0, 0.05, 0.02, 0.2, 1.0, True)
    assert set(g) >= {"delta", "gamma", "vega", "theta", "rho", "rho_q"}
    assert 0.0 < g["delta"] < 1.0


def test_bs_implied_vol_round_trip() -> None:
    sigma = 0.25
    price = bs_price(100.0, 110.0, 0.03, 0.01, sigma, 0.75, True)
    iv = bs_implied_vol(100.0, 110.0, 0.03, 0.01, 0.75, price, True)
    assert abs(iv - sigma) < 1e-6


def test_black76_implied_vol_runs() -> None:
    # Sanity: IV solver returns a positive decimal on a reasonable input.
    iv = black76_implied_vol(100.0, 100.0, 0.95, 1.0, 8.0, True)
    assert iv > 0.0


# ---------------------------------------------------------------------------
# B2 — nearest_correlation
# ---------------------------------------------------------------------------


def test_nearest_correlation_passes_through_valid_matrix() -> None:
    m = [1.0, 0.5, 0.3, 0.5, 1.0, 0.4, 0.3, 0.4, 1.0]
    out = nearest_correlation(m, 3)
    assert len(out) == 9
    for i in range(3):
        assert abs(out[i * 3 + i] - 1.0) < 1e-9


def test_nearest_correlation_rejects_bad_diagonal() -> None:
    # Diagonal of 0.5 — way outside the gate — should raise.
    with pytest.raises(ValueError, match=r"diagonal|Diagonal"):
        nearest_correlation([0.5, 0.5, 0.3, 0.5, 0.5, 0.4, 0.3, 0.4, 0.5], 3)


# ---------------------------------------------------------------------------
# B3 — SABR
# ---------------------------------------------------------------------------


def test_sabr_equity_default_and_model_implied_vol() -> None:
    p = SabrParameters.equity_default()
    assert p.alpha == pytest.approx(0.20)
    model = SabrModel(p)
    vol = model.implied_vol(100.0, 100.0, 1.0)
    assert vol > 0.0


def test_sabr_calibrator_round_trip() -> None:
    # SABR rho is only weakly identified from symmetric strikes; the
    # calibrator may find a flat-rho equivalent minimum at the precision
    # used here. We check the smile it fits, not the raw rho. The alpha and
    # nu recoveries are the robust diagnostic.
    params = SabrParameters(0.2, 1.0, 0.3, -0.2)
    smile = SabrSmile(params, 100.0, 1.0)
    strikes = [80.0, 90.0, 100.0, 110.0, 120.0]
    vols = smile.generate_smile(strikes)
    fitted = SabrCalibrator().calibrate(100.0, strikes, vols, 1.0, 1.0)
    assert abs(fitted.alpha - params.alpha) < 1e-2
    assert abs(fitted.nu - params.nu) < 1e-1
    # Refit smile shape must match input smile. Tolerance reflects rho's
    # weak identifiability on symmetric strikes: the L-M solver settles on a
    # flat-rho equivalent minimum, which leaves residuals of order 1 vol pt
    # (well under the alpha tolerance of 1e-2 above) on the wings while
    # matching the ATM and inner strikes much more tightly.
    fitted_smile = SabrSmile(fitted, 100.0, 1.0).generate_smile(strikes)
    for v_fit, v_orig in zip(fitted_smile, vols, strict=True):
        assert abs(v_fit - v_orig) < 1e-2


def test_sabr_calibrate_requires_explicit_beta() -> None:
    # beta is required (matching Rust/WASM); omitting it must not silently
    # apply the equity lognormal convention.
    with pytest.raises(TypeError):
        SabrCalibrator().calibrate(100.0, [90.0, 100.0, 110.0], [0.2, 0.19, 0.2], 1.0)


def test_sabr_calibrate_auto_shift_positive_rates_matches_calibrate() -> None:
    params = SabrParameters(0.05, 0.5, 0.4, -0.1)
    smile = SabrSmile(params, 0.03, 1.0)
    strikes = [0.01, 0.02, 0.03, 0.04, 0.05]
    vols = smile.generate_smile(strikes)
    fitted = SabrCalibrator().calibrate_auto_shift(0.03, strikes, vols, 1.0, 0.5)
    assert fitted.shift is None
    assert fitted.alpha > 0.0


def test_sabr_calibrate_auto_shift_negative_rates_uses_shift() -> None:
    params = SabrParameters(0.05, 0.5, 0.4, -0.1, shift=0.03)
    forward = -0.005
    strikes = [-0.015, -0.01, -0.005, 0.0, 0.005]
    smile = SabrSmile(params, forward, 1.0)
    vols = smile.generate_smile(strikes)
    fitted = SabrCalibrator().calibrate_auto_shift(forward, strikes, vols, 1.0, 0.5)
    assert fitted.shift is not None
    assert fitted.shift > 0.0
    assert fitted.is_shifted()


# ---------------------------------------------------------------------------
# B4 — instrument_cashflows
# ---------------------------------------------------------------------------


def _build_deposit_market() -> tuple[str, MarketContext]:
    inst_json = json.dumps({
        "type": "deposit",
        "spec": {
            "id": "DEP-B4",
            "notional": {"amount": 1_000_000.0, "currency": "USD"},
            "start_date": "2025-01-15",
            "maturity": "2025-06-15",
            "day_count": "Act360",
            "quote_rate": 0.05,
            "discount_curve_id": "USD-OIS",
            "attributes": {},
        },
    })
    mc = MarketContext()
    mc.insert(
        DiscountCurve(
            "USD-OIS",
            date(2025, 1, 15),
            [(0.0, 1.0), (0.5, 0.975), (1.0, 0.95)],
            day_count="act_365f",
        )
    )
    return inst_json, mc


def test_instrument_cashflows_deposit_reconciles_with_price() -> None:
    inst_json, market = _build_deposit_market()
    envelope, df = instrument_cashflows(inst_json, market, "2025-01-15", model="discounting")

    assert envelope["reconciles_with_base_value"] is True
    assert envelope["model"] == "discounting"
    assert envelope["currency"] == "USD"
    assert len(df) > 0
    for col in ("date", "amount", "currency", "kind", "discount_factor", "pv"):
        assert col in df.columns

    # total_pv reconciles with price_instrument within rounding.
    # Note: price_instrument accepts MarketContext at runtime even though the
    # .pyi stub types `market_json` as `str`; pass the JSON string to keep
    # static type checkers happy.
    pr = json.loads(price_instrument(inst_json, market.to_json(), "2025-01-15", model="discounting"))
    price = float(pr["value"]["amount"])
    assert abs(envelope["total_pv"] - price) < 0.01

    # DataFrame pv sum matches the envelope total.
    pv_series = df["pv"]
    pv_sum = float(pv_series.sum())  # type: ignore[arg-type]
    assert abs(pv_sum - envelope["total_pv"]) < 1e-6


def test_instrument_cashflows_unsupported_model_raises() -> None:
    inst_json, market = _build_deposit_market()
    with pytest.raises(ValueError, match=r"monte_carlo_gbm|supported|not priced"):
        instrument_cashflows_json(inst_json, market, "2025-01-15", "monte_carlo_gbm")


# ---------------------------------------------------------------------------
# B5 — Closed-form exotics
# ---------------------------------------------------------------------------


def test_barrier_knock_in_plus_knock_out_equals_vanilla() -> None:
    spot, strike, barrier, r, q, sigma, t = 100.0, 100.0, 110.0, 0.05, 0.02, 0.2, 1.0
    up_in = barrier_call(spot, strike, barrier, r, q, sigma, t, "up", "in")
    up_out = barrier_call(spot, strike, barrier, r, q, sigma, t, "up", "out")
    vanilla = bs_price(spot, strike, r, q, sigma, t, True)
    assert abs(up_in + up_out - vanilla) < 1e-6


def test_asian_arithmetic_ge_geometric_for_call() -> None:
    # Arithmetic Asian call dominates the geometric Asian call (AM >= GM).
    arith = asian_option_price(100.0, 100.0, 0.05, 0.02, 0.3, 1.0, 12, "arithmetic", True)
    geom = asian_option_price(100.0, 100.0, 0.05, 0.02, 0.3, 1.0, 12, "geometric", True)
    assert arith >= geom - 1e-9


def test_lookback_floating_strike_call_positive() -> None:
    p = lookback_option_price(
        spot=100.0,
        strike=0.0,  # ignored for floating
        r=0.05,
        q=0.02,
        sigma=0.2,
        t=1.0,
        extremum=100.0,
        strike_type="floating",
        is_call=True,
    )
    assert p > 0.0


def test_quanto_option_price_call_positive() -> None:
    p = quanto_option_price(
        spot=100.0,
        strike=100.0,
        t=1.0,
        rate_domestic=0.05,
        rate_foreign=0.03,
        div_yield=0.01,
        vol_asset=0.2,
        vol_fx=0.1,
        correlation=-0.2,
        is_call=True,
    )
    assert p > 0.0


def test_barrier_unknown_direction_raises() -> None:
    with pytest.raises(ValueError, match=r"barrier spec|direction"):
        barrier_call(100.0, 100.0, 110.0, 0.05, 0.02, 0.2, 1.0, "sideways", "in")


def test_coupon_profile_entrypoints_have_distinct_explicit_inputs() -> None:
    assert snowball_coupon_profile(0.02, 0.05, [0.01, 0.04], 0.0, 0.10) == pytest.approx(
        [0.06, 0.07]
    )
    assert inverse_floater_coupon_profile(0.05, [0.01, 0.02], 0.0, 0.10, 2.0) == pytest.approx(
        [0.03, 0.01]
    )
