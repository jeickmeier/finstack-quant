"""Behavioral tests for direct initial-margin Python bindings."""

from __future__ import annotations

import datetime as dt
import json

import pytest

from finstack_quant.core.config import FinstackConfig
from finstack_quant.margin import (
    CollateralAssetClass,
    CsaSpec,
    EligibleCollateralSchedule,
    HaircutImCalculator,
    ImResult,
    ScheduleImCalculator,
    SimmCalculator,
    SimmCurvatureSensitivity,
    SimmSensitivities,
)


def test_simm_calculator_from_sensitivities() -> None:
    sens = SimmSensitivities("USD")
    sens.add_ir_delta("USD", "5Y", 25_000.0)
    sens.add_ir_vega("USD", "5Y", 5_000.0)
    sens.add_credit_qualifying_delta("financial", "BANK_A", "5Y", 12_000.0)
    sens.add_credit_non_qualifying_delta("RMBS-1", "5Y", 3_000.0)
    sens.add_equity_delta("SPX", 40_000.0)
    sens.add_fx_delta("EUR", 15_000.0)

    result = SimmCalculator("v2_6").calculate_from_sensitivities(
        sens,
        "USD",
        dt.date(2025, 1, 15),
    )

    assert isinstance(result, ImResult)
    assert result.amount > 0.0
    assert result.currency == "USD"
    assert str(result.methodology) == "simm"
    assert result.mpor_days == 10
    assert result.as_of == dt.date(2025, 1, 15)
    assert "IR_Delta" in result.breakdown_keys()
    assert result.breakdown_amount("IR_Delta") is not None


def test_ambiguous_credit_delta_methods_are_removed() -> None:
    sensitivities = SimmSensitivities("USD")

    assert not hasattr(sensitivities, "add_credit_delta")
    assert not hasattr(sensitivities, "add_credit_delta_bucketed")


def test_simm_sensitivities_json_round_trip() -> None:
    sens = SimmSensitivities("USD")
    sens.add_ir_delta("USD", "2Y", 10_000.0)
    sens.add_fx_vega("EUR", "USD", 2_500.0)
    sens.add_credit_qualifying_delta("sovereign", "GOVT_A", "5Y", 4_000.0)
    sens.add_commodity_delta("Crude", 7_500.0)
    sens.add_curvature(SimmCurvatureSensitivity("equity", "residual", "ACME", "1Y", 1_250.0))

    out = SimmSensitivities.from_json(sens.to_json())
    parsed = json.loads(out.to_json())

    assert parsed["base_currency"] == "USD"
    assert parsed["credit_qualifying_delta"] == [["sovereign", "GOVT_A", "5Y", 4_000.0]]
    assert not out.is_empty()


def test_schedule_im_gross_and_ngr_paths() -> None:
    calc = ScheduleImCalculator.bcbs_standard()

    gross = calc.calculate_for_notional(
        100_000_000.0,
        "USD",
        "interest_rate",
        5.0,
        dt.date(2025, 1, 15),
    )
    netted = calc.calculate_netting_set_with_ngr(
        [(2_000_000.0, 100_000_000.0, "interest_rate", 5.0), (-1_500_000.0, 80_000_000.0, "interest_rate", 5.0)],
        "USD",
        dt.date(2025, 1, 15),
    )

    assert gross.amount > 0.0
    assert gross.currency == "USD"
    assert str(gross.methodology) == "schedule"
    assert gross.as_of == dt.date(2025, 1, 15)
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
        dt.date(2025, 1, 15),
    )
    with_fx = calc.calculate_for_collateral(
        10_000_000.0,
        "USD",
        cash,
        True,
        dt.date(2025, 1, 15),
    )

    assert str(no_fx.methodology) == "haircut"
    assert no_fx.amount == pytest.approx(0.0)
    assert with_fx.amount > no_fx.amount
    assert with_fx.breakdown_amount(str(cash)) == pytest.approx(with_fx.amount)


def test_simm_rejects_unknown_tenor_instead_of_pricing_zero() -> None:
    """A mistyped tenor used to price silently to zero IM."""
    sens = SimmSensitivities("USD")
    sens.add_ir_delta("USD", "7Y", 50_000.0)

    with pytest.raises(ValueError, match="7Y"):
        sens.validate()
    with pytest.raises(ValueError, match="7Y"):
        SimmCalculator().calculate_from_sensitivities(sens, "USD", "2025-01-15")

    bad_bucket = SimmSensitivities("USD")
    bad_bucket.add_commodity_delta("bucket 18", 1.0)
    with pytest.raises(ValueError, match="bucket"):
        bad_bucket.validate()


def test_simm_sensitivities_dataframe_round_trip_and_helpers() -> None:
    sens = SimmSensitivities("USD")
    sens.add_ir_delta("USD", "5Y", 25_000.0)
    sens.add_ir_vega("USD", "5Y", 5_000.0)
    sens.add_credit_qualifying_delta("financial", "BANK_A", "5Y", 12_000.0)
    sens.add_credit_qualifying_vega("financial", "BANK_A", "5Y", 1_000.0)
    sens.add_credit_non_qualifying_vega("RMBS-1", "5Y", 300.0)
    sens.add_equity_delta("SPX", 40_000.0)
    sens.add_fx_vega("EUR", "USD", 2_500.0)
    sens.add_commodity_delta("Crude", 7_500.0)
    sens.add_commodity_vega("Crude", 750.0)
    sens.add_curvature(SimmCurvatureSensitivity("equity", "residual", "ACME", "1Y", 1_250.0))

    restored = SimmSensitivities.from_dataframe(sens.to_dataframe(), "USD")
    assert json.loads(restored.to_json()) == json.loads(sens.to_json())

    assert sens.total_ir_delta() == pytest.approx(25_000.0)
    assert sens.total_equity_delta() == pytest.approx(40_000.0)
    assert sens.scaled(-2.0).total_ir_delta() == pytest.approx(-50_000.0)
    eur = sens.scaled_to_currency("EUR", 0.9)
    assert eur.base_currency == "EUR"
    assert eur.total_ir_delta() == pytest.approx(22_500.0)

    merged = SimmSensitivities("USD")
    merged.merge(sens)
    merged.merge(sens)
    assert merged.total_ir_delta() == pytest.approx(50_000.0)
    with pytest.raises(ValueError, match="scaled_to_currency"):
        merged.merge(eur)
    assert "ir_delta=1" in repr(sens)


def test_simm_dataframe_requires_and_keeps_the_base_currency() -> None:
    """MSAF-002/004: no invented USD; the Rust ``from_rows`` decodes the frame."""
    with pytest.raises(TypeError):
        SimmSensitivities()  # type: ignore[call-arg]
    sens = SimmSensitivities("EUR")
    sens.add_ir_delta("EUR", "5Y", 1_000.0)
    sens.add_fx_vega("EUR", "USD", 2.0)
    with pytest.raises(TypeError):
        SimmSensitivities.from_dataframe(sens.to_dataframe())  # type: ignore[call-arg]
    restored = SimmSensitivities.from_dataframe(sens.to_dataframe(), "EUR")
    assert restored.base_currency == "EUR"
    assert json.loads(restored.to_json()) == json.loads(sens.to_json())
    with pytest.raises(ValueError, match="unsupported SIMM"):
        SimmSensitivities.from_dataframe(sens.to_dataframe().assign(kind="gamma"), "EUR")


def _margin_overlay(overlay: dict[str, object]) -> FinstackConfig:
    config = FinstackConfig()
    config.set_extension("margin.registry.v1", overlay)
    return config


def test_margin_constructors_apply_a_finstack_config_registry_overlay() -> None:
    """MSAF-007: the FinstackConfig registry-overlay constructors are bound."""
    empty = FinstackConfig()
    assert SimmCalculator.from_finstack_config("v2_6", empty).mpor_days == SimmCalculator("v2_6").mpor_days
    with pytest.raises(ValueError, match="not_a_section"):
        SimmCalculator.from_finstack_config("v2_6", _margin_overlay({"simm": {"not_a_section": 1}}))

    vm = _margin_overlay({"defaults": {"defaults": {"vm": {"mta": 250_000.0}}}})
    configured = json.loads(CsaSpec.regulatory_from_config(vm, "USD", "CSA-1", "USD-OIS").to_json())
    embedded = json.loads(CsaSpec.regulatory("USD", "CSA-1", "USD-OIS").to_json())
    assert configured["vm_params"]["mta"]["amount"] == "250000"
    assert configured["vm_params"]["mta"] != embedded["vm_params"]["mta"]

    desk = _margin_overlay({
        "collateral_schedules": {
            "entries": [
                {
                    "ids": ["desk_cash"],
                    "record": {
                        "eligible": [{"asset_class": "cash", "haircut": 0.01, "fx_haircut_addon": 0.0}],
                        "default_haircut": None,
                        "rehypothecation_allowed": True,
                    },
                }
            ]
        }
    })
    schedule = EligibleCollateralSchedule.from_finstack_config(desk, "desk_cash")
    assert schedule.haircut_for("cash") == pytest.approx(0.01)
    with pytest.raises(ValueError, match="not found"):
        EligibleCollateralSchedule.from_finstack_config(empty, "desk_cash")

    grid = _margin_overlay({
        "schedule_im": {
            "entries": [
                {
                    "ids": ["bcbs_iosco"],
                    "record": {
                        "bucket_boundaries_years": {"short_to_medium": 2.0, "medium_to_long": 5.0},
                        "default_rate": 0.15,
                        "default_asset_class": "interest_rate",
                        "default_maturity_years": 5.0,
                        "mpor_days": 7,
                        "rates": [{"asset_class": "interest_rate", "bucket": "short", "rate": 0.03}],
                    },
                }
            ]
        }
    })
    assert ScheduleImCalculator.from_finstack_config(grid).mpor_days == 7
    assert ScheduleImCalculator.from_finstack_config(empty).mpor_days == ScheduleImCalculator.bcbs_standard().mpor_days
