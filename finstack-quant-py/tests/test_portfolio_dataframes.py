"""Tests for the portfolio-domain ``to_*_dataframe`` accessors.

Every portfolio result type that owns a row collection (or is a flat set of
scalars) gets a pandas exit. These tests pin, for each new frame: the return
type, the documented columns, and the row count against the underlying
collection.

Fixtures are JSON payloads patched inline, or objects built through public
constructors and pipeline functions, so the tests stay self-contained and do
not depend on notebook fixtures or on-disk golden data.
"""

from __future__ import annotations

from datetime import date
import json

import pandas as pd
import pytest

from finstack_quant.core.market_data import DiscountCurve, FxMatrix, MarketContext
from finstack_quant.models.factor.credit import CreditFactorModel
from finstack_quant.models.factor.risk import (
    PositionBudgetEntry,
    PositionRiskDecomposition,
    PositionVarContribution,
    RiskBudgetResult,
    RiskDecomposition,
    StressAttribution,
    TailScenarioBreakdown,
    build_stress_attribution,
)
from finstack_quant.portfolio import (
    Constraint,
    FactorAssignmentReport,
    MetricExpr,
    MissingMetricPolicy,
    Objective,
    PerPositionMetric,
    Portfolio,
    PortfolioAttribution,
    PortfolioMetrics,
    PortfolioOptimizationResult,
    PortfolioOptimizationSpec,
    PositionFilter,
    StressResult,
    TradeSpec,
    WeightingScheme,
    WhatIfResult,
    aggregate_full_cashflows,
    attribute_portfolio_pnl,
    build_credit_vol_report,
    optimize_portfolio,
    value_portfolio,
)

AS_OF = "2025-01-15"
EMPTY_PORTFOLIO = '{"id":"empty","base_currency":"USD","as_of":"2025-01-01","entities":{},"positions":[]}'


# Shared fixtures


def _deposit_position(
    position_id: str,
    instrument_id: str,
    currency: str,
    curve_id: str,
    notional: str = "1000000",
) -> dict[str, object]:
    return {
        "position_id": position_id,
        "entity_id": "FUND",
        "instrument_id": instrument_id,
        "instrument_spec": {
            "type": "deposit",
            "spec": {
                "id": instrument_id,
                "notional": {"amount": notional, "currency": currency},
                "start_date": AS_OF,
                "maturity": "2025-07-15",
                "day_count": "act_360",
                "fixed_rate": "0.04",
                "discount_curve_id": curve_id,
                "attributes": {},
            },
        },
        "quantity": 1.0,
        "unit": "units",
    }


def _portfolio_json() -> str:
    """Single-deposit portfolio priced off the ``USD-OIS`` discount curve."""
    return json.dumps({
        "id": "DF-EXITS",
        "as_of": AS_OF,
        "base_currency": "USD",
        "entities": {"FUND": {"id": "FUND"}},
        "positions": [_deposit_position("USD-POS", "USD-DEP", "USD", "USD-OIS")],
    })


def _two_position_portfolio_json() -> str:
    """Two positions with different notionals, so weight maps cannot collide.

    A one-position portfolio makes every weight map ``{'USD-POS': ...}``; a
    frame that overwrote each cell with the same wrong number would still line
    up. Two differently-sized positions give every cell a distinct value.
    """
    return json.dumps({
        "id": "DF-EXITS-2",
        "as_of": AS_OF,
        "base_currency": "USD",
        "entities": {"FUND": {"id": "FUND"}},
        "positions": [
            _deposit_position("USD-POS", "USD-DEP", "USD", "USD-OIS", notional="1000000"),
            _deposit_position("USD-POS-SMALL", "USD-DEP-SMALL", "USD", "USD-OIS", notional="250000"),
        ],
    })


def _market() -> MarketContext:
    market = MarketContext()
    market.insert(
        DiscountCurve(
            "USD-OIS",
            date.fromisoformat(AS_OF),
            [(0.0, 1.0), (0.5, 0.98), (1.0, 0.95)],
            day_count="act_365f",
        )
    )
    return market


def _materialization_bundle() -> str:
    return json.dumps({
        "schema": "finstack_quant.portfolio_materialization/1",
        "portfolio": {
            "id": "materialized-deposit",
            "base_currency": "USD",
            "as_of": "2025-01-01",
            "entities": {"entity": {"id": "entity", "name": None}},
        },
        "instruments": [
            {
                "artifact_id": "artifact-0",
                "envelope": {
                    "schema": "finstack_quant.instrument/1",
                    "instrument": {
                        "type": "deposit",
                        "spec": {
                            "id": "DEP-0",
                            "notional": {"amount": "1000000", "currency": "USD"},
                            "start_date": "2025-01-01",
                            "maturity": "2025-02-01",
                            "day_count": "act_360",
                            "discount_curve_id": "USD-OIS",
                            "attributes": {},
                        },
                    },
                },
            }
        ],
        "positions": [
            {
                "id": "position-0",
                "entity_id": "entity",
                "instrument_id": "DEP-0",
                "artifact_id": "artifact-0",
                "quantity": 1.0,
                "unit": "units",
            }
        ],
    })


def _risk_decomposition_payload() -> dict[str, object]:
    """Two factors, three position x factor rows, two residual rows.

    ``total_risk`` is deliberately not 1.0, so ``absolute_risk`` and
    ``relative_risk`` hold different numbers (relative = absolute / total). A
    unit total made the two columns identical and hid any swap between them.
    """
    return {
        "total_risk": 1.25,
        "measure": "variance",
        "factor_contributions": [
            {
                "factor_id": "credit::generic",
                "absolute_risk": 0.75,
                "relative_risk": 0.6,
                "marginal_risk": 0.3,
            },
            {
                "factor_id": "rates::USD",
                "absolute_risk": 0.5,
                "relative_risk": 0.4,
                "marginal_risk": 0.2,
            },
        ],
        "residual_risk": 0.0,
        "position_factor_contributions": [
            {
                "position_id": "P1",
                "factor_id": "credit::generic",
                "risk_contribution": 0.4,
            },
            {
                "position_id": "P1",
                "factor_id": "rates::USD",
                "risk_contribution": 0.3,
            },
            {
                "position_id": "P2",
                "factor_id": "credit::generic",
                "risk_contribution": 0.2,
            },
        ],
        "position_residual_contributions": [
            {
                "position_id": "P1",
                "residual_variance": 0.05,
                "source": {"kind": "from_credit_model", "issuer_id": "ACME"},
            },
            {
                "position_id": "P2",
                "residual_variance": 0.02,
                "source": {"kind": "other"},
            },
        ],
    }


def _risk_decomposition() -> RiskDecomposition:
    return RiskDecomposition.from_json(json.dumps(_risk_decomposition_payload()))


def _credit_model() -> CreditFactorModel:
    return CreditFactorModel.from_json(
        json.dumps({
            "schema": "finstack_quant.credit_factor_model/1",
            "as_of": "2024-03-29",
            "calibration_window": {"start": "2022-03-29", "end": "2024-03-29"},
            "policy": "globally_off",
            "generic_factor": {"name": "CDX IG", "series_id": "cdx.ig.5y"},
            "hierarchy": {"levels": ["rating", "region"]},
            "panel_frequency": "monthly",
            "use_returns_or_levels": "returns",
            "bucket_weighting": "equal",
            "config": {
                "factors": [],
                "covariance": {"n": 0, "factor_ids": [], "data": []},
                "matching": {"mapping_table": []},
                "pricing_mode": "delta_based",
            },
            "issuer_betas": [],
            "anchor_state": {"pc": 0.0, "by_level": []},
            "static_correlation": {"factor_ids": [], "data": []},
            "vol_state": {"factors": {}, "idiosyncratic": {}},
            "factor_histories": None,
            "diagnostics": {
                "mode_counts": {},
                "bucket_sizes_per_level": [],
                "fold_ups": [],
                "r_squared_histogram": None,
                "tag_taxonomy": {},
            },
        })
    )


def _credit_vol_decomposition() -> RiskDecomposition:
    """Decomposition whose factor ids follow the credit naming convention."""
    return RiskDecomposition.from_json(
        json.dumps({
            "total_risk": 1.0,
            "measure": "variance",
            "factor_contributions": [
                {
                    "factor_id": "credit::generic",
                    "absolute_risk": 0.10,
                    "relative_risk": 0.10,
                    "marginal_risk": 0.0,
                },
                {
                    "factor_id": "credit::level0::rating::IG",
                    "absolute_risk": 0.20,
                    "relative_risk": 0.20,
                    "marginal_risk": 0.0,
                },
                {
                    "factor_id": "credit::level1::rating.region::IG.EU",
                    "absolute_risk": 0.30,
                    "relative_risk": 0.30,
                    "marginal_risk": 0.0,
                },
            ],
            "residual_risk": 0.40,
            "position_factor_contributions": [
                {
                    "position_id": "POS1",
                    "factor_id": "credit::generic",
                    "risk_contribution": 0.10,
                },
                {
                    "position_id": "POS1",
                    "factor_id": "credit::level0::rating::IG",
                    "risk_contribution": 0.20,
                },
            ],
            "position_residual_contributions": [],
        })
    )


# PortfolioAttribution

ATTRIBUTION_COLUMNS = [
    "currency",
    "total_pnl",
    "carry",
    "rates_curves_pnl",
    "credit_curves_pnl",
    "inflation_curves_pnl",
    "correlations_pnl",
    "fx_pnl",
    "fx_translation_pnl",
    "cross_factor_pnl",
    "vol_pnl",
    "model_params_pnl",
    "market_scalars_pnl",
    "residual",
    "result_invalid",
]

# The nine factor columns this fixture deliberately leaves at zero, so the
# four it does move stand out.
_ATTRIBUTION_UNMOVED = [
    "credit_curves_pnl",
    "inflation_curves_pnl",
    "correlations_pnl",
    "fx_pnl",
    "cross_factor_pnl",
    "vol_pnl",
    "model_params_pnl",
    "market_scalars_pnl",
    "residual",
]


def _moving_attribution() -> tuple[PortfolioAttribution, pd.DataFrame]:
    """Attribution over a repriced two-currency book.

    An empty portfolio drives every one of the thirteen ``Money`` columns to
    ``0.0``, which makes the frame useless as a column-mapping guard: swapping
    ``carry`` with ``vol_pnl``, or ``total_pnl`` with ``residual``, still
    compares equal. Repricing two deposits across a curve *and* an FX move
    gives carry, rates, FX-translation and the total four mutually distinct
    non-zero values.
    """

    def market(
        usd_knots: list[tuple[float, float]],
        eur_knots: list[tuple[float, float]],
        eurusd: float,
    ) -> MarketContext:
        context = MarketContext()
        context.insert(DiscountCurve("USD-OIS", date.fromisoformat(AS_OF), usd_knots, day_count="act_365f"))
        context.insert(DiscountCurve("EUR-OIS", date.fromisoformat(AS_OF), eur_knots, day_count="act_365f"))
        fx = FxMatrix()
        fx.set_quote("EUR", "USD", eurusd)
        context.insert_fx(fx)
        return context

    portfolio = Portfolio.from_spec(
        json.dumps({
            "id": "DF-ATTRIB",
            "as_of": AS_OF,
            "base_currency": "USD",
            "entities": {"FUND": {"id": "FUND"}},
            "positions": [
                _deposit_position("USD-POS", "USD-DEP", "USD", "USD-OIS"),
                _deposit_position("EUR-POS", "EUR-DEP", "EUR", "EUR-OIS"),
            ],
        })
    )
    before = market([(0.0, 1.0), (0.5, 0.98), (1.0, 0.95)], [(0.0, 1.0), (0.5, 0.99), (1.0, 0.97)], 1.10)
    after = market([(0.0, 1.0), (0.5, 0.97), (1.0, 0.93)], [(0.0, 1.0), (0.5, 0.985), (1.0, 0.96)], 1.15)
    result = attribute_portfolio_pnl(portfolio, before, after, "2025-01-15", "2025-02-15", "parallel")
    return result, result.to_dataframe()


def test_portfolio_attribution_to_dataframe_is_single_row() -> None:
    """Flat factor totals collapse to exactly one row plus one currency column."""
    _, df = _moving_attribution()

    assert isinstance(df, pd.DataFrame)
    assert len(df) == 1
    assert list(df.columns) == ATTRIBUTION_COLUMNS
    # Money is flattened: a float column plus one shared currency column,
    # never a nested dict.
    assert df.iloc[0]["currency"] == "USD"
    assert bool(df.iloc[0]["result_invalid"]) is False


def test_portfolio_attribution_columns_carry_their_own_factor() -> None:
    """Each ``Money`` column reads back the accessor it claims to render.

    This is the column-mapping guard: with distinct values on the movers, a
    frame that shuffled two of these columns fails here.
    """
    result, df = _moving_attribution()
    row = df.iloc[0]

    for column in ATTRIBUTION_COLUMNS:
        if column in {"currency", "result_invalid"}:
            continue
        assert row[column] == pytest.approx(getattr(result, column).amount), column


def test_portfolio_attribution_movers_are_mutually_distinct() -> None:
    """The four factors this fixture moves take four different non-zero values."""
    _, df = _moving_attribution()
    row = df.iloc[0]

    movers = {name: float(row[name]) for name in ("total_pnl", "carry", "rates_curves_pnl", "fx_translation_pnl")}
    assert all(value != 0.0 for value in movers.values()), movers
    assert len(set(movers.values())) == len(movers), movers

    # Pinned levels, so a sign flip or a factor rescale is caught too.
    assert movers["carry"] == pytest.approx(3_802.592326, rel=1e-9)
    assert movers["rates_curves_pnl"] == pytest.approx(-13_313.489897, rel=1e-9)
    assert movers["fx_translation_pnl"] == pytest.approx(50_501.803491, rel=1e-9)
    assert movers["total_pnl"] == pytest.approx(40_990.905920, rel=1e-9)


def test_portfolio_attribution_leaves_untouched_factors_at_zero() -> None:
    """The nine factors the fixture never moves stay exactly zero.

    Stated separately from the movers so that "column X went non-zero" is a
    failure rather than an unnoticed change of meaning.
    """
    _, df = _moving_attribution()
    row = df.iloc[0]
    assert {name: float(row[name]) for name in _ATTRIBUTION_UNMOVED} == dict.fromkeys(_ATTRIBUTION_UNMOVED, 0.0)


def test_portfolio_attribution_of_an_empty_book_is_all_zero() -> None:
    """The degenerate case still has to work — it is just not a mapping guard."""
    result = attribute_portfolio_pnl(
        Portfolio.from_spec(EMPTY_PORTFOLIO),
        MarketContext(),
        MarketContext(),
        "2025-01-01",
        "2025-01-02",
        "parallel",
    )
    df = result.to_dataframe()

    assert len(df) == 1
    assert list(df.columns) == ATTRIBUTION_COLUMNS
    assert df.iloc[0]["currency"] == "USD"
    numeric = [c for c in ATTRIBUTION_COLUMNS if c not in {"currency", "result_invalid"}]
    assert {name: float(df.iloc[0][name]) for name in numeric} == dict.fromkeys(numeric, 0.0)


# RiskDecomposition


def test_risk_decomposition_to_factor_dataframe() -> None:
    """One row per factor contribution, with the pinned column order."""
    df = _risk_decomposition().to_factor_dataframe()

    assert isinstance(df, pd.DataFrame)
    assert list(df.columns) == [
        "factor_id",
        "absolute_risk",
        "relative_risk",
        "marginal_risk",
    ]
    assert len(df) == 2
    assert list(df["factor_id"]) == ["credit::generic", "rates::USD"]
    assert df["absolute_risk"].sum() == pytest.approx(1.25)
    assert df["relative_risk"].sum() == pytest.approx(1.0)
    # The three numeric columns hold three different value pairs, so a frame
    # that broadcast one column across the others fails here.
    assert list(df["absolute_risk"]) == pytest.approx([0.75, 0.5])
    assert list(df["relative_risk"]) == pytest.approx([0.6, 0.4])
    assert list(df["marginal_risk"]) == pytest.approx([0.3, 0.2])


def test_risk_decomposition_to_position_factor_dataframe() -> None:
    """One row per position x factor pair, with the pinned column order."""
    df = _risk_decomposition().to_position_factor_dataframe()

    assert isinstance(df, pd.DataFrame)
    assert list(df.columns) == ["position_id", "factor_id", "risk_contribution"]
    assert len(df) == 3
    assert list(df["position_id"]) == ["P1", "P1", "P2"]


def test_risk_decomposition_to_position_residual_dataframe() -> None:
    """Residual rows flatten the tagged ``source`` enum into two columns."""
    df = _risk_decomposition().to_position_residual_dataframe()

    assert isinstance(df, pd.DataFrame)
    assert list(df.columns) == [
        "position_id",
        "residual_variance",
        "source_kind",
        "source_issuer_id",
    ]
    assert len(df) == 2
    assert list(df["source_kind"]) == ["from_credit_model", "other"]
    assert df.iloc[0]["source_issuer_id"] == "ACME"
    # A `None` issuer lands in a pandas string column, so it reads back as the
    # column's NA sentinel (NaN) rather than the Python `None` that was serialized.
    assert pd.isna(df.iloc[1]["source_issuer_id"])


def test_risk_decomposition_residual_dataframe_keeps_schema_when_empty() -> None:
    """No residual allocation still yields the documented columns."""
    payload = _risk_decomposition_payload()
    payload["position_residual_contributions"] = []
    df = RiskDecomposition.from_json(json.dumps(payload)).to_position_residual_dataframe()

    assert len(df) == 0
    assert "residual_variance" in df.columns
    assert "source_kind" in df.columns


# Position VaR / ES


def _position_risk_payload() -> dict[str, object]:
    return {
        "portfolio_var": -100.0,
        "portfolio_es": -140.0,
        "confidence": 0.95,
        "method": "parametric",
        "var_contributions": [
            {
                "position_id": "P1",
                "component_var": -60.0,
                "relative_var": 0.6,
                "marginal_var": -1.2,
                "incremental_var": None,
            },
            {
                "position_id": "P2",
                "component_var": -40.0,
                "relative_var": 0.4,
                "marginal_var": -0.8,
                "incremental_var": None,
            },
        ],
        "es_contributions": [
            {
                "position_id": "P1",
                "component_es": -84.0,
                "relative_es": 0.6,
                "marginal_es": -1.7,
            },
            {
                "position_id": "P2",
                "component_es": -56.0,
                "relative_es": 0.4,
                "marginal_es": None,
            },
        ],
        "n_positions": 2,
        "euler_residual": 0.0,
    }


def test_position_var_contribution_to_dataframe_is_single_row() -> None:
    """Leaf row type exposes a one-row frame for symmetry with its parent."""
    item = PositionVarContribution.from_json(
        json.dumps({
            "position_id": "P1",
            "component_var": -60.0,
            "relative_var": 0.6,
            "marginal_var": -1.2,
            "incremental_var": None,
        })
    )
    df = item.to_dataframe()

    assert isinstance(df, pd.DataFrame)
    assert len(df) == 1
    assert list(df.columns) == [
        "position_id",
        "component_var",
        "relative_var",
        "marginal_var",
        "incremental_var",
    ]
    row = df.iloc[0]
    assert row["position_id"] == "P1"
    # Distinct values per column, so a shuffled schema cannot pass.
    assert row["component_var"] == pytest.approx(-60.0)
    assert row["relative_var"] == pytest.approx(0.6)
    assert row["marginal_var"] == pytest.approx(-1.2)
    assert pd.isna(row["incremental_var"])


def test_position_risk_decomposition_joins_var_and_es() -> None:
    """VaR and ES share a position key space and land on the same row."""
    decomposition = PositionRiskDecomposition.from_json(json.dumps(_position_risk_payload()))
    df = decomposition.to_dataframe()

    assert isinstance(df, pd.DataFrame)
    assert list(df.columns) == [
        "position_id",
        "component_var",
        "relative_var",
        "marginal_var",
        "incremental_var",
        "component_es",
        "relative_es",
        "marginal_es",
    ]
    assert len(df) == len(decomposition.var_contributions) == 2

    p1 = df[df["position_id"] == "P1"].iloc[0]
    assert p1["component_var"] == pytest.approx(-60.0)
    assert p1["component_es"] == pytest.approx(-84.0)
    # Portfolio-level scalars are header metadata, not per-row columns.
    assert "portfolio_var" not in df.columns
    assert "confidence" not in df.columns


# Risk budget


def test_position_budget_entry_to_dataframe_is_single_row() -> None:
    item = PositionBudgetEntry.from_json(
        json.dumps({
            "position_id": "P1",
            "actual_component_var": 1.0,
            "target_component_var": 0.8,
            "utilization": 1.25,
            "excess": 0.2,
        })
    )
    df = item.to_dataframe()

    assert isinstance(df, pd.DataFrame)
    assert len(df) == 1
    assert list(df.columns) == [
        "position_id",
        "actual_component_var",
        "target_component_var",
        "utilization",
        "excess",
    ]
    row = df.iloc[0]
    assert row["position_id"] == "P1"
    assert row["actual_component_var"] == pytest.approx(1.0)
    assert row["target_component_var"] == pytest.approx(0.8)
    assert row["utilization"] == pytest.approx(1.25)
    assert row["excess"] == pytest.approx(0.2)


def test_risk_budget_result_to_dataframe() -> None:
    """One row per budgeted position; the breach scalars stay off the rows."""
    result = RiskBudgetResult.from_json(
        json.dumps({
            "positions": [
                {
                    "position_id": "P1",
                    "actual_component_var": 1.0,
                    "target_component_var": 0.8,
                    "utilization": 1.25,
                    "excess": 0.2,
                },
                {
                    "position_id": "P2",
                    "actual_component_var": 0.4,
                    "target_component_var": 0.8,
                    "utilization": 0.5,
                    "excess": -0.4,
                },
            ],
            "total_overbudget": 0.2,
            "has_breach": True,
        })
    )
    df = result.to_dataframe()

    assert isinstance(df, pd.DataFrame)
    assert list(df.columns) == [
        "position_id",
        "actual_component_var",
        "target_component_var",
        "utilization",
        "excess",
    ]
    assert len(df) == len(result.positions) == 2
    assert "has_breach" not in df.columns


# What-if


def test_what_if_result_to_dataframe_covers_delta_only() -> None:
    """Per-factor deltas become rows; before/after stay as nested wrappers."""
    risk = json.dumps(_risk_decomposition_payload())
    result = WhatIfResult.from_json(
        '{"before":' + risk + ',"after":' + risk + ',"delta":[{"factor_id":"rates::USD","absolute_change":0.1,'
        '"relative_change":0.05}]}'
    )
    df = result.to_dataframe()

    assert isinstance(df, pd.DataFrame)
    assert list(df.columns) == ["factor_id", "absolute_change", "relative_change"]
    assert len(df) == len(result.delta) == 1
    assert df.iloc[0]["factor_id"] == "rates::USD"


# Stress


def test_stress_result_to_dataframe() -> None:
    """One row per ``(position_id, pnl)`` entry."""
    risk = json.dumps(_risk_decomposition_payload())
    result = StressResult.from_json(
        '{"total_pnl":-12.0,"position_pnl":[["P1",-10.0],["P2",-2.0]],"stressed_decomposition":' + risk + "}"
    )
    df = result.to_dataframe()

    assert isinstance(df, pd.DataFrame)
    assert list(df.columns) == ["position_id", "pnl"]
    assert len(df) == len(result.position_pnl) == 2
    assert df["pnl"].sum() == pytest.approx(result.total_pnl)


def test_tail_scenario_breakdown_to_dataframe_takes_parent_ids() -> None:
    """Ids live on the parent, so they are supplied as an argument."""
    breakdown = TailScenarioBreakdown.from_json(
        json.dumps({
            "scenario_index": 3,
            "portfolio_pnl": -12.0,
            "position_pnls": [-10.0, -2.0],
        })
    )
    df = breakdown.to_dataframe(["A", "B"])

    assert isinstance(df, pd.DataFrame)
    assert list(df.columns) == ["position_id", "pnl"]
    assert len(df) == len(breakdown.position_pnls) == 2
    assert list(df["position_id"]) == ["A", "B"]


def test_tail_scenario_breakdown_rejects_id_length_mismatch() -> None:
    """A wrong-length id list is a ValueError, never a silent mislabelling."""
    breakdown = TailScenarioBreakdown.from_json(
        json.dumps({
            "scenario_index": 0,
            "portfolio_pnl": -12.0,
            "position_pnls": [-10.0, -2.0],
        })
    )
    with pytest.raises(ValueError, match="position_ids length"):
        breakdown.to_dataframe(["A"])


def _stress_attribution() -> StressAttribution:
    """Positions supplied smallest-driver-first, so the ranking sort bites.

    ``SMALL`` averages -3.0 across the two tail scenarios and ``LARGE``
    averages -5.0. Feeding them in that order means a frame that merely echoed
    the input order would come out ``[SMALL, LARGE]`` and fail the
    largest-driver-first assertion below.
    """
    pnls_small = [-2.0, -4.0] + [0.5] * 38
    pnls_large = [-8.0, -2.0] + [0.5] * 38
    return build_stress_attribution(["SMALL", "LARGE"], [pnls_small, pnls_large], confidence=0.95)


def test_stress_attribution_to_dataframe() -> None:
    """One row per position contribution, largest driver first."""
    attribution = _stress_attribution()
    df = attribution.to_dataframe()

    assert isinstance(df, pd.DataFrame)
    assert list(df.columns) == [
        "position_id",
        "avg_tail_pnl",
        "pct_of_tail_loss",
        "worst_scenario_pnl",
    ]
    assert len(df) == len(attribution.position_contributions) == 2
    # Ranked by tail contribution, not by the order the caller supplied.
    assert list(df["position_id"]) == ["LARGE", "SMALL"]
    assert list(df["avg_tail_pnl"]) == pytest.approx([-5.0, -3.0])
    assert list(df["worst_scenario_pnl"]) == pytest.approx([-8.0, -4.0])
    # pct_of_tail_loss is a fraction, not a percentage.
    assert list(df["pct_of_tail_loss"]) == pytest.approx([0.625, 0.375])
    assert df["pct_of_tail_loss"].sum() == pytest.approx(1.0)


def test_stress_attribution_to_scenario_dataframe_is_a_matrix() -> None:
    """Scenario x position matrix: positions as columns, scenario index as index.

    The matrix keeps the caller's column order even though
    :meth:`to_dataframe` re-ranks its rows, so the two exports disagree on
    order by design; asserting both pins that.
    """
    attribution = _stress_attribution()
    df = attribution.to_scenario_dataframe()

    assert isinstance(df, pd.DataFrame)
    assert list(df.columns) == attribution.position_ids == ["SMALL", "LARGE"]
    assert len(df) == len(attribution.tail_scenarios) == 2
    assert list(df.index) == [s.scenario_index for s in attribution.tail_scenarios]
    assert df.loc[0, "SMALL"] == pytest.approx(-2.0)
    assert df.loc[0, "LARGE"] == pytest.approx(-8.0)
    assert df.loc[1, "SMALL"] == pytest.approx(-4.0)
    assert df.loc[1, "LARGE"] == pytest.approx(-2.0)


# Factor assignment


def _assignment_report() -> FactorAssignmentReport:
    return FactorAssignmentReport.from_json(
        json.dumps({
            "assignments": [
                {
                    "position_id": "P1",
                    "mappings": [
                        [
                            {"curve": {"id": "USD-OIS", "curve_type": "discount"}},
                            "rates::USD",
                            1.0,
                        ],
                        [
                            {"credit_curve": {"id": "ACME-CDS"}},
                            "credit::generic",
                            0.8,
                        ],
                    ],
                },
                {
                    "position_id": "P2",
                    "mappings": [
                        [
                            {"curve": {"id": "USD-OIS", "curve_type": "discount"}},
                            "rates::USD",
                            1.0,
                        ]
                    ],
                },
            ],
            "unmatched": [{"position_id": "P3", "dependency": {"spot": {"id": "AAPL"}}}],
        })
    )


def test_factor_assignment_report_to_dataframe_is_long_format() -> None:
    """Row count is total mappings, not the number of positions."""
    report = _assignment_report()
    df = report.to_dataframe()

    assert isinstance(df, pd.DataFrame)
    assert list(df.columns) == ["position_id", "factor_id", "beta", "dependency_json"]
    expected_rows = sum(a.n_mappings for a in report.assignments)
    assert len(df) == expected_rows == 3
    assert list(df["position_id"]) == ["P1", "P1", "P2"]
    # The MarketDependency variant tree stays a JSON string, as documented.
    assert json.loads(df.iloc[0]["dependency_json"])["curve"]["id"] == "USD-OIS"


def test_factor_assignment_report_to_unmatched_dataframe() -> None:
    report = _assignment_report()
    df = report.to_unmatched_dataframe()

    assert isinstance(df, pd.DataFrame)
    assert list(df.columns) == ["position_id", "dependency_json"]
    assert len(df) == len(report.unmatched) == 1
    assert json.loads(df.iloc[0]["dependency_json"])["spot"]["id"] == "AAPL"


# Credit vol report (the non-Serialize special case)


def test_credit_vol_report_to_level_dataframe() -> None:
    """One row per hierarchy level; columns built explicitly, not via serde."""
    report = build_credit_vol_report(_credit_vol_decomposition(), _credit_model())
    df = report.to_level_dataframe()

    assert isinstance(df, pd.DataFrame)
    assert list(df.columns) == ["level_name", "total"]
    assert len(df) == len(report.by_level) == 2
    assert list(df["level_name"]) == ["Rating", "Region"]


def test_credit_vol_report_to_position_dataframe() -> None:
    report = build_credit_vol_report(_credit_vol_decomposition(), _credit_model(), by_position=True)
    df = report.to_position_dataframe()

    assert isinstance(df, pd.DataFrame)
    assert list(df.columns) == ["position_id", "factor_total", "idiosyncratic", "total"]
    assert report.by_position is not None
    assert len(df) == len(report.by_position) == 1
    assert df.iloc[0]["position_id"] == "POS1"


def test_credit_vol_report_position_dataframe_keeps_schema_when_absent() -> None:
    """``by_position=None`` must still yield the column schema, not an empty frame.

    ``LevelVolContribution`` / ``PositionVolContribution`` do not derive
    ``Serialize``, so the binding hand-rolls the zero-row-with-schema case that
    ``serde_rows_to_dataframe_with_schema`` would otherwise provide.
    """
    report = build_credit_vol_report(_credit_vol_decomposition(), _credit_model(), by_position=False)
    df = report.to_position_dataframe()

    assert report.by_position is None
    assert isinstance(df, pd.DataFrame)
    assert len(df) == 0
    assert list(df.columns) == ["position_id", "factor_total", "idiosyncratic", "total"]


# Optimization


def test_trade_spec_to_dataframe_is_single_row() -> None:
    trade = TradeSpec.from_json(
        json.dumps({
            "position_id": "P1",
            "instrument_id": "I1",
            "trade_type": "existing",
            "direction": "buy",
            "current_quantity": 1.0,
            "target_quantity": 2.0,
            "delta_quantity": 1.0,
            "current_weight": 0.1,
            "target_weight": 0.2,
        })
    )
    df = trade.to_dataframe()

    assert isinstance(df, pd.DataFrame)
    assert len(df) == 1
    assert list(df.columns) == TRADE_COLUMNS
    row = df.iloc[0]
    assert row["position_id"] == "P1"
    assert row["instrument_id"] == "I1"
    assert row["trade_type"] == "existing"
    assert row["direction"] == "buy"
    assert row["current_quantity"] == pytest.approx(1.0)
    assert row["target_quantity"] == pytest.approx(2.0)
    assert row["delta_quantity"] == pytest.approx(1.0)
    assert row["current_weight"] == pytest.approx(0.1)
    assert row["target_weight"] == pytest.approx(0.2)


TRADE_COLUMNS = [
    "position_id",
    "instrument_id",
    "trade_type",
    "current_quantity",
    "target_quantity",
    "delta_quantity",
    "direction",
    "current_weight",
    "target_weight",
]


def _optimization_result() -> PortfolioOptimizationResult:
    """Optimize a two-position book so every weight cell differs.

    The single-position portfolio this used to run on made all four weight maps
    ``{'USD-POS': ...}`` and produced no trades at all, so both frames were
    vacuous: overwriting every cell with the same wrong number still satisfied
    the key-set and row-count assertions. Two deposits with a 4:1 notional
    split give distinct weights and a genuine rebalance.
    """
    objective = Objective.maximize(MetricExpr.weighted_sum(PerPositionMetric.pv_base()))
    spec = (
        PortfolioOptimizationSpec
        .new(_two_position_portfolio_json(), objective)
        .with_constraint(Constraint.weight_bounds(PositionFilter.all(), 0.0, 1.0, label="position_limits"))
        .with_weighting(WeightingScheme.value_weight())
        .with_missing_metric_policy(MissingMetricPolicy.zero())
        .with_label("dataframe_exits")
    )
    return optimize_portfolio(spec, _market())


def test_portfolio_optimization_result_to_dataframe_joins_weight_maps() -> None:
    """The four position-keyed maps join into one frame indexed by position."""
    result = _optimization_result()
    df = result.to_dataframe()

    assert isinstance(df, pd.DataFrame)
    assert list(df.columns) == [
        "current_weight",
        "optimal_weight",
        "weight_delta",
        "implied_quantity",
    ]
    expected = set(result.current_weights) | set(result.optimal_weights)
    expected |= set(result.weight_deltas) | set(result.implied_quantities)
    assert set(df.index) == expected == {"USD-POS", "USD-POS-SMALL"}
    assert len(df) == len(expected)


def test_portfolio_optimization_result_dataframe_cells_track_their_map() -> None:
    """Every cell equals the map it claims to render, at a distinct value.

    Maximising PV under value weighting concentrates the book into the larger
    deposit, so the eight cells take five different numbers: a frame that
    transposed two columns, or repeated one row, fails here.
    """
    result = _optimization_result()
    df = result.to_dataframe()

    for column, mapping in (
        ("current_weight", result.current_weights),
        ("optimal_weight", result.optimal_weights),
        ("weight_delta", result.weight_deltas),
        ("implied_quantity", result.implied_quantities),
    ):
        for position_id, value in mapping.items():
            assert df.loc[position_id, column] == pytest.approx(value), (position_id, column)

    big = df.loc["USD-POS"]
    small = df.loc["USD-POS-SMALL"]
    assert big["current_weight"] == pytest.approx(0.8)
    assert small["current_weight"] == pytest.approx(0.2)
    assert big["optimal_weight"] == pytest.approx(1.0)
    assert small["optimal_weight"] == pytest.approx(0.0)
    assert big["weight_delta"] == pytest.approx(0.2)
    assert small["weight_delta"] == pytest.approx(-0.2)
    assert big["implied_quantity"] == pytest.approx(1.25)
    assert small["implied_quantity"] == pytest.approx(0.0)


def test_portfolio_optimization_result_to_trade_dataframe() -> None:
    """Trade rows mirror ``to_trade_list`` one-for-one, schema always present."""
    result = _optimization_result()
    trades = result.to_trade_list()
    df = result.to_trade_dataframe()

    assert isinstance(df, pd.DataFrame)
    assert list(df.columns) == TRADE_COLUMNS
    # Non-empty by construction: `0 == 0` would prove nothing about the mirror.
    assert len(trades) == 2
    assert len(df) == len(trades)
    assert list(df["position_id"]) == [trade.position_id for trade in trades]
    assert list(df["delta_quantity"]) == pytest.approx([trade.delta_quantity for trade in trades])


def test_portfolio_optimization_trade_dataframe_renders_each_trade() -> None:
    """The two trades are a close-out and a top-up, and read back distinctly."""
    df = _optimization_result().to_trade_dataframe().set_index("position_id")

    closed = df.loc["USD-POS-SMALL"]
    assert closed["instrument_id"] == "USD-DEP-SMALL"
    assert closed["trade_type"] == "close_out"
    assert closed["direction"] == "sell"
    assert closed["current_quantity"] == pytest.approx(1.0)
    assert closed["target_quantity"] == pytest.approx(0.0)
    assert closed["delta_quantity"] == pytest.approx(-1.0)
    assert closed["current_weight"] == pytest.approx(0.2)
    assert closed["target_weight"] == pytest.approx(0.0)

    topped_up = df.loc["USD-POS"]
    assert topped_up["instrument_id"] == "USD-DEP"
    assert topped_up["trade_type"] == "existing"
    assert topped_up["direction"] == "buy"
    assert topped_up["current_quantity"] == pytest.approx(1.0)
    assert topped_up["target_quantity"] == pytest.approx(1.25)
    assert topped_up["delta_quantity"] == pytest.approx(0.25)
    assert topped_up["current_weight"] == pytest.approx(0.8)
    assert topped_up["target_weight"] == pytest.approx(1.0)


def test_portfolio_optimization_trade_dataframe_keeps_schema_when_no_trades() -> None:
    """A book already at its optimum yields zero rows but the full schema.

    Split out from the populated case so "empty" is asserted deliberately
    rather than being the accidental state of the main fixture.
    """
    objective = Objective.maximize(MetricExpr.weighted_sum(PerPositionMetric.pv_base()))
    spec = (
        PortfolioOptimizationSpec
        .new(_portfolio_json(), objective)
        .with_constraint(Constraint.weight_bounds(PositionFilter.all(), 0.0, 1.0, label="position_limits"))
        .with_weighting(WeightingScheme.value_weight())
        .with_missing_metric_policy(MissingMetricPolicy.zero())
        .with_label("already_optimal")
    )
    result = optimize_portfolio(spec, _market())
    df = result.to_trade_dataframe()

    assert result.to_trade_list() == []
    assert len(df) == 0
    assert list(df.columns) == TRADE_COLUMNS


# Portfolio runtime types


def test_portfolio_valuation_to_dataframe_matches_arrow_columns() -> None:
    """The pandas exit reuses the same table envelope as the Arrow exit."""
    valuation = value_portfolio(Portfolio.from_spec(_portfolio_json()), _market())
    df = valuation.to_dataframe()

    assert isinstance(df, pd.DataFrame)
    assert list(df.columns) == [
        "position_id",
        "entity_id",
        "value_native",
        "value_base",
        "currency_native",
        "currency_base",
        "risk_metrics_complete",
        "risk_error",
    ]
    assert len(df) == len(valuation) == 1
    assert df.iloc[0]["position_id"] == "USD-POS"
    assert df.iloc[0]["currency_base"] == "USD"


def test_portfolio_cashflows_to_dataframe() -> None:
    """One row per dated event, with Money flattened into amount + currency."""
    cashflows = aggregate_full_cashflows(Portfolio.from_spec(_portfolio_json()), _market())
    df = cashflows.to_dataframe()

    assert isinstance(df, pd.DataFrame)
    assert list(df.columns) == [
        "position_id",
        "instrument_id",
        "instrument_type",
        "date",
        "amount",
        "currency",
        "kind",
        "reset_date",
        "accrual_factor",
        "rate",
    ]
    assert len(df) == len(cashflows)
    if len(df) > 0:
        assert set(df["currency"]) == {"USD"}
        # Dates are ISO strings, never Rust date objects.
        assert date.fromisoformat(df.iloc[0]["date"]) >= date.fromisoformat(AS_OF)


def _portfolio_metrics() -> PortfolioMetrics:
    return PortfolioMetrics.from_json(
        json.dumps({
            "aggregated": {
                "pv_base": {"metric_id": "pv_base", "total": 300.0, "by_entity": {"FUND": 300.0}},
                "dv01": {"metric_id": "dv01", "total": -12.0, "by_entity": {"FUND": -12.0}},
            },
            "by_position": {
                "P1": {"currency": "USD", "metrics": {"pv_base": 200.0, "dv01": -8.0}},
                "P2": {"currency": "EUR", "metrics": {"pv_base": 100.0}},
            },
        })
    )


def test_portfolio_metrics_to_aggregated_dataframe() -> None:
    df = _portfolio_metrics().to_aggregated_dataframe()

    assert isinstance(df, pd.DataFrame)
    assert list(df.columns) == ["metric_id", "total"]
    assert len(df) == 2
    assert list(df["metric_id"]) == ["pv_base", "dv01"]


def test_portfolio_metrics_to_position_dataframe_is_long_format() -> None:
    """Row count is (position, metric) pairs, and currency travels with them."""
    df = _portfolio_metrics().to_position_dataframe()

    assert isinstance(df, pd.DataFrame)
    assert list(df.columns) == ["metric_id", "position_id", "currency", "value"]
    assert len(df) == 3
    assert list(df["position_id"]) == ["P1", "P1", "P2"]
    assert df[df["position_id"] == "P2"].iloc[0]["currency"] == "EUR"


def test_materialization_report_to_dataframe_is_single_row() -> None:
    """Flat counters plus flattened phase timings on one row."""
    _, report = Portfolio.from_materialization(_materialization_bundle())
    df = report.to_dataframe()

    assert isinstance(df, pd.DataFrame)
    assert len(df) == 1
    assert list(df.columns) == [
        "unique_instruments",
        "positions",
        "dependencies",
        "cache_hits",
        "input_bytes",
        "truncated",
        "timing_available",
        "phase_parse_nanos",
        "phase_validate_versions_nanos",
        "phase_decode_instruments_nanos",
        "phase_build_positions_nanos",
        "phase_index_build_nanos",
    ]
    row = df.iloc[0]
    assert row["positions"] == report.positions == 1
    assert row["unique_instruments"] == report.unique_instruments == 1
    assert row["dependencies"] == report.dependencies == 1
    assert row["cache_hits"] == report.cache_hits == 0
    # `input_bytes` is the only counter that is neither 0 nor 1, so it also
    # discriminates the counter columns from each other.
    assert row["input_bytes"] == report.input_bytes == len(_materialization_bundle().encode())
    assert bool(row["truncated"]) is False
    assert bool(row["timing_available"]) is report.timing_available
