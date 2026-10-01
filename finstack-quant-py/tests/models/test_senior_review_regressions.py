"""Reject invalid model snapshots and execution inputs at the Rust boundary."""

import json
from typing import Any

import pytest

from finstack_quant.models.liquidity import (
    AlmgrenChrissModel,
    KyleLambdaModel,
    LiquidityProfile,
    TradeParams,
)
from finstack_quant.models.rates.dtsm import (
    DieboldLi,
    FactorTimeSeries,
    YieldForecast,
    YieldPanel,
    YieldPca,
    YieldPcaView,
)


def test_yield_panel_snapshot_rejects_empty_observations() -> None:
    """A serialized panel cannot bypass the ordinary constructor checks."""
    payload = {"yields": [[], 0, 0], "tenors": [1.0], "dates": None}
    with pytest.raises(ValueError, match=r"columns|observations"):
        YieldPanel.from_json(json.dumps(payload))


def test_diebold_li_snapshot_rejects_incomplete_fitted_state() -> None:
    """Partial VAR state fails while loading, before forecast can panic."""
    payload = json.loads(DieboldLi().to_json())
    payload["mu"] = [[0.0, 0.0, 0.0], 3, None]
    with pytest.raises(ValueError, match="fitted together"):
        DieboldLi.from_json(json.dumps(payload))


@pytest.mark.parametrize(
    ("model_type", "payload"),
    [
        (
            FactorTimeSeries,
            {"dates": None, "factors": [[], 0, 3], "residuals": [[], 0, 3], "r_squared": [], "r_squared_avg": 0.0},
        ),
        (
            YieldForecast,
            {
                "horizon": 1,
                "yields": [0.02],
                "tenors": [1.0],
                "factors": [0.02, 0.0, 0.0],
                "lower_95": [],
                "upper_95": [0.03],
            },
        ),
        (
            YieldPca,
            {
                "eigenvalues": [1.0, 0.0],
                "loadings": [[], 0, 0],
                "scores": [[], 0, 0],
                "tenors": [1.0, 2.0],
                "variance_explained": [1.0, 0.0],
                "cumulative_variance": [1.0, 1.0],
                "mean_change": [[0.0, 0.0], 2, None],
            },
        ),
        (
            YieldPcaView,
            {
                "loadings": [[], []],
                "scores": [[0.0], [0.0]],
                "eigenvalues": [1.0],
                "explained_variance_ratio": [1.0],
                "cumulative_variance": [1.0],
                "mean_change": [0.0, 0.0],
                "tenors": [1.0, 2.0],
            },
        ),
    ],
)
def test_dtsm_result_snapshots_reject_misaligned_shapes(model_type: type[Any], payload: dict[str, Any]) -> None:
    """Published result accessors receive only structurally valid snapshots."""
    with pytest.raises(ValueError, match=model_type.__name__):
        model_type.from_json(json.dumps(payload))


@pytest.mark.parametrize(
    ("model_type", "payload"),
    [
        (AlmgrenChrissModel, {"gamma": -1.0, "eta": -1.0, "delta": 1.0}),
        (KyleLambdaModel, {"lambda": -1.0}),
    ],
)
def test_impact_model_snapshots_reject_invalid_parameters(model_type: type[Any], payload: dict[str, Any]) -> None:
    with pytest.raises(ValueError, match="finite and non-negative"):
        model_type.from_json(json.dumps(payload))


@pytest.mark.parametrize("invalid", [float("nan"), float("inf"), -1.0, 0.0])
@pytest.mark.parametrize("field", ["daily_volatility", "reference_price"])
def test_kyle_cost_and_trajectory_reject_invalid_risk_inputs(field: str, invalid: float) -> None:
    profile = LiquidityProfile("ACME", 100.0, 99.0, 101.0, 1_000_000.0, 100.0, 0.001)
    volatility = invalid if field == "daily_volatility" else 0.02
    reference_price = invalid if field == "reference_price" else None
    params = TradeParams(1000.0, 5.0, volatility, profile, reference_price=reference_price)
    model = KyleLambdaModel(0.001)
    with pytest.raises(ValueError, match=field):
        model.estimate_cost(params)
    with pytest.raises(ValueError, match=field):
        model.optimal_trajectory(params, 1)
