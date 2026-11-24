"""Native QuantLib stochastic-volatility equity option golden builders."""

from __future__ import annotations

from typing import Any

import QuantLib as ql  # type: ignore[import-not-found]  # noqa: N813

from .common import (
    SCHEMA,
    VALUATION_DATE,
    constant_vol_surface,
    flat_discount_curve,
    market_snapshot,
    metadata,
    ql_date,
)

CAPTURED_ON = "2026-11-23"
SPOT = 5000.0
RATE = 0.04
DIVIDEND_YIELD = 0.015
QUANTITY = 100.0
HESTON = {"v0": 0.04, "kappa": 1.5, "theta": 0.05, "sigma_v": 0.5, "rho": -0.7}


def _heston_value(option_type: int, strike: float) -> float:
    evaluation_date = ql_date(VALUATION_DATE)
    ql.Settings.instance().evaluationDate = evaluation_date
    day_count = ql.Actual365Fixed()
    process = ql.HestonProcess(
        ql.YieldTermStructureHandle(ql.FlatForward(evaluation_date, RATE, day_count, ql.Continuous)),
        ql.YieldTermStructureHandle(ql.FlatForward(evaluation_date, DIVIDEND_YIELD, day_count, ql.Continuous)),
        ql.QuoteHandle(ql.SimpleQuote(SPOT)),
        HESTON["v0"],
        HESTON["kappa"],
        HESTON["theta"],
        HESTON["sigma_v"],
        HESTON["rho"],
    )
    option = ql.VanillaOption(ql.PlainVanillaPayoff(option_type, strike), ql.EuropeanExercise(ql.Date(30, 4, 2027)))
    option.setPricingEngine(ql.AnalyticHestonEngine(ql.HestonModel(process), 1e-12, 100_000))
    return option.NPV()


def _build_heston_option(*, product: str, name: str, call: bool, strike: float) -> dict[str, Any]:
    side = "call" if call else "put"
    npv = QUANTITY * _heston_value(ql.Option.Call if call else ql.Option.Put, strike)
    meta = metadata(
        name=name,
        domain="equity.equity_option",
        description=f"QuantLib native one-year Heston European equity {side}, strike {strike:.0f}.",
        product=product,
        source_detail=(
            f"QuantLib {ql.__version__} AnalyticHestonEngine (adaptive Gauss-Lobatto, relative tolerance 1e-12): "
            f"spot {SPOT}, flat continuous Act/365F rate {RATE} and dividend yield {DIVIDEND_YIELD}, "
            f"v0 {HESTON['v0']}, kappa {HESTON['kappa']}, theta {HESTON['theta']}, vol-of-vol {HESTON['sigma_v']}, "
            f"rho {HESTON['rho']}, quantity {QUANTITY:.0f}."
        ),
    )
    return {
        "schema": SCHEMA,
        "metadata": {**meta, "captured_on": CAPTURED_ON, "last_reviewed_on": CAPTURED_ON},
        "kind": "pricing",
        "model": "heston_fourier",
        "market": market_snapshot(
            [flat_discount_curve("USD-OIS", RATE)],
            prices={
                "HESTON_KAPPA": {"unitless": HESTON["kappa"]},
                "HESTON_RHO": {"unitless": HESTON["rho"]},
                "HESTON_SIGMA_V": {"unitless": HESTON["sigma_v"]},
                "HESTON_THETA": {"unitless": HESTON["theta"]},
                "HESTON_V0": {"unitless": HESTON["v0"]},
                "SPX-DIVYIELD": {"unitless": DIVIDEND_YIELD},
                "SPX-SPOT": {"price": {"amount": str(SPOT), "currency": "USD"}},
            },
            surfaces=[constant_vol_surface("SPX-VOL-QL", 0.20, strikes=[2500.0, 5000.0, 10000.0])],
        ),
        "instrument": {
            "schema": "finstack_quant.instrument/1",
            "instrument": {
                "type": "equity_option",
                "spec": {
                    "id": name.upper().replace("_", "-"),
                    "underlying_ticker": "SPX",
                    "strike": strike,
                    "option_type": side,
                    "expiry": "2027-04-30",
                    "quantity": QUANTITY,
                    "currency": "USD",
                    "discount_curve_id": "USD-OIS",
                    "spot_id": "SPX-SPOT",
                    "vol_surface_id": "SPX-VOL-QL",
                    "div_yield_id": "SPX-DIVYIELD",
                    "attributes": {"tags": ["golden", "quantlib"], "meta": {}},
                },
            },
        },
        "expected": {"npv": npv},
        "tolerances": {
            "npv": {
                "rel": 1e-6,
                "tolerance_reason": (
                    "Same Heston characteristic function, different quadrature (composite Gauss-Legendre "
                    "against QuantLib's adaptive Gauss-Lobatto): the 1e-6 relative bound adjudicated for the "
                    "model-level Heston suite in models/tests/golden."
                ),
            }
        },
    }


def build_heston_equity_call() -> dict[str, Any]:
    """Build a one-year at-the-money Heston equity call."""
    return _build_heston_option(
        product="heston_equity_call", name="spx_heston_atm_call_1y_quantlib", call=True, strike=5000.0
    )


def build_heston_equity_put() -> dict[str, Any]:
    """Build a one-year 10% out-of-the-money Heston equity put."""
    return _build_heston_option(
        product="heston_equity_put", name="spx_heston_otm_put_1y_quantlib", call=False, strike=4500.0
    )
