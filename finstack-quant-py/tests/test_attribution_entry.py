"""Behavioral tests for the attribution execution entry points.

The execute path (JSON in → spec → execute → JSON out) previously had no
behavioral coverage, which is how the bare-string method regression shipped
despite a notebook exercising it.
"""

from __future__ import annotations

from datetime import date
import json

import pytest

from finstack_quant.attribution import (
    PnlAttribution,
    ReturnContributionResult,
    attribute_pnl,
    attribute_pnl_many,
    attribute_return_contribution,
    default_waterfall_order,
    pnl_bridge,
    validate_attribution_json,
    validate_return_contribution_json,
)
from finstack_quant.core.market_data import DiscountCurve, MarketContext

AS_OF_T0 = "2025-01-15"
AS_OF_T1 = "2025-01-16"


def _bond_json() -> str:
    return json.dumps({
        "schema": "finstack_quant.instrument/1",
        "instrument": {
            "type": "bond",
            "spec": {
                "id": "ENTRY-TEST-BOND",
                "notional": {"amount": "1000000", "currency": "USD"},
                "issue_date": "2024-01-15",
                "maturity": "2029-01-15",
                "cashflow_spec": {
                    "fixed": {
                        "coupon_type": "cash",
                        "rate": "0.05",
                        "frequency": {"count": 6, "unit": "months"},
                        "day_count": "30_360",
                        "business_day_convention": "following",
                        "calendar_id": "weekends_only",
                        "stub": "none",
                        "end_of_month": False,
                        "payment_lag_days": 0,
                    }
                },
                "discount_curve_id": "USD-OIS",
                "call_put": None,
                "attributes": {"tags": [], "meta": {}},
            },
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
    """The documented canonical ``method="parallel"`` form must work.

    ``py_to_json_value`` previously required the Python str to already be
    valid JSON, so the bare unit-variant names raised
    ``ValueError: invalid method JSON``.
    """
    attr = attribute_pnl(
        _bond_json(),
        _market_json(AS_OF_T0),
        _market_json(AS_OF_T1, shift=0.002),
        AS_OF_T0,
        AS_OF_T1,
        "parallel",
    )
    assert isinstance(attr, PnlAttribution)
    assert attr.method == "parallel"
    assert attr.total_pnl != 0.0
    # Rates moved between T0 and T1; the parallel method must attribute it.
    assert attr.rates_curves_pnl != 0.0


def test_attribution_rejects_legacy_pascal_case_method() -> None:
    with pytest.raises(ValueError, match="unknown variant"):
        attribute_pnl(
            _bond_json(),
            _market_json(AS_OF_T0),
            _market_json(AS_OF_T1),
            AS_OF_T0,
            AS_OF_T1,
            "Parallel",
        )


def test_default_waterfall_order_uses_canonical_factor_names() -> None:
    assert default_waterfall_order() == [
        "carry",
        "rates_curves",
        "credit_curves",
        "inflation_curves",
        "correlations",
        "fx",
        "volatility",
        "model_parameters",
        "market_scalars",
    ]


def test_attribute_pnl_accepts_dict_method_forms() -> None:
    attr = attribute_pnl(
        _bond_json(),
        _market_json(AS_OF_T0),
        _market_json(AS_OF_T1, shift=0.002),
        AS_OF_T0,
        AS_OF_T1,
        {"waterfall": ["carry", "rates_curves"]},
    )
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
            "parallel",
        )


def test_validate_attribution_json_rejects_wrong_schema() -> None:
    """Regression: validation applies the schema-version gate.

    The same gate execution applies — validation must not green-light
    payloads that execute would reject.
    """
    envelope = {
        "schema": "finstack_quant.attribution/99",
        "spec": {
            "instrument": json.loads(_bond_json()),
            "market_t0": json.loads(_market_json(AS_OF_T0)),
            "market_t1": json.loads(_market_json(AS_OF_T1)),
            "as_of_t0": AS_OF_T0,
            "as_of_t1": AS_OF_T1,
            "method": "parallel",
        },
    }
    with pytest.raises(ValueError, match=r"finstack_quant\.attribution/99"):
        validate_attribution_json(json.dumps(envelope))


def test_empty_detail_dataframes_keep_schema_columns() -> None:
    """Regression: zero-row detail frames keep the column schema.

    Cross-instrument pipelines filter/aggregate the documented columns, so
    instruments without detail blocks must not produce column-less frames.
    """
    attr = attribute_pnl(
        _bond_json(),
        _market_json(AS_OF_T0),
        _market_json(AS_OF_T1),
        AS_OF_T0,
        AS_OF_T1,
        "parallel",
    )
    expected_columns = ["kind", "factor", "sub", "key_a", "key_b", "amount", "currency"]
    for df in (
        attr.to_credit_factor_dataframe(),
        attr.to_carry_detail_dataframe(),
        attr.to_long_dataframe(),
    ):
        assert list(df.columns) == expected_columns or len(df) > 0, (
            f"empty detail frame must carry schema columns, got {list(df.columns)}"
        )


def test_attribute_pnl_typed_inputs_many_and_bridge() -> None:
    """Typed MarketContext / date inputs, the batch table and the scalar bridge."""
    market_t0 = MarketContext.from_json(_market_json(AS_OF_T0))
    market_t1 = MarketContext.from_json(_market_json(AS_OF_T1, shift=0.002))
    attr = attribute_pnl(
        _bond_json(),
        market_t0,
        market_t1,
        date.fromisoformat(AS_OF_T0),
        date.fromisoformat(AS_OF_T1),
        "parallel",
    )
    assert attr.t0 == date.fromisoformat(AS_OF_T0)
    assert attr.required_metrics() == []
    assert "sub" in attr.to_long_dataframe().columns

    table = attribute_pnl_many([_bond_json(), _bond_json()], market_t0, market_t1, AS_OF_T0, AS_OF_T1, "parallel")
    assert len(table) == 2
    assert table["total_pnl"].iloc[0] == pytest.approx(attr.total_pnl)

    bridge = pnl_bridge(_bond_json(), market_t0, market_t1, AS_OF_T0, AS_OF_T1, "USD")
    assert bridge.currency.code == "USD"
    assert attr.mark_to_market_pnl == pytest.approx(bridge.amount, rel=1e-9)


def test_attribute_return_contribution_json_entrypoint() -> None:
    spec = {
        "as_of": "2026-01-02",
        "weighting": "gross",
        "positions": [
            {
                "id": "AAPL.XNAS",
                "market_value": 9000.0,
                "return": 0.012,
                "groups": {"sector": "tech", "strategy": "value:1"},
                "benchmark_weight": 0.85,
                "benchmark_return": 0.010,
            },
            {
                "id": "XOM.XNYS",
                "market_value": 1000.0,
                "return": -0.004,
                "groups": {"sector": "energy"},
                "benchmark_weight": 0.15,
                "benchmark_return": -0.002,
            },
        ],
        "factors": [{"factor": "value", "exposure": 0.10, "factor_return": 0.02}],
    }

    validate_return_contribution_json(json.dumps(spec))
    result = attribute_return_contribution(json.dumps(spec))
    assert attribute_return_contribution(spec).portfolio_return == pytest.approx(result.portfolio_return)
    assert list(result.to_dataframe().columns) == ["id", "weight", "return", "contribution", "active_contribution"]
    assert set(result.to_group_dataframe()["dimension"]) == {"sector", "strategy"}
    assert list(result.to_factor_dataframe()["factor"]) == ["value"]

    assert isinstance(result, ReturnContributionResult)
    assert result.portfolio_return == pytest.approx(0.0104)
    assert result.instrument_contribution[0]["id"] == "AAPL.XNAS"
    assert any(row["key"] == "unknown" for row in result.group_contribution["strategy"])
    relative = result.benchmark_relative
    assert relative is not None
    assert relative["residual"] == pytest.approx(0.0, abs=1e-12)


def test_requested_reporting_currency_requires_fx() -> None:
    with pytest.raises(KeyError, match="fx"):
        attribute_pnl(
            _bond_json(),
            _market_json(AS_OF_T0),
            _market_json(AS_OF_T1),
            AS_OF_T0,
            AS_OF_T1,
            "parallel",
            config={"target_currency": "EUR"},
        )


@pytest.mark.parametrize("method", ["metrics_based", {"taylor": {}}])
def test_spec_methods_preserve_configured_rounding(method: object) -> None:
    attr = attribute_pnl(
        _bond_json(),
        _market_json(AS_OF_T0),
        _market_json(AS_OF_T1),
        AS_OF_T0,
        AS_OF_T1,
        method,
        config={"rounding_scale": 4, "metrics": ["dv01"]},
    )
    assert json.loads(attr.to_json())["meta"]["rounding"]["output_scale_by_currency"]["USD"] == 4


def test_brinson_rejects_offsetting_group() -> None:
    spec = {
        "as_of": AS_OF_T0,
        "positions": [
            {
                "id": "L",
                "weight": 0.5,
                "return": 0.1,
                "groups": {"sector": "tech"},
                "benchmark_weight": 0.5,
                "benchmark_return": 0.0,
            },
            {
                "id": "S",
                "weight": -0.5,
                "return": 0.0,
                "groups": {"sector": "tech"},
                "benchmark_weight": 0.5,
                "benchmark_return": 0.0,
            },
            {
                "id": "C",
                "weight": 1.0,
                "return": 0.0,
                "groups": {"sector": "cash"},
                "benchmark_weight": 0.0,
                "benchmark_return": 0.0,
            },
        ],
    }
    with pytest.raises(ValueError, match="zero net portfolio weight"):
        attribute_return_contribution(spec)
