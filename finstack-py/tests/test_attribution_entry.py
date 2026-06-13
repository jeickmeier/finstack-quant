"""Behavioral tests for the attribution execution entry points.

The execute path (JSON in → spec → execute → JSON out) previously had no
behavioral coverage, which is how the bare-string method regression shipped
despite a notebook exercising it.
"""

from __future__ import annotations

from datetime import date
import json

from finstack.core.market_data import DiscountCurve, MarketContext
import pytest

from finstack.attribution import (
    PnlAttribution,
    attribute_pnl,
    validate_attribution_json,
)

AS_OF_T0 = "2025-01-15"
AS_OF_T1 = "2025-01-16"


def _bond_json() -> str:
    return json.dumps({
        "type": "bond",
        "spec": {
            "id": "ENTRY-TEST-BOND",
            "notional": {"amount": "1000000", "currency": "USD"},
            "issue_date": "2024-01-15",
            "maturity": "2029-01-15",
            "cashflow_spec": {
                "Fixed": {
                    "coupon_type": "Cash",
                    "rate": 0.05,
                    "freq": {"count": 6, "unit": "months"},
                    "dc": "Thirty360",
                    "bdc": "following",
                    "calendar_id": "weekends_only",
                    "stub": "None",
                    "end_of_month": False,
                    "payment_lag_days": 0,
                }
            },
            "discount_curve_id": "USD-OIS",
            "call_put": None,
            "attributes": {"tags": [], "meta": {}},
            "pricing_overrides": {},
        },
    })


def _market_json(as_of: str, shift: float = 0.0) -> str:
    base = date.fromisoformat(as_of)
    mc = MarketContext()
    knots = [
        (0.0, 1.0),
        (0.5, 0.980 - shift),
        (1.0, 0.960 - shift),
        (2.0, 0.920 - shift),
        (3.0, 0.880 - shift),
        (5.0, 0.800 - shift),
        (10.0, 0.650 - shift),
    ]
    mc.insert(DiscountCurve("USD-OIS", base, knots, day_count="act_365f"))
    return mc.to_json()


def test_attribute_pnl_accepts_bare_method_strings() -> None:
    """Regression (M11): the documented ``method="Parallel"`` form must work.

    ``py_to_json_value`` previously required the Python str to already be
    valid JSON, so the bare unit-variant names raised
    ``ValueError: invalid method JSON``.
    """
    out = attribute_pnl(
        _bond_json(),
        _market_json(AS_OF_T0),
        _market_json(AS_OF_T1, shift=0.002),
        AS_OF_T0,
        AS_OF_T1,
        "Parallel",
    )
    attr = PnlAttribution.from_json(out)
    assert attr.total_pnl != 0.0
    # Rates moved between T0 and T1; the parallel method must attribute it.
    assert attr.rates_curves_pnl != 0.0


def test_attribute_pnl_accepts_dict_method_forms() -> None:
    out = attribute_pnl(
        _bond_json(),
        _market_json(AS_OF_T0),
        _market_json(AS_OF_T1, shift=0.002),
        AS_OF_T0,
        AS_OF_T1,
        {"Waterfall": ["Carry", "RatesCurves"]},
    )
    attr = PnlAttribution.from_json(out)
    assert attr.rates_curves_pnl != 0.0


def test_attribute_pnl_missing_market_data_raises_key_error() -> None:
    """Regression (M12): operational failures must not surface as ValueError.

    A spec whose markets lack the instrument's discount curve is a routine
    production failure (bad/incomplete market snapshot) and must raise
    ``KeyError`` per the binding error taxonomy, so pipelines catching
    ``ValueError`` for malformed user input do not silently swallow it.
    """
    empty_market = MarketContext().to_json()
    with pytest.raises(KeyError):
        attribute_pnl(
            _bond_json(),
            empty_market,
            empty_market,
            AS_OF_T0,
            AS_OF_T1,
            "Parallel",
        )


def test_validate_attribution_json_rejects_wrong_schema() -> None:
    """Regression: validation applies the schema-version gate.

    The same gate execution applies — validation must not green-light
    payloads that execute would reject.
    """
    envelope = {
        "schema": "finstack.attribution/99",
        "spec": {
            "instrument": json.loads(_bond_json()),
            "market_t0": json.loads(_market_json(AS_OF_T0)),
            "market_t1": json.loads(_market_json(AS_OF_T1)),
            "as_of_t0": AS_OF_T0,
            "as_of_t1": AS_OF_T1,
            "method": "Parallel",
        },
    }
    with pytest.raises(ValueError, match="schema"):
        validate_attribution_json(json.dumps(envelope))


def test_empty_detail_dataframes_keep_schema_columns() -> None:
    """Regression: zero-row detail frames keep the column schema.

    Cross-instrument pipelines filter/aggregate the documented columns, so
    instruments without detail blocks must not produce column-less frames.
    """
    out = attribute_pnl(
        _bond_json(),
        _market_json(AS_OF_T0),
        _market_json(AS_OF_T1),
        AS_OF_T0,
        AS_OF_T1,
        "Parallel",
    )
    attr = PnlAttribution.from_json(out)
    expected_columns = ["kind", "factor", "key_a", "key_b", "amount", "currency"]
    for df in (
        attr.to_credit_factor_dataframe(),
        attr.to_carry_detail_dataframe(),
        attr.to_long_dataframe(),
    ):
        assert list(df.columns) == expected_columns or len(df) > 0, (
            f"empty detail frame must carry schema columns, got {list(df.columns)}"
        )
