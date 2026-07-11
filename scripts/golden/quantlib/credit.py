"""Native QuantLib credit-product golden builders."""

from __future__ import annotations

from typing import Any

import QuantLib as ql  # type: ignore[import-not-found]  # noqa: N813

from .common import SCHEMA_VERSION, metadata, tolerance

CDS_VALUATION_DATE = "2026-01-05"
NOTIONAL = 10_000_000.0
RECOVERY = 0.40
COUPON = 0.01
FLAT_RATE = 0.02
FLAT_HAZARD = 0.01
BUMP_BP = 1e-4
CANONICAL_RISKY_ANNUITY = 4.744408070298757


def _quantlib_cds(
    rate: float = FLAT_RATE,
    hazard: float = FLAT_HAZARD,
) -> ql.CreditDefaultSwap:
    today = ql.Date(5, ql.January, 2026)
    ql.Settings.instance().evaluationDate = today
    schedule = ql.Schedule(
        today,
        ql.Date(20, ql.December, 2030),
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
    discount = ql.YieldTermStructureHandle(ql.FlatForward(today, rate, ql.Actual365Fixed(), ql.Continuous, ql.Annual))
    default_curve = ql.DefaultProbabilityTermStructureHandle(
        ql.FlatHazardRate(
            today,
            ql.QuoteHandle(ql.SimpleQuote(hazard)),
            ql.Actual365Fixed(),
        )
    )
    cds.setPricingEngine(
        ql.IsdaCdsEngine(
            default_curve,
            RECOVERY,
            discount,
            False,
            ql.IsdaCdsEngine.Taylor,
            ql.IsdaCdsEngine.HalfDayBias,
            ql.IsdaCdsEngine.Piecewise,
        )
    )
    return cds


def build_single_name_cds() -> dict[str, Any]:
    """Build the existing flat-hazard single-name CDS decomposition fixture."""
    cds = _quantlib_cds()
    premium_leg_pv = abs(cds.couponLegNPV())
    dv01 = (_quantlib_cds(rate=FLAT_RATE + BUMP_BP).NPV() - _quantlib_cds(rate=FLAT_RATE - BUMP_BP).NPV()) / 2.0
    cs01 = (_quantlib_cds(hazard=FLAT_HAZARD + BUMP_BP).NPV() - _quantlib_cds(hazard=FLAT_HAZARD - BUMP_BP).NPV()) / 2.0
    quantlib_reason = "Strict executable QuantLib IsdaCdsEngine decomposition benchmark."
    canonical_reason = (
        "Canonical sum of discounted survival-weighted accruals, excluding the "
        "accrual-on-default contribution included in QuantLib couponLegNPV."
    )
    return {
        "schema_version": SCHEMA_VERSION,
        "metadata": metadata(
            name="cds_quantlib_flat_hazard_decomposition",
            domain="credit.cds",
            description="QuantLib IsdaCdsEngine flat hazard/flat discount CDS decomposition benchmark.",
            product="single_name_cds",
            valuation_date=CDS_VALUATION_DATE,
            source_detail=(
                f"QuantLib {ql.__version__}; IsdaCdsEngine(Taylor, HalfDayBias, Piecewise), "
                "flat 1% Actual/365F hazard and flat 2% continuous Actual/365F discount curve."
            ),
        ),
        "kind": "pricing",
        "model": "hazard_rate",
        "market": {
            "kind": "envelope",
            "envelope": {
                "schema": "finstack_quant.calibration",
                "plan": {
                    "id": "cds_quantlib_flat_hazard_decomposition_tier_a_wrap",
                    "description": "Pre-built deterministic flat curves; no calibration steps required.",
                    "quote_sets": {},
                    "steps": [],
                    "settings": {
                        "fx": {
                            "pivot_currency": "USD",
                            "enable_triangulation": True,
                            "cache_capacity": 256,
                        }
                    },
                },
                "market_data": [],
                "prior_market": [
                    {
                        "kind": "hazard_curve",
                        "id": "QL-FLAT-HAZARD",
                        "base": CDS_VALUATION_DATE,
                        "knot_points": [[0.0, FLAT_HAZARD], [10.0, FLAT_HAZARD]],
                        "recovery_rate": RECOVERY,
                        "issuer": None,
                        "seniority": None,
                        "currency": None,
                        "day_count": "Act365F",
                        "par_points": [],
                        "par_interp": "Linear",
                    },
                    {
                        "kind": "discount_curve",
                        "id": "USD-FLAT-2PCT",
                        "base": CDS_VALUATION_DATE,
                        "day_count": "Act365F",
                        "knot_points": [[0.0, 1.0], [10.0, 0.8187307530779818]],
                        "interp_style": "log_linear",
                        "extrapolation": "flat_forward",
                        "min_forward_rate": None,
                        "allow_non_monotonic": False,
                        "min_forward_tenor": 1e-6,
                        "rate_calibration": None,
                    },
                ],
            },
        },
        "instrument": {
            "type": "credit_default_swap",
            "spec": {
                "id": "QL-FLAT-HAZARD-CDS-5Y-20260105",
                "notional": {"amount": "10000000", "currency": "USD"},
                "side": "pay",
                "convention": "isda_na",
                "premium": {
                    "start": "2025-09-22",
                    "end": "2030-12-20",
                    "frequency": {"count": 3, "unit": "months"},
                    "calendar_id": "weekends",
                    "day_count": "Act360",
                    "spread_bp": "100.0",
                    "discount_curve_id": "USD-FLAT-2PCT",
                },
                "protection": {
                    "credit_curve_id": "QL-FLAT-HAZARD",
                    "recovery_rate": RECOVERY,
                    "settlement_delay": 0,
                },
                "pricing_overrides": {
                    "cds_aod_half_day_bias": True,
                    "cds_act360_include_last_day": True,
                },
                "valuation_convention": "quant_lib_isda_parity",
                "doc_clause": "xr14",
                "protection_effective_date": CDS_VALUATION_DATE,
                "attributes": {
                    "tags": ["golden", "quantlib", "cds", "flat-hazard", "decomposition"],
                    "meta": {
                        "quantlib_engine": "IsdaCdsEngine",
                        "quantlib_version": ql.__version__,
                        "schedule_generation": "DateGeneration.CDS",
                        "calendar": "WeekendsOnly",
                        "discount_rate_continuous": "0.02",
                        "flat_hazard_rate": "0.01",
                    },
                },
            },
        },
        "expected": {
            "npv": cds.NPV(),
            "par_spread": cds.fairSpread() * 10_000.0,
            "risky_annuity": CANONICAL_RISKY_ANNUITY,
            "risky_pv01": CANONICAL_RISKY_ANNUITY * NOTIONAL * BUMP_BP,
            "protection_leg_pv": cds.defaultLegNPV(),
            "premium_leg_pv": premium_leg_pv,
            "dv01": dv01,
            "cs01": cs01,
        },
        "tolerances": {
            "npv": tolerance(1.0, quantlib_reason),
            "par_spread": tolerance(0.01, quantlib_reason),
            "risky_annuity": tolerance(0.0001, canonical_reason),
            "risky_pv01": tolerance(0.1, canonical_reason),
            "protection_leg_pv": tolerance(1.0, quantlib_reason),
            "premium_leg_pv": tolerance(1.0, quantlib_reason),
            "dv01": tolerance(0.1, "QuantLib central finite difference under a 1bp parallel continuous-rate bump."),
            "cs01": tolerance(1.0, "QuantLib central finite difference under a direct 1bp hazard-rate bump."),
        },
    }
