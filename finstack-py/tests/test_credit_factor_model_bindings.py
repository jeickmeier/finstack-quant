"""Smoke tests for the credit factor hierarchy Python bindings.

Covers:
- CreditFactorModel.from_json / to_json round-trip.
- CreditCalibrator.calibrate on a small synthetic panel.
- decompose_levels: single-snapshot decomposition.
- decompose_period: period-over-period delta.
- FactorCovarianceForecast: covariance_at + idiosyncratic_vol.
"""

from __future__ import annotations

from datetime import date as dt_date
import json
import math

from finstack.factor_model.credit import (
    CreditCalibrator,
    CreditFactorModel,
    FactorCovarianceForecast,
    LevelsAtDate,
    PeriodDecomposition,
    decompose_levels,
    decompose_period,
)
import pytest

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

_N_MONTHS = 24


def _monthly_dates(n: int, end_year: int, end_month: int, end_day: int) -> list[str]:
    """Generate n ISO-date strings stepping backwards ~30 days from end."""
    from datetime import timedelta

    end = dt_date(end_year, end_month, end_day)
    dates = []
    current = end
    for _ in range(n):
        dates.append(current.isoformat())
        current = current - timedelta(days=30)
    dates.reverse()
    return dates


def _fixture_inputs() -> dict:
    """Synthetic 24-month panel with 6 issuers.

    6 issuers: 3 IG × (EU, NA, APAC) and
    3 HY × (EU, NA, APAC).  Structure mirrors the Rust integration test.
    """
    n = _N_MONTHS
    as_of_str = "2024-03-31"
    dates = _monthly_dates(n, 2024, 3, 31)

    generic_values = [100.0 + 0.5 * math.sin(i) for i in range(n)]

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
    asof_spreads: dict[str, float] = {}

    for idx, (issuer_id, rating, region) in enumerate(issuer_specs):
        base = 100.0 + idx * 25.0
        beta_pc = 0.7 + 0.05 * idx
        series: list[float | None] = [
            base + beta_pc * (generic_values[i] - 100.0) + 0.1 * math.cos(idx + i * 0.5) for i in range(n)
        ]
        spreads[issuer_id] = series
        tags[issuer_id] = {"rating": rating, "region": region}
        asof_spreads[issuer_id] = float(series[-1])  # type: ignore[arg-type]

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
        "asof_spreads": asof_spreads,
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
        "annualization_factor": 12.0,
    }


def _calibrate() -> CreditFactorModel:
    config = _calibration_config()
    cal = CreditCalibrator(json.dumps(config))
    inputs = _fixture_inputs()
    return cal.calibrate(json.dumps(inputs))


# ---------------------------------------------------------------------------
# Minimal golden JSON — just enough for from_json / to_json tests.
# We use a trivial model with no issuers and no factors.
# ---------------------------------------------------------------------------

_MINIMAL_MODEL_JSON = json.dumps({
    "schema_version": "finstack.credit_factor_model/1",
    "as_of": "2024-03-29",
    "calibration_window": {
        "start": "2022-03-29",
        "end": "2024-03-29",
    },
    "policy": "globally_off",
    "generic_factor": {"name": "CDX IG", "series_id": "cdx.ig.5y"},
    "hierarchy": {"levels": ["rating", "region"]},
    "config": {
        "factors": [],
        "covariance": {"n": 0, "factor_ids": [], "data": []},
        "matching": {"MappingTable": []},
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


# ---------------------------------------------------------------------------
# T1 — CreditFactorModel JSON round-trip
# ---------------------------------------------------------------------------


def test_credit_factor_model_from_json_minimal() -> None:
    model = CreditFactorModel.from_json(_MINIMAL_MODEL_JSON)
    assert model.schema_version == "finstack.credit_factor_model/1"
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
    assert parsed["schema_version"] == "finstack.credit_factor_model/1"
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


# ---------------------------------------------------------------------------
# T2 — CreditCalibrator.calibrate produces a valid artifact
# ---------------------------------------------------------------------------


def test_calibrator_produces_valid_artifact() -> None:
    model = _calibrate()
    assert isinstance(model, CreditFactorModel)
    assert model.schema_version == "finstack.credit_factor_model/1"
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
    assert parsed["schema_version"] == "finstack.credit_factor_model/1"
    assert len(parsed["issuer_betas"]) == 6


def test_calibrated_model_round_trips_json() -> None:
    """from_json(to_json(model)) should preserve key structural fields."""
    model = _calibrate()
    json1 = model.to_json()
    model2 = CreditFactorModel.from_json(json1)
    # Schema version, as_of, issuer count, and factor count must be preserved.
    assert model2.schema_version == model.schema_version
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


@pytest.mark.parametrize(
    "vol_model",
    [
        "garch",
        "egarch",
        {"ewma": {"lambda": 0.94}},
    ],
)
def test_calibrator_reserved_vol_models_raise_cleanly(vol_model: object) -> None:
    config = _calibration_config()
    config["vol_model"] = vol_model
    cal = CreditCalibrator(json.dumps(config))

    with pytest.raises(ValueError, match="Sample only"):
        cal.calibrate(json.dumps(_fixture_inputs()))


def test_calibrator_bad_inputs_raises() -> None:
    config = _calibration_config()
    cal = CreditCalibrator(json.dumps(config))
    with pytest.raises(ValueError, match=r"missing field|unknown field|invalid"):
        cal.calibrate('{"not_inputs": true}')


# ---------------------------------------------------------------------------
# T3 — decompose_levels
# ---------------------------------------------------------------------------


def _simple_decompose_model_and_spreads() -> tuple[CreditFactorModel, dict[str, float]]:
    """Return a calibrated model + a spread map for decomposition tests."""
    model = _calibrate()
    # Use the asof_spreads from the fixture as observed spreads.
    inputs = _fixture_inputs()
    observed: dict[str, float] = inputs["asof_spreads"]
    return model, observed


def test_decompose_levels_runs_without_error() -> None:
    model, observed = _simple_decompose_model_and_spreads()
    spreads_json = json.dumps(observed)
    snap = decompose_levels(model, spreads_json, 100.0, "2024-03-31")
    assert isinstance(snap, LevelsAtDate)
    assert snap.date == "2024-03-31"
    assert snap.generic == pytest.approx(100.0)


def test_decompose_levels_n_levels_matches_model() -> None:
    model, observed = _simple_decompose_model_and_spreads()
    snap = decompose_levels(model, json.dumps(observed), 100.0, "2024-03-31")
    assert snap.n_levels == model.n_levels


def test_decompose_levels_level_values_are_floats() -> None:
    model, observed = _simple_decompose_model_and_spreads()
    snap = decompose_levels(model, json.dumps(observed), 100.0, "2024-03-31")
    for k in range(snap.n_levels):
        vals = snap.level_values(k)
        assert isinstance(vals, dict)
        for v in vals.values():
            assert isinstance(v, float)


def test_decompose_levels_adder_covers_all_issuers() -> None:
    model, observed = _simple_decompose_model_and_spreads()
    snap = decompose_levels(model, json.dumps(observed), 100.0, "2024-03-31")
    adder = snap.adder()
    assert set(adder.keys()) == set(observed.keys())


def test_decompose_levels_level_index_out_of_range_raises() -> None:
    model, observed = _simple_decompose_model_and_spreads()
    snap = decompose_levels(model, json.dumps(observed), 100.0, "2024-03-31")
    with pytest.raises(ValueError, match=r"out of range"):
        snap.level_values(999)


def test_decompose_levels_unknown_issuer_raises() -> None:
    model, _ = _simple_decompose_model_and_spreads()
    # Supply a spread for an issuer not in the model and not in runtime_tags.
    bad = json.dumps({"UNKNOWN-ISSUER": 200.0})
    with pytest.raises(ValueError, match="UNKNOWN-ISSUER"):
        decompose_levels(model, bad, 100.0, "2024-03-31")


def test_decompose_levels_runtime_tags_resolves_unknown_issuer() -> None:
    model, observed = _simple_decompose_model_and_spreads()
    extra = dict(observed)
    extra["RUNTIME-ISSUER"] = 150.0
    runtime_tags = {"RUNTIME-ISSUER": {"rating": "IG", "region": "EU"}}
    snap = decompose_levels(
        model,
        json.dumps(extra),
        100.0,
        "2024-03-31",
        json.dumps(runtime_tags),
    )
    adder = snap.adder()
    assert "RUNTIME-ISSUER" in adder


# ---------------------------------------------------------------------------
# T4 — decompose_period
# ---------------------------------------------------------------------------


def test_decompose_period_runs_without_error() -> None:
    model, observed = _simple_decompose_model_and_spreads()
    spreads_json = json.dumps(observed)
    snap_t0 = decompose_levels(model, spreads_json, 100.0, "2024-02-29")
    snap_t1 = decompose_levels(model, spreads_json, 101.5, "2024-03-31")
    period = decompose_period(snap_t0, snap_t1)
    assert isinstance(period, PeriodDecomposition)
    assert period.from_date == "2024-02-29"
    assert period.to_date == "2024-03-31"
    assert period.d_generic == pytest.approx(1.5)


def test_decompose_period_level_deltas() -> None:
    model, observed = _simple_decompose_model_and_spreads()
    spreads_json = json.dumps(observed)
    snap_t0 = decompose_levels(model, spreads_json, 100.0, "2024-02-29")
    snap_t1 = decompose_levels(model, spreads_json, 100.0, "2024-03-31")
    period = decompose_period(snap_t0, snap_t1)
    # Same spreads + same generic → all deltas should be zero.
    for k in range(period.n_levels):
        deltas = period.level_deltas(k)
        for v in deltas.values():
            assert abs(v) < 1e-9, f"level {k} delta should be zero for unchanged spreads"


def test_decompose_period_date_order_error() -> None:
    model, observed = _simple_decompose_model_and_spreads()
    spreads_json = json.dumps(observed)
    snap_t0 = decompose_levels(model, spreads_json, 100.0, "2024-01-31")
    snap_t1 = decompose_levels(model, spreads_json, 100.0, "2024-03-31")
    # Reverse order: from > to should raise.
    with pytest.raises(ValueError, match=r"from.*to|date"):
        decompose_period(snap_t1, snap_t0)


# ---------------------------------------------------------------------------
# T5 — FactorCovarianceForecast
# ---------------------------------------------------------------------------


def test_factor_covariance_forecast_one_step_returns_valid_json() -> None:
    model = _calibrate()
    fcf = FactorCovarianceForecast(model)
    cov_json = fcf.covariance_at("one_step")
    parsed = json.loads(cov_json)
    assert "factor_ids" in parsed
    assert "data" in parsed
    n = len(parsed["factor_ids"])
    assert len(parsed["data"]) == n * n


def test_factor_covariance_forecast_unconditional_matches_one_step() -> None:
    """Under the Sample vol model, one_step and unconditional must agree."""
    model = _calibrate()
    fcf = FactorCovarianceForecast(model)
    cov_one = json.loads(fcf.covariance_at("one_step"))
    cov_unc = json.loads(fcf.covariance_at("unconditional"))
    assert cov_one["factor_ids"] == cov_unc["factor_ids"]
    for a, b in zip(cov_one["data"], cov_unc["data"], strict=True):
        assert abs(a - b) < 1e-12


def test_factor_covariance_forecast_n_steps_scales_variance() -> None:
    """NSteps(4) covariance should be 4× the OneStep covariance."""
    model = _calibrate()
    fcf = FactorCovarianceForecast(model)
    cov_one = json.loads(fcf.covariance_at("one_step"))
    cov_four = json.loads(fcf.covariance_at('{"n_steps": 4}'))
    for a, b in zip(cov_one["data"], cov_four["data"], strict=True):
        assert abs(4.0 * a - b) < 1e-10, f"expected 4·{a} ≈ {b}"


def test_factor_covariance_forecast_n_steps_zero_is_zero() -> None:
    model = _calibrate()
    fcf = FactorCovarianceForecast(model)
    cov = json.loads(fcf.covariance_at('{"n_steps": 0}'))
    for v in cov["data"]:
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
    config_json = fcf.factor_model_at("one_step", '"variance"')
    parsed = json.loads(config_json)
    assert "factors" in parsed
    assert "covariance" in parsed


def test_factor_covariance_forecast_repr_is_informative() -> None:
    model = _calibrate()
    fcf = FactorCovarianceForecast(model)
    r = repr(fcf)
    assert "FactorCovarianceForecast" in r
