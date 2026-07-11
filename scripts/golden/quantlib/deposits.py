"""Native QuantLib money-market deposit golden builders."""

from __future__ import annotations

import math
from typing import Any

import QuantLib as ql  # type: ignore[import-not-found]  # noqa: N813

from .common import SCHEMA_VERSION, VALUATION_DATE, metadata, tolerance

NOTIONAL = 10_000_000.0
QUOTE_RATE = 0.041
BUMP_BP = 1e-4
CURVE_TIMES = [0.0, 0.25, 0.5, 1.0, 2.0, 5.0]
CURVE_DISCOUNTS = [1.0, 0.99, 0.9802, 0.9608, 0.9231, 0.8187]


def build_deposit() -> dict[str, Any]:
    """Build the existing three-month USD deposit fixture."""
    start = ql.Date(30, ql.April, 2026)
    maturity = ql.Date(30, ql.July, 2026)
    discount_time = ql.Actual365Fixed().yearFraction(start, maturity)
    accrual = ql.Actual360().yearFraction(start, maturity)
    interpolation = ql.LogLinearInterpolation(CURVE_TIMES, CURVE_DISCOUNTS)
    maturity_discount = interpolation(discount_time)
    maturity_cashflow = NOTIONAL * (1.0 + QUOTE_RATE * accrual)

    def holder_value(rate_shift: float) -> float:
        return maturity_cashflow * maturity_discount * math.exp(-rate_shift * discount_time)

    formula_reason = (
        "QuantLib Actual/365F time, Actual/360 accrual, and LogLinearInterpolation "
        "on the committed synthetic discount-curve pillars."
    )
    return {
        "schema_version": SCHEMA_VERSION,
        "metadata": metadata(
            name="usd_deposit_3m",
            domain="rates.deposit",
            description="QuantLib-parity USD 3M money-market deposit on a committed discount curve.",
            product="deposit",
            source_detail=(
                f"QuantLib {ql.__version__}; Actual/365F log-linear discount interpolation, "
                "Actual/360 deposit accrual, and holder-view settlement-date flow exclusion."
            ),
        ),
        "kind": "pricing",
        "model": "discounting",
        "market": {
            "kind": "envelope",
            "envelope": {
                "schema": "finstack_quant.calibration",
                "plan": {
                    "id": "usd_deposit_3m",
                    "description": "Pre-built synthetic-pillar USD-OIS curve; no calibration steps required.",
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
                        "kind": "discount_curve",
                        "id": "USD-OIS",
                        "base": VALUATION_DATE,
                        "day_count": "Act365F",
                        "knot_points": list(zip(CURVE_TIMES, CURVE_DISCOUNTS, strict=True)),
                        "interp_style": "log_linear",
                        "extrapolation": "flat_forward",
                        "min_forward_rate": None,
                        "allow_non_monotonic": False,
                        "min_forward_tenor": 1e-6,
                        "rate_calibration": None,
                    }
                ],
            },
        },
        "instrument": {
            "schema": "finstack_quant.instrument/1",
            "instrument": {
                "type": "deposit",
                "spec": {
                    "id": "USD-DEP-3M-GOLDEN",
                    "notional": {"amount": "10000000", "currency": "USD"},
                    "start_date": VALUATION_DATE,
                    "maturity": "2026-07-30",
                    "day_count": "Act360",
                    "quote_rate": str(QUOTE_RATE),
                    "discount_curve_id": "USD-OIS",
                    "attributes": {"tags": ["golden", "quantlib"], "meta": {}},
                    "spot_lag_days": 0,
                    "calendar_id": None,
                },
            },
        },
        "expected": {
            "npv": holder_value(0.0),
            "deposit_par_rate": (1.0 / maturity_discount - 1.0) / accrual,
            "dv01": (holder_value(BUMP_BP) - holder_value(-BUMP_BP)) / 2.0,
        },
        "tolerances": {
            "npv": tolerance(1e-6, f"{formula_reason} NPV excludes the valuation-date initial flow."),
            "deposit_par_rate": tolerance(1e-12, formula_reason),
            "dv01": tolerance(1e-6, f"{formula_reason} Central 1bp continuous-rate bump."),
        },
    }
