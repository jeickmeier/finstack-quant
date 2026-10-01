"""Coupon entitlement and accrued interest share the canonical record boundary."""

import datetime as dt
import json

import pytest

from finstack_quant.cashflows import accrued_interest, build_cashflow_schedule_json
from finstack_quant.cashflows.accrual import ExCouponRule


def test_record_date_retains_coupon_until_next_settlement() -> None:
    rule = ExCouponRule(7, "gblo")
    payment = dt.date(2025, 7, 1)
    record = dt.date(2025, 6, 20)
    assert rule.ex_date(payment) == record
    assert not rule.is_ex_coupon(payment, record)
    assert rule.is_ex_coupon(payment, dt.date(2025, 6, 23))
    assert not rule.is_ex_coupon(payment, payment)


def test_json_accrual_uses_settlement_after_record_date() -> None:
    schedule = json.dumps({
        "flows": [
            {
                "date": "2025-07-01",
                "amount": {"amount": "25000", "currency": "GBP"},
                "kind": "fixed",
                "accrual_factor": 0.5,
                "rate": 0.05,
                "accrual": {
                    "start": "2025-01-01",
                    "end": "2025-07-01",
                    "day_count": "act_act_isma",
                    "coupon_period": ["2025-01-01", "2025-07-01"],
                    "end_is_termination_date": False,
                },
            }
        ],
        "notional": {
            "initial": {"amount": "1000000", "currency": "GBP"},
            "amort": "none",
        },
        "day_count": "act_act_isma",
        "meta": {"issue_date": "2025-01-01", "calendar_ids": []},
    })
    config = json.dumps({
        "method": "linear",
        "ex_coupon": {"days_before_coupon": 7, "calendar_id": "gblo"},
        "include_pik": True,
        "frequency": {"count": 6, "unit": "months"},
    })
    assert accrued_interest(schedule, "2025-06-20", config) == pytest.approx(25_000 * 170 / 181)
    assert accrued_interest(schedule, "2025-06-23", config) < 0


def test_ex_coupon_rebates_future_principal_segments() -> None:
    schedule = build_cashflow_schedule_json(
        json.dumps({
            "notional": {"initial": {"amount": "100", "currency": "USD"}, "amort": "none"},
            "issue_date": "2025-01-01",
            "maturity": "2025-07-01",
            "coupon_program": [
                {
                    "kind": "fixed",
                    "spec": {
                        "coupon_type": "cash",
                        "rate": "0.12",
                        "frequency": {"count": 6, "unit": "months"},
                        "day_count": "act_360",
                        "business_day_convention": "unadjusted",
                        "calendar_id": "weekends_only",
                        "stub": "short_back",
                    },
                }
            ],
            "principal_events": [
                {
                    "date": "2025-06-28",
                    "payment_date": "2025-06-28",
                    "kind": "notional",
                    "delta": {"amount": "100", "currency": "USD"},
                }
            ],
        })
    )
    config = json.dumps({
        "method": "linear",
        "include_pik": True,
        "ex_coupon": {"days_before_coupon": 7, "calendar_id": None},
    })
    assert accrued_interest(schedule, "2025-06-25", config) == pytest.approx(-0.3)
    assert accrued_interest(schedule, "2025-06-29", config) == pytest.approx(-0.12 * 200 * 2 / 360)
