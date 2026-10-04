"""Acceptance and table contracts after the valuations simplicity changes."""

import json

import pandas as pd
import pytest

from finstack_quant.valuations.instruments import AdvanceRate, TrancheCashflows


def test_advance_rate_reconstruction_uses_construction_validation() -> None:
    rate = AdvanceRate("*", 0.8)
    reconstruct, args = rate.__reduce__()
    assert reconstruct(*args).rate == 0.8
    payload = json.loads(rate.to_json())
    payload["rate"] = 1.1
    with pytest.raises(ValueError, match="advance rate"):
        AdvanceRate("*", 1.1)
    with pytest.raises(ValueError, match="advance rate"):
        AdvanceRate.from_json(json.dumps(payload))


def empty_tranche() -> dict:
    zero = {"amount": "0", "currency": "USD"}
    return {
        "tranche_id": "EMPTY",
        "cashflows": [],
        "detailed_flows": [],
        "accrual_periods": [],
        "interest_flows": [],
        "principal_flows": [],
        "pik_flows": [],
        "deferred_flows": [],
        "writedown_flows": [],
        "final_balance": zero,
        "total_interest": zero,
        "total_principal": zero,
        "total_pik": zero,
        "total_deferred": zero,
        "total_writedown": zero,
    }


def test_empty_tranche_table_keeps_columns_and_numeric_dtypes() -> None:
    frame = TrancheCashflows.from_json(json.dumps(empty_tranche())).to_dataframe()
    assert frame.empty
    assert list(frame.columns) == ["date", "cashflow", "interest", "principal", "pik", "deferred", "writedown"]
    assert pd.api.types.is_string_dtype(frame["date"].dtype)
    assert all(pd.api.types.is_float_dtype(frame[name].dtype) for name in frame.columns[1:])


def test_tranche_table_rejects_mixed_currency_component() -> None:
    payload = empty_tranche()
    payload["interest_flows"] = [["2025-01-02", {"amount": "1", "currency": "EUR"}]]
    cashflows = TrancheCashflows.from_json(json.dumps(payload))
    with pytest.raises(ValueError, match=r"currency|Currency"):
        cashflows.to_dataframe()
