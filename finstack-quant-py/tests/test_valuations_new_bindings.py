"""Smoke tests for valuation-owned cashflow and coupon-profile bindings.

Covers:
- B4: `instrument_cashflows_json` and the `instrument_cashflows`
      DataFrame helper.
- Product-specific coupon-profile entry points.
"""

from __future__ import annotations

from datetime import date
import json

import pytest

from finstack_quant.core.market_data import DiscountCurve, ForwardCurve, HazardCurve, MarketContext
from finstack_quant.valuations import (
    instrument_cashflows,
    inverse_floater_coupon_profile,
    snowball_coupon_profile,
)
from finstack_quant.valuations.instruments import (
    instrument_cashflows_json,
    price_instrument,
    validate_instrument_json,
)


def _instrument_json(instrument: dict[str, object]) -> str:
    return json.dumps({"schema": "finstack_quant.instrument/1", "instrument": instrument})


# B4 — instrument_cashflows


def _build_deposit_market() -> tuple[str, MarketContext]:
    inst_json = _instrument_json({
        "type": "deposit",
        "spec": {
            "id": "DEP-B4",
            "notional": {"amount": "1000000", "currency": "USD"},
            "start_date": "2025-01-15",
            "maturity": "2025-06-15",
            "day_count": "act_360",
            "quote_rate": "0.05",
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
    pr = price_instrument(inst_json, market.to_json(), "2025-01-15", model="discounting")
    price = float(pr.price)
    assert abs(envelope["total_pv"] - price) < 0.01

    # DataFrame pv sum matches the envelope total.
    pv_series = df["pv"]
    pv_sum = float(pv_series.sum())  # type: ignore[arg-type]
    assert abs(pv_sum - envelope["total_pv"]) < 1e-6


def test_instrument_cashflows_unsupported_model_raises() -> None:
    inst_json, market = _build_deposit_market()
    with pytest.raises(ValueError, match=r"monte_carlo_gbm|supported|not priced"):
        instrument_cashflows_json(inst_json, market, "2025-01-15", "monte_carlo_gbm")


def _revolving_credit_json(*, gearing: str | None = None, credit_curve: bool = False) -> str:
    return _instrument_json({
        "type": "revolving_credit",
        "spec": {
            "id": "RC-PY-BINDING",
            "commitment_amount": {"amount": "50000000", "currency": "USD"},
            "drawn_amount": {"amount": "10000000", "currency": "USD"},
            "commitment_date": "2024-01-01",
            "maturity": "2027-01-01",
            "base_rate_spec": (
                {"fixed": {"rate": 0.05}}
                if gearing is None
                else {
                    "floating": {
                        "index_id": "USD-SOFR-3M",
                        "spread_bp": "250",
                        "gearing": gearing,
                        "gearing_includes_spread": True,
                        "floor_bp": "0",
                        "all_in_floor_bp": None,
                        "cap_bp": None,
                        "index_cap_bp": None,
                        "fixing_calendar_id": None,
                        "reset_frequency": {"count": 3, "unit": "months"},
                        "reset_lag_days": 2,
                    }
                }
            ),
            "day_count": "act_360",
            "frequency": {"count": 3, "unit": "months"},
            "fees": {
                "commitment_fee_tiers": [{"threshold": "0", "bp": "25"}],
                "usage_fee_tiers": [{"threshold": "0", "bp": "10"}],
                "facility_fee_bp": 5.0,
            },
            "draw_repay_spec": {
                "deterministic": [
                    {
                        "date": "2024-06-01",
                        "amount": {"amount": "5000000", "currency": "USD"},
                        "is_draw": True,
                    }
                ]
            },
            "discount_curve_id": "USD-OIS",
            "credit_curve_id": "USD-HZ" if credit_curve else None,
            "recovery_rate": 0.4 if credit_curve else 0.0,
            "stub": "short_front",
            "attributes": {"tags": [], "meta": {}},
        },
    })


def _revolving_credit_market(*, credit_curve: bool = False) -> MarketContext:
    market = MarketContext()
    market.insert(
        DiscountCurve(
            "USD-OIS",
            date(2024, 1, 1),
            [(0.0, 1.0), (1.0, 0.97), (5.0, 0.85)],
            day_count="act_365f",
        )
    )
    market.insert(
        ForwardCurve(
            "USD-SOFR-3M",
            0.25,
            date(2024, 1, 1),
            [(0.0, 0.03), (5.0, 0.03)],
            day_count="act_360",
        )
    )
    if credit_curve:
        market.insert(HazardCurve("USD-HZ", date(2024, 1, 1), [(1.0, 0.02), (5.0, 0.02)], recovery_rate=0.4))
    return market


def test_revolving_credit_binding_validates_floating_rate_spec() -> None:
    with pytest.raises(ValueError, match="gearing"):
        validate_instrument_json(_revolving_credit_json(gearing="0"))


def test_revolving_credit_custom_metrics_use_as_of_balance() -> None:
    # ``drawn_amount`` (10M of 50M) is the balance at the valuation date; the
    # fixture's 5M draw on 2024-06-01 lies in the future of 2024-03-01 and
    # must not enter the as-of metrics.
    result = price_instrument(
        _revolving_credit_json(),
        _revolving_credit_market().to_json(),
        "2024-03-01",
        "discounting",
        ["utilization_rate", "available_capacity"],
    )
    assert result.get_metric("utilization_rate") == pytest.approx(0.20)
    assert result.get_metric("available_capacity") == pytest.approx(40_000_000.0)


def test_revolving_credit_rejects_events_on_or_before_the_valuation_date() -> None:
    with pytest.raises(ValueError, match="simulation anchor"):
        price_instrument(
            _revolving_credit_json(),
            _revolving_credit_market().to_json(),
            "2024-07-01",
            "discounting",
            ["utilization_rate"],
        )


def test_revolving_credit_credit_cashflows_fail_closed() -> None:
    with pytest.raises(ValueError, match="model-specific cashflow decomposition"):
        instrument_cashflows_json(
            _revolving_credit_json(credit_curve=True),
            _revolving_credit_market(credit_curve=True),
            "2024-01-01",
            "discounting",
        )


def test_coupon_profile_entrypoints_have_distinct_explicit_inputs() -> None:
    assert snowball_coupon_profile(0.02, 0.05, [0.01, 0.04], 0.0, 0.10) == pytest.approx([0.06, 0.07])
    assert inverse_floater_coupon_profile(0.05, [0.01, 0.02], 0.0, 0.10, 2.0) == pytest.approx([0.03, 0.01])
