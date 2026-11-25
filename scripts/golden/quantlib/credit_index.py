"""Native QuantLib CDS index golden builders.

A CDS index priced on one flat index hazard curve with an index factor of 1.0
is a single-name CDS on that curve, so the reference is QuantLib's
``CreditDefaultSwap`` priced with ``IsdaCdsEngine`` on index-style terms.
"""

from __future__ import annotations

import math
from typing import Any

import QuantLib as ql  # type: ignore[import-not-found]  # noqa: N813

from .common import SCHEMA, metadata, ql_date, ql_date_to_iso, tolerance

INDEX_VALUATION_DATE = "2026-01-05"
INDEX_MATURITY = "2030-12-20"
NOTIONAL = 10_000_000.0
RECOVERY = 0.40
COUPON = 0.01
FLAT_RATE = 0.03
FLAT_HAZARD = 0.015
BUMP_BP = 1e-4
DISCOUNT_CURVE_ID = "USD-FLAT-3PCT"
HAZARD_CURVE_ID = "QL-INDEX-FLAT-HAZARD"
FIXTURE_NAME = "cds_index_quantlib_flat_hazard_single_curve"


def _quantlib_curves(
    rate: float,
    hazard: float,
) -> tuple[ql.YieldTermStructure, ql.DefaultProbabilityTermStructure]:
    today = ql_date(INDEX_VALUATION_DATE)
    ql.Settings.instance().evaluationDate = today
    discount = ql.FlatForward(today, rate, ql.Actual365Fixed(), ql.Continuous, ql.Annual)
    default_curve = ql.FlatHazardRate(
        today,
        ql.QuoteHandle(ql.SimpleQuote(hazard)),
        ql.Actual365Fixed(),
    )
    return discount, default_curve


def _quantlib_index_cds(
    rate: float = FLAT_RATE,
    hazard: float = FLAT_HAZARD,
) -> ql.CreditDefaultSwap:
    discount, default_curve = _quantlib_curves(rate, hazard)
    schedule = ql.Schedule(
        ql_date(INDEX_VALUATION_DATE),
        ql_date(INDEX_MATURITY),
        ql.Period(3, ql.Months),
        ql.WeekendsOnly(),
        ql.Following,
        ql.Unadjusted,
        ql.DateGeneration.CDS,
        False,
    )
    cds = ql.CreditDefaultSwap(
        ql.Protection.Buyer,
        NOTIONAL,
        COUPON,
        schedule,
        ql.Following,
        ql.Actual360(True),
        True,
        True,
    )
    cds.setPricingEngine(
        ql.IsdaCdsEngine(
            ql.DefaultProbabilityTermStructureHandle(default_curve),
            RECOVERY,
            ql.YieldTermStructureHandle(discount),
            False,
            ql.IsdaCdsEngine.Taylor,
            ql.IsdaCdsEngine.HalfDayBias,
            ql.IsdaCdsEngine.Piecewise,
        )
    )
    return cds


def _canonical_risky_annuity(cds: ql.CreditDefaultSwap) -> float:
    """Sum discounted survival-weighted accruals over the QuantLib coupons."""
    discount, default_curve = _quantlib_curves(FLAT_RATE, FLAT_HAZARD)
    annuity = 0.0
    for cashflow in cds.coupons():
        coupon = ql.as_fixed_rate_coupon(cashflow)
        annuity += (
            coupon.accrualPeriod()
            * discount.discount(coupon.date())
            * default_curve.survivalProbability(coupon.accrualEndDate())
        )
    return annuity


def build_cds_index() -> dict[str, Any]:
    """Build the flat-hazard single-curve CDS index decomposition fixture."""
    cds = _quantlib_index_cds()
    premium_start = ql_date_to_iso(ql.as_fixed_rate_coupon(cds.coupons()[0]).accrualStartDate())
    risky_annuity = _canonical_risky_annuity(cds)
    dv01 = (
        _quantlib_index_cds(rate=FLAT_RATE + BUMP_BP).NPV() - _quantlib_index_cds(rate=FLAT_RATE - BUMP_BP).NPV()
    ) / 2.0
    quantlib_reason = (
        "Strict executable QuantLib IsdaCdsEngine decomposition benchmark; same tolerance as the "
        "single-name cds_quantlib_flat_hazard_decomposition fixture, which a single-curve index "
        "with index_factor 1.0 must reproduce."
    )
    canonical_reason = (
        "Canonical sum of discounted survival-weighted accruals over the QuantLib coupon schedule "
        "and flat curves (not an engine output), excluding the accrual-on-default contribution "
        "included in QuantLib couponLegNPV; same tolerance as the single-name CDS fixture."
    )
    fixture = {
        "schema": SCHEMA,
        "metadata": metadata(
            name=FIXTURE_NAME,
            domain="credit.cds_index",
            description=(
                "QuantLib IsdaCdsEngine flat hazard/flat discount CDS index benchmark priced on a "
                "single index hazard curve with index factor 1.0."
            ),
            product="cds_index",
            valuation_date=INDEX_VALUATION_DATE,
            source_detail=(
                f"QuantLib {ql.__version__}; CreditDefaultSwap with IsdaCdsEngine(Taylor, HalfDayBias, "
                "Piecewise), flat 1.5% Actual/365F index hazard and flat 3% continuous Actual/365F "
                "discount curve."
            ),
        ),
        "kind": "pricing",
        "model": "hazard_rate",
        "market": {
            "kind": "envelope",
            "envelope": {
                "schema": "finstack_quant.calibration/1",
                "plan": {
                    "id": f"{FIXTURE_NAME}_tier_a_wrap",
                    "description": "Pre-built deterministic flat curves; no calibration steps required.",
                    "quote_sets": {},
                    "steps": [],
                    "settings": {
                        "fx": {
                            "pivot_currency": "USD",
                            "cache_capacity": 256,
                        }
                    },
                },
                "market_data": [],
                "prior_market": [
                    {
                        "kind": "hazard_curve",
                        "id": HAZARD_CURVE_ID,
                        "base": INDEX_VALUATION_DATE,
                        "knot_points": [[10.0, FLAT_HAZARD]],
                        "recovery_rate": RECOVERY,
                        "issuer": None,
                        "seniority": None,
                        "currency": None,
                        "day_count": "act_365f",
                    },
                    {
                        "kind": "discount_curve",
                        "id": DISCOUNT_CURVE_ID,
                        "base": INDEX_VALUATION_DATE,
                        "day_count": "act_365f",
                        "knot_points": [[0.0, 1.0], [10.0, math.exp(-FLAT_RATE * 10.0)]],
                        "interp_style": "log_linear",
                        "extrapolation": "extend",
                        "rate_calibration": None,
                        "fx_policy": None,
                    },
                ],
            },
        },
        "instrument": {
            "schema": "finstack_quant.instrument/1",
            "instrument": {
                "type": "cds_index",
                "spec": {
                    "id": "QL-FLAT-HAZARD-CDS-INDEX-5Y-20260105",
                    "include_valuation_date_cashflows": False,
                    "index_name": "QL.FLAT.INDEX",
                    "series": 1,
                    "version": 1,
                    "notional": {"amount": "10000000", "currency": "USD"},
                    "index_factor": 1.0,
                    "side": "pay",
                    "convention": "isda_na",
                    "premium_leg": {
                        "start": premium_start,
                        "end": INDEX_MATURITY,
                        "frequency": {"count": 3, "unit": "months"},
                        "date_regime": "standard",
                        "calendar_id": "weekends_only",
                        "day_count": "act_360",
                        "coupon_bp": "100.0",
                        "discount_curve_id": DISCOUNT_CURVE_ID,
                    },
                    "protection_leg": {
                        "credit_curve_id": HAZARD_CURVE_ID,
                        "recovery_rate": RECOVERY,
                        "settlement_delay": 0,
                    },
                    "pricing": "single_curve",
                    "constituents": [],
                    "num_constituents": 125,
                    "valuation_convention": "quant_lib_isda_parity",
                    "attributes": {
                        "tags": ["golden", "quantlib", "cds-index", "flat-hazard", "single-curve"],
                        "meta": {
                            "quantlib_engine": "IsdaCdsEngine",
                            "quantlib_version": ql.__version__,
                            "schedule_generation": "DateGeneration.CDS",
                            "calendar": "WeekendsOnly",
                            "discount_rate_continuous": "0.03",
                            "flat_hazard_rate": "0.015",
                        },
                    },
                },
            },
        },
        "expected": {
            "npv": cds.NPV(),
            "par_spread": cds.fairSpread() * 10_000.0,
            "risky_pv01": risky_annuity * NOTIONAL * BUMP_BP,
            "protection_leg_pv": cds.defaultLegNPV(),
            "premium_leg_pv": abs(cds.couponLegNPV()),
            "dv01": dv01,
        },
        "tolerances": {
            "npv": tolerance(1.0, quantlib_reason),
            "par_spread": tolerance(0.01, quantlib_reason),
            "risky_pv01": tolerance(0.1, canonical_reason),
            "protection_leg_pv": tolerance(1.0, quantlib_reason),
            "premium_leg_pv": tolerance(1.0, quantlib_reason),
            "dv01": tolerance(
                0.1,
                "QuantLib central finite difference under a 1bp parallel continuous-rate bump with the "
                "index hazard curve held fixed; same tolerance as the single-name CDS fixture.",
            ),
        },
    }
    return fixture
