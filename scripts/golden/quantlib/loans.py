"""Term loan golden builders."""

from __future__ import annotations

from collections.abc import Callable
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
    tolerance,
)

ISSUE_DATE = "2026-02-15"
MATURITY = "2029-02-15"
NOTIONAL = 10_000_000.0
COUPON = 0.06
AMORTIZATION = 0.025
CURVE_RATE = 0.04


def build_amortizing_term_loan() -> dict[str, Any]:
    """Build a seasoned fixed-rate declining-balance term loan fixture.

    Each quarter repays 2.5% of the balance then outstanding and pays Act/360
    interest on it; the rest is repaid at maturity. QuantLib builds the same
    flows as an amortising fixed-rate bond on unadjusted dates and discounts
    those after the valuation date.
    """
    evaluation_date = ql_date(VALUATION_DATE)
    ql.Settings.instance().evaluationDate = evaluation_date
    schedule = ql.Schedule(
        ql_date(ISSUE_DATE),
        ql_date(MATURITY),
        ql.Period(3, ql.Months),
        ql.NullCalendar(),
        ql.Unadjusted,
        ql.Unadjusted,
        ql.DateGeneration.Backward,
        False,
    )
    periods = len(schedule) - 1
    balances = [NOTIONAL * (1.0 - AMORTIZATION) ** period for period in range(periods)]
    loan = ql.AmortizingFixedRateBond(0, balances, schedule, [COUPON], ql.Actual360(), ql.Unadjusted)

    def npv(rate: float) -> float:
        curve = ql.YieldTermStructureHandle(ql.FlatForward(evaluation_date, rate, ql.Actual365Fixed(), ql.Continuous))
        return ql.CashFlows.npv(loan.cashflows(), curve, False, evaluation_date, evaluation_date)

    expected = {"dv01": central_difference(npv, CURVE_RATE), "npv": npv(CURVE_RATE)}
    reason = (
        "Matched schedule, balances, day count and curve: value within 0.0001 per 100 of the limit and DV01 "
        "within 0.1%."
    )
    name = "usd_fixed_amortizing_term_loan_3y_quantlib"
    return {
        "schema": SCHEMA,
        "metadata": metadata(
            name=name,
            domain="fixed_income.term_loan",
            description="Seasoned three-year fixed-rate term loan repaying 2.5% of the balance each quarter.",
            product="amortizing_term_loan",
            source_detail=(
                f"QuantLib {ql.__version__} AmortizingFixedRateBond on the declining balances, flows after the "
                "valuation date discounted on a flat continuous curve with CashFlows.npv; DV01 is a central "
                "difference of one basis point in the curve rate."
            ),
        ),
        "kind": "pricing",
        "model": "discounting",
        "market": market_snapshot([flat_discount_curve("USD-OIS", CURVE_RATE)]),
        "instrument": {
            "schema": "finstack_quant.instrument/1",
            "instrument": {
                "type": "term_loan",
                "spec": {
                    "id": name.upper().replace("_", "-"),
                    "currency": "USD",
                    "notional_limit": {"amount": str(int(NOTIONAL)), "currency": "USD"},
                    "issue_date": ISSUE_DATE,
                    "maturity": MATURITY,
                    "rate": {"fixed": {"rate": COUPON}},
                    "frequency": {"count": 3, "unit": "months"},
                    "quote_compounding": {"periodic": 4},
                    "day_count": "act_360",
                    "yield_basis": "act_360",
                    "business_day_convention": "unadjusted",
                    "calendar_id": None,
                    "stub": "none",
                    "discount_curve_id": "USD-OIS",
                    "credit_curve_id": None,
                    "amortization": {"percent_of_remaining_per_period": {"pct": AMORTIZATION}},
                    "coupon_type": "cash",
                    "upfront_fee": None,
                    "ddtl": None,
                    "covenants": None,
                    "oid_eir": None,
                    "call_schedule": None,
                    "settlement_days": 0,
                    "attributes": {"tags": ["golden", "quantlib"], "meta": {}},
                },
            },
        },
        "expected": expected,
        "tolerances": {
            "npv": tolerance(NOTIONAL * 1e-6, reason),
            "dv01": {"rel": 0.001, "tolerance_reason": reason},
        },
    }


LOAN_BUILDERS: dict[str, tuple[str, Callable[[], dict[str, Any]]]] = {
    "amortizing_term_loan": (
        "term_loan/usd_fixed_amortizing_term_loan_3y_quantlib.json",
        build_amortizing_term_loan,
    ),
}
