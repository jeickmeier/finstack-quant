"""Tests for Phase 4 envelope diagnostics surface."""

from __future__ import annotations

from collections.abc import Callable
import json
import pickle

import pytest

from finstack_quant.calibration import (
    CalibrationEnvelopeError,
    CalibrationPlan,
    CalibrationStep,
    RateQuote,
    calibrate,
    dry_run,
    validate_calibration_json,
)


@pytest.mark.parametrize("same_set", [True, False])
def test_attached_quote_id_rejects_conflicting_payloads(same_set: bool) -> None:
    steps = [
        CalibrationStep.discount(
            "A", "USD", "2026-05-08", quote_set="shared", quotes=[RateQuote.deposit("D", "USD-Deposit", "1Y", 0.03)]
        ),
        CalibrationStep.discount(
            "B",
            "USD",
            "2026-05-08",
            quote_set="shared" if same_set else "other",
            quotes=[RateQuote.deposit("D", "USD-Deposit", "1Y", 0.08)],
        ),
    ]
    with pytest.raises(ValueError, match="conflicting attached payloads"):
        CalibrationPlan(steps)


@pytest.mark.parametrize("explicit", [True, False])
def test_attached_quotes_survive_set_registration_and_deduplicate(explicit: bool) -> None:
    quote = RateQuote.deposit("D", "USD-Deposit", "1Y", 0.03)
    steps = [
        CalibrationStep.discount(curve, "USD", "2026-05-08", quotes=[quote], quote_set="shared") for curve in ["A", "B"]
    ]
    plan = CalibrationPlan(steps, quote_sets={"shared": ["D"]} if explicit else None)
    assert len(plan.market_data) == 1
    assert calibrate(plan).success


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
