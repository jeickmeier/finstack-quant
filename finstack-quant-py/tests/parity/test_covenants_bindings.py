"""Focused parity checks for the covenant binding slice."""

from __future__ import annotations

import datetime
import json
import math
import pickle

import pandas as pd
import pytest

from finstack_quant import covenants


def _engine_json(spec: dict[str, object]) -> str:
    return json.dumps({"specs": [spec], "breach_history": [], "windows": [], "waivers": []})


def test_covenant_template_roundtrip_and_evaluate() -> None:
    specs_json = covenants.lbo_standard_json(5.0, 1.5, 1.2, 10_000_000.0)
    specs = json.loads(specs_json)

    canonical_engine = covenants.validate_covenant_engine_json(_engine_json(specs[0]))
    reports = covenants.evaluate_engine(
        canonical_engine,
        json.dumps({"debt_to_ebitda": 4.0}),
        "2026-03-31",
    )

    report = reports["max_debt_ebitda"]
    assert isinstance(report, covenants.CovenantReport)
    assert report.passed is True
    assert report.actual_value == pytest.approx(4.0)
    assert report.threshold == pytest.approx(5.0)
    assert report.headroom is not None
    assert report.headroom > 0.0
    assert "numeric_mode" in report.meta


def test_evaluate_engine_accepts_dict_metrics_and_specs_only_document() -> None:
    """`{"specs": [...]}` is a complete engine document and metrics may be a dict."""
    specs = json.loads(covenants.lbo_standard_json(5.0, 1.5, 1.2, 10_000_000.0))
    engine = json.dumps({"specs": specs})
    metrics = {
        "debt_to_ebitda": 4.0,
        "interest_coverage": 3.0,
        "fixed_charge_coverage": 1.5,
        "capex": 5_000_000.0,
    }
    reports = covenants.evaluate_engine(engine, metrics, "2026-03-31")
    assert list(reports) == ["max_debt_ebitda", "min_interest_coverage", "min_fcc", "max_capex"]
    assert covenants.evaluate_engine(engine, json.dumps(metrics), "2026-03-31") == reports
    assert json.loads(covenants.validate_covenant_engine_json(engine))["waivers"] == []


def test_evaluate_engine_missing_metric_raises_key_error() -> None:
    specs = json.loads(covenants.cov_lite_json(7.0, 4.5))
    with pytest.raises(KeyError, match="senior_leverage"):
        covenants.evaluate_engine(json.dumps({"specs": specs}), {"total_leverage": 5.0}, "2026-03-31")


def test_evaluate_engine_report_dataframe_and_pickle() -> None:
    spec = json.loads(covenants.lbo_standard_json(5.0, 1.5, 1.2, 10_000_000.0))[0]
    report = covenants.evaluate_engine(
        _engine_json(spec),
        json.dumps({"debt_to_ebitda": 5.5}),
        "2026-03-31",
    )["max_debt_ebitda"]

    assert report.passed is False

    frame = report.to_dataframe()
    assert list(frame.columns) == [
        "covenant_type",
        "covenant_id",
        "passed",
        "actual_value",
        "threshold",
        "headroom",
        "details",
    ]
    assert len(frame) == 1
    assert not bool(frame["passed"].iloc[0])

    revived = pickle.loads(pickle.dumps(report))  # noqa: S301
    assert revived.to_json() == report.to_json()
    assert revived == report
    assert repr(report).startswith('CovenantReport(covenant_id="max_debt_ebitda"')
    assert "passed=False" in repr(report)
    assert "Some(" not in repr(report)


def test_covenant_report_json_roundtrip() -> None:
    report = {
        "covenant_type": "Debt/EBITDA <= 5.00x",
        "covenant_id": "max_debt_to_ebitda",
        "passed": False,
        "actual_value": 5.5,
        "threshold": 5.0,
        "details": "Exceeded",
        "headroom": -0.1,
    }

    canonical = json.loads(covenants.validate_covenant_report_json(json.dumps(report)))

    # Canonicalization stamps the policy envelope onto the report; every field
    # the caller supplied must survive it byte-for-byte.
    meta = canonical.pop("meta")
    assert canonical == report
    assert meta["numeric_mode"] == "f64"
    assert meta["rounding"]["mode"] == "bankers"

    typed = covenants.CovenantReport.from_json(json.dumps(report))
    assert typed.covenant_type == "Debt/EBITDA <= 5.00x"
    assert typed.covenant_id == "max_debt_to_ebitda"
    assert typed.passed is False
    assert typed.actual_value == pytest.approx(5.5)
    assert typed.threshold == pytest.approx(5.0)
    assert typed.details == "Exceeded"
    assert typed.headroom == pytest.approx(-0.1)


def test_covenant_engine_rejects_unknown_fields() -> None:
    engine = {
        "specs": [],
        "breach_history": [],
        "windows": [],
        "waviers": [],
    }

    with pytest.raises(ValueError, match="unknown field"):
        covenants.validate_covenant_engine_json(json.dumps(engine))


def test_threshold_schedule_validation_maps_to_value_error() -> None:
    spec = json.loads(covenants.lbo_standard_json(5.0, 1.5, 1.2, 10_000_000.0))[0]
    spec["threshold_schedule"] = [
        ["2025-01-01", 5.0],
        ["2025-01-01", 4.5],
    ]

    with pytest.raises(ValueError, match="duplicate date"):
        covenants.validate_covenant_spec_json(json.dumps(spec))


@pytest.mark.parametrize("bad", [math.nan, math.inf, -1.0])
def test_templates_reject_non_finite_and_negative_thresholds(bad: float) -> None:
    with pytest.raises(ValueError, match="finite and non-negative"):
        covenants.lbo_standard_json(bad, 2.0, 1.1, 50.0)
    with pytest.raises(ValueError, match="finite and non-negative"):
        covenants.cov_lite(7.0, bad)


def test_typed_templates_match_json_twins() -> None:
    typed = covenants.lbo_standard(6.0, 2.0, 1.1, 50.0)
    raw = json.loads(covenants.lbo_standard_json(6.0, 2.0, 1.1, 50.0))
    assert [json.loads(s.to_json()) for s in typed] == raw
    assert [s.covenant.label for s in covenants.project_finance(1.2, 1.1, 10.0, 7.0)] == [
        "min_dscr_default",
        "min_dscr_lockup",
        "min_liquidity",
        "max_net_debt_ebitda",
    ]
    assert [s.metric_id for s in covenants.real_estate(1.25, 0.08, 0.75)] == ["dscr", "debt_yield", "ltv"]
    assert [s.covenant.scope for s in covenants.cov_lite(7.0, 4.5)] == ["incurrence"] * 3


def test_typed_engine_matches_json_bridge() -> None:
    covenant = (
        covenants
        .Covenant(covenants.CovenantType.max_debt_to_ebitda(4.5), "3M", "max_total_leverage")
        .with_cure_period(60)
        .with_consequence(covenants.CovenantConsequence.rate_increase(200.0))
    )
    assert covenant.cure_period_days == 60
    assert str(covenant.test_frequency) == "3M"
    assert [c.kind for c in covenant.consequences] == ["rate_increase"]
    assert covenant.scope == "maintenance"
    assert covenant.with_scope("incurrence").scope == "incurrence"
    with pytest.raises(ValueError, match="scope"):
        covenant.with_scope("sometimes")

    engine = covenants.CovenantEngine().add_spec(covenants.CovenantSpec(covenant, "debt_to_ebitda"))
    assert len(engine) == 1
    typed = engine.evaluate({"debt_to_ebitda": 3.2}, datetime.date(2025, 3, 31))
    bridged = covenants.evaluate_engine(engine.to_json(), {"debt_to_ebitda": 3.2}, "2025-03-31")
    assert typed == bridged
    assert typed["max_total_leverage"].passed is True
    assert typed["max_total_leverage"].threshold == pytest.approx(4.5)

    with pytest.raises(KeyError, match="debt_to_ebitda"):
        engine.evaluate({}, "2025-03-31")
    with pytest.raises(ValueError, match="bool"):
        engine.evaluate({"debt_to_ebitda": True}, "2025-03-31")

    revived = pickle.loads(pickle.dumps(engine))  # noqa: S301
    assert revived == engine
    assert covenants.CovenantEngine.from_json('{"specs": []}').specs == []


def test_threshold_schedule_and_waiver_typed() -> None:
    schedule = covenants.ThresholdSchedule([(datetime.date(2027, 1, 1), 6.0), ("2026-01-01", 6.5)])
    assert schedule.entries == [(datetime.date(2026, 1, 1), 6.5), (datetime.date(2027, 1, 1), 6.0)]
    assert schedule.threshold_for("2025-12-31") is None
    assert schedule.threshold_for("2026-06-30") == pytest.approx(6.5)
    with pytest.raises(ValueError, match="duplicate date"):
        covenants.ThresholdSchedule([("2026-01-01", 6.5), ("2026-01-01", 6.0)])

    covenant = covenants.Covenant(covenants.CovenantType.max_debt_to_ebitda(7.0), "3M", "max_leverage")
    spec = covenants.CovenantSpec(covenant, "debt_to_ebitda").with_threshold_schedule(schedule)
    assert spec.threshold_schedule == schedule
    engine = covenants.CovenantEngine.from_specs([spec])
    assert engine.evaluate({"debt_to_ebitda": 6.2}, "2025-12-31")["max_leverage"].threshold == pytest.approx(7.0)
    assert engine.evaluate({"debt_to_ebitda": 6.2}, "2026-03-31")["max_leverage"].threshold == pytest.approx(6.5)
    assert engine.evaluate({"debt_to_ebitda": 6.2}, "2027-03-31")["max_leverage"].passed is False

    waiver = covenants.CovenantWaiver("max_leverage", "2027-01-01", "2027-12-31", amended_threshold=8.0)
    engine.add_waiver(waiver)
    assert engine.waivers == [waiver]
    assert engine.evaluate({"debt_to_ebitda": 6.2}, "2027-03-31")["max_leverage"].threshold == pytest.approx(8.0)
    full = covenants.CovenantWaiver("max_leverage", "2028-01-01", description="Full waiver")
    engine.add_waiver(full)
    report = engine.evaluate({"debt_to_ebitda": 9.0}, "2028-06-30")["max_leverage"]
    assert report.passed is True
    assert report.details == "Waived by lender agreement"
    assert full.expiry_date is None
    assert pickle.loads(pickle.dumps(waiver)) == waiver  # noqa: S301


def test_springing_condition_gates_evaluation() -> None:
    condition = covenants.SpringingCondition("revolver_utilization", "minimum", 0.30)
    covenant = covenants.Covenant(
        covenants.CovenantType.max_total_leverage(5.0), "3M", "springing_leverage"
    ).with_springing_condition(condition)
    assert covenant.springing_condition == condition
    engine = covenants.CovenantEngine.from_specs([covenants.CovenantSpec(covenant, "total_leverage")])
    unsprung = engine.evaluate({"total_leverage": 6.0, "revolver_utilization": 0.1}, "2026-03-31")
    assert unsprung["springing_leverage"].passed is True
    assert unsprung["springing_leverage"].actual_value is None
    sprung = engine.evaluate({"total_leverage": 6.0, "revolver_utilization": 0.5}, "2026-03-31")
    assert sprung["springing_leverage"].passed is False
    with pytest.raises(ValueError, match="maximum"):
        covenants.SpringingCondition("x", "between", 1.0)


def test_evaluate_and_track_records_and_cures_breaches() -> None:
    engine = covenants.CovenantEngine.from_specs(covenants.cov_lite(7.0, 4.5))
    engine.evaluate_and_track({"total_leverage": 7.5, "senior_leverage": 3.0}, "2026-03-31", "incurrence")
    breach = engine.breach_history[0]
    assert breach.covenant_id == "max_total_leverage"
    assert breach.breach_date == datetime.date(2026, 3, 31)
    assert breach.cure_deadline == datetime.date(2026, 4, 30)
    assert breach.is_cured is False
    engine.evaluate_and_track({"total_leverage": 6.5, "senior_leverage": 3.0}, "2026-04-15", "incurrence")
    assert engine.breach_history[0].is_cured is True
    assert pickle.loads(pickle.dumps(breach)) == breach  # noqa: S301


def test_evaluate_series_and_reports_to_dataframe() -> None:
    engine = covenants.CovenantEngine.from_specs(covenants.cov_lite(7.0, 4.5))
    frame = pd.DataFrame(
        {"total_leverage": [6.0, 7.5], "senior_leverage": [3.0, 3.5]},
        index=pd.to_datetime(["2026-03-31", "2026-06-30"]),
    )
    out = engine.evaluate_series(frame)
    assert list(out.columns) == [
        "as_of",
        "covenant",
        "covenant_type",
        "passed",
        "actual_value",
        "threshold",
        "headroom",
        "details",
    ]
    assert len(out) == 6
    assert out.loc[out["covenant"] == "max_total_leverage", "passed"].tolist() == [True, False]
    assert out["as_of"].iloc[0] == "2026-03-31"

    reports = engine.evaluate({"total_leverage": 5.0, "senior_leverage": 3.0}, "2026-03-31")
    table = covenants.reports_to_dataframe(reports)
    assert table["covenant"].tolist() == ["max_total_leverage", "max_senior_leverage", "negative"]
    assert bool(table["passed"].all())


def test_forecast_covenant_and_breaches_from_dataframe() -> None:
    spec = covenants.CovenantSpec(
        covenants.Covenant(covenants.CovenantType.max_debt_to_ebitda(4.5), "3M", "max_leverage"),
        "debt_to_ebitda",
    )
    frame = pd.DataFrame(
        {"debt_to_ebitda": [4.0, 4.4, 4.8]},
        index=pd.to_datetime(["2026-03-31", "2026-06-30", "2026-09-30"]),
    )
    forecast = covenants.forecast_covenant(spec, frame)
    assert forecast.test_dates == [datetime.date(2026, 3, 31), datetime.date(2026, 6, 30), datetime.date(2026, 9, 30)]
    assert forecast.breach_probability == [0.0, 0.0, 1.0]
    assert forecast.first_breach_date == datetime.date(2026, 9, 30)
    assert forecast.comparator == "at_most"
    table = forecast.to_dataframe()
    assert list(table.columns) == [
        "test_date",
        "projected_value",
        "threshold",
        "headroom",
        "breach_probability",
        "breach_probability_stderr",
    ]
    assert len(table) == 3
    assert pickle.loads(pickle.dumps(forecast)) == forecast  # noqa: S301

    config = covenants.CovenantForecastConfig(stochastic=True, volatility=0.25, reference_date="2025-12-31")
    stochastic = covenants.forecast_covenant(spec, frame, config)
    assert all(0.0 < p < 1.0 for p in stochastic.breach_probability[:2])
    with pytest.raises(ValueError, match="volatility"):
        covenants.forecast_covenant(spec, frame, covenants.CovenantForecastConfig(stochastic=True))
    with pytest.raises(KeyError, match="debt_to_ebitda"):
        covenants.forecast_covenant(spec, frame.rename(columns={"debt_to_ebitda": "other"}))

    explicit = covenants.CovenantSpec(
        covenants.Covenant(covenants.CovenantType.max_debt_to_ebitda(4.5), "3M", "adjusted"),
        "adjusted_leverage",
    )
    with pytest.raises(KeyError, match="adjusted_leverage"):
        covenants.forecast_covenant(explicit, frame)

    engine = covenants.CovenantEngine.from_specs(covenants.cov_lite(7.0, 4.5))
    projections = pd.DataFrame(
        {"total_leverage": [6.0, 7.5], "senior_leverage": [3.0, 5.0]},
        index=pd.to_datetime(["2026-03-31", "2026-06-30"]),
    )
    breaches = covenants.forecast_breaches(
        engine, projections, covenants.CovenantForecastConfig().with_scope("incurrence")
    )
    # Rust sorts breaches by (breach_date, covenant_id), not by spec order.
    assert [(b.covenant_id, b.breach_date) for b in breaches] == [
        ("max_senior_leverage", datetime.date(2026, 6, 30)),
        ("max_total_leverage", datetime.date(2026, 6, 30)),
    ]
    assert covenants.breaches_to_dataframe(breaches)["breach_date"].tolist() == ["2026-06-30", "2026-06-30"]
    assert next(iter(covenants.breaches_to_dataframe([]).columns)) == "covenant_id"


def test_forecast_rejects_duplicate_columns_and_incomplete_observations() -> None:
    spec = covenants.CovenantSpec(
        covenants.Covenant(covenants.CovenantType.max_debt_to_ebitda(4.5), "3M", "leverage"), "x"
    )
    engine = covenants.CovenantEngine.from_specs([spec])
    index = pd.to_datetime(["2026-03-31"])
    duplicate = pd.DataFrame([[8.0, 2.0]], columns=["x", "x"], index=index)
    for call in (lambda: engine.evaluate_series(duplicate), lambda: covenants.forecast_covenant(spec, duplicate)):
        with pytest.raises(ValueError, match="duplicate metric columns"):
            call()
    for value in (math.nan, math.inf, -math.inf):
        frame = pd.DataFrame({"x": [value]}, index=index)
        with pytest.raises(ValueError, match="finite"):
            engine.evaluate({"x": value}, "2026-03-31")
        with pytest.raises(ValueError, match="finite"):
            covenants.evaluate_engine(engine.to_json(), {"x": value}, "2026-03-31")
        with pytest.raises(ValueError, match="finite"):
            covenants.forecast_breaches(engine, frame)
    with pytest.raises(KeyError):
        covenants.forecast_breaches(engine, pd.DataFrame({"other": [1.0]}, index=index))


def test_window_terms_and_waivers_reconcile_spot_forecast_and_history() -> None:
    spec = covenants.CovenantSpec(
        covenants.Covenant(covenants.CovenantType.max_debt_to_ebitda(4.5), "3M", "leverage").with_consequence(
            covenants.CovenantConsequence.rate_increase(100.0)
        ),
        "x",
    )
    engine = covenants.CovenantEngine.from_json(
        json.dumps({
            "specs": [],
            "windows": [{"start": "2026-01-01", "end": "2026-12-31", "covenants": [json.loads(spec.to_json())]}],
        })
    )
    engine.evaluate_and_track({"x": 5.0}, "2026-03-31", "maintenance")
    breach = engine.breach_history[0]
    assert breach.cure_deadline == datetime.date(2026, 4, 30)
    assert [c.kind for c in breach.consequences] == ["rate_increase"]
    assert covenants.CovenantEngine.from_json(engine.to_json()) == engine
    frame = pd.DataFrame({"x": [5.0]}, index=pd.to_datetime(["2026-06-30"]))
    assert len(covenants.forecast_breaches(engine, frame)) == 1
    engine.add_waiver(covenants.CovenantWaiver("leverage", "2026-06-01", amended_threshold=6.0))
    assert engine.evaluate({"x": 5.0}, "2026-06-30")["leverage"].passed
    assert covenants.forecast_breaches(engine, frame) == []
    engine.add_waiver(covenants.CovenantWaiver("leverage", "2026-07-01", amended_threshold=4.0))
    with pytest.raises(ValueError, match="overlapping waivers"):
        engine.validate()


def test_net_cash_and_negative_earnings_have_distinct_outcomes() -> None:
    spec = covenants.CovenantSpec(
        covenants.Covenant(covenants.CovenantType.max_net_debt_to_ebitda(4.5), "3M", "net"), "net_ratio"
    ).with_denominator_metric("adjusted_ebitda")
    assert spec.denominator_metric_id == "adjusted_ebitda"
    engine = covenants.CovenantEngine.from_specs([spec])
    for earnings in (50.0, -50.0, 0.0):
        metrics = {"net_ratio": -1.0, "adjusted_ebitda": earnings}
        spot = engine.evaluate(metrics, "2026-03-31")["net"]
        assert spot.passed == (earnings > 0)
        assert (spot.headroom is not None) == (earnings > 0)
        assert covenants.evaluate_engine(engine.to_json(), metrics, "2026-03-31")["net"] == spot
        forecast = covenants.forecast_covenant(spec, pd.DataFrame([metrics], index=pd.to_datetime(["2026-03-31"])))
        assert forecast.breach_probability == [0.0 if earnings > 0 else 1.0]
    with pytest.raises(KeyError):
        engine.evaluate({"net_ratio": -1.0}, "2026-03-31")


def test_invalid_trigger_and_inactive_forecast_cannot_create_false_decisions() -> None:
    covenant = covenants.Covenant(covenants.CovenantType.max_debt_to_ebitda(4.5), "3M", "leverage")
    invalid = covenant.with_springing_condition(covenants.SpringingCondition("utilization", "minimum", math.nan))
    with pytest.raises(ValueError, match="threshold finite"):
        covenants.CovenantEngine.from_specs([covenants.CovenantSpec(invalid, "x")]).validate()
    raw = json.loads(covenants.CovenantSpec(covenant, "x").to_json())
    raw["covenant"]["is_active"] = False
    inactive = covenants.CovenantSpec.from_json(json.dumps(raw))
    frame = pd.DataFrame({"other": [1.0]}, index=pd.to_datetime(["2026-03-31"]))
    forecast = covenants.forecast_covenant(inactive, frame)
    assert forecast.breach_probability == [0.0]
    assert forecast.projected_values == [None]


def test_forecast_scope_and_mc_boundaries() -> None:
    engine = covenants.CovenantEngine.from_specs(covenants.cov_lite(7.0, 4.5))
    frame = pd.DataFrame({"total_leverage": [8.0], "senior_leverage": [5.0]}, index=pd.to_datetime(["2026-03-31"]))
    assert covenants.forecast_breaches(engine, frame) == []
    config = covenants.CovenantForecastConfig().with_scope("incurrence")
    assert config.scope == "incurrence"
    assert len(covenants.forecast_breaches(engine, frame, config)) == 2
    with pytest.raises(ValueError, match="scope"):
        config.with_scope("invalid")
    with pytest.raises(ValueError, match="independent samples"):
        covenants.forecast_breaches(
            engine, frame, covenants.CovenantForecastConfig(stochastic=True, volatility=0.2, num_paths=1)
        )
