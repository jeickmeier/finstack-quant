"""Cross-host goldens for the calibration members that WASM binds as free functions.

``tests/data/calibration_wasm_parity.json`` holds the Python outputs for fixed
inputs. This module pins Python to that file and
``finstack-quant-wasm/tests/facade/calibration_parity.test.mjs`` pins the WASM
twins (``rateQuoteDeposit`` ..., ``calibrationStepDiscount`` ...,
``rateBoundsForCurrency``, ``calibrationResultStepReport``,
``calibrateHullWhiteToSwaptions`` ...) to the same file: both hosts call the
same Rust constructors and calibrators.
"""

from __future__ import annotations

import datetime
import json
import math
from pathlib import Path
from typing import Any

import pytest

from finstack_quant.calibration import (
    CalibrationEnvelope,
    CalibrationEnvelopeError,
    CalibrationStep,
    CdsQuote,
    RateBounds,
    RateQuote,
    VolQuote,
    calibrate,
    dry_run,
    validate_calibration,
)
from finstack_quant.calibration.hull_white import (
    CapFloorCalibrationConfig,
    CapFloorQuote,
    HullWhiteParams,
    PiecewiseSigmaCalibrationConfig,
    SwaptionQuote,
    bootstrap_hull_white_sigma_schedule_to_cap_floors,
    calibrate_hull_white_to_cap_floors,
    calibrate_hull_white_to_swaptions,
)
from finstack_quant.core.market_data import DiscountCurve

GOLDEN: dict[str, Any] = json.loads((Path(__file__).parent / "data" / "calibration_wasm_parity.json").read_text())
HW: dict[str, Any] = GOLDEN["hull_white"]
BASE = "2026-05-08"

QUOTES = {
    "deposit": lambda: RateQuote.deposit("D3M", "USD-SOFR-OIS", "3M", 0.052),
    "fra": lambda: RateQuote.fra("F", "USD-SOFR-OIS", "3M", "6M", 0.05),
    "futures": lambda: RateQuote.futures("FUT", "CME:SR3", "2026-09-15", 96.5),
    "swap": lambda: RateQuote.swap("S5", "USD-SOFR-OIS", "5Y", 0.045),
    "swap_dated": lambda: RateQuote.swap("S5d", "USD-SOFR-OIS", "2031-05-08", 0.045, 0.001),
    "par_spread": lambda: CdsQuote.par_spread("ACME-5Y", "ACME", "USD", "isda_na", "5Y", 80.0, 0.4),
    "upfront": lambda: CdsQuote.upfront("ACME-5Y-U", "ACME", "USD", "isda_na", "5Y", 100.0, 0.01, 0.4),
    "option_vol": lambda: VolQuote.option_vol("O", "AAPL", "2027-05-08", 155.0, 0.28),
    "swaption_vol": lambda: VolQuote.swaption_vol("SV", "2027-05-08", "2032-05-08", 0.04, 0.0072),
    "cap_floor_vol": lambda: VolQuote.cap_floor_vol("CF", "2027-05-08", 0.04, 0.0072, "normal", False),
}

STEPS = {
    "discount": lambda: CalibrationStep.discount("USD-OIS", "USD", BASE),
    "discount_named": lambda: CalibrationStep.discount(
        "d2", "USD", BASE, quote_set="qs", curve_id="USD-OIS", interpolation="linear"
    ),
    "forward": lambda: CalibrationStep.forward("USD-SOFR-3M", "USD", BASE, 0.25, "USD-OIS"),
    "hazard": lambda: CalibrationStep.hazard("ACME", "ACME", "USD", BASE, "USD-OIS", 0.4),
    "inflation": lambda: CalibrationStep.inflation("USA-CPI", "USD", BASE, "USD-OIS", "USA-CPI-U", "3M", 310.0),
    "vol_surface": lambda: CalibrationStep.vol_surface("AAPL-VOL", BASE, "AAPL"),
    "swaption_vol": lambda: CalibrationStep.swaption_vol("USD-SWPT", BASE, "USD-OIS", "USD"),
    "base_correlation": lambda: CalibrationStep.base_correlation(
        "CDX-CORR", "CDX.NA.IG", 42, 5.0, BASE, "USD-OIS", "USD"
    ),
    "student_t": lambda: CalibrationStep.student_t("T", "TRANCHE-1", "CDX.NA.IG_CORR"),
    "hull_white": lambda: CalibrationStep.hull_white("HW", "USD-OIS", "USD", BASE, fit_tolerance=1e-4),
    "cap_floor_hull_white": lambda: CalibrationStep.cap_floor_hull_white(
        "HWCF", "USD-OIS", "USD-SOFR-3M", "USD", BASE, fit_tolerance=1e-4
    ),
    "svi_surface": lambda: CalibrationStep.svi_surface("AAPL-SVI", BASE, "AAPL"),
    "xccy_basis": lambda: CalibrationStep.xccy_basis("EUR-XCCY", "EUR", BASE, 1.1, "USD-OIS"),
    "parametric": lambda: CalibrationStep.parametric("USD-NS", BASE),
}


def discount_curve() -> DiscountCurve:
    """Flat 3% curve of the shared golden."""
    spec = HW["curve"]
    flat = spec["knots"]
    knots = list(zip(flat[0::2], flat[1::2], strict=True))
    return DiscountCurve(spec["id"], datetime.date.fromisoformat(spec["base_date"]), knots, day_count=spec["day_count"])


def cap_floor_quotes() -> list[CapFloorQuote]:
    """Cap quotes of the shared golden."""
    return [CapFloorQuote.from_json(json.dumps(quote)) for quote in HW["cap_floor_quotes"]]


@pytest.mark.parametrize("name", sorted(QUOTES))
def test_quote_constructors_match_the_shared_golden(name: str) -> None:
    assert json.loads(QUOTES[name]().to_json()) == GOLDEN["quotes"][name]


def test_quote_constructors_reject_invalid_inputs() -> None:
    with pytest.raises(ValueError, match="invalid pillar"):
        RateQuote.deposit("D", "USD-SOFR-OIS", "soon", 0.05)
    with pytest.raises(ValueError, match="invalid VolQuote"):
        VolQuote.option_vol("O", "AAPL", "2027-05-08", 155.0, 0.28, "straddle")
    with pytest.raises(ValueError, match="invalid CdsQuote"):
        CdsQuote.par_spread("C", "ACME", "USD", "not_a_clause", "5Y", 80.0, 0.4)


@pytest.mark.parametrize("name", sorted(STEPS))
def test_step_constructors_match_the_shared_golden(name: str) -> None:
    assert json.loads(STEPS[name]().to_json()) == GOLDEN["steps"][name]


def test_step_constructors_reject_unknown_fields() -> None:
    with pytest.raises(ValueError, match="invalid discount step"):
        CalibrationStep.discount("USD-OIS", "USD", BASE, bogus=1)


def test_rate_bounds_match_the_shared_golden() -> None:
    bounds = GOLDEN["rate_bounds"]
    assert json.loads(RateBounds.for_currency("USD").to_json()) == bounds["USD"]
    assert json.loads(RateBounds.for_currency("JPY").to_json()) == bounds["JPY"]
    assert json.loads(RateBounds.emerging_markets().to_json()) == bounds["emerging_markets"]
    with pytest.raises(ValueError, match="not-a-currency"):
        RateBounds.for_currency("not-a-currency")


def test_validate_calibration_and_dry_run_accept_the_shared_envelope() -> None:
    envelope = GOLDEN["envelope"]
    validated = validate_calibration(envelope)
    assert validated.plan.id == envelope["plan"]["id"]
    assert validated.plan.step_ids == ["USD-OIS"]
    assert CalibrationEnvelope.from_json(json.dumps(envelope)).dry_run().is_valid
    assert dry_run(envelope).is_valid
    broken = json.loads(json.dumps(envelope))
    broken["plan"]["steps"][0]["quote_set"] = "missing"
    with pytest.raises(CalibrationEnvelopeError) as rejected:
        validate_calibration(broken)
    assert rejected.value.kind == "undefined_quote_set"


def test_result_step_report_and_residuals_match_the_shared_golden() -> None:
    result = calibrate(GOLDEN["envelope"])
    expected = GOLDEN["step_report"]
    report = result.step_report("USD-OIS")
    assert report.success is expected["success"]
    assert report.iterations == expected["iterations"]
    assert sorted(report.residuals) == expected["residual_ids"]
    assert json.loads(result.step_report_json("USD-OIS"))["iterations"] == expected["iterations"]
    frame = result.residuals("USD-OIS")
    assert list(frame["quote_id"]) == expected["residual_ids"]
    assert all(abs(value) < 1e-8 for value in frame["residual"])
    assert all(math.isnan(value) for value in frame["target"])
    with pytest.raises(KeyError) as missing:
        result.step_report("nope")
    assert "calibration step 'nope'; available steps: [\"USD-OIS\"]" in missing.value.args[0]


def test_hull_white_piecewise_bootstrap_matches_the_shared_golden() -> None:
    config = HW["piecewise_config"]
    params, report = bootstrap_hull_white_sigma_schedule_to_cap_floors(
        discount_curve(),
        cap_floor_quotes(),
        PiecewiseSigmaCalibrationConfig(
            config["fixed_kappa"], config["sigma_min"], config["sigma_max"], config["fit_tolerance"]
        ),
    )
    expected = HW["piecewise"]
    assert report.success is expected["success"]
    assert params.kappa == expected["params"]["kappa"]
    assert params.times == expected["params"]["volatility"]["times"]
    assert params.values == pytest.approx(expected["params"]["volatility"]["values"], rel=1e-8)
    assert params.sigma_at(1.5) == pytest.approx(expected["sigma_at_1_5"], rel=1e-8)
    assert params.sigma_at(0.5) == pytest.approx(expected["sigma_at_0_5"], rel=1e-8)
    exact = HullWhiteParams.from_json(json.dumps(expected["params"]))
    assert exact.sigma_at(1.5) == expected["sigma_at_1_5"]


def test_hull_white_cap_floor_fit_matches_the_shared_golden() -> None:
    config = CapFloorCalibrationConfig(HW["scalar_config"]["fit_tolerance"], fixed_kappa=0.03)
    curve = discount_curve()
    quotes = cap_floor_quotes()[1:2]
    params, report = calibrate_hull_white_to_cap_floors(curve, quotes, config)
    expected = HW["scalar"]
    assert report.success is expected["success"]
    assert params.kappa == expected["params"]["kappa"]
    assert params.sigma == pytest.approx(expected["params"]["sigma"], rel=1e-8)
    explicit, _ = calibrate_hull_white_to_cap_floors(curve, quotes, config, forward=curve)
    assert explicit == params
    with pytest.raises(ValueError, match="invalid swap frequency 'monthly'"):
        CapFloorCalibrationConfig(1e-6, frequency="monthly")


def test_hull_white_swaption_fit_matches_the_shared_golden() -> None:
    quotes = [SwaptionQuote.from_json(json.dumps(quote)) for quote in HW["swaption_quotes"]]
    params, report = calibrate_hull_white_to_swaptions(discount_curve(), quotes, HW["swaption_fit_tolerance"])
    expected = HW["swaptions"]
    assert report.success is expected["success"]
    assert params.kappa == pytest.approx(expected["params"]["kappa"], rel=1e-6)
    assert params.sigma == pytest.approx(expected["params"]["sigma"], rel=1e-6)
