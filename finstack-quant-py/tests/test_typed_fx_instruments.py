"""Typed FxForward / FxOption bindings."""

from __future__ import annotations

import datetime
import json

import pytest

from finstack_quant.core.currency import Currency
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money
from finstack_quant.valuations import ValuationResult
from finstack_quant.valuations.instruments import FxForward, FxOption, price_instrument
from tests.golden.conftest import fixture_path
from tests.golden.schema import GoldenFixture
from tests.tests_typed_helpers import build_fx_forward as _forward

# The typed-vs-JSON-loader NPV gap observed for this fixture is ~3.6e-11
# (float64 JSON round-trip noise on the same Rust pricing engine, not a
# model difference) -- see TestFxForwardTyped.test_matches_quantlib_golden_npv.
_TYPED_GOLDEN_NPV_ABS_TOLERANCE = 1e-6


class TestFxForwardTyped:
    def test_envelope_json_and_round_trip(self) -> None:
        payload = json.loads(_forward().to_json())
        assert payload["instrument"]["type"] == "fx_forward"
        assert FxForward.from_json(_forward().to_json()).id == "EURUSD-FWD-6M"

    def test_same_base_and_quote_currency_rejected(self) -> None:
        with pytest.raises(ValueError, match="must differ from quote_currency"):
            (
                FxForward
                .builder()
                .id("BAD")
                .base_currency(Currency("USD"))
                .quote_currency(Currency("USD"))
                .maturity(datetime.date(2025, 6, 15))
                .notional(Money(1.0, Currency("USD")))
                .domestic_discount_curve_id("USD-OIS")
                .foreign_discount_curve_id("USD-OIS")
                .build()
            )

    def test_matches_quantlib_golden_npv(self) -> None:
        """A typed FxForward reproduces the checked-in QuantLib golden NPV.

        Builds the instrument via the typed `FxForward.builder()` (not the
        JSON loader the `tests/golden` runners use) but prices it against
        the exact same market snapshot, valuation date, and model recorded
        in the `eurusd_1y_forward_quantlib` fixture, per the market-setup
        convention in `tests/golden/runners/pricing_common.py`.

        The tolerance is far tighter than the fixture's own QuantLib-vs-Finstack
        tolerance (`abs=0.01`, for cross-library agreement): both this test and
        the golden runner exercise the same Rust discounting engine, just via a
        different instrument-construction path (typed builder -> instrument envelope
        vs. JSON loader), so the two NPVs are expected to agree to within
        float64 JSON round-trip noise (empirically ~3.6e-11), not model-level
        tolerance.
        """
        fixture = GoldenFixture.from_path(fixture_path("pricing/quantlib/fx_forward/eurusd_1y_forward_quantlib.json"))
        spec = fixture.body["instrument"]["instrument"]["spec"]
        market = MarketContext.from_json(json.dumps(fixture.body["market"]["data"]))

        forward = (
            FxForward
            .builder()
            .id(spec["id"])
            .base_currency(Currency(spec["base_currency"]))
            .quote_currency(Currency(spec["quote_currency"]))
            .maturity(datetime.date.fromisoformat(spec["maturity"]))
            .notional(Money(float(spec["notional"]["amount"]), Currency(spec["notional"]["currency"])))
            .contract_rate(spec["contract_rate"])
            .domestic_discount_curve_id(spec["domestic_discount_curve_id"])
            .foreign_discount_curve_id(spec["foreign_discount_curve_id"])
            .build()
        )

        result = price_instrument(forward, market, fixture.metadata.valuation_date, model=fixture.body["model"])
        assert isinstance(result, ValuationResult)
        actual_npv = float(result.price)
        expected_npv = fixture.expected["npv"]
        assert actual_npv == pytest.approx(expected_npv, abs=_TYPED_GOLDEN_NPV_ABS_TOLERANCE)


class TestFxOptionTyped:
    def test_builder_round_trip(self) -> None:
        option = (
            FxOption
            .builder()
            .id("EURUSD-CALL-1Y")
            .base_currency(Currency("EUR"))
            .quote_currency(Currency("USD"))
            .strike(1.12)
            .option_type("call")
            .delta_convention({"kind": "forward", "premium_currency": "USD", "venue": "generic_interbank"})
            .expiry(datetime.date(2025, 12, 15))
            .notional(Money(1_000_000.0, Currency("EUR")))
            .domestic_discount_curve_id("USD-OIS")
            .foreign_discount_curve_id("EUR-OIS")
            .vol_surface_id("EURUSD-VOL")
            .build()
        )
        payload = json.loads(option.to_json())
        assert payload["instrument"]["type"] == "fx_option"
        assert FxOption.from_json(option.to_json()).id == "EURUSD-CALL-1Y"

    def test_invalid_option_type_raises(self) -> None:
        with pytest.raises(ValueError, match="invalid option_type"):
            FxOption.builder().option_type("straddle")

    def test_delta_convention_wrong_case_is_rejected(self) -> None:
        with pytest.raises(ValueError, match="invalid FxDeltaConvention"):
            FxOption.builder().delta_convention({"kind": "Forward", "premium_currency": "USD", "venue": "test"})

    def test_delta_convention_blank_venue_is_rejected(self) -> None:
        with pytest.raises(ValueError, match="venue must not be blank"):
            FxOption.builder().delta_convention({"kind": "forward", "premium_currency": "USD", "venue": "  "})

    def test_delta_convention_getter_round_trips_into_setter(self) -> None:
        example = FxOption.example()
        builder = FxOption.builder().delta_convention(example.delta_convention)
        assert "'venue': 'generic_interbank'" in repr(builder)
        json_builder = FxOption.builder().delta_convention(json.dumps(example.delta_convention))
        assert "'kind': 'forward'" in repr(json_builder)

    @pytest.mark.parametrize(
        "value",
        ["spot", "forward", "premium_adjusted_spot", "premium_adjusted_forward"],
    )
    def test_every_delta_convention_literal_value_accepted(self, value: str) -> None:
        option = (
            FxOption
            .builder()
            .id("EURUSD-DELTA")
            .base_currency(Currency("EUR"))
            .quote_currency(Currency("USD"))
            .strike(1.12)
            .option_type("call")
            .delta_convention({"kind": value, "premium_currency": Currency("EUR"), "venue": "test"})
            .expiry(datetime.date(2025, 12, 15))
            .notional(Money(1_000_000.0, Currency("EUR")))
            .domestic_discount_curve_id("USD-OIS")
            .foreign_discount_curve_id("EUR-OIS")
            .vol_surface_id("EURUSD-VOL")
            .build()
        )
        payload = json.loads(option.to_json())
        assert payload["instrument"]["spec"]["delta_convention"]["kind"] == value
