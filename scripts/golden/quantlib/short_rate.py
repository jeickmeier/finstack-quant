"""Native QuantLib Hull-White one-factor golden builders."""

from __future__ import annotations

from typing import Any

import QuantLib as ql  # type: ignore[import-not-found]  # noqa: N813

from .common import (
    SCHEMA,
    VALUATION_DATE,
    central_difference,
    constant_vol_surface,
    flat_discount_curve,
    flat_forward_curve,
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


def _hull_white_payer_swaption(rate_shift: float = 0.0) -> float:
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
    npv = _hull_white_payer_swaption()
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
        "expected": {"dv01": central_difference(_hull_white_payer_swaption, 0.0), "npv": npv},
        "tolerances": {
            "dv01": {"rel": 2.5e-3, "tolerance_reason": reason},
            "npv": {"rel": 2.5e-3, "tolerance_reason": reason},
        },
    }


EXERCISE_DATES = [f"{year}-04-30" for year in range(2027, 2032)]


def _hull_white_bermudan_payer_swaption(rate_shift: float = 0.0, sigma_shift: float = 0.0) -> float:
    """Price the Bermudan option on the 5Y annual payer swap with QuantLib finite differences.

    Exercise is allowed on each annual date from the swap start. The engine
    runs on an 800 by 1600 time-by-rate grid.
    """
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
    swaption = ql.Swaption(swap, ql.BermudanExercise([ql_date(date) for date in EXERCISE_DATES]))
    model = ql.HullWhite(curve, MEAN_REVERSION, SIGMA + sigma_shift)
    swaption.setPricingEngine(ql.FdHullWhiteSwaptionEngine(model, 800, 1600))
    return swaption.NPV()


def build_hull_white_bermudan_swaption() -> dict[str, Any]:
    """Build a Hull-White one-factor Bermudan payer swaption fixture."""
    name = "usd_hull_white_5nc1_bermudan_payer_swaption_quantlib"
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
    reason = (
        "QuantLib's finite-difference Hull-White engine (800 time by 1600 rate steps, within 1e-6 relative "
        "of the exact Jamshidian price on the European exercise) against Finstack's "
        f"{TREE_STEPS}-step trinomial tree, whose first-order time discretization is bounded here by one "
        "step in 400. DV01 is a 1bp parallel continuous zero shift with the model parameters held fixed; "
        "the sigma vega is the PV change per 0.01 of Hull-White sigma from a central difference."
    )
    expected = {
        "dv01": central_difference(_hull_white_bermudan_payer_swaption, 0.0),
        "hw_sigma_vega": central_difference(lambda shift: _hull_white_bermudan_payer_swaption(sigma_shift=shift), 0.0)
        / 1e-4
        * 0.01,
        "npv": _hull_white_bermudan_payer_swaption(),
    }
    return {
        "schema": SCHEMA,
        "metadata": _captured(
            metadata(
                name=name,
                domain="rates.bermudan_swaption",
                description="QuantLib native Hull-White one-factor Bermudan payer swaption, five annual exercises.",
                product="hull_white_bermudan_swaption",
                source_detail=(
                    f"QuantLib {ql.__version__} FdHullWhiteSwaptionEngine (800 time steps, 1600 rate steps) on "
                    f"HullWhite(a={MEAN_REVERSION}, sigma={SIGMA}) over a flat {FLAT_RATE} continuous Act/365F "
                    "curve; co-terminal exercise into a single-curve annual Act/365F swap ending 2032-04-30, "
                    "exercisable on each 30 April from 2027 to 2031, physical settlement."
                ),
            )
        ),
        "kind": "pricing",
        "model": "hull_white_1f",
        "market": market_snapshot([flat_discount_curve("USD-OIS", FLAT_RATE)]),
        "instrument": {
            "schema": "finstack_quant.instrument/1",
            "instrument": {
                "type": "bermudan_swaption",
                "spec": {
                    "id": name.upper().replace("_", "-"),
                    "option_type": "call",
                    "notional": {"amount": "10000000", "currency": "USD"},
                    "settlement": "physical",
                    "bermudan_type": "co_terminal",
                    "exercise_schedule": {"exercise_dates": EXERCISE_DATES, "lockout_end": None},
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
        "expected": expected,
        "tolerances": {metric: {"rel": 2.5e-3, "tolerance_reason": reason} for metric in expected},
    }


# Every quarterly date from 3 August 2026 to 3 August 2028 is a weekday, so no
# accrual or payment date is rolled and both libraries see one schedule.
CAP_STRIKE = 0.041
CAP_START = "2026-08-03"
CAP_END = "2028-08-03"


def _hull_white_cap(rate_shift: float = 0.0) -> float:
    """Price a two-year quarterly cap with QuantLib's closed-form Hull-White engine."""
    evaluation_date = ql_date(VALUATION_DATE)
    ql.Settings.instance().evaluationDate = evaluation_date
    curve = ql.YieldTermStructureHandle(
        ql.FlatForward(evaluation_date, FLAT_RATE + rate_shift, ql.Actual365Fixed(), ql.Continuous)
    )
    accrual = ql.Actual360()
    index = ql.IborIndex(
        "USD-TERM-3M",
        ql.Period(3, ql.Months),
        0,
        ql.USDCurrency(),
        ql.NullCalendar(),
        ql.Unadjusted,
        False,
        accrual,
        curve,
    )
    schedule = ql.Schedule(
        ql_date(CAP_START),
        ql_date(CAP_END),
        ql.Period(3, ql.Months),
        ql.NullCalendar(),
        ql.Unadjusted,
        ql.Unadjusted,
        ql.DateGeneration.Forward,
        False,
    )
    cap = ql.Cap(ql.IborLeg([NOTIONAL], schedule, index, accrual, ql.Unadjusted, [0]), [CAP_STRIKE])
    cap.setPricingEngine(ql.AnalyticCapFloorEngine(ql.HullWhite(curve, MEAN_REVERSION, SIGMA), curve))
    return cap.NPV()


def build_hull_white_cap() -> dict[str, Any]:
    """Build a Hull-White one-factor quarterly cap fixture."""
    name = "usd_hull_white_cap_2y_quantlib"
    reason = (
        "QuantLib AnalyticCapFloorEngine and Finstack both price each term caplet as an exact Hull-White "
        "discount-bond option; the bound is the 5e-9 relative normal-CDF agreement between the libraries. "
        "DV01 is a 1bp parallel continuous zero shift with the model parameters held fixed."
    )
    return {
        "schema": SCHEMA,
        "metadata": _captured(
            metadata(
                name=name,
                domain="rates.cap_floor",
                description="QuantLib native two-year quarterly Hull-White one-factor cap.",
                product="hull_white_cap",
                source_detail=(
                    f"QuantLib {ql.__version__} AnalyticCapFloorEngine on HullWhite(a={MEAN_REVERSION}, "
                    f"sigma={SIGMA}) over a flat {FLAT_RATE} continuous Act/365F curve; eight quarterly Act/360 "
                    f"caplets on a single-curve three-month index from {CAP_START} to {CAP_END}, strike {CAP_STRIKE}."
                ),
            )
        ),
        "kind": "pricing",
        "model": "hull_white_1f",
        "market": market_snapshot(
            [flat_discount_curve("USD-OIS", FLAT_RATE), flat_forward_curve("USD-TERM-3M", FLAT_RATE)],
            surfaces=[
                constant_vol_surface("USD-CAPFLOOR-VOL-UNUSED", 0.01, quote_type="normal", strikes=[0.01, 0.04, 0.10])
            ],
        ),
        "instrument": {
            "schema": "finstack_quant.instrument/1",
            "instrument": {
                "type": "cap_floor",
                "spec": {
                    "id": name.upper().replace("_", "-"),
                    "rate_option_type": "cap",
                    "notional": {"amount": "10000000", "currency": "USD"},
                    "strike": str(CAP_STRIKE),
                    "start_date": CAP_START,
                    "maturity": CAP_END,
                    "frequency": {"count": 3, "unit": "months"},
                    "day_count": "act_360",
                    "calendar_id": "weekends_only",
                    "discount_curve_id": "USD-OIS",
                    "forward_curve_id": "USD-TERM-3M",
                    "vol_surface_id": "USD-CAPFLOOR-VOL-UNUSED",
                    "vol_type": "normal",
                    "instrument_pricing_overrides": {
                        "model_config": {"hw1f_mean_reversion": MEAN_REVERSION, "hw1f_sigma": SIGMA}
                    },
                    "attributes": {"tags": ["golden", "quantlib"], "meta": {}},
                },
            },
        },
        "expected": {"dv01": central_difference(_hull_white_cap, 0.0), "npv": _hull_white_cap()},
        "tolerances": {
            "dv01": {"rel": 5e-9, "tolerance_reason": reason},
            "npv": {"rel": 5e-9, "tolerance_reason": reason},
        },
    }
