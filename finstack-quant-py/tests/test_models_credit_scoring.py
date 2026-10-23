"""Behavioral coverage for models-owned credit-scoring bindings."""

import pytest

from finstack_quant.models.credit import pd, scoring


def test_altman_score_does_not_publish_uncalibrated_pd() -> None:
    args = (0.10, 0.20, 0.15, 1.50, 1.80)

    result = scoring.altman_z_score(*args)
    assert isinstance(result, scoring.ScoringResult)
    assert result.score > 2.99
    assert result.zone == "safe"
    assert result.implied_pd is None
    assert result.model == "Altman Z-Score (1968)"


def test_scoring_result_round_trips_and_exports() -> None:
    result = scoring.zmijewski_score(0.05, 0.5, 1.5)
    assert result.implied_pd is not None
    assert scoring.ScoringResult.from_json(result.to_json()) == result
    frame = result.to_dataframe()
    assert list(frame.columns) == ["model", "score", "zone", "implied_pd"]
    assert frame["zone"].iloc[0] == result.zone
    assert repr(result).startswith("ScoringResult(")


def test_master_scale_maps_scoring_results_with_implied_pd() -> None:
    scale = pd.MasterScale.default_calibration()
    probit = scoring.zmijewski_score(0.05, 0.5, 1.5)
    assert scale.map_score(probit).grade == scale.map_pd(probit.implied_pd).grade
    with pytest.raises(ValueError, match="implied PD"):
        scale.map_score(scoring.altman_z_score(0.10, 0.20, 0.15, 1.50, 1.80))


def test_pit_cycle_sign_matches_documented_convention() -> None:
    ttc_pd = 0.02
    downturn = pd.ttc_to_pit(ttc_pd, 0.12, -1.0)
    benign = pd.ttc_to_pit(ttc_pd, 0.12, 1.0)
    assert downturn > ttc_pd > benign


def test_ohlson_indicators_must_be_exactly_binary() -> None:
    args = (8.0, 0.4, 0.2, 0.5, 0.0, 0.1, 0.3, 0.0, 0.1)
    scoring.ohlson_o_score(*args)

    with pytest.raises(ValueError, match="exactly 0 or 1"):
        scoring.ohlson_o_score(*args[:4], 0.5, *args[5:])
    with pytest.raises(ValueError, match="exactly 0 or 1"):
        scoring.ohlson_o_score(*args[:7], 2.0, args[8])
