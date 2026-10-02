"""Liquidity binding behavior."""

import pickle

import pytest

from finstack_quant.models.liquidity import (
    AlmgrenChrissModel,
    ImpactEstimate,
    KyleLambdaModel,
    LiquidityProfile,
    LvarBangiaScalar,
    TradeParams,
    almgren_chriss_impact,
    kyle_lambda,
    liquidity_tier,
    lvar_bangia,
)


def test_almgren_chriss_impact_returns_typed_estimate() -> None:
    """The Python projection exposes the canonical ``ImpactEstimate`` fields."""
    result = almgren_chriss_impact(
        position_size=10_000.0,
        avg_daily_volume=1_000_000.0,
        volatility=0.02,
        execution_horizon_days=1.0,
        permanent_impact_coef=0.0,
        temporary_impact_coef=0.01,
        reference_price=100.0,
    )

    assert isinstance(result, ImpactEstimate)
    assert result.to_series().index.tolist() == [
        "permanent_impact",
        "temporary_impact",
        "total_cost",
        "cost_bp",
        "execution_risk",
    ]
    assert result.to_dataframe().shape == (1, 5)
    assert result.total_cost == pytest.approx(result.permanent_impact + result.temporary_impact)
    assert result.cost_bp == pytest.approx(result.total_cost / (10_000.0 * 100.0) * 10_000.0)
    assert result.execution_risk >= 0.0
    assert ImpactEstimate.from_json(result.to_json()) == result
    assert pickle.loads(pickle.dumps(result)) == result  # noqa: S301


def test_lvar_bangia_returns_typed_scalar() -> None:
    """``lvar_bangia`` returns the typed Rust result with pandas exits."""
    result = lvar_bangia(-100.0, 0.01, 0.005, 0.99, 1_000_000.0)

    assert isinstance(result, LvarBangiaScalar)
    assert result.lvar <= result.var <= 0.0
    assert result.spread_cost >= 0.0
    assert result.to_dataframe().columns.tolist() == ["var", "spread_cost", "lvar", "lvar_ratio"]
    assert LvarBangiaScalar.from_json(result.to_json()) == result
    assert pickle.loads(pickle.dumps(result)) == result  # noqa: S301


def test_kyle_lambda_uses_reference_price() -> None:
    """Kyle lambda is calibrated in price space with ``(returns, volumes)`` order."""
    assert kyle_lambda([0.01, -0.02], [100.0, 200.0], 50.0) == pytest.approx(0.005)
    assert kyle_lambda([0.01], [100.0], 0.0) is None


def test_liquidity_tier_accepts_custom_thresholds() -> None:
    # Same cases as finstack-quant-wasm/tests/facade/models_liquidity.test.mjs:
    # the Rust `liquidity_tier` owns the thresholds rule for both hosts.
    assert liquidity_tier(3.0) == "tier2"
    assert liquidity_tier(3.0, (0.5, 2.0, 10.0, 30.0)) == "tier3"
    assert liquidity_tier(12.0, (1.0, 2.0, 3.0, 4.0)) == "tier5"
    with pytest.raises(ValueError, match="greater than the prior threshold"):
        liquidity_tier(3.0, (5.0, 2.0, 10.0, 30.0))
    for bad in ((-1.0, 0.0, 1.0, 2.0), (0.0, 1.0, 2.0, 3.0), (1.0, float("nan"), 2.0, 3.0)):
        with pytest.raises(ValueError, match="must be finite and positive"):
            liquidity_tier(3.0, bad)


def test_impact_models_round_trip_and_trade() -> None:
    profile = LiquidityProfile("ACME", 100.0, 99.95, 100.05, 1_000_000.0, 500.0, 0.0002)
    assert profile.spread == pytest.approx(0.1)
    assert profile.spread_volatility_kind == "relative"
    assert LiquidityProfile.from_json(profile.to_json()) == profile

    params = TradeParams(10_000.0, 2.0, 0.02, profile)
    assert params.effective_reference_price == 100.0

    kyle = KyleLambdaModel.from_amihud(0.0001, 50.0)
    assert kyle.lambda_ == pytest.approx(0.005)
    trajectory = kyle.optimal_trajectory(params, 4)
    frame = trajectory.to_dataframe()
    assert frame.columns.tolist() == ["t", "holdings", "trade"]
    assert len(frame) == 5
    assert trajectory.remaining[-1] == 0.0

    model = AlmgrenChrissModel.from_profile(profile, 0.02)
    assert model.delta == 0.5
    assert model.estimate_cost(params).total_cost > 0.0
    assert AlmgrenChrissModel.from_json(model.to_json()) == model
    with pytest.raises(ValueError, match="delta = 1"):
        model.optimal_trajectory(params, 4)


def test_liquidity_moved_without_portfolio_aliases() -> None:
    """Liquidity APIs exist only under the models-owned namespace."""
    from finstack_quant import portfolio

    assert not hasattr(portfolio, "almgren_chriss_impact")
    assert not hasattr(portfolio, "kyle_lambda")
