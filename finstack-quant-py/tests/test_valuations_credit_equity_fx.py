"""Typed credit / equity / FX instrument bindings: getters, presets, pricing.

Covers the B1b remediation of the valuations parity audit: every typed
wrapper exposes its Rust fields, the Rust convenience constructors and
``example()`` factories, ``price`` / ``metric`` on the instrument itself, the
typed helper classes (``CDSIndexParams``, ``CDSIndexConstituent``,
``CDSTrancheParams``, ``ConversionSpec``, ``CallPutSchedule``) and the loose
input forms (ISO date strings, ``float | Bps``, dict-or-JSON specs).
"""

from __future__ import annotations

import datetime
import json
import math
from pathlib import Path
import pickle

import pytest

from finstack_quant.calibration import calibrate
from finstack_quant.core.currency import Currency
from finstack_quant.core.dates import DayCount, Tenor
from finstack_quant.core.market_data import (
    DiscountCurve,
    FxMatrix,
    MarketContext,
    VolSurface,
)
from finstack_quant.core.money import Money
from finstack_quant.core.types import Attributes, Bps
from finstack_quant.valuations import ValuationResult
from finstack_quant.valuations.instruments import (
    CallPutSchedule,
    CDSIndex,
    CDSIndexConstituent,
    CDSIndexParams,
    CDSTranche,
    CDSTrancheParams,
    ConversionSpec,
    ConvertibleBond,
    CreditDefaultSwap,
    EquityOption,
    FxForward,
    FxOption,
    PremiumLegSpec,
    ProtectionLegSpec,
)

USD = Currency("USD")
EUR = Currency("EUR")
BASE = datetime.date(2024, 1, 1)


def _discount(curve_id: str, rate: float) -> DiscountCurve:
    knots = [(t, math.exp(-rate * t)) for t in (0.0, 1.0, 5.0, 10.0)]
    return DiscountCurve(curve_id, BASE, knots, day_count="act_365f")


def _credit_market() -> MarketContext:
    fixture_path = (
        Path(__file__).parents[2] / "finstack-quant/calibration/examples/market_bootstrap/03_single_name_hazard.json"
    )
    template = json.loads(fixture_path.read_text())

    def calibrated_market(curve_id: str, entity: str, spread_bp: float | None = None) -> MarketContext:
        envelope = json.loads(json.dumps(template))
        envelope.pop("$schema", None)
        for step in envelope["plan"]["steps"]:
            step["base_date"] = BASE.isoformat()
            if step["kind"] == "hazard":
                step.update({"id": curve_id, "curve_id": curve_id, "entity": entity})
        for quote in envelope["market_data"]:
            if quote["kind"] == "cds_quote":
                quote["entity"] = entity
                if spread_bp is not None:
                    quote["spread_bp"] = spread_bp
        return calibrate(json.dumps(envelope)).market

    market = calibrated_market("CORP-HAZARD", "CORP")
    index_market = calibrated_market("CDX.NA.IG.HAZARD", "CDX.NA.IG", 60.0)
    market.insert(index_market.get_hazard("CDX.NA.IG.HAZARD"))
    return market


def _equity_market() -> MarketContext:
    market = MarketContext()
    market.insert(_discount("USD-OIS", 0.04))
    market.insert_price("EQUITY-SPOT", 4600.0, currency="USD")
    market.insert_price("EQUITY-DIVYIELD", 0.01)
    market.insert(
        VolSurface(
            "EQUITY-VOL",
            [0.25, 1.0, 2.0],
            [4000.0, 4500.0, 5000.0],
            [[0.2, 0.2, 0.2], [0.2, 0.2, 0.2], [0.2, 0.2, 0.2]],
        )
    )
    return market


def _fx_market() -> MarketContext:
    market = MarketContext()
    market.insert(_discount("USD-OIS", 0.04))
    market.insert(_discount("EUR-OIS", 0.02))
    fx = FxMatrix()
    fx.set_quote(EUR, USD, 1.10)
    market.insert_fx(fx)
    market.insert(
        VolSurface(
            "EURUSD-VOL",
            [0.5, 1.0, 2.0],
            [1.0, 1.12, 1.3],
            [[0.1, 0.1, 0.1], [0.1, 0.1, 0.1], [0.1, 0.1, 0.1]],
        )
    )
    return market


def _cds_legs() -> tuple[PremiumLegSpec, ProtectionLegSpec]:
    premium = PremiumLegSpec(
        datetime.date(2024, 3, 20),
        datetime.date(2029, 6, 20),
        Tenor.quarterly(),
        DayCount.ACT_360,
        100.0,
        "USD-OIS",
    )
    return premium, ProtectionLegSpec("CORP-HAZARD", 0.4, 3)


class TestCreditDefaultSwap:
    def test_example_prices_and_exposes_typed_getters(self) -> None:
        cds = CreditDefaultSwap.example()
        assert cds.id == "CDS-CORP-5Y"
        assert cds.side == "pay"
        assert cds.convention == "isda_na"
        assert isinstance(cds.notional, Money)
        assert isinstance(cds.premium, PremiumLegSpec)
        assert isinstance(cds.protection, ProtectionLegSpec)
        assert cds.upfront is None
        assert cds.doc_clause is None
        assert cds.doc_clause_effective == "xr14"
        assert cds.protection_start == datetime.date(2024, 3, 20)
        assert cds.valuation_convention == "bloomberg_cdsw_clean"
        assert isinstance(cds.default_model, str)
        assert isinstance(cds.attributes, Attributes)
        deps = cds.market_dependencies()
        assert "curves" in deps
        assert cds.to_dict()["id"] == "CDS-CORP-5Y"

        result = cds.price(_credit_market(), "2024-06-20", "hazard_rate")
        assert isinstance(result, ValuationResult)
        assert math.isfinite(result.price)

    def test_upfront_builds_prices_and_reports_cs01(self) -> None:
        premium, protection = _cds_legs()
        upfront_date = datetime.date(2024, 6, 25)
        cds = (
            CreditDefaultSwap
            .builder()
            .id("CDS-UPF")
            .notional(Money(10_000_000.0, USD))
            .side("pay")
            .convention("isda_na")
            .premium(premium)
            .protection(protection)
            .upfront((upfront_date, Money(-250_000.0, USD)))
            .doc_clause("xr14")
            .attributes({"desk": "credit", "tags": ["ig"]})
            .build()
        )
        date, amount = cds.upfront
        assert date == upfront_date
        assert amount.amount == pytest.approx(-250_000.0)
        assert cds.attributes.get_meta("desk") == "credit"
        assert json.loads(cds.to_json())["instrument"]["spec"]["upfront"] is not None

        result = cds.price(_credit_market(), "2024-06-20", "hazard_rate", metrics=["cs01"])
        keys = result.metric_keys()
        assert any(key.startswith("cs01") for key in keys), keys
        cs01 = cds.metric(_credit_market(), "2024-06-20", "cs01", "hazard_rate")
        assert math.isfinite(cs01)

    def test_get_par_spread_is_close_to_flat_hazard_spread(self) -> None:
        cds = CreditDefaultSwap.example()
        par_bp = cds.get_par_spread(_credit_market(), "2024-06-20")
        # 2% flat hazard, 40% recovery -> ~120bp credit-triangle par spread.
        assert 90.0 < par_bp < 150.0

    def test_convention_error_lists_accepted_strings(self) -> None:
        with pytest.raises(ValueError, match="isda_na"):
            CreditDefaultSwap.builder().convention("snac")

    def test_builder_repr_is_python_style(self) -> None:
        builder = CreditDefaultSwap.builder().id("CDS-R").notional(Money(1_000_000.0, USD)).side("pay")
        text = repr(builder)
        assert text.startswith('CreditDefaultSwapBuilder(id="CDS-R", notional=Money(')
        assert 'side="pay"' in text

    def test_instrument_repr_carries_economics(self) -> None:
        text = repr(CreditDefaultSwap.example())
        assert 'side="pay"' in text
        assert "spread_bp=100" in text
        assert "datetime.date(2029, 3, 20)" in text

    def test_iso_string_dates_and_margin_spec_dict(self) -> None:
        premium, protection = _cds_legs()
        cds = (
            CreditDefaultSwap
            .builder()
            .id("CDS-ISO")
            .notional(Money(1_000_000.0, USD))
            .side("receive")
            .convention("isda_eu")
            .premium(premium)
            .protection(protection)
            .protection_effective_date("2024-06-20")
            .valuation_convention("isda_dirty")
            .build()
        )
        assert cds.protection_effective_date == datetime.date(2024, 6, 20)
        assert cds.valuation_convention == "isda_dirty"
        assert cds.doc_clause_effective == "mm14"

    def test_pickle_round_trip(self) -> None:
        cds = CreditDefaultSwap.example()
        assert pickle.loads(pickle.dumps(cds)).to_json() == cds.to_json()  # noqa: S301


class TestCDSIndex:
    def test_from_preset_mirrors_rust(self) -> None:
        preset = CDSIndexParams.cdx_na_ig(42, 1, 100.0)
        assert preset == CDSIndexParams("CDX.NA.IG", 42, 1, Bps(100.0), num_constituents=125)
        index = CDSIndex.from_preset(
            preset,
            "CDX-IG-42-5Y",
            Money(10_000_000.0, USD),
            "pay",
            "2024-03-20",
            "2029-06-20",
            0.4,
            "USD-OIS",
            "CDX.NA.IG.HAZARD",
        )
        assert index.index_name == "CDX.NA.IG"
        assert index.series == 42
        assert index.version == 1
        assert index.num_constituents == 125
        assert index.index_factor == 1.0
        assert index.pricing == "single_curve"
        assert index.constituents == []
        assert index.convention == "isda_na"
        assert isinstance(index.premium, PremiumLegSpec)

        market = _credit_market()
        result = index.price(market, "2024-06-20")
        assert math.isfinite(result.price)
        assert index.risky_pv01(market, "2024-06-20") > 0.0
        assert 40.0 < index.par_spread(market, "2024-06-20") < 80.0

    def test_example_prices(self) -> None:
        index = CDSIndex.example()
        assert index.id == "CDX-IG-42"
        result = index.price(_credit_market(), "2024-06-20", "hazard_rate")
        assert math.isfinite(result.price)

    def test_constituents_accept_typed_dict_and_json(self) -> None:
        row = CDSIndexConstituent("ACME-CORP", 0.4, "CORP-HAZARD", 0.5)
        as_dict = json.loads(row.to_json())
        premium, protection = _cds_legs()

        def build(constituents: object) -> CDSIndex:
            return (
                CDSIndex
                .builder()
                .id("CDX-CONS")
                .index_name("CDX.NA.IG")
                .series(42)
                .version(1)
                .notional(Money(10_000_000.0, USD))
                .index_factor(1.0)
                .side("pay")
                .convention("isda_na")
                .premium(premium)
                .protection(protection)
                .pricing("constituents")
                .constituents(constituents)
                .build()
            )

        typed = build([row, row])
        via_dict = build([as_dict, as_dict])
        via_json = build(json.dumps([as_dict, as_dict]))
        assert typed.to_json() == via_dict.to_json() == via_json.to_json()
        assert len(typed.constituents) == 2
        assert typed.constituents[0].reference_entity == "ACME-CORP"
        assert typed.constituents[0].defaulted is False
        assert pickle.loads(pickle.dumps(row)).to_json() == row.to_json()  # noqa: S301

    def test_params_reject_unknown_convention(self) -> None:
        with pytest.raises(ValueError, match="isda_na"):
            CDSIndexParams("CDX.NA.IG", 42, 1, 100.0, convention="isda_2014")


class TestCDSTranche:
    def test_standard_and_example(self) -> None:
        params = CDSTrancheParams.mezzanine_tranche("CDX.NA.IG", 42, Money(10_000_000.0, USD), "2029-12-20", Bps(100.0))
        assert (params.attach_pct, params.detach_pct) == (3.0, 7.0)
        assert params.running_coupon_bp == 100.0
        tranche = CDSTranche.standard("CDX-42-3X7", params, "USD-OIS", "CDX.NA.IG.HAZARD", "buy_protection")
        assert tranche.side == "buy_protection"
        assert tranche.day_count == "act_360"
        assert tranche.frequency == Tenor.quarterly()
        assert tranche.business_day_convention == "following"
        assert tranche.maturity == datetime.date(2029, 12, 20)
        assert tranche.upfront is None

        example = CDSTranche.example()
        assert (example.attach_pct, example.detach_pct) == (0.0, 3.0)
        assert example.to_dict()["index_name"] == "CDX.NA.IG"

    def test_builder_new_setters(self) -> None:
        tranche = (
            CDSTranche
            .builder()
            .id("CDX-BLD")
            .index_name("CDX.NA.IG")
            .series(42)
            .attach_pct(3.0)
            .detach_pct(7.0)
            .notional(Money(10_000_000.0, USD))
            .maturity("2029-06-20")
            .running_coupon_bp(Bps(100.0))
            .frequency("3M")
            .day_count("act_360")
            .business_day_convention("modified_following")
            .discount_curve_id("USD-OIS")
            .credit_index_id("CDX-IG-42-CURVE")
            .side("buy_protection")
            .upfront(("2024-06-25", Money(100_000.0, USD)))
            .attributes(Attributes())
            .build()
        )
        assert tranche.running_coupon_bp == 100.0
        assert tranche.upfront[0] == datetime.date(2024, 6, 25)
        assert tranche.business_day_convention == "modified_following"
        with pytest.raises(ValueError, match="accumulated_loss"):
            CDSTrancheParams("CDX.NA.IG", 42, 0.0, 3.0, Money(1.0, USD), "2029-12-20", 100.0, accumulated_loss=2.0)


class TestConvertibleBond:
    def test_example_and_accessors(self) -> None:
        bond = ConvertibleBond.example()
        assert bond.conversion_ratio == 25.0
        assert bond.effective_conversion_ratio == 25.0
        assert isinstance(bond.conversion, ConversionSpec)
        assert bond.conversion.policy == "voluntary"
        assert bond.underlying_equity_id == "TECH"
        assert bond.credit_curve_id == "USD-CREDIT-BBB"
        assert bond.call_put is None
        assert bond.fixed_coupon is not None
        assert bond.floating_coupon is None
        assert bond.issue_date == datetime.date(2024, 1, 15)
        mandatory = ConvertibleBond.example_mandatory()
        assert isinstance(mandatory.call_put, CallPutSchedule)
        assert mandatory.soft_call_trigger["threshold_pct"] == 130.0

    def test_conversion_accepts_typed_dict_and_json(self) -> None:
        spec = ConversionSpec(ratio=20.0, anti_dilution="full_ratchet")
        as_dict = json.loads(spec.to_json())

        def build(conversion: object) -> ConvertibleBond:
            return (
                ConvertibleBond
                .builder()
                .id("CONV-X")
                .notional(Money(1_000.0, USD))
                .issue_date("2024-01-15")
                .maturity("2029-01-15")
                .discount_curve_id("USD-OIS")
                .conversion(conversion)
                .underlying_equity_id("ACME")
                .build()
            )

        assert build(spec).to_json() == build(as_dict).to_json() == build(json.dumps(as_dict)).to_json()
        assert build(spec).conversion.anti_dilution == "full_ratchet"

    def test_call_put_schedule_pickles_and_round_trips(self) -> None:
        sched = CallPutSchedule(
            calls=[{"start_date": "2026-01-15", "end_date": "2029-01-15", "price_pct_of_par": 101.0}],
            puts='[{"start_date": "2027-01-15", "end_date": "2027-01-15", "price_pct_of_par": 100.0}]',
        )
        assert len(sched.calls) == 1
        assert sched.puts[0]["price_pct_of_par"] == 100.0
        assert pickle.loads(pickle.dumps(sched)).to_json() == sched.to_json()  # noqa: S301
        bond = (
            ConvertibleBond
            .builder()
            .id("CONV-CP")
            .notional(Money(1_000.0, USD))
            .issue_date("2024-01-15")
            .maturity("2029-01-15")
            .discount_curve_id("USD-OIS")
            .underlying_equity_id("ACME-EQ")
            .conversion(ConversionSpec(price=50.0))
            .call_put(sched)
            .soft_call_trigger({"threshold_pct": 130.0, "observation_days": 30, "required_days_above": 20})
            .build()
        )
        assert bond.call_put.to_json() == sched.to_json()
        assert bond.conversion_ratio == 20.0
        assert bond.soft_call_trigger["required_days_above"] == 20

    def test_invalid_conversion_raises_value_error(self) -> None:
        with pytest.raises(ValueError, match="invalid conversion"):
            ConvertibleBond.builder().conversion({"ratio": 20.0, "policy": "Voluntary"})


class TestEquityOption:
    def test_example_prices_and_greeks(self) -> None:
        option = EquityOption.example()
        assert option.underlying_ticker == "SPX"
        assert option.strike == 4500.0
        assert option.expiry == datetime.date(2024, 6, 21)
        assert option.day_count == "act_365f"
        assert option.theta_day_basis == "calendar_365"
        assert option.settlement == "cash"
        assert option.exercise is None
        assert option.div_yield_id == "EQUITY-DIVYIELD"
        assert option.discrete_dividends == []
        assert option.exercise_schedule is None

        market = _equity_market()
        result = option.price(market, "2024-01-15")
        assert result.price > 0.0
        greeks = option.greeks(market, "2024-01-15")
        # Greeks are notional-scaled; per-unit delta lies in (0, 1) for a call.
        assert 0.0 < greeks["delta"] / option.notional.amount < 1.0
        assert option.delta(market, "2024-01-15") == pytest.approx(greeks["delta"])
        implied = option.implied_vol(market, "2024-01-15", result.price)
        assert implied == pytest.approx(0.2, abs=2e-3)

    def test_european_call_defaults_match_rust(self) -> None:
        option = EquityOption.european_call("AAPL-C", "AAPL", 200.0, "2025-06-20", 100.0)
        assert option.option_type == "call"
        assert option.exercise_style == "european"
        assert option.spot_id == "EQUITY-SPOT"
        assert option.vol_surface_id == "EQUITY-VOL"
        assert option.notional.currency.code == "USD"
        custom = EquityOption.european_call(
            "AAPL-C2", "AAPL", 200.0, "2025-06-20", Money(100.0, USD), spot_id="AAPL", div_yield_id=None
        )
        assert custom.spot_id == "AAPL"
        assert custom.div_yield_id is None

    def test_builder_day_count_and_repr(self) -> None:
        builder = (
            EquityOption
            .builder()
            .id("EQ-R")
            .underlying_ticker("AAPL")
            .strike(200.0)
            .option_type("call")
            .expiry("2025-06-20")
            .day_count("act_360")
            .notional(Money(100.0, USD))
            .discount_curve_id("USD-OIS")
            .spot_id("AAPL")
            .vol_surface_id("AAPL-VOL")
        )
        assert "day_count=DayCount(" in repr(builder)
        option = builder.build()
        assert option.day_count == "act_360"
        assert "strike=200.0" in repr(option)


class TestFxForward:
    def test_example_prices_and_market_forward(self) -> None:
        fwd = FxForward.example()
        assert fwd.base_currency == EUR
        assert fwd.quote_currency == USD
        assert fwd.contract_rate == 1.12
        assert fwd.maturity == datetime.date(2025, 6, 15)
        assert fwd.spot_rate_override is None
        market = _fx_market()
        result = fwd.price(market, "2025-01-15")
        assert math.isfinite(result.price)
        forward = fwd.market_forward_rate(market, "2025-01-15")
        assert 1.10 < forward < 1.13

    def test_from_trade_date_and_forward_points(self) -> None:
        fwd = FxForward.from_trade_date(
            "EURUSD-3M", "EUR", "USD", "2025-01-15", "3M", 1_000_000.0, "USD-OIS", "EUR-OIS"
        )
        assert fwd.contract_rate is None
        assert fwd.maturity > datetime.date(2025, 4, 10)
        assert FxForward.standard_spot_days("EUR", "USD") == 2
        with_points = fwd.with_forward_points(1.10, 0.0025)
        assert with_points.contract_rate == pytest.approx(1.1025)
        with_pips = fwd.with_forward_pips(1.10, 25.0)
        assert with_pips.contract_rate == pytest.approx(1.1025)
        with pytest.raises(ValueError, match=r"spot_rate must be positive"):
            fwd.with_forward_points(-1.0, 0.0)


class TestFxOption:
    def test_example_getters(self) -> None:
        opt = FxOption.example()
        assert opt.option_type == "call"
        assert opt.strike == 1.12
        assert opt.delta_convention == {
            "kind": "forward",
            "premium_currency": "USD",
            "venue": "generic_interbank",
        }
        assert opt.day_count == "act_365f"
        assert opt.notional.currency == EUR

    def test_european_and_pricing(self) -> None:
        opt = FxOption.european(
            "EURUSD-CALL",
            "EUR",
            "USD",
            1.12,
            "2025-06-15",
            1_000_000.0,
            "EURUSD-VOL",
            "call",
            "spot",
            "USD",
            "desk",
        )
        assert opt.domestic_discount_curve_id == "USD-OIS"
        assert opt.foreign_discount_curve_id == "EUR-OIS"
        market = _fx_market()
        result = opt.price(market, "2025-01-15")
        assert result.price > 0.0
        greeks = opt.greeks(market, "2025-01-15")
        # Greeks are notional-scaled; per-unit delta lies in (0, 1) for a call.
        assert 0.0 < greeks["delta"] / opt.notional.amount < 1.0
        assert opt.implied_vol(market, "2025-01-15", result.price) == pytest.approx(0.1, abs=2e-3)

    def test_builder_day_count_and_attributes(self) -> None:
        opt = (
            FxOption
            .builder()
            .id("FXO-B")
            .base_currency("EUR")
            .quote_currency(USD)
            .strike(1.1)
            .option_type("put")
            .delta_convention("spot", "USD", "desk")
            .expiry("2025-06-15")
            .day_count("act_360")
            .notional(Money(1_000_000.0, EUR))
            .domestic_discount_curve_id("USD-OIS")
            .foreign_discount_curve_id("EUR-OIS")
            .vol_surface_id("EURUSD-VOL")
            .attributes({"book": "fx"})
            .build()
        )
        assert opt.day_count == "act_360"
        assert opt.attributes.get_meta("book") == "fx"
        assert 'option_type="put"' in repr(opt)
