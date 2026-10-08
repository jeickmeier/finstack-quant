"""Calibration audit trail: typed solver method, residual units, Jacobian and curve recipes.

The calibration report states which numerical procedure produced it and in
which units its residuals are expressed, the diagnostics export the Jacobian a
global solve built, and calibrated curves expose the recipe (quotes and
conventions) they were calibrated from.
"""

from __future__ import annotations

import copy
import json
from pathlib import Path
from typing import Any

from finstack_quant.calibration import CalibrationDiagnostics, CalibrationReport, calibrate
from finstack_quant.core.market_data import DiscountCurve, HazardCurve

GOLDEN: dict[str, Any] = json.loads((Path(__file__).parent / "data" / "calibration_wasm_parity.json").read_text())

SOLVER_METHODS = {
    "sequential_bootstrap",
    "global_fit_lm_weighted_lsq",
    "per_slice_least_squares",
    "scalar_root_find",
    "scalar_minimization",
    "plan_execution",
}


def test_step_report_states_solver_method_and_residual_units() -> None:
    result = calibrate(GOLDEN["envelope"])
    report = result.step_report("USD-OIS")

    assert report.solver_method in SOLVER_METHODS
    assert report.residual_units == "pv_per_unit_notional"
    # The typed field and the metadata entry are written together.
    assert report.metadata["residual_units"] == report.residual_units
    # Only the equity SABR and SVI surface steps fit per-expiry smiles.
    assert report.fitted_slices == []

    wire = json.loads(report.to_json())
    assert wire["solver_method"] == report.solver_method
    assert wire["residual_units"] == report.residual_units


def test_plan_report_is_marked_as_an_aggregate_in_ratio_units() -> None:
    plan = calibrate(GOLDEN["envelope"]).report
    assert plan.solver_method == "plan_execution"
    assert plan.residual_units == "absolute_residual_over_step_tolerance"
    assert plan.metadata["residual_units"] == plan.residual_units


def test_report_json_without_the_typed_fields_still_loads() -> None:
    wire = json.loads(calibrate(GOLDEN["envelope"]).step_report("USD-OIS").to_json())
    del wire["solver_method"]
    del wire["residual_units"]
    legacy = CalibrationReport.from_json(json.dumps(wire))
    assert legacy.solver_method is None
    assert legacy.residual_units is None
    assert legacy.fitted_slices == []


def test_diagnostics_jacobian_follows_the_solver_method() -> None:
    envelope = copy.deepcopy(GOLDEN["envelope"])
    envelope["plan"]["settings"] = {"compute_diagnostics": True}
    report = calibrate(envelope).step_report("USD-OIS")
    diagnostics = report.diagnostics
    assert diagnostics is not None
    assert len(diagnostics.per_quote) == len(report.residuals)
    if report.solver_method == "sequential_bootstrap":
        # A sequential bootstrap solves one knot per quote and builds no Jacobian.
        assert diagnostics.jacobian is None
    else:
        assert report.solver_method == "global_fit_lm_weighted_lsq"
        jacobian = diagnostics.jacobian
        assert jacobian is not None
        assert len(jacobian) == len(diagnostics.per_quote)
        for row, quote in zip(jacobian, diagnostics.per_quote, strict=True):
            assert quote.sensitivity == max(abs(value) for value in row)


def test_diagnostics_jacobian_round_trips_and_reproduces_the_sensitivities() -> None:
    rows = [
        {"quote_label": "A", "quote_value": 0.04, "residual": 1e-10, "sensitivity": 5.0},
        {"quote_label": "B", "quote_value": 0.05, "residual": -2e-10, "sensitivity": 8.0},
    ]
    diagnostics = CalibrationDiagnostics.from_json(
        json.dumps({
            "per_quote": rows,
            "condition_number": 12.5,
            "jacobian": [[5.0, -0.5], [1.0, -8.0]],
            "max_residual": 2e-10,
            "rms_residual": 1.5e-10,
        })
    )
    assert diagnostics.jacobian == [[5.0, -0.5], [1.0, -8.0]]
    for row, quote in zip(diagnostics.jacobian, diagnostics.per_quote, strict=True):
        assert quote.sensitivity == max(abs(value) for value in row)
    assert json.loads(diagnostics.to_json())["jacobian"] == diagnostics.jacobian


def test_calibrated_discount_curve_exposes_its_rate_calibration_recipe() -> None:
    curve = calibrate(GOLDEN["envelope"]).market.get_discount("USD-OIS")
    recipe = curve.rate_calibration
    assert recipe is not None
    assert recipe == json.loads(curve.to_json())["rate_calibration"]
    assert recipe["currency"] == "USD"
    assert len(recipe["quotes"]) == len(GOLDEN["envelope"]["plan"]["quote_sets"]["USD-OIS"])


def test_directly_built_curves_have_no_calibration_recipe() -> None:
    discount = DiscountCurve("USD-OIS", "2025-01-01", [(0.0, 1.0), (1.0, 0.95)])
    assert discount.rate_calibration is None
    assert discount.to_forward_curve("USD-FWD", 0.25).rate_calibration is None
    hazard = HazardCurve("ACME-HZD", "2025-01-01", [(1.0, 0.02), (5.0, 0.03)], recovery_rate=0.4)
    assert hazard.hazard_calibration is None


def test_hazard_curve_exposes_its_hazard_calibration_recipe() -> None:
    wire = json.loads(
        HazardCurve("RECIPE", "2025-01-01", [(1.0, 0.01), (5.0, 0.02)], recovery_rate=0.4).to_json()
    )
    quote_input = {
        "quote": {"type": "cds_par_spread", "id": "CDS-5Y"},
        "pillar_date": "2026-01-01",
        "pillar_time": 1.0,
    }
    wire["hazard_calibration"] = {
        "hazard_params": {"curve_id": "RECIPE"},
        "calibration_inputs": [quote_input],
        "spread_risk_inputs": [quote_input],
        "calibration_config": {"fail_on_bad_fit": True},
    }
    curve = HazardCurve.from_json(json.dumps(wire))
    assert curve.hazard_calibration == wire["hazard_calibration"]
    assert curve.hazard_calibration == json.loads(curve.to_json())["hazard_calibration"]
