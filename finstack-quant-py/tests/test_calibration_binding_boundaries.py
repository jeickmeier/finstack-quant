"""Calibration bindings preserve canonical validation and loading limits."""

import json

import pytest

from finstack_quant.calibration import (
    CalibrationPlan,
    CalibrationResult,
    CalibrationStep,
    RateQuote,
    SolverConfig,
    calibrate,
)
from finstack_quant.calibration.hull_white import CapFloorQuote
from finstack_quant.portfolio import ContractLimitExceededError


@pytest.mark.parametrize("spread", [float("nan"), float("inf"), float("-inf")])
def test_swap_quote_rejects_nonfinite_optional_spread(spread: float) -> None:
    with pytest.raises(ValueError, match="spread_decimal must be finite"):
        RateQuote.swap("USD-SWAP-2Y", "USD-SOFR-OIS", "2Y", 0.04, spread_decimal=spread)


@pytest.mark.parametrize("spread", [None, 0.0, 0.001, -0.001])
def test_swap_quote_preserves_finite_or_absent_spread(spread: float | None) -> None:
    quote = RateQuote.swap("USD-SWAP-2Y", "USD-SOFR-OIS", "2Y", 0.04, spread_decimal=spread)
    assert json.loads(quote.to_json())["spread_decimal"] == spread
    result = calibrate(CalibrationPlan([CalibrationStep.discount("USD-OIS", "USD", "2026-09-30", quotes=[quote])]))
    assert result.success


@pytest.mark.parametrize("seasonal_factors", [None, {"monthly_adjustments": [0.01, -0.01] * 6}])
def test_inflation_step_rejects_removed_seasonal_factors(seasonal_factors: object) -> None:
    with pytest.raises(ValueError, match="seasonal_factors"):
        CalibrationStep.inflation(
            "USD-CPI",
            "USD",
            "2026-09-30",
            "USD-OIS",
            "USA-CPI-U",
            "3M",
            320.0,
            quote_set="cpi-quotes",
            seasonal_factors=seasonal_factors,
        )


def test_cap_floor_step_preserves_required_index_and_rejects_frequency_override() -> None:
    arguments = {
        "id": "HW-CAPS",
        "discount_curve_id": "EUR-OIS",
        "forward_curve_id": "EUR-EURIBOR-3M",
        "index_id": "EUR-EURIBOR-3M",
        "currency": "EUR",
        "base_date": "2026-09-30",
        "quote_set": "caps",
        "fit_tolerance": 1e-4,
    }
    step = CalibrationStep.cap_floor_hull_white(**arguments)
    encoded = json.loads(step.to_json())
    assert encoded["index_id"] == "EUR-EURIBOR-3M"
    assert "payment_frequency" not in encoded
    assert CalibrationStep.from_json(step.to_json()).params["index_id"] == "EUR-EURIBOR-3M"

    with pytest.raises(ValueError, match="payment_frequency"):
        CalibrationStep.cap_floor_hull_white(**arguments, payment_frequency="quarterly")

    del encoded["index_id"]
    with pytest.raises(ValueError, match="index_id"):
        CalibrationStep.from_json(json.dumps(encoded))


@pytest.mark.parametrize("settings", [{"tolerance": -1.0}, {"tolerance": 0.0}, {"max_iterations": 0}])
def test_solver_config_constructor_and_json_reject_invalid_settings(settings: dict) -> None:
    with pytest.raises(ValueError, match=r"must be .*positive"):
        SolverConfig(**settings)
    with pytest.raises(ValueError, match=r"must be .*positive"):
        SolverConfig.from_json(json.dumps(settings))


def test_result_loading_rejects_metadata_beyond_canonical_depth_limit() -> None:
    payload = json.loads(calibrate(CalibrationPlan([])).to_json())
    metadata: dict = {"leaf": 1}
    for _ in range(110):
        metadata = {"child": metadata}
    payload["result"]["report"]["explanation"] = {
        "type": "calibration",
        "entries": [{"kind": "computation_step", "name": "probe", "description": "nested", "metadata": metadata}],
    }
    with pytest.raises(ContractLimitExceededError, match="depth"):
        CalibrationResult.from_json(json.dumps(payload))


def test_cap_floor_quote_rejects_lognormal_construction_and_json() -> None:
    with pytest.raises(ValueError, match="normal/Bachelier"):
        CapFloorQuote(5.0, 0.04, 0.2, is_normal_vol=False)
    with pytest.raises(ValueError, match="normal/Bachelier"):
        CapFloorQuote.from_json(
            json.dumps({"maturity": 5.0, "strike": 0.04, "volatility": 0.2, "is_cap": True, "is_normal_vol": False})
        )
