"""Tests for Merton structural credit Python bindings."""

from __future__ import annotations

import datetime

import pytest

from finstack_quant.models.credit import (
    AssetDynamics,
    MertonBarrierType,
    MertonModel,
)


def test_from_equity_recovers_known_values() -> None:
    known = MertonModel(100.0, 0.20, 80.0, 0.05)
    equity, equity_vol = known.try_implied_equity(1.0)
    calibrated = MertonModel.from_equity(equity, equity_vol, 80.0, 0.05, 0.0, 1.0)
    assert abs(calibrated.asset_value - 100.0) < 0.5
    assert abs(calibrated.asset_vol - 0.20) < 0.05


def test_try_implied_equity_rejects_non_positive_horizon() -> None:
    model = MertonModel(100.0, 0.20, 80.0, 0.05)
    with pytest.raises(ValueError, match="horizon must be > 0"):
        model.try_implied_equity(0.0)


def test_from_equity_rejects_invalid_inputs() -> None:
    with pytest.raises(ValueError, match="must be positive"):
        MertonModel.from_equity(0.0, 0.30, 80.0, 0.05, 0.0, 1.0)
    with pytest.raises(ValueError, match="must be non-negative"):
        MertonModel.from_equity(25.0, -0.30, 80.0, 0.05, 0.0, 1.0)


def test_from_cds_spread_and_from_target_pd_smoke() -> None:
    from_cds = MertonModel.from_cds_spread(200.0, 0.40, 80.0, 0.05, 5.0, 100.0, 0.0)
    assert from_cds.asset_value == 100.0
    assert from_cds.asset_vol > 0.0
    assert 0.0 < from_cds.default_probability(5.0) < 1.0
    assert from_cds.cds_par_spread(5.0, 0.40) == pytest.approx(0.02, abs=1e-6)

    from_pd = MertonModel.from_target_pd(100.0, 0.25, 0.05, 0.0, 0.05, 1.0)
    assert abs(from_pd.default_probability(1.0) - 0.05) < 1e-4


def test_from_target_pd_payout_rate_lowers_the_calibrated_barrier() -> None:
    no_payout = MertonModel.from_target_pd(100.0, 0.25, 0.05, 0.0, 0.05, 1.0)
    with_payout = MertonModel.from_target_pd(100.0, 0.25, 0.05, 0.03, 0.05, 1.0)
    assert with_payout.debt_barrier < no_payout.debt_barrier
    assert with_payout.default_probability(1.0) == pytest.approx(0.05, abs=1e-4)


def test_physical_measure_default_probability_is_below_the_risk_neutral_one() -> None:
    model = MertonModel(100.0, 0.25, 80.0, 0.05)
    assert model.distance_to_default_with_drift(0.05, 1.0) == pytest.approx(model.distance_to_default(1.0))
    assert model.default_probability_with_drift(0.12, 1.0) < model.default_probability(1.0)


def test_kmv_default_point_is_short_term_plus_half_long_term_debt() -> None:
    assert MertonModel.kmv_default_point(40.0, 60.0) == pytest.approx(70.0)
    with pytest.raises(ValueError, match="short_term_debt must be finite and >= 0"):
        MertonModel.kmv_default_point(-1.0, 60.0)


def test_debt_spread_is_below_the_exogenous_recovery_spread() -> None:
    model = MertonModel(100.0, 0.25, 80.0, 0.05)
    assert 0.0 < model.debt_spread(1.0) < model.implied_spread(1.0, 0.40)


def test_cds_par_spread_exceeds_the_zero_coupon_implied_spread() -> None:
    model = MertonModel(100.0, 0.25, 80.0, 0.05)
    assert model.cds_par_spread(5.0, 0.40) > model.implied_spread(5.0, 0.40)


def test_new_with_dynamics_first_passage() -> None:
    model = MertonModel.new_with_dynamics(
        100.0,
        0.25,
        80.0,
        0.05,
        0.0,
        MertonBarrierType.first_passage(0.0),
        AssetDynamics.geometric_brownian(),
    )
    assert isinstance(model.barrier_type, MertonBarrierType)
    assert isinstance(model.dynamics, AssetDynamics)
    terminal = MertonModel(100.0, 0.25, 80.0, 0.05)
    assert model.default_probability(1.0) >= terminal.default_probability(1.0)


def test_to_hazard_curve_returns_hazard_curve() -> None:
    model = MertonModel(100.0, 0.25, 80.0, 0.05)
    curve = model.to_hazard_curve(
        "ACME-HZD",
        datetime.date(2024, 1, 15),
        [1.0, 3.0, 5.0],
        0.40,
        "act_365f",
    )
    assert curve.id == "ACME-HZD"


def test_to_hazard_curve_accepts_a_day_count_override() -> None:
    model = MertonModel(100.0, 0.25, 80.0, 0.05)
    base_date = datetime.date(2024, 1, 15)
    curve = model.to_hazard_curve("ACME", base_date, [1.0, 5.0], 0.40, "act_360")
    assert curve.sp(5.0) == pytest.approx(1.0 - model.default_probability(5.0))
    with pytest.raises(ValueError, match="Invalid day_count"):
        model.to_hazard_curve("ACME", base_date, [1.0, 5.0], 0.40, "not_a_day_count")


def test_simulate_paths_deterministic_with_seed() -> None:
    model = MertonModel(100.0, 0.20, 80.0, 0.05)
    a = model.simulate_paths(10, 60, 5.0, 42, antithetic=False)
    b = model.simulate_paths(10, 60, 5.0, 42, antithetic=False)
    assert a.num_paths == 10
    assert a.num_steps == 60
    assert a.path(0) == b.path(0)
    assert a.get(0, 0) == pytest.approx(100.0)


def test_credit_grades_and_accessors() -> None:
    cg = MertonModel.credit_grades(50.0, 0.30, 40.0, 0.05, 0.10, 0.40)
    assert cg.asset_value > 0.0
    assert cg.debt_barrier > 0.0
    assert 0.0 <= cg.default_probability(1.0) <= 1.0
