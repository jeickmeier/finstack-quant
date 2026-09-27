"""Typed EquityOption / CdsTranche / ConvertibleBond bindings."""

from __future__ import annotations

import datetime
import json

import pytest

from finstack_quant.cashflows.builder import RollRule
from finstack_quant.core.currency import Currency
from finstack_quant.core.dates import DayCount, Tenor
from finstack_quant.core.money import Money
from finstack_quant.valuations.instruments import (
    CdsTranche,
    ConvertibleBond,
    EquityOption,
)
from tests.tests_typed_helpers import (
    build_cds_tranche as _cds_tranche,
    build_convertible as _convertible,
    build_equity_option as _equity_option,
)


class TestEquityOptionTyped:
    def test_builder_round_trip(self) -> None:
        option = _equity_option()
        payload = json.loads(option.to_json())
        assert payload["instrument"]["type"] == "equity_option"
        assert EquityOption.from_json(option.to_json()).id == "AAPL-C-200"

    def test_invalid_option_type_raises(self) -> None:
        with pytest.raises(ValueError, match="invalid option_type"):
            EquityOption.builder().option_type("butterfly")

    def test_put_option_type_and_american_exercise_style(self) -> None:
        option = (
            EquityOption
            .builder()
            .id("AAPL-P-180")
            .underlying_ticker("AAPL")
            .strike(180.0)
            .option_type("put")
            .exercise_style("american")
            .expiry(datetime.date(2025, 6, 20))
            .quantity(100.0)
            .currency("USD")
            .discount_curve_id("USD-OIS")
            .spot_id("AAPL")
            .vol_surface_id("AAPL-VOL")
            .build()
        )
        payload = json.loads(option.to_json())
        assert payload["instrument"]["type"] == "equity_option"

    def test_invalid_exercise_style_raises(self) -> None:
        with pytest.raises(ValueError, match="invalid exercise_style"):
            EquityOption.builder().exercise_style("bahamian")

    def test_div_yield_id_and_discrete_dividends_and_exercise_schedule(self) -> None:
        option = (
            EquityOption
            .builder()
            .id("AAPL-C-200-DIV")
            .underlying_ticker("AAPL")
            .strike(200.0)
            .option_type("call")
            .exercise_style("bermudan")
            .expiry(datetime.date(2025, 6, 20))
            .quantity(100.0)
            .currency("USD")
            .discount_curve_id("USD-OIS")
            .spot_id("AAPL")
            .vol_surface_id("AAPL-VOL")
            .div_yield_id("AAPL-DIVYIELD")
            .discrete_dividends([(datetime.date(2025, 3, 1), 0.5)])
            .exercise_dates([datetime.date(2025, 3, 20), datetime.date(2025, 6, 20)])
            .build()
        )
        payload = json.loads(option.to_json())
        assert payload["instrument"]["type"] == "equity_option"
        assert EquityOption.from_json(option.to_json()).id == "AAPL-C-200-DIV"

    def test_settlement_and_exercise_lifecycle_round_trip(self) -> None:
        expiry = datetime.date(2025, 6, 20)
        settlement = datetime.date(2025, 6, 23)
        option = (
            EquityOption
            .builder()
            .id("AAPL-C-200-EXERCISED")
            .underlying_ticker("AAPL")
            .strike(200.0)
            .option_type("call")
            .exercise_style("european")
            .expiry(expiry)
            .settlement("physical")
            .exercise(expiry, 215.0, settlement, True)
            .quantity(100.0)
            .currency("USD")
            .discount_curve_id("USD-OIS")
            .spot_id("AAPL")
            .vol_surface_id("AAPL-VOL")
            .build()
        )

        payload = json.loads(option.to_json())["instrument"]["spec"]
        assert payload["settlement"] == "physical"
        # `theta_day_basis` is a metric pricing override, not an EquityOption field.
        assert "theta_day_basis" not in payload
        assert payload["exercise"] == {
            "date": "2025-06-20",
            "spot": 215.0,
            "settlement_date": "2025-06-23",
            "exercised": True,
        }
        assert EquityOption.from_json(option.to_json()).id == "AAPL-C-200-EXERCISED"


class TestCdsTrancheTyped:
    def test_builder_round_trip(self) -> None:
        tranche = _cds_tranche()
        payload = json.loads(tranche.to_json())
        assert payload["instrument"]["type"] == "cds_tranche"
        assert CdsTranche.from_json(tranche.to_json()).id == "CDX-IG-42-3-7"

    def test_sell_protection_side_and_overrides(self) -> None:
        tranche = (
            CdsTranche
            .builder()
            .id("CDX-IG-42-0-3")
            .index_name("CDX.NA.IG")
            .series(42)
            .attach_pct(0.0)
            .detach_pct(3.0)
            .notional(Money(5_000_000.0, Currency("USD")))
            .maturity(datetime.date(2029, 6, 20))
            .coupon_bp(500.0)
            .frequency(Tenor.quarterly())
            .day_count(DayCount.ACT_360)
            .calendar_id("NYSE")
            .discount_curve_id("USD-OIS")
            .credit_index_id("CDX-IG-42-CURVE")
            .side("receive")
            .start_date(datetime.date(2024, 6, 20))
            .realized_loss(0.01)
            .roll_rule(RollRule.NONE)
            .build()
        )
        payload = json.loads(tranche.to_json())
        assert payload["instrument"]["type"] == "cds_tranche"

    def test_invalid_side_case_raises(self) -> None:
        with pytest.raises(ValueError, match="invalid side"):
            CdsTranche.builder().side("buy_protection")  # schema-rejection-test: now "pay"


class TestConvertibleBondTyped:
    def test_builder_with_conversion_round_trips(self) -> None:
        bond = _convertible()
        payload = json.loads(bond.to_json())
        assert payload["instrument"]["type"] == "convertible_bond"
        assert ConvertibleBond.from_json(bond.to_json()).id == "CONV-1"

    def test_invalid_conversion_raises(self) -> None:
        with pytest.raises(ValueError, match="invalid conversion"):
            ConvertibleBond.builder().conversion("{nope")

    def test_conversion_pascal_case_enum_raises(self) -> None:
        conversion = json.dumps({
            "ratio": 20.0,
            "price": None,
            "policy": "Voluntary",  # schema-rejection-test
            "anti_dilution": "FullRatchet",  # schema-rejection-test
            "dividend_adjustment": "none",
            "dilution_events": [],
        })
        with pytest.raises(ValueError, match="invalid conversion"):
            ConvertibleBond.builder().conversion(conversion)

    def test_credit_curve_and_recovery_and_settlement_days(self) -> None:
        conversion = json.dumps({
            "ratio": None,
            "price": 50.0,
            "policy": "voluntary",
            "anti_dilution": "none",
            "dividend_adjustment": "adjust_ratio",
            "dilution_events": [],
        })
        bond = (
            ConvertibleBond
            .builder()
            .id("CONV-2")
            .notional(Money(1_000.0, Currency("USD")))
            .issue_date(datetime.date(2024, 1, 15))
            .maturity(datetime.date(2029, 1, 15))
            .discount_curve_id("USD-OIS")
            .credit_curve_id("USD-CREDIT-BBB")
            .conversion(conversion)
            .cashflow_spec(ConvertibleBond.example().cashflow_spec)
            .spot_id("ACME")
            .vol_surface_id("ACME-VOL")
            .settlement_days(2)
            .recovery_rate(0.4)
            .build()
        )
        payload = json.loads(bond.to_json())
        assert payload["instrument"]["type"] == "convertible_bond"
