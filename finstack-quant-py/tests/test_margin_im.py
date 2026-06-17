"""Behavioral tests for direct initial-margin Python bindings."""

from __future__ import annotations

import json

import pytest

from finstack_quant.margin import (
    CollateralAssetClass,
    HaircutImCalculator,
    ImResult,
    ScheduleImCalculator,
    SimmCalculator,
    SimmSensitivities,
)


def test_simm_calculator_from_sensitivities() -> None:
    sens = SimmSensitivities("USD")
    sens.add_ir_delta("USD", "5Y", 25_000.0)
    sens.add_ir_vega("USD", "5Y", 5_000.0)
    sens.add_credit_delta_bucketed("financial", "BANK_A", "5Y", 12_000.0)
    sens.add_equity_delta("SPX", 40_000.0)
    sens.add_fx_delta("EUR", 15_000.0)

    result = SimmCalculator("v2_6").calculate_from_sensitivities(
        sens,
        "USD",
        2025,
        1,
        15,
    )

    assert isinstance(result, ImResult)
    assert result.amount > 0.0
    assert result.currency == "USD"
    assert str(result.methodology) == "simm"
    assert result.mpor_days == 10
    assert result.as_of == "2025-01-15"
    assert "IR_Delta" in result.breakdown_keys()
    assert result.breakdown_amount("IR_Delta") is not None


def test_simm_sensitivities_json_round_trip() -> None:
    sens = SimmSensitivities("USD")
    sens.add_ir_delta("USD", "2Y", 10_000.0)
    sens.add_fx_vega("EUR", "USD", 2_500.0)
    sens.add_commodity_delta("energy", 7_500.0)
    sens.add_curvature("equity", 1_250.0)

    out = SimmSensitivities.from_json(sens.to_json())
    parsed = json.loads(out.to_json())

    assert parsed["base_currency"] == "USD"
    assert not out.is_empty()


def test_schedule_im_gross_and_ngr_paths() -> None:
    calc = ScheduleImCalculator.bcbs_standard()

    gross = calc.calculate_for_notional(
        100_000_000.0,
        "USD",
        "interest_rate",
        5.0,
        2025,
        1,
        15,
    )
    netted = calc.calculate_netting_set_with_ngr(
        [(2_000_000.0, 100_000_000.0), (-1_500_000.0, 80_000_000.0)],
        "USD",
        "interest_rate",
        5.0,
        2025,
        1,
        15,
    )

    assert gross.amount > 0.0
    assert gross.currency == "USD"
    assert str(gross.methodology) == "schedule"
    assert gross.as_of == "2025-01-15"
    assert gross.breakdown_amount("interest_rate") == pytest.approx(gross.amount)

    assert netted is not None
    assert netted.amount > 0.0
    assert netted.amount < gross.amount * 2.0
    assert netted.breakdown_amount("interest_rate_ngr") == pytest.approx(netted.amount)


def test_haircut_im_calculator_applies_fx_addon() -> None:
    calc = HaircutImCalculator.bcbs_standard()
    cash = CollateralAssetClass.cash()

    no_fx = calc.calculate_for_collateral(
        10_000_000.0,
        "USD",
        cash,
        False,
        2025,
        1,
        15,
    )
    with_fx = calc.calculate_for_collateral(
        10_000_000.0,
        "USD",
        cash,
        True,
        2025,
        1,
        15,
    )

    assert str(no_fx.methodology) == "haircut"
    assert no_fx.amount == pytest.approx(0.0)
    assert with_fx.amount > no_fx.amount
    assert with_fx.breakdown_amount(str(cash)) == pytest.approx(with_fx.amount)
