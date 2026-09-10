"""The native host preserves gross carry and explicit financing detail."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from finstack_quant.attribution import attribute_pnl


def test_funded_bond_carry_matches_gross_endpoint_pnl() -> None:
    fixture = json.loads(
        (
            Path(__file__).resolve().parents[2]
            / "finstack-quant/attribution/tests/fixtures/production_gross_carry.json"
        ).read_text()
    )
    result = attribute_pnl(
        json.dumps(fixture["instrument"]),
        json.dumps(fixture["market"]),
        json.dumps(fixture["market"]),
        fixture["as_of_t0"],
        fixture["as_of_t1"],
        "metrics_based",
        {"metrics": ["theta", "carry_total", "coupon_income", "pull_to_par", "roll_down", "funding_cost"]},
    )
    payload = json.loads(result.to_json())
    detail = payload["carry_detail"]
    assert float(detail["funding_cost"]["amount"]) > 0
    assert abs(float(payload["carry"]["amount"]) - float(payload["total_pnl"]["amount"])) < 0.01
    assert abs(float(payload["residual"]["amount"])) < 0.01


@pytest.mark.parametrize("method", ["metrics_based", {"taylor": {}}])
def test_convertible_credit_move_is_not_counted_as_rates(method: str | dict) -> None:
    fixture = json.loads(
        (
            Path(__file__).resolve().parents[2]
            / "finstack-quant/attribution/tests/fixtures/production_convertible_credit.json"
        ).read_text()
    )
    result = attribute_pnl(
        json.dumps(fixture["instrument"]),
        json.dumps(fixture["market_t0"]),
        json.dumps(fixture["market_t1"]),
        fixture["as_of_t0"],
        fixture["as_of_t1"],
        method,
    )
    payload = json.loads(result.to_json())
    assert float(payload["rates_curves_pnl"]["amount"]) == 0.0
    credit = float(payload["credit_curves_pnl"]["amount"])
    assert credit < 0.0
    assert abs(credit) > 5.0 * abs(float(payload["residual"]["amount"]))
