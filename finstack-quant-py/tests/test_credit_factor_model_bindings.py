"""Smoke tests for the credit factor hierarchy Python bindings.

Covers:
- CreditFactorModel.from_json / to_json round-trip.
- CreditCalibrator.calibrate on a small synthetic panel.
- decompose_levels: single-snapshot decomposition.
- decompose_period: period-over-period delta.
- FactorCovarianceForecast: covariance_at + idiosyncratic_vol.
"""

from __future__ import annotations

from calendar import monthrange
from datetime import date as dt_date
import json
import math

import pytest

from finstack_quant.models.factor.credit import (
    CreditCalibrator,
    CreditFactorModel,
    FactorCovarianceForecast,
    FactorCovarianceMatrix,
    FactorModelConfig,
    LevelsAtDate,
    PeriodDecomposition,
    VolHorizon,
    decompose_levels,
    decompose_period,
)

_N_MONTHS = 24


def _add_months_eom(origin: dt_date, months: int) -> dt_date:
    """Step `months` from `origin`, preserving end-of-month when origin is EOM."""
    year = origin.year + (origin.month - 1 + months) // 12
    month = (origin.month - 1 + months) % 12 + 1
    last = monthrange(year, month)[1]
    origin_eom = origin.day == monthrange(origin.year, origin.month)[1]
    day = last if origin_eom else min(origin.day, last)
    return dt_date(year, month, day)


def _monthly_dates(n: int, end_year: int, end_month: int, end_day: int) -> list[str]:
    """Generate n regular month-end-aware dates ending at the given day."""
    end = dt_date(end_year, end_month, end_day)
    origin = _add_months_eom(end, -(n - 1))
    return [_add_months_eom(origin, i).isoformat() for i in range(n)]


def _fixture_inputs() -> dict:
    """Synthetic 24-month panel with 6 issuers.

    6 issuers: 3 IG × (EU, NA, APAC) and
    3 HY × (EU, NA, APAC).  Structure mirrors the Rust integration test.
    """
    n = _N_MONTHS
    as_of_str = "2024-03-31"
    dates = _monthly_dates(n, 2024, 3, 31)

    generic_values = [0.0100 + 0.00005 * math.sin(i) for i in range(n)]

    issuer_specs = [
        ("ISSUER-A", "IG", "EU"),
        ("ISSUER-B", "IG", "NA"),
        ("ISSUER-C", "IG", "APAC"),
        ("ISSUER-D", "HY", "EU"),
        ("ISSUER-E", "HY", "NA"),
        ("ISSUER-F", "HY", "APAC"),
    ]

    spreads: dict[str, list[float | None]] = {}
    tags: dict[str, dict[str, str]] = {}
    as_of_spreads: dict[str, float] = {}

    for idx, (issuer_id, rating, region) in enumerate(issuer_specs):
        base = 0.0100 + idx * 0.0025
        beta_pc = 0.7 + 0.05 * idx
        series: list[float | None] = [
            base + beta_pc * (generic_values[i] - 0.0100) + 0.00001 * math.cos(idx + i * 0.5) for i in range(n)
        ]
        spreads[issuer_id] = series
        tags[issuer_id] = {"rating": rating, "region": region}
        as_of_spreads[issuer_id] = float(series[-1])  # type: ignore[arg-type]

    return {
        "history_panel": {
            "dates": dates,
            "spreads": spreads,
        },
        "issuer_tags": {
            "tags": tags,
        },
        "generic_factor": {
            "spec": {"name": "CDX IG 5Y", "series_id": "cdx.ig.5y"},
            "values": generic_values,
        },
        "as_of": as_of_str,
        "as_of_spreads": as_of_spreads,
        "idiosyncratic_overrides": {},
    }


def _calibration_config() -> dict:
    return {
        "policy": "globally_off",
        "hierarchy": {
            "levels": ["rating", "region"],
        },
        "min_bucket_size_per_level": {"per_level": [3, 3]},
        "vol_model": "sample",
        "covariance_strategy": "diagonal",
        "beta_shrinkage": "none",
        "use_returns_or_levels": "returns",
        "panel_frequency": "monthly",
        "bucket_weighting": "equal",
    }


def _calibrate() -> CreditFactorModel:
    config = _calibration_config()
    cal = CreditCalibrator(json.dumps(config))
    inputs = _fixture_inputs()
    return cal.calibrate(json.dumps(inputs))


# Minimal golden JSON — just enough for from_json / to_json tests.
# We use a trivial model with no issuers and no factors.

_MINIMAL_MODEL_JSON = json.dumps({
    "schema": "finstack_quant.credit_factor_model/1",
    "as_of": "2024-03-29",
    "calibration_window": {
        "start": "2022-03-29",
        "end": "2024-03-29",
    },
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


# T1 — CreditFactorModel JSON round-trip


def test_credit_factor_model_from_json_minimal() -> None:
    model = CreditFactorModel.from_json(_MINIMAL_MODEL_JSON)
    assert model.schema == "finstack_quant.credit_factor_model/1"
    assert model.as_of == "2024-03-29"
    assert model.n_levels == 2
    assert model.n_issuers == 0
    assert model.n_factors == 0
    assert model.level_names() == ["Rating", "Region"]
    assert model.issuer_ids() == []
    assert model.factor_ids() == []


def test_credit_factor_model_to_json_round_trips() -> None:
    model = CreditFactorModel.from_json(_MINIMAL_MODEL_JSON)
    out = model.to_json()
    parsed = json.loads(out)
    assert parsed["schema"] == "finstack_quant.credit_factor_model/1"
    assert parsed["as_of"] == "2024-03-29"
    # Structural fields preserved
    assert len(parsed["issuer_betas"]) == 0


def test_credit_factor_model_repr_is_informative() -> None:
    model = CreditFactorModel.from_json(_MINIMAL_MODEL_JSON)
    r = repr(model)
    assert "CreditFactorModel" in r
    assert "2024-03-29" in r


def test_credit_factor_model_bad_json_raises() -> None:
    with pytest.raises(ValueError, match=r"missing field|unknown field|invalid"):
        CreditFactorModel.from_json('{"not": "a model"}')


# T2 — CreditCalibrator.calibrate produces a valid artifact


def test_calibrator_produces_valid_artifact() -> None:
    model = _calibrate()
    assert isinstance(model, CreditFactorModel)
    assert model.schema == "finstack_quant.credit_factor_model/1"
    # At least the generic factor should exist.
    assert model.n_factors >= 1
    # The panel has 6 issuers.
    assert model.n_issuers == 6


def test_calibrated_model_level_names() -> None:
    model = _calibrate()
    names = model.level_names()
    assert "Rating" in names
    assert "Region" in names


def test_calibrated_model_serializes_to_json() -> None:
    model = _calibrate()
    out = model.to_json()
    parsed = json.loads(out)
    assert parsed["schema"] == "finstack_quant.credit_factor_model/1"
    assert len(parsed["issuer_betas"]) == 6


def test_calibrated_model_round_trips_json() -> None:
    """from_json(to_json(model)) should preserve key structural fields."""
    model = _calibrate()
    json1 = model.to_json()
    model2 = CreditFactorModel.from_json(json1)
    # Schema version, as_of, issuer count, and factor count must be preserved.
    assert model2.schema == model.schema
    assert model2.as_of == model.as_of
    assert model2.n_issuers == model.n_issuers
    assert model2.n_factors == model.n_factors
    assert model2.n_levels == model.n_levels
    assert model2.issuer_ids() == model.issuer_ids()
    # Double round-trip: from_json(to_json(model2)) should also succeed.
    json2 = model2.to_json()
    model3 = CreditFactorModel.from_json(json2)
    assert model3.n_issuers == model.n_issuers


def test_calibrator_bad_config_raises() -> None:
    with pytest.raises(ValueError, match=r"missing field|unknown field|invalid"):
        CreditCalibrator('{"completely": "wrong"}')


@pytest.mark.parametrize("vol_model", ["garch", "egarch"])
def test_calibrator_removed_vol_models_rejected_at_parse(vol_model: str) -> None:
    """Garch/Egarch were removed in 0.6: the config no longer parses."""
    config = _calibration_config()
    config["vol_model"] = vol_model
    with pytest.raises(ValueError, match="unknown variant"):
        CreditCalibrator(json.dumps(config))


def test_calibrator_ewma_vol_model_calibrates() -> None:
    """EWMA calibrates and persists the estimator in vol_state JSON."""
    config = _calibration_config()
    config["vol_model"] = {"ewma": {"lambda": 0.94}}
    cal = CreditCalibrator(json.dumps(config))
    model = cal.calibrate(json.dumps(_fixture_inputs()))
    assert model.n_factors >= 1
    artifact = json.loads(model.to_json())
    factor_models = artifact["vol_state"]["factors"]
    assert factor_models, "vol_state.factors must not be empty"
    for entry in factor_models.values():
        assert "ewma" in entry, f"expected ewma vol model, got {entry}"
        assert abs(entry["ewma"]["lambda"] - 0.94) < 1e-12
        assert entry["ewma"]["variance"] >= 0.0


def test_calibrator_ledoit_wolf_strategy_calibrates() -> None:
    """Ledoit-Wolf covariance strategy is accepted end-to-end via JSON."""
    config = _calibration_config()
    config["covariance_strategy"] = "ledoit_wolf"
    cal = CreditCalibrator(json.dumps(config))
    model = cal.calibrate(json.dumps(_fixture_inputs()))
    assert model.n_factors >= 1


def test_calibrator_ewma_bad_lambda_rejected() -> None:
    """Lambda outside (0, 1) fails calibration validation."""
    config = _calibration_config()
    config["vol_model"] = {"ewma": {"lambda": 1.0}}
    cal = CreditCalibrator(json.dumps(config))
    with pytest.raises(ValueError, match="ewma lambda"):
        cal.calibrate(json.dumps(_fixture_inputs()))


def test_calibrator_bad_inputs_raises() -> None:
    config = _calibration_config()
    cal = CreditCalibrator(json.dumps(config))
    with pytest.raises(ValueError, match=r"missing field|unknown field|invalid"):
        cal.calibrate('{"not_inputs": true}')


# T3 — decompose_levels


def _simple_decompose_model_and_spreads() -> tuple[CreditFactorModel, dict[str, float]]:
    """Return a calibrated model + a spread map for decomposition tests."""
    model = _calibrate()
    # Use the as_of_spreads from the fixture as observed spreads.
    inputs = _fixture_inputs()
    observed: dict[str, float] = inputs["as_of_spreads"]
    return model, observed


def test_decompose_levels_runs_without_error() -> None:
    model, observed = _simple_decompose_model_and_spreads()
    spreads_json = json.dumps(observed)
    snap = decompose_levels(model, spreads_json, 0.0100, "2024-03-31")
    assert isinstance(snap, LevelsAtDate)
    assert snap.date == "2024-03-31"
    assert snap.generic == pytest.approx(100.0)


def test_decompose_levels_n_levels_matches_model() -> None:
    model, observed = _simple_decompose_model_and_spreads()
    snap = decompose_levels(model, json.dumps(observed), 0.0100, "2024-03-31")
    assert snap.n_levels == model.n_levels


def test_decompose_levels_level_values_are_floats() -> None:
    model, observed = _simple_decompose_model_and_spreads()
    snap = decompose_levels(model, json.dumps(observed), 0.0100, "2024-03-31")
    for k in range(snap.n_levels):
        vals = snap.level_values(k)
        assert isinstance(vals, dict)
        for v in vals.values():
            assert isinstance(v, float)


def test_decompose_levels_adder_covers_all_issuers() -> None:
    model, observed = _simple_decompose_model_and_spreads()
    snap = decompose_levels(model, json.dumps(observed), 0.0100, "2024-03-31")
    adder = snap.adder()
    assert set(adder.keys()) == set(observed.keys())


def test_decompose_levels_level_index_out_of_range_raises() -> None:
    model, observed = _simple_decompose_model_and_spreads()
    snap = decompose_levels(model, json.dumps(observed), 0.0100, "2024-03-31")
    with pytest.raises(ValueError, match=r"out of range"):
        snap.level_values(999)


def test_decompose_levels_unknown_issuer_raises() -> None:
    model, _ = _simple_decompose_model_and_spreads()
    # Supply a spread for an issuer not in the model and not in runtime_tags.
    bad = {"UNKNOWN-ISSUER": 0.0200}
    with pytest.raises(KeyError, match="UNKNOWN-ISSUER"):
        decompose_levels(model, bad, 0.0100, "2024-03-31")


def test_decompose_levels_runtime_tags_resolves_unknown_issuer() -> None:
    model, observed = _simple_decompose_model_and_spreads()
    extra = dict(observed)
    extra["RUNTIME-ISSUER"] = 0.0150
    runtime_tags = {"RUNTIME-ISSUER": {"rating": "IG", "region": "EU"}}
    snap = decompose_levels(
        model,
        json.dumps(extra),
        0.0100,
        "2024-03-31",
        json.dumps(runtime_tags),
    )
    adder = snap.adder()
    assert "RUNTIME-ISSUER" in adder


# T4 — decompose_period


def test_decompose_period_runs_without_error() -> None:
    model, observed = _simple_decompose_model_and_spreads()
    spreads_json = json.dumps(observed)
    snap_t0 = decompose_levels(model, spreads_json, 0.0100, "2024-02-29")
    snap_t1 = decompose_levels(model, spreads_json, 0.01015, "2024-03-31")
    period = decompose_period(snap_t0, snap_t1)
    assert isinstance(period, PeriodDecomposition)
    assert period.from_date == "2024-02-29"
    assert period.to_date == "2024-03-31"
    assert period.d_generic == pytest.approx(1.5)


def test_decompose_period_level_deltas() -> None:
    model, observed = _simple_decompose_model_and_spreads()
    spreads_json = json.dumps(observed)
    snap_t0 = decompose_levels(model, spreads_json, 0.0100, "2024-02-29")
    snap_t1 = decompose_levels(model, spreads_json, 0.0100, "2024-03-31")
    period = decompose_period(snap_t0, snap_t1)
    # Same spreads + same generic → all deltas should be zero.
    for k in range(period.n_levels):
        deltas = period.level_deltas(k)
        for v in deltas.values():
            assert abs(v) < 1e-9, f"level {k} delta should be zero for unchanged spreads"


def test_decompose_period_date_order_error() -> None:
    model, observed = _simple_decompose_model_and_spreads()
    spreads_json = json.dumps(observed)
    snap_t0 = decompose_levels(model, spreads_json, 0.0100, "2024-01-31")
    snap_t1 = decompose_levels(model, spreads_json, 0.0100, "2024-03-31")
    # Reverse order: from > to should raise.
    with pytest.raises(ValueError, match=r"from.*to|date"):
        decompose_period(snap_t1, snap_t0)


# T5 — FactorCovarianceForecast


def test_factor_covariance_forecast_one_step_returns_typed_matrix() -> None:
    model = _calibrate()
    fcf = FactorCovarianceForecast(model)
    covariance = fcf.covariance_at("one_step")
    assert isinstance(covariance, FactorCovarianceMatrix)
    assert len(covariance.data) == covariance.n_factors * covariance.n_factors
    assert covariance.variance(covariance.factor_ids[0]) == pytest.approx(covariance.data[0])
    assert FactorCovarianceMatrix.from_json(covariance.to_json()).factor_ids == covariance.factor_ids


def test_factor_covariance_forecast_unconditional_matches_one_step() -> None:
    """Under the Sample vol model, one_step and unconditional must agree."""
    model = _calibrate()
    fcf = FactorCovarianceForecast(model)
    cov_one = fcf.covariance_at("one_step")
    cov_unc = fcf.covariance_at("unconditional")
    assert cov_one.factor_ids == cov_unc.factor_ids
    for a, b in zip(cov_one.data, cov_unc.data, strict=True):
        assert abs(a - b) < 1e-12


def test_factor_covariance_forecast_n_steps_scales_variance() -> None:
    """NSteps(4) covariance should be 4× the OneStep covariance."""
    model = _calibrate()
    fcf = FactorCovarianceForecast(model)
    cov_one = fcf.covariance_at("one_step")
    cov_four = fcf.covariance_at('{"n_steps": 4}')
    for a, b in zip(cov_one.data, cov_four.data, strict=True):
        assert abs(4.0 * a - b) < 1e-10, f"expected 4·{a} ≈ {b}"


def test_factor_covariance_forecast_n_steps_zero_is_zero() -> None:
    model = _calibrate()
    fcf = FactorCovarianceForecast(model)
    cov = fcf.covariance_at('{"n_steps": 0}')
    for v in cov.data:
        assert abs(v) < 1e-12


def test_factor_covariance_forecast_idiosyncratic_vol_unknown_raises() -> None:
    model = _calibrate()
    fcf = FactorCovarianceForecast(model)
    with pytest.raises(ValueError, match="MISSING-ISSUER"):
        fcf.idiosyncratic_vol("MISSING-ISSUER", "one_step")


def test_factor_covariance_forecast_idiosyncratic_vol_known_issuers() -> None:
    model = _calibrate()
    fcf = FactorCovarianceForecast(model)
    for issuer_id in model.issuer_ids():
        vol = fcf.idiosyncratic_vol(issuer_id, "one_step")
        assert vol >= 0.0, f"idio vol must be non-negative for {issuer_id}"


def test_factor_covariance_forecast_invalid_horizon_raises() -> None:
    model = _calibrate()
    fcf = FactorCovarianceForecast(model)
    with pytest.raises(ValueError, match=r"horizon|invalid"):
        fcf.covariance_at("not_a_horizon")


def test_factor_covariance_forecast_factor_model_at_runs() -> None:
    model = _calibrate()
    fcf = FactorCovarianceForecast(model)
    config = fcf.factor_model_at("one_step", "variance")
    assert isinstance(config, FactorModelConfig)
    assert config.factor_ids == config.covariance.factor_ids
    assert config.risk_measure == "variance"
    var_config = fcf.factor_model_at(VolHorizon.n_steps(2), {"var": {"confidence": 0.99}})
    assert var_config.risk_measure == {"var": {"confidence": 0.99}}
    assert fcf.factor_model_at("one_step").risk_measure == "variance"
    assert FactorModelConfig.from_json(config.to_json()).factor_ids == config.factor_ids


def test_factor_covariance_forecast_repr_is_informative() -> None:
    model = _calibrate()
    fcf = FactorCovarianceForecast(model)
    r = repr(fcf)
    assert "FactorCovarianceForecast" in r


def test_idiosyncratic_override_is_decimal_spread_per_sqrt_year() -> None:
    inputs = _fixture_inputs()
    inputs["idiosyncratic_overrides"] = {"ISSUER-A": 0.001}
    model = CreditCalibrator(_calibration_config()).calibrate(inputs)
    row = model.to_dataframe().set_index("issuer_id").loc["ISSUER-A"]
    assert row["adder_vol_annualized"] == pytest.approx(10.0)
    assert row["adder_vol_source"] == "caller_supplied"
