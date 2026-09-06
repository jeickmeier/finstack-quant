"""Typed result-return contract for the portfolio attribution/performance surface.

Every computation entry point returns a typed ``Py*`` wrapper; the exact JSON
string the old API returned is still available from the paired ``<name>_json``
wire surface. Each wrapper carries typed getters, ``to_json``, a static
``from_json``, and ``to_dataframe``.
"""

from __future__ import annotations

from datetime import date
import json
from typing import Any

import pytest

from finstack_quant.core.market_data import DiscountCurve, MarketContext
import finstack_quant.portfolio as pf

AS_OF = "2025-01-15"


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


def _brinson_sectors() -> str:
    return json.dumps([
        {
            "sector": "TECH",
            "portfolio_weight": 0.60,
            "benchmark_weight": 0.40,
            "portfolio_return": 0.08,
            "benchmark_return": 0.06,
        },
        {
            "sector": "ENERGY",
            "portfolio_weight": 0.40,
            "benchmark_weight": 0.60,
            "portfolio_return": 0.01,
            "benchmark_return": 0.03,
        },
    ])


def _campisi_snapshot(sector: str, weight: float, r: float) -> dict[str, Any]:
    return {
        "sector": sector,
        "weight": weight,
        "total_return": r,
        "yield_annual": 0.05,
        "modified_duration": 5.0,
        "spread_duration": 4.0,
        "spread": 0.01,
        "delta_treasury_yield": 0.001,
        "delta_spread": -0.0005,
    }


_CAMPISI_PORTFOLIO = json.dumps([_campisi_snapshot("CORP", 0.5, 0.02), _campisi_snapshot("GOVT", 0.5, 0.01)])
_CAMPISI_BENCHMARK = json.dumps([_campisi_snapshot("CORP", 0.4, 0.015), _campisi_snapshot("GOVT", 0.6, 0.012)])
_CAMPISI_CONFIG = json.dumps({"period_years": 0.25})


def _reference_table_json() -> str:
    reference = [
        {"duration": 0.5, "total_return": 0.01},
        {"duration": 1.5, "total_return": 0.02},
        {"duration": 2.5, "total_return": 0.03},
    ]
    return pf.cell_returns_from_reference_json(json.dumps(reference), "UST", json.dumps({"width": 1.0}))


_GRID_PORTFOLIO = json.dumps([
    {"cell": "0-3", "sector": "GOVT", "weight": 0.5, "total_return": 0.02},
    {"cell": "3-7", "sector": "CORP", "weight": 0.5, "total_return": 0.03},
])
_GRID_BENCHMARK = json.dumps([
    {"cell": "0-3", "sector": "GOVT", "weight": 0.6, "total_return": 0.01},
    {"cell": "3-7", "sector": "CORP", "weight": 0.4, "total_return": 0.02},
])

_FACTOR_BRINSON_INPUT = json.dumps({
    "asset_ids": ["A"],
    "asset_returns": [0.02],
    "exposures": [1.0],
    "factor_names": ["Market"],
    "portfolio_weights": [1.0],
    "benchmark_weights": [1.0],
})


def _portfolio_json() -> str:
    return json.dumps({
        "id": "TYPED-RESULTS",
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
                        "quote_rate": "0.04",
                        "discount_curve_id": "USD-OIS",
                        "attributes": {},
                    },
                },
                "quantity": 1.0,
                "unit": "units",
            }
        ],
    })


def _market(as_of: str = AS_OF, bump: float = 0.0) -> MarketContext:
    market = MarketContext()
    market.insert(
        DiscountCurve(
            "USD-OIS",
            date.fromisoformat(as_of),
            [(0.0, 1.0), (0.5, 0.98 - bump), (1.0, 0.95 - bump)],
            day_count="act_365f",
        )
    )
    return market


def _scenario_batch_json() -> str:
    from finstack_quant.scenarios import ScenarioSpec

    scenarios = []
    for scenario_id, bp in (("up_10bp", 10.0), ("down_15bp", -15.0)):
        operations = [
            {
                "kind": "curve_parallel_bp",
                "curve_kind": "discount",
                "curve_id": "USD-OIS",
                "discount_curve_id": None,
                "bp": bp,
            }
        ]
        spec = ScenarioSpec.from_json(json.dumps({"id": scenario_id, "operations": operations}))
        scenarios.append(json.loads(spec.to_json()))
    return json.dumps(scenarios)


_ALLOCATION_SPEC = json.dumps({
    "scheme": "inverse_volatility",
    "total_capital": 1_000_000.0,
    "strategies": [
        {"id": "S1", "fixed_weight": None, "returns": [0.01, -0.02, 0.015, 0.005], "risk_budget": None},
        {"id": "S2", "fixed_weight": None, "returns": [0.002, -0.001, 0.003, 0.001], "risk_budget": None},
    ],
    "covariance": None,
})


def _approx_equal(a: Any, b: Any) -> bool:
    """Structural equality with float tolerance.

    The workspace's ``serde_json`` does not enable ``float_roundtrip``, so a
    JSON parse can land one ULP away from the serialized double; exact
    equality on re-serialized documents is therefore too strict.
    """
    if isinstance(a, float) and isinstance(b, float):
        return a == pytest.approx(b, rel=1e-12, abs=1e-15)
    if isinstance(a, dict) and isinstance(b, dict):
        return a.keys() == b.keys() and all(_approx_equal(a[k], b[k]) for k in a)
    if isinstance(a, list) and isinstance(b, list):
        return len(a) == len(b) and all(_approx_equal(x, y) for x, y in zip(a, b, strict=True))
    return bool(a == b)


def _assert_contract(wrapper: Any, cls: type) -> None:
    """The mandatory accessor set on every result wrapper."""
    assert isinstance(wrapper, cls)
    assert isinstance(wrapper.to_json(), str)
    restored = cls.from_json(wrapper.to_json())
    assert _approx_equal(json.loads(restored.to_json()), json.loads(wrapper.to_json()))
    frame = wrapper.to_dataframe()
    assert hasattr(frame, "columns")


def _frame_columns(wrapper: Any) -> list[str]:
    return list(wrapper.to_dataframe().columns)


# ---------------------------------------------------------------------------
# Brinson
# ---------------------------------------------------------------------------


def test_brinson_fachler_returns_typed_result() -> None:
    result = pf.brinson_fachler(_brinson_sectors())
    expected = json.loads(pf.brinson_fachler_json(_brinson_sectors()))

    _assert_contract(result, pf.BrinsonPeriodResult)
    assert result.total_allocation == pytest.approx(expected["total_allocation"])
    assert result.total_excess_return == pytest.approx(expected["total_excess_return"])
    assert [s["sector"] for s in result.sectors] == ["TECH", "ENERGY"]
    assert _frame_columns(result) == ["sector", "allocation", "selection", "interaction", "total"]
    assert json.loads(result.to_json()) == expected


def test_carino_link_returns_typed_result() -> None:
    periods = json.dumps([json.loads(_brinson_sectors()), json.loads(_brinson_sectors())])
    result = pf.carino_link(periods)
    expected = json.loads(pf.carino_link_json(periods))

    _assert_contract(result, pf.CarinoLinkedAttribution)
    assert result.portfolio_return_compounded == pytest.approx(expected["portfolio_return_compounded"])
    assert result.linked_allocation == pytest.approx(expected["linked_allocation"])
    assert len(result.periods) == 2
    assert _frame_columns(result) == ["sector", "allocation", "selection", "interaction", "total"]


# ---------------------------------------------------------------------------
# Campisi
# ---------------------------------------------------------------------------


def test_campisi_attribution_returns_typed_result() -> None:
    result = pf.campisi_attribution(_CAMPISI_PORTFOLIO, _CAMPISI_BENCHMARK, _CAMPISI_CONFIG)
    expected = json.loads(pf.campisi_attribution_json(_CAMPISI_PORTFOLIO, _CAMPISI_BENCHMARK, _CAMPISI_CONFIG))

    _assert_contract(result, pf.FiAttributionResult)
    assert result.active_return == pytest.approx(expected["active_return"])
    assert result.total_allocation == pytest.approx(expected["total_allocation"])
    assert result.portfolio_components["total"] == pytest.approx(expected["portfolio_components"]["total"])
    assert _frame_columns(result) == [
        "sector",
        "portfolio_weight",
        "benchmark_weight",
        "portfolio_return",
        "benchmark_return",
        "allocation",
        "active_carry",
        "active_treasury",
        "active_spread",
        "selection",
        "total_active",
    ]


def test_campisi_carino_link_returns_typed_result() -> None:
    period = pf.campisi_attribution_json(_CAMPISI_PORTFOLIO, _CAMPISI_BENCHMARK, _CAMPISI_CONFIG)
    periods = json.dumps([json.loads(period), json.loads(period)])
    result = pf.campisi_carino_link(periods)
    expected = json.loads(pf.campisi_carino_link_json(periods))

    _assert_contract(result, pf.FiCarinoLinkedResult)
    assert result.linked_allocation == pytest.approx(expected["linked_allocation"])
    assert result.portfolio_return_compounded == pytest.approx(expected["portfolio_return_compounded"])
    assert _frame_columns(result) == [
        "sector",
        "allocation",
        "active_carry",
        "active_treasury",
        "active_spread",
        "selection",
        "total_active",
    ]


def test_campisi_carino_link_from_snapshots_returns_typed_result() -> None:
    periods = json.dumps([
        {"portfolio": json.loads(_CAMPISI_PORTFOLIO), "benchmark": json.loads(_CAMPISI_BENCHMARK)},
        {"portfolio": json.loads(_CAMPISI_PORTFOLIO), "benchmark": json.loads(_CAMPISI_BENCHMARK)},
    ])
    result = pf.campisi_carino_link_from_snapshots(periods, _CAMPISI_CONFIG)
    expected = json.loads(pf.campisi_carino_link_from_snapshots_json(periods, _CAMPISI_CONFIG))

    _assert_contract(result, pf.FiCarinoLinkedResult)
    assert result.linked_selection == pytest.approx(expected["linked_selection"])


def test_campisi_reconciliation_check_returns_typed_report() -> None:
    period = pf.campisi_attribution_json(_CAMPISI_PORTFOLIO, _CAMPISI_BENCHMARK, _CAMPISI_CONFIG)
    report = pf.campisi_reconciliation_check(period, 1e-10)
    expected = json.loads(pf.campisi_reconciliation_check_json(period, 1e-10))

    _assert_contract(report, pf.FiReconciliationReport)
    assert report.is_reconciled is expected["is_reconciled"]
    assert report.total_residual == pytest.approx(expected["total_residual"], abs=1e-15)
    assert report.tolerance == pytest.approx(1e-10)
    assert _frame_columns(report) == ["total_residual", "is_reconciled", "tolerance"]


# ---------------------------------------------------------------------------
# Excess returns
# ---------------------------------------------------------------------------


def test_cell_returns_from_reference_returns_typed_table() -> None:
    reference = json.dumps([{"duration": 1.0, "total_return": 0.02}])
    table = pf.cell_returns_from_reference(reference, "UST", json.dumps({"width": 2.0}))
    expected = json.loads(pf.cell_returns_from_reference_json(reference, "UST", json.dumps({"width": 2.0})))

    _assert_contract(table, pf.DurationCellTable)
    assert table.base_label == "UST"
    assert table.cells[0]["base_return"] == pytest.approx(expected["cells"][0]["base_return"])
    assert _frame_columns(table) == ["label", "lower", "upper", "base_return", "observed"]


def test_cell_returns_from_curves_returns_typed_table() -> None:
    start = DiscountCurve.flat("start", date(2025, 1, 1), 0.02)
    end = DiscountCurve.flat("end", date(2025, 4, 1), 0.03)
    config = json.dumps({"width": 1.0})

    table = pf.cell_returns_from_curves(start, end, 0.25, 2.0, "UST", config)
    expected = json.loads(pf.cell_returns_from_curves_json(start, end, 0.25, 2.0, "UST", config))

    _assert_contract(table, pf.DurationCellTable)
    assert len(table.cells) == len(expected["cells"]) == 2


def test_excess_returns_returns_typed_result() -> None:
    table_json = _reference_table_json()
    positions = json.dumps([
        {"id": "B1", "weight": 0.5, "duration": 0.5, "total_return": 0.03},
        {"id": "B2", "weight": 0.5, "duration": 1.5, "total_return": 0.025},
    ])

    result = pf.excess_returns(positions, table_json)
    expected = json.loads(pf.excess_returns_json(positions, table_json))

    _assert_contract(result, pf.ExcessReturnResult)
    assert result.portfolio_excess_return == pytest.approx(expected["portfolio_excess_return"])
    assert [p["id"] for p in result.positions] == ["B1", "B2"]
    assert _frame_columns(result) == ["id", "cell", "base_return", "excess_return"]


# ---------------------------------------------------------------------------
# Grid attribution
# ---------------------------------------------------------------------------


def test_grid_attribution_returns_typed_result() -> None:
    result = pf.grid_attribution(_GRID_PORTFOLIO, _GRID_BENCHMARK)
    expected = json.loads(pf.grid_attribution_json(_GRID_PORTFOLIO, _GRID_BENCHMARK))

    _assert_contract(result, pf.GridAttributionResult)
    assert result.active_return == pytest.approx(expected["active_return"])
    assert result.total_selection == pytest.approx(expected["total_selection"])
    assert _frame_columns(result) == [
        "cell",
        "portfolio_weight",
        "benchmark_weight",
        "benchmark_cell_return",
        "curve_effect",
    ]
    assert list(result.to_sector_effects_dataframe().columns) == ["cell", "sector", "allocation_effect"]
    assert list(result.to_selection_effects_dataframe().columns) == ["cell", "sector", "selection_effect"]


def test_grid_carino_link_returns_typed_result() -> None:
    period = pf.grid_attribution_json(_GRID_PORTFOLIO, _GRID_BENCHMARK)
    periods = json.dumps([json.loads(period), json.loads(period)])

    result = pf.grid_carino_link(periods)
    expected = json.loads(pf.grid_carino_link_json(periods))

    _assert_contract(result, pf.GridCarinoLinkedResult)
    assert result.linked_selection == pytest.approx(expected["linked_selection"])
    assert len(result.periods) == 2
    assert _frame_columns(result) == [
        "portfolio_return_compounded",
        "benchmark_return_compounded",
        "linked_curve",
        "linked_sector",
        "linked_selection",
    ]


# ---------------------------------------------------------------------------
# Factor Brinson
# ---------------------------------------------------------------------------


def test_factor_brinson_attribution_returns_typed_result() -> None:
    result = pf.factor_brinson_attribution(_FACTOR_BRINSON_INPUT, [0.02])
    expected = json.loads(pf.factor_brinson_attribution_json(_FACTOR_BRINSON_INPUT, [0.02]))

    _assert_contract(result, pf.FactorBrinsonResult)
    assert result.active_return == pytest.approx(expected["active_return"])
    assert result.allocation == pytest.approx(expected["allocation"])
    assert _frame_columns(result) == ["factor", "active_loading", "factor_return", "contribution"]
    assert list(result.to_asset_contributions_dataframe().columns) == [
        "asset",
        "specific_return",
        "active_weight",
        "contribution",
    ]


# ---------------------------------------------------------------------------
# Performance
# ---------------------------------------------------------------------------


def test_twrr_linked_returns_typed_result() -> None:
    returns = json.dumps([0.05, 0.03])
    result = pf.twrr_linked(returns, 1.0)
    expected = json.loads(pf.twrr_linked_json(returns, 1.0))

    _assert_contract(result, pf.LinkedReturn)
    assert result.cumulative == pytest.approx(expected["cumulative"])
    assert result.annualised == pytest.approx(expected["annualised"])
    assert result.num_periods == 2
    assert _frame_columns(result) == ["cumulative", "annualised", "num_periods"]


# ---------------------------------------------------------------------------
# Metrics aggregation
# ---------------------------------------------------------------------------


def test_aggregate_metrics_returns_typed_portfolio_metrics() -> None:
    portfolio = pf.Portfolio.from_spec(_portfolio_json())
    market = _market()
    valuation = pf.value_portfolio(portfolio, market)

    metrics = pf.aggregate_metrics(valuation, "USD", market, AS_OF)
    expected = json.loads(pf.aggregate_metrics_json(valuation, "USD", market, AS_OF))

    assert isinstance(metrics, pf.PortfolioMetrics)
    assert json.loads(metrics.to_json()) == expected
    restored = pf.PortfolioMetrics.from_json(metrics.to_json())
    assert json.loads(restored.to_json()) == expected
    assert hasattr(metrics.to_dataframe(), "columns")


# ---------------------------------------------------------------------------
# Replay
# ---------------------------------------------------------------------------


def test_replay_portfolio_returns_typed_result() -> None:
    portfolio = pf.Portfolio.from_spec(_portfolio_json())
    snapshots = json.dumps([
        {"date": "2025-01-15", "market": json.loads(_market("2025-01-15").to_json())},
        {"date": "2025-01-16", "market": json.loads(_market("2025-01-16", bump=0.001).to_json())},
    ])
    config = json.dumps({"mode": "pv_only"})

    result = pf.replay_portfolio(portfolio, snapshots, config)
    expected = json.loads(pf.replay_portfolio_json(portfolio, snapshots, config))

    _assert_contract(result, pf.ReplayResult)
    assert result.summary["num_steps"] == expected["summary"]["num_steps"] == 2
    assert len(result.steps) == 2
    assert _frame_columns(result) == ["date", "value", "daily_mtm_pnl", "cumulative_mtm_pnl"]


# ---------------------------------------------------------------------------
# Allocation
# ---------------------------------------------------------------------------


def test_allocate_weights_returns_typed_result() -> None:
    result = pf.allocate_weights(_ALLOCATION_SPEC)
    expected = json.loads(pf.allocate_weights_json(_ALLOCATION_SPEC))

    _assert_contract(result, pf.WeightAllocationResult)
    assert json.loads(result.to_json()) == expected
    assert result.scheme == expected["scheme"]
    assert [row["id"] for row in result.allocations] == ["S1", "S2"]
    assert result.diagnostics["weights_sum"] == pytest.approx(expected["diagnostics"]["weights_sum"])
    assert _frame_columns(result) == ["id", "weight", "capital", "volatility", "risk_contribution"]


def test_allocation_rejects_unsupported_target_volatility() -> None:
    spec = json.loads(_ALLOCATION_SPEC)
    spec["target_volatility"] = None
    with pytest.raises(ValueError, match="target_volatility"):
        pf.allocate_weights(json.dumps(spec))


# ---------------------------------------------------------------------------
# Scenario P&L batch
# ---------------------------------------------------------------------------


def test_scenario_pnl_batch_returns_typed_items() -> None:
    portfolio = pf.Portfolio.from_spec(_portfolio_json())
    market = _market()
    scenarios_json = _scenario_batch_json()

    batch = pf.scenario_pnl_batch(portfolio, scenarios_json, market)
    expected = json.loads(pf.scenario_pnl_batch_json(portfolio, scenarios_json, market))

    assert isinstance(batch, list)
    assert [item.scenario_id for item in batch] == ["up_10bp", "down_15bp"]
    for item, expected_item in zip(batch, expected, strict=True):
        _assert_contract(item, pf.ScenarioPnlBatchItem)
        assert isinstance(item.pnl, pf.ScenarioPnl)
        assert item.pnl.total == pytest.approx(float(expected_item["pnl"]["total"]["amount"]))
        assert item.report.operations_applied == expected_item["report"]["operations_applied"]
        assert list(item.to_dataframe().columns) == ["scenario_id", "position_id", "pnl"]
    assert pf.scenario_pnl_batch(portfolio, "[]", market) == []


def test_json_twins_return_wire_strings() -> None:
    """Every ``_json`` twin returns a parseable JSON string."""
    assert isinstance(json.loads(pf.brinson_fachler_json(_brinson_sectors())), dict)
    assert isinstance(json.loads(pf.twrr_linked_json(json.dumps([0.01]), 0.0)), dict)
    assert isinstance(json.loads(pf.allocate_weights_json(_ALLOCATION_SPEC)), dict)
