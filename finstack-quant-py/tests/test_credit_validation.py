"""Credit APIs reject non-finite inputs without panicking."""

import math

import pytest

from finstack_quant.models import credit


@pytest.mark.parametrize("invalid", [float("nan"), float("inf"), -float("inf")])
def test_credit_non_finite_inputs_raise_value_error(invalid: float) -> None:
    with pytest.raises(ValueError, match=r"(?i)quantile.*finite"):
        credit.lgd.beta_recovery_quantile(0.4, 0.2, invalid)
    with pytest.raises(ValueError, match=r"(?i)WARF.*finite"):
        credit.migration.RatingScale.standard().rating_from_warf(invalid)
    with pytest.raises(ValueError, match=r"(?i)non-finite"):
        credit.pd.ttc_to_pit(0.02, 0.2, invalid)
    with pytest.raises(ValueError, match=r"(?i)non-finite"):
        credit.pd.pit_to_ttc(0.02, 0.2, invalid)


def test_lgd_typed_surface() -> None:
    import pickle

    stats = credit.lgd.seniority_recovery_stats("senior_secured")
    assert isinstance(stats, credit.lgd.BetaRecovery)
    assert stats.mean == pytest.approx(0.52)
    assert stats.mean_lgd == pytest.approx(1.0 - stats.mean)
    assert credit.lgd.BetaRecovery.from_json(stats.to_json()).alpha == pytest.approx(stats.alpha)
    assert pickle.loads(pickle.dumps(stats)).beta_param == pytest.approx(stats.beta_param)  # noqa: S301 - trusted in-process round trip
    assert stats.sample_seeded(3, 42) == credit.lgd.beta_recovery_sample(stats.mean, stats.std_dev, 3, 42)
    assert stats.quantile(0.5) == pytest.approx(credit.lgd.beta_recovery_quantile(stats.mean, stats.std_dev, 0.5))

    result = credit.lgd.workout_lgd(100.0, [("real_estate", 80.0, 0.30)], 0.05, 0.03, 2.0, 0.05)
    assert isinstance(result, credit.lgd.WorkoutLgdResult)
    expected_net = 48.0 / (1.05 * 1.05)
    assert result.net_recovery == pytest.approx(expected_net)
    assert result.lgd == pytest.approx(1.0 - expected_net / 100.0)
    assert result.recovery_rate == pytest.approx(1.0 - result.lgd)

    model = (
        credit.lgd.WorkoutLgd
        .builder()
        .collateral(credit.lgd.CollateralPiece("real_estate", 80.0, 0.30))
        .workout_years(2.0)
        .discount_rate(0.05)
        .costs(credit.lgd.WorkoutCosts(0.05, 0.03))
        .build()
    )
    assert model.evaluate(100.0) == result
    assert model.collateral[0].liquidation_value == pytest.approx(56.0)
    assert credit.lgd.WorkoutLgd.from_json(model.to_json()).lgd(100.0) == pytest.approx(result.lgd)
    with pytest.raises(ValueError, match="unknown collateral type"):
        credit.lgd.CollateralPiece("real-estate", 1.0, 0.0)

    floor = credit.lgd.DownturnLgd.regulatory_floor(0.05, 0.25)
    assert floor.method == "regulatory_floor"
    assert floor.adjust(0.10) == pytest.approx(credit.lgd.downturn_lgd_regulatory_floor(0.10, 0.05, 0.25))
    assert credit.lgd.DownturnLgd.basel_unsecured().method == "regulatory_floor"
    assert pickle.loads(pickle.dumps(floor)).adjust(0.10) == pytest.approx(0.25)  # noqa: S301 - trusted in-process round trip

    revolver = credit.lgd.EadCalculator.revolver(60.0, 40.0)
    assert revolver.ead == pytest.approx(90.0)
    assert revolver.utilization == pytest.approx(0.6)
    assert revolver.leq_from_observed_ead(80.0) == pytest.approx(0.5)
    assert credit.lgd.EadCalculator.term_loan(100.0).leq_from_observed_ead(100.0) is None


def test_structural_spec_constructors_and_equality() -> None:
    inverse = credit.DynamicRecoverySpec.inverse_linear(0.4, 100.0)
    assert inverse.kind == "inverse_linear"
    assert inverse.recovery_at_notional(200.0) == pytest.approx(0.2)
    assert credit.DynamicRecoverySpec.floored_inverse(0.4, 100.0, 0.3).recovery_at_notional(400.0) == pytest.approx(0.3)
    assert credit.DynamicRecoverySpec.linear_decline(0.4, 100.0, 0.1, 0.2).kind == "linear_decline"
    assert credit.DynamicRecoverySpec.inverse_power(0.4, 100.0, 1.0) != inverse
    assert credit.DynamicRecoverySpec.constant(0.4) == credit.DynamicRecoverySpec.constant(0.4)

    tabular = credit.EndogenousHazardSpec.tabular([2.0, 6.0], [0.01, 0.05])
    assert tabular.kind == "tabular"
    assert tabular.hazard_at_leverage(4.0) == pytest.approx(0.03)
    assert credit.EndogenousHazardSpec.exponential(0.02, 4.0, 0.5).hazard_at_leverage(4.0) == pytest.approx(0.02)

    rule = credit.ToggleExerciseModel.stochastic("leverage", 100.0, 0.0)
    assert rule.kind == "stochastic"
    assert rule.should_pik_with_uniform(credit.CreditState(leverage=5.0), 0.5)
    threshold = credit.ToggleExerciseModel.threshold("leverage", 5.0, "above")
    assert threshold.should_pik_with_uniform(credit.CreditState(leverage=6.0), 0.0)
    assert not threshold.should_pik_with_uniform(credit.CreditState(leverage=4.0), 0.0)
    with pytest.raises(ValueError, match="hazard_rate, distance_to_default, leverage"):
        credit.ToggleExerciseModel.threshold("ebitda", 1.0, "above")

    assert credit.MertonBarrierType.terminal() == credit.MertonBarrierType.terminal()
    assert repr(credit.MertonBarrierType.terminal()) == "MertonBarrierType.terminal()"
    assert (
        repr(credit.MertonBarrierType.first_passage(0.02))
        == "MertonBarrierType.first_passage(barrier_growth_rate=0.02)"
    )
    model = credit.MertonModel(100.0, 0.25, 80.0, 0.05)
    assert model == credit.MertonModel.from_json(model.to_json())
    series = model.default_probabilities([1.0, 2.0])
    assert list(series.index) == ["1", "2"]
    assert series.iloc[0] == pytest.approx(model.default_probability(1.0))
    paths = model.simulate_paths(3, 4, 1.0, 7)
    assert paths.values_per_path == 5
    frame = paths.to_dataframe()
    assert list(frame.columns) == ["path", "time", "asset_value"]
    assert len(frame) == 15

    table = credit.RatingFactorTable.moodys_standard()
    assert table.get_factor("B") == credit.moodys_warf_factor("B") == 2720.0
    assert credit.RatingFactorTable.from_json(table.to_json()).default_factor == table.default_factor


def test_generator_matrix_exposes_extraction_diagnostics() -> None:
    scale = credit.migration.RatingScale.custom_with_default(["A", "D"], "D")
    direct = credit.migration.GeneratorMatrix(scale, [-0.1, 0.1, 0.0, 0.0])
    assert direct.regularization_l1 == 0.0
    assert direct.round_trip_error == 0.0

    transition = credit.migration.TransitionMatrix(scale, [0.9, 0.1, 0.0, 1.0], 1.0)
    extracted = credit.migration.GeneratorMatrix.from_transition_matrix(transition)
    assert math.isfinite(extracted.regularization_l1)
    assert math.isfinite(extracted.round_trip_error)
    assert extracted.regularization_l1 >= 0.0
    assert extracted.round_trip_error >= 0.0
