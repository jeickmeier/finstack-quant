"""Structural tests for the `to_arrow_*` producer methods added in task 6.

Covers `StatementResult.to_arrow_long` / `to_arrow_wide` and
`PortfolioValuation.to_arrow_positions`. These assert the returned object is
an `ArrowTable` exposing the same columns/row-count as the corresponding
`to_table_*` / `positions_to_table` Rust producers (and their existing
`to_pandas_*` bindings). Behavioral pyarrow/polars round-trip coverage lands
in task 7.
"""

from __future__ import annotations

from datetime import date
import json

from finstack_quant import statements
from finstack_quant.core.market_data import DiscountCurve, MarketContext
from finstack_quant.core.table import ArrowTable
from finstack_quant.portfolio import Portfolio, value_portfolio

AS_OF = date(2025, 1, 15)


def _statement_result() -> object:
    b = statements.ModelBuilder("demo")
    b.periods("2025Q1..Q2", None)
    b.value("revenue", [("2025Q1", 100.0), ("2025Q2", 110.0)])
    b.value("ebitda", [("2025Q1", 27.0), ("2025Q2", 31.0)])
    return statements.Evaluator().evaluate(b.build())


def test_to_arrow_long_matches_pandas_long_columns() -> None:
    res = _statement_result()
    at = res.to_arrow_long()

    assert isinstance(at, ArrowTable)
    assert at.column_names() == [
        "node_id",
        "period_id",
        "value",
        "value_money",
        "currency",
        "value_type",
    ]

    df = res.to_dataframe(orient="long")
    assert at.num_rows == len(df)


def test_to_arrow_wide_matches_pandas_wide_shape() -> None:
    res = _statement_result()
    at = res.to_arrow_wide()

    assert isinstance(at, ArrowTable)
    assert set(at.column_names()) == {"period_id", "revenue", "ebitda"}
    assert at.num_rows == 2  # two periods: 2025Q1, 2025Q2


def _portfolio_spec_json() -> str:
    return json.dumps({
        "id": "arrow-test",
        "as_of": AS_OF.isoformat(),
        "base_currency": "USD",
        "entities": {"FUND": {"id": "FUND"}},
        "positions": [
            {
                "position_id": "POS-0",
                "entity_id": "FUND",
                "instrument_id": "DEP-0",
                "instrument_spec": {
                    "type": "deposit",
                    "spec": {
                        "id": "DEP-0",
                        "notional": {"amount": "1000000", "currency": "USD"},
                        "start_date": AS_OF.isoformat(),
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


def _market() -> MarketContext:
    knots = [(0.0, 1.0), (0.5, 0.98), (1.0, 0.96), (2.0, 0.92)]
    return MarketContext().insert(DiscountCurve("USD-OIS", AS_OF, knots))


def test_to_arrow_positions_matches_positions_to_table_columns() -> None:
    portfolio = Portfolio.from_spec(_portfolio_spec_json())
    valuation = value_portfolio(portfolio, _market())

    at = valuation.to_arrow_positions()

    assert isinstance(at, ArrowTable)
    assert at.column_names() == [
        "position_id",
        "entity_id",
        "value_native",
        "value_base",
        "currency_native",
        "currency_base",
    ]
    assert at.num_rows == len(valuation)
