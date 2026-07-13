"""Cashflow JSON bridge tests."""

from __future__ import annotations

import json
import math

import pytest

from finstack_quant.cashflows import (
    accrued_interest_json,
    build_cashflow_schedule_json,
    dated_flows_json,
    validate_cashflow_schedule_json,
)
from finstack_quant.valuations.instruments import bond_from_cashflows_json, price_instrument


def _cashflow_spec() -> str:
    return json.dumps({
        "notional": {
            "initial": {"amount": "1000000", "currency": "USD"},
            "amort": "None",
        },
        "issue": "2024-08-31",
        "maturity": "2025-08-31",
        "coupon_program": [
            {
                "kind": "fixed",
                "spec": {
                    "coupon_type": "Cash",
                    "rate": "0.06",
                    "freq": {"count": 12, "unit": "months"},
                    "dc": "Thirty360",
                    "bdc": "following",
                    "calendar_id": "weekends_only",
                    "stub": "None",
                    "end_of_month": False,
                    "payment_lag_days": 0,
                },
            }
        ],
    })


def _market_json() -> str:
    return json.dumps({
        "version": 2,
        "curves": [
            {
                "type": "discount",
                "id": "USD-OIS",
                "base": "2024-01-01",
                "day_count": "Act365F",
                "knot_points": [[0.0, 1.0], [1.0, 0.95], [5.0, 0.80]],
                "interp_style": "linear",
                "extrapolation": "flat_forward",
                "min_forward_rate": None,
                "allow_non_monotonic": False,
                "min_forward_tenor": 1e-6,
            }
        ],
        "fx": None,
        "surfaces": [],
        "prices": {},
        "series": [],
        "inflation_indices": [],
        "dividends": [],
        "credit_indices": [],
        "fx_delta_vol_surfaces": [],
        "vol_cubes": [],
        "collateral": {},
    })


def _floating_cashflow_spec() -> str:
    return json.dumps({
        "notional": {
            "initial": {"amount": "1000000", "currency": "USD"},
            "amort": "None",
        },
        "issue": "2025-01-15",
        "maturity": "2026-01-15",
        "coupon_program": [
            {
                "kind": "floating",
                "spec": {
                    "rate_spec": {
                        "index_id": "USD-SOFR-3M",
                        "spread_bp": "150.0",
                        "gearing": "1.0",
                        "gearing_includes_spread": True,
                        "index_floor_bp": None,
                        "all_in_floor_bp": None,
                        "all_in_cap_bp": None,
                        "index_cap_bp": None,
                        "reset_freq": {"count": 3, "unit": "months"},
                        "reset_lag_days": 0,
                        "fixing_calendar_id": None,
                        "overnight_compounding": None,
                        "overnight_basis": None,
                    },
                    "coupon_type": "Cash",
                    "freq": {"count": 3, "unit": "months"},
                    "dc": "Act360",
                    "bdc": "following",
                    "calendar_id": "weekends_only",
                    "stub": "None",
                    "end_of_month": False,
                    "payment_lag_days": 0,
                },
            }
        ],
    })


def _floating_market_json() -> str:
    market = json.loads(_market_json())
    market["curves"].append({
        "type": "forward",
        "id": "USD-SOFR-3M",
        "base": "2025-01-15",
        "reset_lag": 0,
        "day_count": "Act360",
        "tenor": 0.25,
        "knot_points": [[0.0, 0.03], [1.0, 0.04], [5.0, 0.05]],
        "interp_style": "linear",
        "extrapolation": "flat_forward",
    })
    return json.dumps(market)


def test_cashflows_namespace_build_validate_accrual_and_price_bond() -> None:
    schedule_json = build_cashflow_schedule_json(_cashflow_spec())
    schedule = json.loads(schedule_json)
    assert schedule["meta"]["issue_date"] == "2024-08-31"

    assert json.loads(validate_cashflow_schedule_json(schedule_json)) == schedule
    flows = json.loads(dated_flows_json(schedule_json))
    assert len(flows) == len(schedule["flows"])
    assert accrued_interest_json(schedule_json, "2025-02-28") > 0.0

    instrument_json = bond_from_cashflows_json("CUSTOM-CF", schedule_json, "USD-OIS", 99.0)
    instrument = json.loads(instrument_json)
    assert instrument["type"] == "bond"

    result = json.loads(price_instrument(instrument_json, _market_json(), "2024-09-03", "discounting"))
    assert result["instrument_id"] == "CUSTOM-CF"
    assert result["value"]["currency"] == "USD"


def test_cashflows_builds_floating_schedule_with_market_json() -> None:
    schedule_json = build_cashflow_schedule_json(_floating_cashflow_spec(), _floating_market_json())
    schedule = json.loads(schedule_json)

    float_flows = [flow for flow in schedule["flows"] if flow["kind"] == "FloatReset"]
    assert float_flows
    assert all(flow["rate"] > 0.015 for flow in float_flows)


def test_cashflows_builds_step_up_with_payment_program() -> None:
    spec = json.loads(_cashflow_spec())
    spec["issue"] = "2024-01-01"
    spec["maturity"] = "2026-01-01"
    fixed = spec["coupon_program"][0]["spec"]
    initial_rate = fixed.pop("rate")
    spec["coupon_program"] = [
        {
            "kind": "step_up",
            "spec": {
                **fixed,
                "initial_rate": initial_rate,
                "step_schedule": [["2025-01-01", "0.07"]],
            },
        }
    ]
    spec["payment_program"] = [
        {
            "kind": "program",
            "steps": [
                {"date": "2025-01-01", "split": "PIK"},
                {"date": "2026-01-01", "split": "Cash"},
            ],
        }
    ]

    schedule = json.loads(build_cashflow_schedule_json(json.dumps(spec)))
    assert any(flow["kind"] == "PIK" for flow in schedule["flows"])


def test_cashflows_accrued_interest_accepts_config_json() -> None:
    schedule_json = build_cashflow_schedule_json(_cashflow_spec())

    config_json = json.dumps({
        "method": "Linear",
        "include_pik": True,
        "frequency": {"count": 12, "unit": "months"},
    })

    assert accrued_interest_json(schedule_json, "2025-02-28", config_json) > 0.0


def test_cashflows_accrued_interest_rejects_unknown_config_json_fields() -> None:
    schedule_json = build_cashflow_schedule_json(_cashflow_spec())
    config_json = json.dumps({
        "method": "Linear",
        "include_pik": True,
        "strict_issue_date": True,
    })

    with pytest.raises(ValueError, match="invalid accrual config JSON"):
        accrued_interest_json(schedule_json, "2025-02-28", config_json)


def test_cashflows_bond_from_cashflows_allows_missing_quoted_clean() -> None:
    schedule_json = build_cashflow_schedule_json(_cashflow_spec())
    instrument_json = bond_from_cashflows_json("CUSTOM-CF-NO-QUOTE", schedule_json, "USD-OIS")
    instrument = json.loads(instrument_json)

    assert instrument["type"] == "bond"
    assert instrument["spec"]["id"] == "CUSTOM-CF-NO-QUOTE"


def test_cashflows_reject_malformed_json_and_invalid_dates() -> None:
    schedule_json = build_cashflow_schedule_json(_cashflow_spec())

    # Validation/parse failures map to ValueError per the binding error contract
    # (finstack-quant-py/src/errors.rs `core_to_py`).
    with pytest.raises(ValueError, match="invalid cashflow schedule JSON"):
        validate_cashflow_schedule_json("{not json")

    with pytest.raises(ValueError, match="invalid cashflow schedule JSON"):
        validate_cashflow_schedule_json(
            json.dumps({
                "schema_version": "finstack_quant.cashflows.schedule/1",
                "schedule": json.loads(schedule_json),
            })
        )

    with pytest.raises(ValueError, match="invalid ISO date"):
        accrued_interest_json(schedule_json, "2025-02-30")


def test_cashflows_reject_malformed_market_json() -> None:
    with pytest.raises(ValueError, match="invalid market context JSON"):
        build_cashflow_schedule_json(_floating_cashflow_spec(), "{not json")


def test_cashflows_missing_forward_curve_raises_key_error() -> None:
    # Missing-id lookups (forward curve not in market) map to KeyError per
    # the binding error contract (finstack-quant-py/src/errors.rs `core_to_py`).
    with pytest.raises(KeyError, match="USD-SOFR-3M"):
        build_cashflow_schedule_json(_floating_cashflow_spec(), _market_json())


def test_cashflows_build_is_deterministic_and_accrual_is_finite() -> None:
    # Cross-language determinism fixture: the same spec is exercised in
    # finstack-quant-wasm/tests/facade/cashflows.test.mjs so the surfaces are comparable.
    first = build_cashflow_schedule_json(_cashflow_spec())
    second = build_cashflow_schedule_json(_cashflow_spec())
    assert first == second  # byte-identical canonical JSON

    accrued = accrued_interest_json(first, "2025-02-28")
    assert isinstance(accrued, float)
    assert math.isfinite(accrued)


def test_cashflows_reject_amortization_over_notional() -> None:
    schedule = json.loads(build_cashflow_schedule_json(_cashflow_spec()))
    schedule["flows"].append({
        "date": "2025-03-31",
        "reset_date": None,
        "amount": {"amount": "1000011", "currency": "USD"},
        "kind": "Amortization",
        "accrual_factor": 0.0,
        "rate": None,
    })

    with pytest.raises(ValueError, match="total amortization"):
        validate_cashflow_schedule_json(json.dumps(schedule))
