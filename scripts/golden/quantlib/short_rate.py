"""Native QuantLib Hull-White one-factor golden builders."""

from __future__ import annotations

from typing import Any

import QuantLib as ql  # type: ignore[import-not-found]  # noqa: N813

from .common import (
    SCHEMA,
    VALUATION_DATE,
    central_difference,
    flat_discount_curve,
    market_snapshot,
    metadata,
    ql_date,
)

CAPTURED_ON = "2026-11-23"
NOTIONAL = 10_000_000.0
MEAN_REVERSION = 0.03
SIGMA = 0.01
FLAT_RATE = 0.04
STRIKE = 0.042
TREE_STEPS = 400


def _captured(meta: dict[str, Any]) -> dict[str, Any]:
    return {**meta, "captured_on": CAPTURED_ON, "last_reviewed_on": CAPTURED_ON}


def _jamshidian_payer_swaption(rate_shift: float = 0.0) -> float:
    """Price a 1Y into 5Y annual payer swaption with QuantLib's exact Jamshidian engine."""
    evaluation_date = ql_date(VALUATION_DATE)
    ql.Settings.instance().evaluationDate = evaluation_date
    day_count = ql.Actual365Fixed()
    curve = ql.YieldTermStructureHandle(
        ql.FlatForward(evaluation_date, FLAT_RATE + rate_shift, day_count, ql.Continuous)
    )
    index = ql.IborIndex(
        "USD-TERM-1Y",
        ql.Period(1, ql.Years),
        0,
        ql.USDCurrency(),
        ql.NullCalendar(),
        ql.Unadjusted,
        False,
        day_count,
        curve,
    )
    schedule = ql.Schedule(
        ql.Date(30, 4, 2027),
        ql.Date(30, 4, 2032),
        ql.Period(1, ql.Years),
        ql.NullCalendar(),
        ql.Unadjusted,
        ql.Unadjusted,
        ql.DateGeneration.Forward,
        False,
    )
    swap = ql.VanillaSwap(
        ql.VanillaSwap.Payer, NOTIONAL, schedule, STRIKE, day_count, schedule, index, 0.0, day_count, ql.Unadjusted
    )
    swaption = ql.Swaption(swap, ql.EuropeanExercise(ql.Date(30, 4, 2027)))
    model = ql.HullWhite(curve, MEAN_REVERSION, SIGMA)
    swaption.setPricingEngine(ql.JamshidianSwaptionEngine(model, curve))
    return swaption.NPV()


def build_hull_white_swaption() -> dict[str, Any]:
    """Build a Hull-White one-factor European payer swaption fixture."""
    name = "usd_hull_white_1y5y_payer_swaption_quantlib"
    npv = _jamshidian_payer_swaption()
    reason = (
        "QuantLib's Jamshidian decomposition is exact for Hull-White; Finstack prices the same model on a "
        f"{TREE_STEPS}-step trinomial tree, whose first-order time discretization is bounded here by one "
        "step in 400. DV01 is a 1bp parallel continuous zero shift with the model parameters held fixed."
    )
    leg = {
        "discount_curve_id": "USD-OIS",
        "frequency": {"count": 12, "unit": "months"},
        "day_count": "act_365f",
        "calendar_id": "weekends_only",
        "business_day_convention": "unadjusted",
        "stub": "none",
        "start": "2027-04-30",
        "end": "2032-04-30",
        "payment_lag_days": 0,
    }
    return {
        "schema": SCHEMA,
        "metadata": _captured(
            metadata(
                name=name,
                domain="rates.swaption",
                description="QuantLib native 1Y into 5Y Hull-White one-factor payer swaption.",
                product="hull_white_swaption",
                source_detail=(
                    f"QuantLib {ql.__version__} JamshidianSwaptionEngine on HullWhite(a={MEAN_REVERSION}, "
                    f"sigma={SIGMA}) over a flat {FLAT_RATE} continuous Act/365F curve; single-curve annual "
                    "Act/365F fixed and floating legs, unadjusted dates, physical settlement."
                ),
            )
        ),
        "kind": "pricing",
        "model": "hull_white_1f",
        "market": market_snapshot([flat_discount_curve("USD-OIS", FLAT_RATE)]),
        "instrument": {
            "schema": "finstack_quant.instrument/1",
            "instrument": {
                "type": "swaption",
                "spec": {
                    "id": name.upper().replace("_", "-"),
                    "option_type": "call",
                    "notional": {"amount": "10000000", "currency": "USD"},
                    "expiry": "2027-04-30",
                    "settlement": "physical",
                    "cash_settlement_method": "par_yield",
                    "vol_model": "normal",
                    "vol_surface_id": "USD-SWAPTION-VOL-UNUSED",
                    "underlying_fixed_leg": {**leg, "rate": str(STRIKE)},
                    "underlying_float_leg": {
                        **leg,
                        "index_id": "USD-OIS",
                        "forward_curve_id": "USD-OIS",
                        "spread_bp": "0",
                        "reset_lag_days": 0,
                        "fixing_calendar_id": "weekends_only",
                        "compounding": "simple",
                    },
                    "sabr_params": None,
                    "instrument_pricing_overrides": {
                        "model_config": {
                            "tree_steps": TREE_STEPS,
                            "hw1f_mean_reversion": MEAN_REVERSION,
                            "hw1f_sigma": SIGMA,
                        }
                    },
                    "attributes": {"tags": ["golden", "quantlib"], "meta": {}},
                },
            },
        },
        "expected": {"dv01": central_difference(_jamshidian_payer_swaption, 0.0), "npv": npv},
        "tolerances": {
            "dv01": {"rel": 2.5e-3, "tolerance_reason": reason},
            "npv": {"rel": 2.5e-3, "tolerance_reason": reason},
        },
    }
