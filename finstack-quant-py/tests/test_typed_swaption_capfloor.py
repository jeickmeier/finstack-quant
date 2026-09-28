"""Typed Swaption and CapFloor bindings: build, round-trip, dispatch."""

from __future__ import annotations

import datetime
import json

import pytest

from finstack_quant.core.currency import Currency
from finstack_quant.core.dates import DayCount, Tenor
from finstack_quant.core.money import Money
from finstack_quant.valuations.instruments import (
    CapFloor,
    Swaption,
    TermLoan,
)
from tests.tests_typed_helpers import build_capfloor as _cap, build_swaption as _payer_swaption, swaption_legs as _legs


class TestSwaptionTyped:
    def test_builder_and_tagged_json(self) -> None:
        payload = json.loads(_payer_swaption().to_json())
        assert payload["instrument"]["type"] == "swaption"
        assert payload["instrument"]["spec"]["id"] == "SWPT-1"

    def test_bermudan_exercise_style_round_trips(self) -> None:
        fixed, float_leg = _legs()
        swpt = (
            Swaption
            .builder()
            .id("SWPT-B")
            .option_type("call")
            .notional(Money(1_000_000.0, Currency("USD")))
            .expiry(datetime.date(2025, 1, 13))
            .exercise_style("bermudan")
            .settlement("physical")
            .cash_settlement_method("par_yield")
            .vol_model("normal")
            .vol_surface_id("USD-SWPT-VOL")
            .underlying_fixed_leg(fixed)
            .underlying_float_leg(float_leg)
            .build()
        )
        assert Swaption.from_json(swpt.to_json()).id == "SWPT-B"

    def test_american_exercise_style_accepted(self) -> None:
        fixed, float_leg = _legs()
        swpt = (
            Swaption
            .builder()
            .id("SWPT-A")
            .option_type("put")
            .notional(Money(1_000_000.0, Currency("USD")))
            .expiry(datetime.date(2025, 1, 13))
            .exercise_style("american")
            .settlement("physical")
            .cash_settlement_method("isda_par_par")
            .vol_model("black")
            .vol_surface_id("USD-SWPT-VOL")
            .underlying_fixed_leg(fixed)
            .underlying_float_leg(float_leg)
            .build()
        )
        assert swpt.id == "SWPT-A"

    @pytest.mark.parametrize("value", ["call", "put"])
    def test_every_option_type_literal_value_accepted(self, value: str) -> None:
        assert _payer_swaption(option_type=value).id == "SWPT-1"

    @pytest.mark.parametrize("value", ["european", "bermudan", "american"])
    def test_every_exercise_style_literal_value_accepted(self, value: str) -> None:
        assert _payer_swaption(exercise_style=value).id == "SWPT-1"

    @pytest.mark.parametrize("value", ["physical", "cash"])
    def test_every_settlement_literal_value_accepted(self, value: str) -> None:
        assert _payer_swaption(settlement=value).id == "SWPT-1"

    @pytest.mark.parametrize(
        "value",
        ["collateralized_cash_price", "par_yield", "isda_par_par", "zero_coupon"],
    )
    def test_every_cash_settlement_method_literal_value_accepted(self, value: str) -> None:
        assert _payer_swaption(cash_settlement_method=value).id == "SWPT-1"

    @pytest.mark.parametrize("value", ["black", "normal"])
    def test_every_vol_model_literal_value_accepted(self, value: str) -> None:
        assert _payer_swaption(vol_model=value).id == "SWPT-1"

    def test_sabr_params_json_string_accepted(self) -> None:
        fixed, float_leg = _legs()
        swpt = (
            Swaption
            .builder()
            .id("SWPT-SABR")
            .option_type("call")
            .notional(Money(1_000_000.0, Currency("USD")))
            .expiry(datetime.date(2025, 1, 13))
            .exercise_style("european")
            .settlement("cash")
            .cash_settlement_method("par_yield")
            .vol_model("black")
            .vol_surface_id("USD-SWPT-VOL")
            .underlying_fixed_leg(fixed)
            .underlying_float_leg(float_leg)
            .sabr_params(json.dumps({"alpha": 0.025, "beta": 0.5, "nu": 0.4, "rho": -0.3, "shift": None}))
            .build()
        )
        payload = json.loads(swpt.to_json())
        assert payload["instrument"]["spec"]["sabr_params"]["alpha"] == 0.025

    def test_invalid_exercise_style_raises(self) -> None:
        with pytest.raises(ValueError, match="invalid exercise_style"):
            Swaption.builder().exercise_style("asian")

    def test_from_json_rejects_wrong_type(self) -> None:
        with pytest.raises(ValueError, match="swaption"):
            Swaption.from_json(TermLoan.example().to_json())


class TestCapFloorTyped:
    def test_builder_and_round_trip(self) -> None:
        cap = _cap()
        payload = json.loads(cap.to_json())
        assert payload["instrument"]["type"] == "cap_floor"
        assert CapFloor.from_json(cap.to_json()).id == "CAP-1"

    @pytest.mark.parametrize("value", ["cap", "floor", "caplet", "floorlet"])
    def test_every_rate_option_type_literal_value_accepted(self, value: str) -> None:
        assert _cap(rate_option_type=value).id == "CAP-1"

    @pytest.mark.parametrize("value", ["lognormal", "shifted_lognormal", "normal", "auto"])
    def test_every_vol_type_literal_value_accepted(self, value: str) -> None:
        assert _cap(vol_type=value).id == "CAP-1"

    def test_optional_setters_accepted(self) -> None:
        cap = (
            CapFloor
            .builder()
            .id("CAP-2")
            .rate_option_type("floor")
            .notional(Money(2_000_000.0, Currency("USD")))
            .strike(0.03)
            .spread_bp(10.0)
            .premium(
                datetime.date(2024, 2, 15),
                Money(25_000.0, Currency("USD")),
            )
            .start_date(datetime.date(2024, 1, 15))
            .maturity(datetime.date(2027, 1, 15))
            .frequency(Tenor.quarterly())
            .day_count(DayCount.ACT_360)
            .calendar_id("nyse")
            .discount_curve_id("USD-OIS")
            .forward_curve_id("USD-SOFR-3M")
            .vol_surface_id("USD-CAP-VOL")
            .vol_type("shifted_lognormal")
            .vol_shift(0.02)
            .build()
        )
        payload = json.loads(cap.to_json())
        assert payload["instrument"]["spec"]["spread_bp"] == "10"
        assert payload["instrument"]["spec"]["premium"] == [
            "2024-02-15",
            {"amount": "25000", "currency": "USD"},
        ]
        assert payload["instrument"]["spec"]["vol_shift"] == 0.02
        assert payload["instrument"]["spec"]["calendar_id"] == "nyse"

    def test_missing_required_field_raises(self) -> None:
        with pytest.raises(ValueError, match="missing required field"):
            CapFloor.builder().id("CAP-BAD").build()

    def test_nan_strike_raises(self) -> None:
        with pytest.raises(ValueError, match="invalid strike"):
            CapFloor.builder().strike(float("nan"))

    def test_rate_option_type_wrong_case_is_rejected(self) -> None:
        with pytest.raises(ValueError, match="invalid rate_option_type"):
            CapFloor.builder().rate_option_type("Cap")

    def test_vol_type_wrong_case_is_rejected(self) -> None:
        with pytest.raises(ValueError, match="invalid vol_type"):
            CapFloor.builder().vol_type("Auto")
