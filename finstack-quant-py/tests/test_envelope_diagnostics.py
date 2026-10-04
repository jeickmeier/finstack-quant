"""Tests for Phase 4 envelope diagnostics surface."""

from __future__ import annotations

from collections.abc import Callable
import json
import pickle

import pytest

from finstack_quant.calibration import (
    CalibrationEnvelope,
    CalibrationEnvelopeError,
    CalibrationPlan,
    CalibrationStep,
    RateQuote,
    calibrate,
    dry_run,
    validate_calibration_json,
)


def test_shared_quotes_are_explicit_envelope_inputs() -> None:
    quote = RateQuote.deposit("D", "USD-Deposit", "1Y", 0.03)
    steps = [CalibrationStep.discount(curve, "USD", "2026-05-08", quote_set="shared") for curve in ["A", "B"]]
    plan = CalibrationPlan(steps, quote_sets={"shared": ["D"]})
    envelope = CalibrationEnvelope(plan, market_data=[quote])
    assert calibrate(envelope).success
    assert not hasattr(plan, "market_data")
    assert not hasattr(steps[0], "quotes")


def test_removed_inline_quotes_and_plan_execution_are_rejected() -> None:
    with pytest.raises(ValueError, match="quotes"):
        CalibrationStep.discount("A", "USD", "2026-05-08", quotes=[])
    for operation in (calibrate, dry_run, validate_calibration_json):
        with pytest.raises((TypeError, CalibrationEnvelopeError)):
            operation(CalibrationPlan([]))


def _empty_envelope() -> dict:
    return {
        "schema": "finstack_quant.calibration/1",
        "plan": {
            "id": "smoke",
            "description": None,
            "quote_sets": {},
            "steps": [],
            "settings": {},
        },
    }


def test_parametric_step_rejects_removed_separate_discount_option() -> None:
    with pytest.raises(ValueError, match="discount_curve_id"):
        CalibrationStep.parametric("NS", "2026-05-08", quote_set="rates", discount_curve_id="USD-OIS")


def test_dry_run_returns_json_report() -> None:
    report = dry_run(json.dumps(_empty_envelope()))
    assert report.errors == []
    assert report.is_valid
    assert "dependency_graph" in json.loads(report.to_json())


def test_calibration_result_pickles_through_top_level_module() -> None:
    result = calibrate(json.dumps(_empty_envelope()))
    restored = pickle.loads(pickle.dumps(result))  # noqa: S301 - trusted in-process round trip
    assert restored.success is True
    assert type(restored).__module__ == "finstack_quant.calibration"


def test_dry_run_surfaces_undefined_quote_set_with_suggestion() -> None:
    envelope = _empty_envelope()
    envelope["plan"]["quote_sets"] = {"usd_quotes": []}
    envelope["plan"]["steps"] = [
        {
            "id": "discount_step",
            "quote_set": "usd_quotess",
            "kind": "discount",
            "curve_id": "USD-OIS",
            "currency": "USD",
            "base_date": "2026-05-08",
        }
    ]
    report = json.loads(dry_run(json.dumps(envelope)).to_json())
    undef = next(
        (e for e in report["errors"] if e["kind"] == "undefined_quote_set"),
        None,
    )
    assert undef is not None, report["errors"]
    assert undef["ref_name"] == "usd_quotess"
    assert undef["suggestion"] == "usd_quotes"


@pytest.mark.parametrize("operation", [validate_calibration_json, calibrate])
def test_calibration_entry_points_enforce_semantic_validation(
    operation: Callable[[str], str],
) -> None:
    envelope = _empty_envelope()
    envelope["plan"]["steps"] = [
        {
            "id": "discount_step",
            "quote_set": "missing_quotes",
            "kind": "discount",
            "curve_id": "USD-OIS",
            "currency": "USD",
            "base_date": "2026-05-08",
        }
    ]

    with pytest.raises(CalibrationEnvelopeError) as excinfo:
        operation(json.dumps(envelope))

    assert excinfo.value.kind == "undefined_quote_set"
    details = json.loads(excinfo.value.details)
    assert details["category"] == "undefined_quote_set"
    assert details["stage"] == "ingestion"
    assert details["step_id"] == "discount_step"
    assert details["solver_diagnostics"] is None
    envelope_details = details["envelope_error"]
    assert envelope_details["ref_name"] == "missing_quotes"
    assert excinfo.value.stage == "ingestion"
    assert excinfo.value.solver_diagnostics is None


def test_calibration_envelope_error_inherits_runtime_error() -> None:
    """Backwards-compat: existing `except RuntimeError` callers still catch it."""
    assert issubclass(CalibrationEnvelopeError, RuntimeError)


@pytest.mark.parametrize(
    "operation",
    [validate_calibration_json, calibrate, dry_run],
)
def test_all_calibration_entry_points_expose_execution_error_details(
    operation: Callable[[str], object],
) -> None:
    with pytest.raises(CalibrationEnvelopeError) as excinfo:
        operation("{ malformed")

    exc = excinfo.value
    assert exc.kind == "strict_load"
    assert exc.stage == "ingestion"
    assert exc.step_id is None
    assert exc.solver_diagnostics is None
    payload = json.loads(exc.details)
    assert set(payload) == {
        "stage",
        "step_id",
        "category",
        "solver_diagnostics",
        "cause",
        "envelope_error",
        "diagnostics",
    }
    assert payload["category"] == exc.kind
    assert payload["stage"] == exc.stage
    assert payload["step_id"] == exc.step_id
    assert payload["solver_diagnostics"] == exc.solver_diagnostics
    assert payload["envelope_error"]["kind"] == exc.kind


def test_runtime_error_handler_catches_calibration_envelope_error() -> None:
    """Existing pre-Phase-4 `except RuntimeError` callers continue to work.

    Catching as the broader ``RuntimeError`` parent must still produce the
    typed subclass so legacy code paths that introspect via ``isinstance``
    keep functioning.
    """
    with pytest.raises(RuntimeError) as excinfo:
        dry_run("garbage")
    assert isinstance(excinfo.value, CalibrationEnvelopeError)


@pytest.mark.parametrize("entry", [calibrate, dry_run, validate_calibration_json])
def test_malformed_json_reports_the_rust_parse_diagnostic(entry: Callable[[str], object]) -> None:
    # A str goes straight to the Rust strict loader (same diagnostic as WASM).
    with pytest.raises(CalibrationEnvelopeError) as info:
        entry("not json")
    assert info.value.kind == "strict_load"
    assert info.value.diagnostics[0]["code"] == "contract/parse-error"
    assert info.value.diagnostics[0]["pointer"] is None


@pytest.mark.parametrize("solver", [{"tolerance": 0.0}, {"tolerance": -1.0}, {"max_iterations": 0}])
def test_unusable_solver_settings_are_rejected_everywhere(solver: dict) -> None:
    from finstack_quant.calibration import CalibrationConfig, SolverConfig

    envelope = _empty_envelope()
    envelope["plan"]["settings"] = {"solver": solver}
    for entry in (calibrate, dry_run, validate_calibration_json):
        with pytest.raises(CalibrationEnvelopeError, match=r"solver (tolerance|max_iterations)"):
            entry(json.dumps(envelope))
    with pytest.raises(ValueError, match=r"solver (tolerance|max_iterations)"):
        SolverConfig.from_json(json.dumps(solver))
    with pytest.raises(ValueError, match=r"solver (tolerance|max_iterations)"):
        SolverConfig(**solver)
    with pytest.raises(ValueError, match=r"solver (tolerance|max_iterations)"):
        CalibrationConfig(**solver)


def test_dry_run_json_is_the_wire_twin_of_dry_run() -> None:
    from finstack_quant.calibration import dry_run_json

    envelope = json.dumps(_empty_envelope())
    assert json.loads(dry_run_json(envelope)) == json.loads(dry_run(envelope).to_json())


@pytest.mark.parametrize("key", ["interp_style", "extrapolation_policy"])
def test_discount_solve_settings_reject_removed_curve_shape_keys(key: str) -> None:
    envelope = {
        "schema": "finstack_quant.calibration/1",
        "plan": {"id": "p", "steps": [], "settings": {"discount_curve": {key: "linear"}}},
    }
    with pytest.raises(CalibrationEnvelopeError, match=f"unknown field `{key}`") as info:
        validate_calibration_json(envelope)
    assert info.value.kind == "strict_load"


def test_market_freshness_age_is_a_plain_integer() -> None:
    envelope = {
        "schema": "finstack_quant.calibration/1",
        "plan": {"id": "p", "steps": [], "settings": {"market_freshness": {"max_age_seconds": 3600}}},
    }
    assert dry_run(envelope).errors == []
    canonical = json.loads(validate_calibration_json(envelope))
    assert canonical["plan"]["settings"]["market_freshness"]["max_age_seconds"] == 3600


def _request() -> CalibrationEnvelope:
    quotes = [
        RateQuote.deposit("USD-DEP-3M", "USD-SOFR-OIS", "3M", 0.052),
        RateQuote.swap("USD-SWAP-2Y", "USD-SOFR-OIS", "2Y", 0.049),
    ]
    step = CalibrationStep.discount("USD-OIS", "USD", "2026-05-08")
    plan = CalibrationPlan([step], quote_sets={"USD-OIS": [quote.id for quote in quotes]})
    return CalibrationEnvelope(plan, market_data=quotes)


def test_steps_and_plans_round_trip_as_plain_values() -> None:
    envelope = _request()
    plan = envelope.plan
    step = plan.steps[0]
    for value in (step, plan):
        restored = type(value).from_json(value.to_json())
        assert restored.to_json() == value.to_json()
        assert pickle.loads(pickle.dumps(value)).to_json() == value.to_json()  # noqa: S301
    assert "quotes" not in json.loads(step.to_json())
    assert "market_data" not in json.loads(plan.to_json())
    assert "schema" not in json.loads(plan.to_json())


def test_envelope_round_trip_retains_market_data_and_quote_sets() -> None:
    envelope = _request()
    restored = CalibrationEnvelope.from_json(envelope.to_json())
    pickled = pickle.loads(pickle.dumps(envelope))  # noqa: S301
    assert restored.market_data == envelope.market_data
    assert pickled.to_json() == envelope.to_json()
    assert restored.plan.quote_sets == envelope.plan.quote_sets
    assert calibrate(restored).to_json() == calibrate(envelope).to_json()
    rebuilt = CalibrationEnvelope(envelope.plan, market_data=envelope.market_data)
    assert rebuilt.to_json() == envelope.to_json()


def test_envelope_rejects_duplicate_market_data_during_validation() -> None:
    envelope = _request()
    repeated = CalibrationEnvelope(envelope.plan, market_data=envelope.market_data * 2)
    assert any(error["kind"] == "duplicate_market_datum_id" for error in repeated.dry_run().errors)
    with pytest.raises(CalibrationEnvelopeError, match="duplicate"):
        calibrate(repeated)
