"""Behavioral coverage for models-owned credit-scoring bindings."""

import pickle

import pytest

from finstack_quant.models.credit import pd, scoring

ALTMAN_Z = {
    "working_capital_to_total_assets": 0.10,
    "retained_earnings_to_total_assets": 0.20,
    "ebit_to_total_assets": 0.15,
    "market_equity_to_total_liabilities": 1.50,
    "sales_to_total_assets": 1.80,
}
ZMIJEWSKI = {
    "net_income_to_total_assets": 0.05,
    "total_liabilities_to_total_assets": 0.5,
    "current_assets_to_current_liabilities": 1.5,
}
OHLSON = {
    "log_total_assets_adjusted": 8.0,
    "total_liabilities_to_total_assets": 0.4,
    "working_capital_to_total_assets": 0.2,
    "current_liabilities_to_current_assets": 0.5,
    "liabilities_exceed_assets": 0.0,
    "net_income_to_total_assets": 0.1,
    "funds_from_operations_to_total_liabilities": 0.3,
    "negative_net_income_two_years": 0.0,
    "net_income_change": 0.1,
}


def test_altman_score_does_not_publish_uncalibrated_pd() -> None:
    result = scoring.altman_z_score(scoring.AltmanZScoreInput(**ALTMAN_Z))
    assert isinstance(result, scoring.ScoringResult)
    assert result.score > 2.99
    assert result.zone == "safe"
    assert result.implied_pd is None
    assert result.model == "Altman Z-Score (1968)"


def test_input_classes_are_keyword_only_and_round_trip() -> None:
    ratios = scoring.ZmijewskiInput(**ZMIJEWSKI)
    assert ratios.current_assets_to_current_liabilities == 1.5
    assert scoring.ZmijewskiInput.from_json(ratios.to_json()) == ratios
    assert pickle.loads(pickle.dumps(ratios)) == ratios  # noqa: S301 - trusted in-process round trip
    assert repr(ratios).startswith("ZmijewskiInput(")
    with pytest.raises(TypeError, match="positional"):
        scoring.ZmijewskiInput(0.05, 0.5, 1.5)  # type: ignore[misc]
    with pytest.raises(ValueError, match="missing field"):
        scoring.ZmijewskiInput.from_json('{"net_income_to_total_assets": 0.05}')


def test_scoring_result_round_trips_and_exports() -> None:
    result = scoring.zmijewski_score(scoring.ZmijewskiInput(**ZMIJEWSKI))
    assert result.implied_pd is not None
    assert scoring.ScoringResult.from_json(result.to_json()) == result
    frame = result.to_dataframe()
    assert list(frame.columns) == ["model", "score", "zone", "implied_pd"]
    assert frame["zone"].iloc[0] == result.zone
    assert repr(result).startswith("ScoringResult(")


def test_master_scale_maps_scoring_results_with_implied_pd() -> None:
    scale = pd.MasterScale.default_calibration()
    probit = scoring.zmijewski_score(scoring.ZmijewskiInput(**ZMIJEWSKI))
    assert scale.map_score(probit).grade == scale.map_pd(probit.implied_pd).grade
    with pytest.raises(ValueError, match="implied PD"):
        scale.map_score(scoring.altman_z_score(scoring.AltmanZScoreInput(**ALTMAN_Z)))


def test_credit_score_report_grades_on_the_default_master_scale() -> None:
    report = scoring.credit_score_report({"model": "zmijewski_score", "input": ZMIJEWSKI})
    direct = scoring.zmijewski_score(scoring.ZmijewskiInput(**ZMIJEWSKI))
    assert report.result == direct
    assert report.grade == pd.MasterScale.default_calibration().map_score(direct)
    assert scoring.CreditScoreReport.from_json(report.to_json()) == report

    altman = scoring.credit_score_report({"model": "altman_z_score", "input": ALTMAN_Z})
    assert altman.result.implied_pd is None
    assert altman.grade is None


def test_credit_score_report_rejects_malformed_requests() -> None:
    with pytest.raises(ValueError, match="unknown variant `beaver`"):
        scoring.credit_score_report({"model": "beaver", "input": ZMIJEWSKI})
    with pytest.raises(ValueError, match="unknown field `extra`"):
        scoring.credit_score_report({"model": "zmijewski_score", "input": {**ZMIJEWSKI, "extra": 1.0}})
    with pytest.raises(ValueError, match="CreditScoreRequest"):
        scoring.credit_score_report({
            "model": "zmijewski_score",
            "input": {**ZMIJEWSKI, "net_income_to_total_assets": float("nan")},
        })


def test_pit_cycle_sign_matches_documented_convention() -> None:
    ttc_pd = 0.02
    downturn = pd.ttc_to_pit(ttc_pd, 0.12, -1.0)
    benign = pd.ttc_to_pit(ttc_pd, 0.12, 1.0)
    assert downturn > ttc_pd > benign


def test_ohlson_indicators_must_be_exactly_binary() -> None:
    scoring.ohlson_o_score(scoring.OhlsonOScoreInput(**OHLSON))

    with pytest.raises(ValueError, match="exactly 0 or 1"):
        scoring.ohlson_o_score(scoring.OhlsonOScoreInput(**{**OHLSON, "liabilities_exceed_assets": 0.5}))
    with pytest.raises(ValueError, match="exactly 0 or 1"):
        scoring.ohlson_o_score(scoring.OhlsonOScoreInput(**{**OHLSON, "negative_net_income_two_years": 2.0}))
