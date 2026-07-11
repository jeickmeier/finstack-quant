"""Native QuantLib rate-instrument golden builders."""

from __future__ import annotations

import math
from typing import Any

import QuantLib as ql  # type: ignore[import-not-found]  # noqa: N813

from .common import (
    SCHEMA_VERSION,
    VALUATION_DATE,
    central_difference,
    flat_discount_curve,
    flat_forward_curve,
    market_snapshot,
    metadata,
    ql_date,
    tolerance,
)


def build_fra() -> dict[str, Any]:
    """Build a native QuantLib USD 3x6 FRA golden fixture."""
    evaluation_date = ql_date(VALUATION_DATE)
    start_date = ql.Date(3, 8, 2026)
    maturity_date = ql.Date(3, 11, 2026)
    notional = 10_000_000.0
    fixed_rate = 0.0425
    discount_rate = 0.04
    forward_rate = 0.043
    day_count = ql.Actual360()
    calendar = ql.UnitedStates(ql.UnitedStates.SOFR)
    ql.Settings.instance().evaluationDate = evaluation_date
    ql.IndexManager.instance().clearHistories()

    def forward_handle(simple_rate: float) -> ql.YieldTermStructureHandle:
        accrual = day_count.yearFraction(start_date, maturity_date)
        curve_time = ql.Actual365Fixed().yearFraction(start_date, maturity_date)
        continuous_rate = math.log1p(simple_rate * accrual) / curve_time
        return ql.YieldTermStructureHandle(
            ql.FlatForward(evaluation_date, continuous_rate, ql.Actual365Fixed(), ql.Continuous)
        )

    def npv(shift: float) -> float:
        discount_curve = ql.YieldTermStructureHandle(
            ql.FlatForward(evaluation_date, discount_rate + shift, ql.Actual365Fixed(), ql.Continuous)
        )
        forward_curve = forward_handle(forward_rate + shift)
        index = ql.IborIndex(
            "USD-Term-SOFR-3M",
            ql.Period(3, ql.Months),
            2,
            ql.USDCurrency(),
            calendar,
            ql.ModifiedFollowing,
            False,
            day_count,
            forward_curve,
        )
        fra = ql.ForwardRateAgreement(
            index,
            start_date,
            ql.Position.Short,
            fixed_rate,
            notional,
            discount_curve,
        )
        return fra.NPV()

    forward_curve = forward_handle(forward_rate)
    index = ql.IborIndex(
        "USD-Term-SOFR-3M",
        ql.Period(3, ql.Months),
        2,
        ql.USDCurrency(),
        calendar,
        ql.ModifiedFollowing,
        False,
        day_count,
        forward_curve,
    )
    par_rate = index.fixing(index.fixingDate(start_date))
    expected = {
        "dv01": central_difference(npv, 0.0),
        "npv": npv(0.0),
        "par_rate": par_rate,
    }
    reason = "QuantLib native FRA benchmark; tolerance allows only cross-engine floating-point residual."
    return {
        "schema_version": SCHEMA_VERSION,
        "metadata": metadata(
            name="usd_fra_3x6_quantlib",
            domain="rates.fra",
            description="QuantLib native USD 3x6 term-SOFR-style FRA on deterministic flat curves.",
            product="fra",
        ),
        "kind": "pricing",
        "model": "discounting",
        "market": market_snapshot([
            flat_discount_curve("USD-OIS", discount_rate),
            flat_forward_curve("USD-SOFR-3M", forward_rate),
        ]),
        "instrument": {
            "schema": "finstack_quant.instrument/1",
            "instrument": {
                "type": "forward_rate_agreement",
                "spec": {
                    "id": "USD-FRA-3X6-QUANTLIB",
                    "notional": {"amount": "10000000", "currency": "USD"},
                    "fixing_date": "2026-07-30",
                    "start_date": "2026-08-03",
                    "maturity": "2026-11-03",
                    "fixed_rate": "0.0425",
                    "day_count": "Act360",
                    "reset_lag": 2,
                    "fixing_calendar_id": "usny",
                    "fixing_bdc": "modified_following",
                    "discount_curve_id": "USD-OIS",
                    "forward_curve_id": "USD-SOFR-3M",
                    "side": "receive",
                    "attributes": {"tags": ["golden", "quantlib"], "meta": {}},
                },
            },
        },
        "expected": expected,
        "tolerances": {metric: tolerance(1e-8, reason) for metric in expected},
    }


def _term_sofr_index(curve: ql.YieldTermStructureHandle) -> ql.IborIndex:
    """Build the custom term-SOFR-style index used by rate goldens."""
    return ql.IborIndex(
        "USD-Term-SOFR-3M",
        ql.Period(3, ql.Months),
        2,
        ql.USDCurrency(),
        ql.UnitedStates(ql.UnitedStates.SOFR),
        ql.ModifiedFollowing,
        False,
        ql.Actual360(),
        curve,
    )


def build_irs() -> dict[str, Any]:
    """Build a native QuantLib five-year receive-fixed USD IRS fixture."""
    evaluation_date = ql_date(VALUATION_DATE)
    start_date = ql.Date(4, 5, 2026)
    end_date = ql.Date(4, 5, 2031)
    notional = 10_000_000.0
    fixed_rate = 0.0425
    discount_rate = 0.04
    projection_rate = 0.043
    calendar = ql.UnitedStates(ql.UnitedStates.SOFR)
    ql.Settings.instance().evaluationDate = evaluation_date
    ql.IndexManager.instance().clearHistories()

    fixed_schedule = ql.Schedule(
        start_date,
        end_date,
        ql.Period(6, ql.Months),
        calendar,
        ql.ModifiedFollowing,
        ql.ModifiedFollowing,
        ql.DateGeneration.Backward,
        False,
    )
    float_schedule = ql.Schedule(
        start_date,
        end_date,
        ql.Period(3, ql.Months),
        calendar,
        ql.ModifiedFollowing,
        ql.ModifiedFollowing,
        ql.DateGeneration.Backward,
        False,
    )

    def priced_swap(shift: float) -> ql.VanillaSwap:
        discount_curve = ql.YieldTermStructureHandle(
            ql.FlatForward(evaluation_date, discount_rate + shift, ql.Actual365Fixed(), ql.Continuous)
        )
        projection_curve = ql.YieldTermStructureHandle(
            ql.FlatForward(evaluation_date, projection_rate + shift, ql.Actual365Fixed(), ql.Continuous)
        )
        swap = ql.VanillaSwap(
            ql.VanillaSwap.Receiver,
            notional,
            fixed_schedule,
            fixed_rate,
            ql.Thirty360(ql.Thirty360.BondBasis),
            float_schedule,
            _term_sofr_index(projection_curve),
            0.0,
            ql.Actual360(),
            ql.ModifiedFollowing,
        )
        swap.setPricingEngine(ql.DiscountingSwapEngine(discount_curve))
        return swap

    base_swap = priced_swap(0.0)
    expected = {
        "dv01": central_difference(lambda shift: priced_swap(shift).NPV(), 0.0),
        "npv": base_swap.NPV(),
        "par_rate": base_swap.fairRate(),
    }
    reason = "QuantLib native VanillaSwap benchmark; tolerance allows only cross-engine floating-point residual."
    return {
        "schema_version": SCHEMA_VERSION,
        "metadata": metadata(
            name="usd_sofr_5y_quantlib",
            domain="rates.irs",
            description="QuantLib native five-year USD receive-fixed term-SOFR-style swap.",
            product="irs",
        ),
        "kind": "pricing",
        "model": "discounting",
        "market": market_snapshot([
            flat_discount_curve("USD-OIS", discount_rate),
            flat_forward_curve("USD-SOFR-3M", projection_rate),
        ]),
        "instrument": {
            "schema": "finstack_quant.instrument/1",
            "instrument": {
                "type": "interest_rate_swap",
                "spec": {
                    "id": "USD-SOFR-5Y-QUANTLIB",
                    "notional": {"amount": "10000000", "currency": "USD"},
                    "side": "receive",
                    "fixed": {
                        "discount_curve_id": "USD-OIS",
                        "rate": "0.0425",
                        "frequency": {"count": 6, "unit": "months"},
                        "day_count": "Thirty360",
                        "calendar_id": "usny",
                        "stub": "None",
                        "start": "2026-05-04",
                        "end": "2031-05-04",
                        "par_method": None,
                        "payment_lag_days": 0,
                        "compounding_simple": True,
                    },
                    "float": {
                        "discount_curve_id": "USD-OIS",
                        "forward_curve_id": "USD-SOFR-3M",
                        "spread_bp": "0",
                        "frequency": {"count": 3, "unit": "months"},
                        "day_count": "Act360",
                        "calendar_id": "usny",
                        "stub": "None",
                        "reset_lag_days": 2,
                        "payment_lag_days": 0,
                        "fixing_calendar_id": "usny",
                        "start": "2026-05-04",
                        "end": "2031-05-04",
                        "compounding": "Simple",
                    },
                    "attributes": {"tags": ["golden", "quantlib"], "meta": {}},
                },
            },
        },
        "expected": expected,
        "tolerances": {metric: tolerance(1e-8, reason) for metric in expected},
    }


def build_sofr_future() -> dict[str, Any]:
    """Build a native SOFR future-rate-helper quote fixture."""
    ql.Settings.instance().evaluationDate = ql_date(VALUATION_DATE)
    quoted_price = 95.7
    helper = ql.SofrFutureRateHelper(quoted_price, ql.June, 2026, ql.Quarterly, 0.0)
    if helper.quote().value() != quoted_price:
        raise RuntimeError("QuantLib SOFR future helper did not retain its input quote")
    expected = {
        "convexity_adjustment": 0.0,
        "dv01": -250.0,
        "futures_price": quoted_price,
        "implied_forward": (100.0 - quoted_price) / 100.0,
        "npv": 0.0,
    }
    zero_reason = "At-market QuantLib SofrFutureRateHelper quote with explicit zero convexity adjustment."
    return {
        "schema_version": SCHEMA_VERSION,
        "metadata": metadata(
            name="sofr_3m_quarterly_quantlib",
            domain="rates.ir_future",
            description="QuantLib native quarterly SOFR future helper with CME SR3 tick economics.",
            product="sofr_future",
        ),
        "kind": "pricing",
        "model": "discounting",
        "market": market_snapshot([flat_discount_curve("USD-OIS", 0.04), flat_forward_curve("USD-SOFR-3M", 0.043)]),
        "instrument": {
            "schema": "finstack_quant.instrument/1",
            "instrument": {
                "type": "interest_rate_future",
                "spec": {
                    "id": "SOFR-3M-QUARTERLY-QUANTLIB",
                    "notional": {"amount": "10000000", "currency": "USD"},
                    "expiry": "2026-06-17",
                    "fixing_date": "2026-06-17",
                    "period_start": "2026-06-19",
                    "period_end": "2026-09-18",
                    "quoted_price": quoted_price,
                    "day_count": "Act360",
                    "position": "long",
                    "contract_specs": {
                        "face_value": 1000000.0,
                        "tick_size": 0.0025,
                        "tick_value": 6.25,
                        "delivery_months": 3,
                        "convexity_adjustment": 0.0,
                    },
                    "discount_curve_id": "USD-OIS",
                    "forward_curve_id": "USD-SOFR-3M",
                    "vol_surface_id": None,
                    "attributes": {"tags": ["golden", "quantlib"], "meta": {}},
                },
            },
        },
        "expected": expected,
        "tolerances": {
            "convexity_adjustment": tolerance(1e-12, zero_reason),
            "dv01": tolerance(1e-9),
            "futures_price": tolerance(1e-12),
            "implied_forward": tolerance(1e-12),
            "npv": tolerance(1e-9),
        },
    }
