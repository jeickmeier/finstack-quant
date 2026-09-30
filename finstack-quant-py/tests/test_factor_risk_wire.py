"""Cross-host factor-risk and liquidity cases shared with the WASM facade.

The same inputs and assertions live in
``finstack-quant-wasm/tests/facade/portfolio.test.mjs`` (factor risk),
``finstack-quant-wasm/tests/facade/models_liquidity.test.mjs`` (liquidity tiers)
and ``finstack-quant-wasm/tests/facade/models_audit.test.mjs`` (portfolio loss):
both hosts return the canonical Rust result types and read the same Rust
defaults.
"""

from __future__ import annotations

import json

import pytest

from finstack_quant.models.correlation import PortfolioLossResult
from finstack_quant.models.factor.risk import (
    DecompositionConfig,
    build_stress_attribution,
    evaluate_risk_budget,
    historical_var_decomposition,
    parametric_es_decomposition,
    parametric_var_decomposition,
)

VAR_IDS = ["A", "B"]
VAR_WEIGHTS = [0.6, 0.4]
VAR_COVARIANCE = [[0.04, 0.01], [0.01, 0.09]]
PARAMETRIC_VAR_95 = -0.3015066497719077
DECOMPOSITION_KEYS = [
    "confidence",
    "es_contributions",
    "euler_residual",
    "method",
    "n_positions",
    "portfolio_es",
    "portfolio_var",
    "var_contributions",
]


def _scenario_pnls(n_scenarios: int = 20) -> list[list[float]]:
    return [
        [float(i - n_scenarios // 2) for i in range(n_scenarios)],
        [(i - n_scenarios // 2) / 2.0 for i in range(n_scenarios)],
    ]


def test_parametric_var_decomposition_wire_shape_matches_wasm() -> None:
    result = parametric_var_decomposition(VAR_IDS, VAR_WEIGHTS, VAR_COVARIANCE)
    doc = json.loads(result.to_json())
    assert sorted(doc) == DECOMPOSITION_KEYS
    assert sorted(doc["var_contributions"][0]) == [
        "component_var",
        "incremental_var",
        "marginal_var",
        "position_id",
        "relative_var",
    ]
    assert sorted(doc["es_contributions"][0]) == ["component_es", "marginal_es", "position_id", "relative_es"]
    # Omitted confidence resolves to the Rust `parametric_95()` preset.
    assert doc["confidence"] == 0.95
    assert doc["method"] == "parametric"
    assert doc["portfolio_var"] == pytest.approx(PARAMETRIC_VAR_95, abs=1e-12)
    assert doc["var_contributions"][0]["incremental_var"] is None

    explicit = parametric_var_decomposition(VAR_IDS, VAR_WEIGHTS, VAR_COVARIANCE, 0.99, True)
    assert explicit.confidence == 0.99
    assert explicit.var_contributions[0].incremental_var is not None


def test_parametric_es_decomposition_is_the_es_view() -> None:
    view = parametric_es_decomposition(VAR_IDS, VAR_WEIGHTS, VAR_COVARIANCE)
    doc = json.loads(view.to_json())
    assert sorted(doc) == ["confidence", "contributions", "n_positions", "portfolio_es", "portfolio_var"]
    assert doc["confidence"] == 0.95


def test_decompositions_no_longer_take_a_config_keyword() -> None:
    with pytest.raises(TypeError):
        parametric_var_decomposition(  # type: ignore[call-arg]
            VAR_IDS, VAR_WEIGHTS, VAR_COVARIANCE, config=DecompositionConfig.parametric_95()
        )


def test_historical_decomposition_has_es_rows_and_rust_default_confidence() -> None:
    pnls = _scenario_pnls()
    result = historical_var_decomposition(VAR_IDS, pnls, 0.9)
    doc = json.loads(result.to_json())
    assert sorted(doc) == DECOMPOSITION_KEYS
    assert doc["method"] == "historical"
    assert len(doc["es_contributions"]) == 2
    assert doc["euler_residual"] is None
    # Two tail scenarios at 95% need at least 40 scenarios.
    longer = _scenario_pnls(40)
    assert historical_var_decomposition(VAR_IDS, longer).confidence == DecompositionConfig.historical_95().confidence


def test_scenario_major_pnls_are_rejected_like_wasm() -> None:
    """Only the Rust position-major layout is read; no orientation guessing."""
    pnls = _scenario_pnls()
    scenario_major = [[a, b] for a, b in zip(pnls[0], pnls[1], strict=True)]
    with pytest.raises(ValueError, match="must have 2 rows, got 20"):
        historical_var_decomposition(VAR_IDS, scenario_major, 0.9)
    with pytest.raises(ValueError, match="must have 2 rows, got 20"):
        build_stress_attribution(VAR_IDS, scenario_major, 0.9)


def test_stress_attribution_confidence_defaults_to_the_rust_preset() -> None:
    pnls = _scenario_pnls(40)
    assert build_stress_attribution(VAR_IDS, pnls).to_json() == build_stress_attribution(VAR_IDS, pnls, 0.95).to_json()


def test_risk_budget_wire_shape_matches_wasm() -> None:
    budget = evaluate_risk_budget(VAR_IDS, [60.0, 40.0], [0.5, 0.5], 100.0, 1.1)
    doc = json.loads(budget.to_json())
    assert sorted(doc) == ["has_breach", "positions", "total_overbudget"]
    assert sorted(doc["positions"][0]) == [
        "actual_component_var",
        "excess",
        "position_id",
        "target_component_var",
        "utilization",
    ]
    assert doc["has_breach"] is True
    assert doc["total_overbudget"] == pytest.approx(10.0)


def test_portfolio_loss_result_from_losses_matches_wasm() -> None:
    result = PortfolioLossResult.from_losses([0.0, 1.0, 2.0, 5.0, 10.0], 0.75)
    assert result.var == 5.0
    assert result.expected_shortfall == pytest.approx(9.0)
    assert result.expected_loss == pytest.approx(3.6)
    assert PortfolioLossResult.from_json(result.to_json()).to_json() == result.to_json()
    doc = json.loads(result.to_json())
    with pytest.raises(ValueError, match="do not match the losses"):
        PortfolioLossResult.from_json(json.dumps({**doc, "var": 99.0}))
    with pytest.raises(ValueError, match="must be finite and non-negative"):
        PortfolioLossResult.from_json(json.dumps({**doc, "losses": [-1.0, 2.0, 3.0, 4.0, 5.0]}))
    with pytest.raises(ValueError, match="must not be empty"):
        PortfolioLossResult.from_losses([], 0.75)
