"""Behavioral tests for the Arrow PyCapsule interchange exports.

Verifies that ``pyarrow.table(...)`` and ``polars.DataFrame(...)`` consume
finstack ``ArrowTable`` objects via ``__arrow_c_stream__`` and reproduce the
values of the existing pandas exports exactly, including null handling.
"""

from __future__ import annotations

from datetime import date
import json
import math

import polars as pl
import pyarrow as pa

from finstack_quant import statements
from finstack_quant.core.market_data import DiscountCurve, MarketContext
from finstack_quant.core.table import ArrowTable
from finstack_quant.portfolio import Portfolio, value_portfolio

AS_OF = "2025-01-15"


def _statement_result() -> object:
    b = statements.ModelBuilder("demo")
    b.periods("2025Q1..Q2", None)
    b.value("revenue", [("2025Q1", 100.0), ("2025Q2", 110.0)])
    b.value("ebitda", [("2025Q1", 27.0), ("2025Q2", 31.0)])
    return statements.Evaluator().evaluate(b.build())


def _portfolio_valuation() -> object:
    portfolio_json = json.dumps({
        "id": "ARROW-TEST",
        "as_of": AS_OF,
        "base_currency": "USD",
        "entities": {"FUND": {"id": "FUND"}},
        "positions": [
            {
                "position_id": "USD-POS",
                "entity_id": "FUND",
                "instrument_id": "USD-DEP",
                "instrument_spec": {
                    "type": "deposit",
                    "spec": {
                        "id": "USD-DEP",
                        "notional": {"amount": "1000000", "currency": "USD"},
                        "start_date": AS_OF,
                        "maturity": "2025-07-15",
                        "day_count": "act_360",
                        "fixed_rate": "0.04",
                        "discount_curve_id": "USD-OIS",
                        "attributes": {},
                    },
                },
                "quantity": 1.0,
                "unit": "units",
            }
        ],
    })
    market = MarketContext()
    market.insert(
        DiscountCurve(
            "USD-OIS",
            date.fromisoformat(AS_OF),
            [(0.0, 1.0), (0.5, 0.98), (1.0, 0.95)],
            day_count="act_365f",
        )
    )
    return value_portfolio(Portfolio.from_spec(portfolio_json), market)


def test_to_arrow_long_returns_arrow_table_with_stream_protocol() -> None:
    at = _statement_result().to_arrow_long()
    assert isinstance(at, ArrowTable)
    assert hasattr(at, "__arrow_c_stream__")
    assert at.num_rows == 4
    assert at.column_names() == [
        "node_id",
        "period_id",
        "value",
        "value_money",
        "currency",
        "value_type",
    ]


def test_pyarrow_values_match_pandas_export_exactly() -> None:
    res = _statement_result()
    tbl = pa.table(res.to_arrow_long())
    df = res.to_dataframe(orient="long")
    assert tbl.num_rows == len(df)
    assert tbl.column("node_id").to_pylist() == df["node_id"].tolist()
    assert tbl.column("period_id").to_pylist() == df["period"].tolist()
    # Exact float equality: values pass through bit-identically.
    assert tbl.column("value").to_pylist() == df["value"].tolist()
    assert tbl.column("value_type").to_pylist() == df["value_type"].tolist()


def test_null_handling_for_scalar_nodes() -> None:
    # Scalar (non-Money) nodes leave value_money/currency null in the long table.
    tbl = pa.table(_statement_result().to_arrow_long())
    assert tbl.column("value_money").null_count == tbl.num_rows
    assert tbl.column("currency").null_count == tbl.num_rows
    assert all(v is None for v in tbl.column("value_money").to_pylist())


def test_polars_roundtrip_matches_values_exactly() -> None:
    res = _statement_result()
    df_pl = pl.DataFrame(res.to_arrow_long())
    df_pd = res.to_dataframe(orient="long")
    assert df_pl.get_column("value").to_list() == df_pd["value"].tolist()
    assert df_pl.get_column("node_id").to_list() == df_pd["node_id"].tolist()
    assert df_pl.get_column("value_money").null_count() == df_pl.height


def test_arrow_table_is_reusable_across_consumers() -> None:
    at = _statement_result().to_arrow_long()
    first = pa.table(at)
    second = pa.table(at)  # fresh stream per __arrow_c_stream__ call
    assert first.equals(second)


def test_field_metadata_carries_roles() -> None:
    tbl = pa.table(_statement_result().to_arrow_long())
    md = tbl.schema.field("node_id").metadata or {}
    assert md.get(b"finstack:role") == b"dimension"


def test_to_arrow_wide_shape() -> None:
    res = _statement_result()
    tbl = pa.table(res.to_arrow_wide())
    assert tbl.column_names[0] == "period_id"
    assert set(tbl.column_names) == {"period_id", "revenue", "ebitda"}
    assert tbl.column("period_id").to_pylist() == ["2025Q1", "2025Q2"]
    assert tbl.column("revenue").to_pylist() == [100.0, 110.0]


def test_portfolio_positions_to_arrow() -> None:
    val = _portfolio_valuation()
    tbl = pa.table(val.to_arrow_positions())
    assert tbl.num_rows == len(val)
    assert tbl.column("position_id").to_pylist() == ["USD-POS"]
    assert tbl.column("entity_id").to_pylist() == ["FUND"]
    assert tbl.column("currency_base").to_pylist() == ["USD"]
    value_base = tbl.column("value_base").to_pylist()[0]
    assert math.isfinite(value_base)
    assert value_base > 0.0
    # Values agree exactly with polars consumption of the same export.
    df_pl = pl.DataFrame(val.to_arrow_positions())
    assert df_pl.get_column("value_base").to_list() == [value_base]
