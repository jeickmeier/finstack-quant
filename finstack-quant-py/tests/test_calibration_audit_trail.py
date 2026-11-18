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

import pytest

from finstack_quant.calibration import CalibrationDiagnostics, CalibrationReport, calibrate
from finstack_quant.core.market_data import DiscountCurve, ForwardCurve, HazardCurve, MarketContext
from finstack_quant.valuations.instruments import price_instrument

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
    assert "residual_units" not in report.metadata
    # Only the equity SABR and SVI surface steps fit per-expiry smiles.
    assert report.fitted_slices == []

    wire = json.loads(report.to_json())
    assert wire["solver_method"] == report.solver_method
    assert wire["residual_units"] == report.residual_units


def test_plan_report_is_marked_as_an_aggregate_in_ratio_units() -> None:
    plan = calibrate(GOLDEN["envelope"]).report
    assert plan.solver_method == "plan_execution"
    assert plan.residual_units == "absolute_residual_over_step_tolerance"
    assert "residual_units" not in plan.metadata


def test_manually_authored_report_without_units_has_unknown_units() -> None:
    wire = json.loads(calibrate(GOLDEN["envelope"]).step_report("USD-OIS").to_json())
    del wire["solver_method"]
    del wire["residual_units"]
    wire["metadata"]["residual_units"] = "quoted_volatility"
    manual = CalibrationReport.from_json(json.dumps(wire))
    assert manual.solver_method is None
    assert manual.residual_units is None
    assert manual.fitted_slices == []


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
    quote_ids = GOLDEN["envelope"]["plan"]["quote_sets"]["USD-OIS"]
    assert {binding["quote_id"] for binding in recipe["execution"]["bindings"]} == set(quote_ids)
    assert all(binding["pillar_time"] > 0.0 for binding in recipe["execution"]["bindings"])
    assert "solver" in recipe["execution"]["calibration_config"]
    restored = MarketContext.from_json(MarketContext().insert(curve).to_json())
    assert restored.get_discount("USD-OIS").rate_calibration == recipe
    assert len(recipe["quotes"]) == len(GOLDEN["envelope"]["plan"]["quote_sets"]["USD-OIS"])


def test_cds_dv01_replays_the_restored_discount_curve() -> None:
    fixture = (
        Path(__file__).resolve().parents[2]
        / "finstack-quant/valuations/tests/golden/data/pricing/quantlib/cds/cds_quantlib_flat_hazard_decomposition.json"
    )
    case = json.loads(fixture.read_text())
    envelope = copy.deepcopy(GOLDEN["envelope"])
    as_of = case["metadata"]["valuation_date"]
    step = envelope["plan"]["steps"][0]
    step.update(base_date=as_of, curve_id="USD-FLAT-2PCT", extrapolation="extend")
    envelope["prior_market"] = [
        item for item in case["market"]["envelope"]["prior_market"] if item["kind"] == "hazard_curve"
    ]
    state = json.loads(calibrate(envelope).market.to_json())

    def dv01() -> float:
        restored = MarketContext.from_json(json.dumps(state))
        result = price_instrument(case["instrument"], restored, as_of, model=case["model"], metrics=["dv01"])
        return result.metrics["dv01"]

    def repriced(shift: float) -> float:
        shocked = copy.deepcopy(envelope)
        for quote in shocked["market_data"]:
            quote["rate"] += shift
        market = calibrate(shocked).market
        return price_instrument(case["instrument"], market, as_of, model=case["model"]).price

    assert dv01() == pytest.approx((repriced(0.0001) - repriced(-0.0001)) / 2.0, abs=1e-6)
    discount = next(curve for curve in state["curves"] if curve["id"] == "USD-FLAT-2PCT")
    discount["knot_points"][-1][1] *= 0.95
    with pytest.raises((ValueError, RuntimeError), match="source factor"):
        dv01()


def test_directly_built_curves_have_no_calibration_recipe() -> None:
    discount = DiscountCurve(
        "USD-OIS",
        "2025-01-01",
        [(0.0, 1.0), (1.0, 0.95)],
        interp="monotone_convex",
        extrapolation="extend",
        day_count="act_365f",
    )
    assert discount.rate_calibration is None
    assert ForwardCurve.from_discount_curve("USD-FWD", discount).rate_calibration is None
    hazard = HazardCurve("ACME-HZD", "2025-01-01", [(1.0, 0.02), (5.0, 0.03)], recovery_rate=0.4, day_count="act_365f")
    assert hazard.hazard_calibration is None


def test_hazard_curve_exposes_its_hazard_calibration_recipe() -> None:
    wire = json.loads(
        HazardCurve(
            "RECIPE", "2025-01-01", [(1.0, 0.01), (5.0, 0.02)], recovery_rate=0.4, day_count="act_365f"
        ).to_json()
    )
    quote_input = {
        "quote": {
            "type": "cds_par_spread",
            "id": "CDS-5Y",
            "entity": "ACME",
            "convention": {"currency": "USD", "doc_clause": "isda_na"},
            "pillar": {"date": "2030-01-01"},
            "spread_bp": 120.0,
            "recovery_rate": 0.4,
        },
        "pillar_date": "2030-01-01",
        "pillar_time": 5.0,
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
    market = MarketContext()
    market.insert(curve)
    state = json.loads(market.to_json())
    risk = state["curves"][0]["hazard_calibration"]["spread_risk_inputs"][0]
    risk.update(pillar_date="2035-01-01", pillar_time=10.0)
    risk["quote"]["pillar"] = {"date": "2035-01-01"}
    with pytest.raises(ValueError, match=r"validation|pillar"):
        MarketContext.from_json(json.dumps(state))
