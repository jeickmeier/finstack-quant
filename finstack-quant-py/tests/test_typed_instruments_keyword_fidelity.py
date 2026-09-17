"""Keyword-argument fidelity sweep across every typed instrument.

A defect class shipped twice in sibling work: a builder setter or
constructor parameter bound under a Rust name different from what its
``text_signature``/``.pyi`` advertises. Positional-argument tests don't
catch this — only calling with the documented keyword does. This module
builds every typed instrument, builder, and result wrapper end to end using
ONLY keyword arguments, so a name mismatch shows up as a ``TypeError``
("unexpected keyword argument") instead of silently passing.

``InterestRateSwap``/``FixedLegSpec``/``FloatLegSpec`` (rates) and
``CreditDefaultSwap``/``CDSIndex`` (credit) already have dedicated
all-keyword tests in their own family modules
(``test_typed_rates_instruments.py``, ``test_typed_credit_instruments.py``);
this module covers the remaining families plus the structured-credit spec
models and the JSON-analytics result wrappers.
"""

from __future__ import annotations

import datetime
import json

from finstack_quant.core.currency import Currency
from finstack_quant.core.dates import DayCount, Tenor
from finstack_quant.core.market_data import DiscountCurve, ForwardCurve, MarketContext, ScalarTimeSeries
from finstack_quant.core.money import Money
from finstack_quant.valuations.instruments import (
    AssetPool,
    CapFloor,
    CDSTranche,
    ConvertibleBond,
    EquityOption,
    FixedLegSpec,
    FloatLegSpec,
    FxForward,
    FxOption,
    OasResult,
    PremiumLegSpec,
    ProtectionLegSpec,
    RepLine,
    ScenarioTable,
    StructuredCredit,
    Swaption,
    Tranche,
    TrancheMetrics,
    TrancheStructure,
    structured_credit_tranche_breakeven_cdr,
)
from tests.tests_typed_helpers import canonical_structured_credit_json


def _single_tranche() -> Tranche:
    return (
        Tranche
        .builder()
        .id("A")
        .attachment_point(0.0)
        .detachment_point(100.0)
        .seniority("senior")
        .original_balance(Money(1.0, Currency("USD")))
        .coupon_fixed(0.05)
        .maturity(datetime.date(2031, 1, 15))
        .build()
    )


def _single_rep_line() -> RepLine:
    return RepLine(
        "LINE-1",
        Money(80_000_000.0, Currency("USD")),
        0.07,
        datetime.date(2031, 1, 15),
        12,
        DayCount.ACT_360,
        asset_type={"type": "first_lien_loan", "industry": None},
    )


def test_swaption_builder_setters_accept_keyword_value() -> None:
    start = datetime.date(2025, 1, 15)
    end = datetime.date(2030, 1, 15)
    fixed = FixedLegSpec(
        discount_curve_id="USD-OIS",
        rate=0.04,
        frequency=Tenor.semi_annual(),
        day_count=DayCount.THIRTY_360,
        start=start,
        end=end,
        compounding_simple=False,
    )
    float_leg = FloatLegSpec(
        discount_curve_id="USD-OIS",
        forward_curve_id="USD-SOFR-3M",
        spread_bp=0.0,
        frequency=Tenor.quarterly(),
        day_count=DayCount.ACT_360,
        start=start,
        end=end,
    )
    swpt = (
        Swaption
        .builder()
        .id(value="SWPT-KW")
        .option_type(value="call")
        .notional(value=Money(10_000_000.0, Currency("USD")))
        .expiry(value=datetime.date(2025, 1, 13))
        .exercise_style(value="european")
        .settlement(value="cash")
        .cash_settlement_method(value="par_yield")
        .vol_model(value="normal")
        .vol_surface_id(value="USD-SWPT-VOL")
        .underlying_fixed_leg(value=fixed)
        .underlying_float_leg(value=float_leg)
        .sabr_params_json(value=json.dumps({"alpha": 0.025, "beta": 0.5, "nu": 0.4, "rho": -0.3, "shift": None}))
        .build()
    )
    assert swpt.id == "SWPT-KW"


def test_capfloor_builder_setters_accept_keyword_value() -> None:
    cap = (
        CapFloor
        .builder()
        .id(value="CAP-KW")
        .rate_option_type(value="cap")
        .notional(value=Money(5_000_000.0, Currency("USD")))
        .strike(value=0.05)
        .spread(value=0.001)
        .start_date(value=datetime.date(2024, 1, 15))
        .maturity(value=datetime.date(2027, 1, 15))
        .frequency(value=Tenor.quarterly())
        .day_count(value=DayCount.ACT_360)
        .calendar_id(value="nyse")
        .discount_curve_id(value="USD-OIS")
        .forward_curve_id(value="USD-SOFR-3M")
        .vol_surface_id(value="USD-CAP-VOL")
        .vol_type(value="normal")
        .vol_shift(value=0.02)
        .build()
    )
    assert cap.id == "CAP-KW"


def test_fx_forward_builder_setters_accept_keyword_value() -> None:
    fwd = (
        FxForward
        .builder()
        .id(value="FXFWD-KW")
        .base_currency(value=Currency("EUR"))
        .quote_currency(value=Currency("USD"))
        .maturity(value=datetime.date(2025, 6, 15))
        .notional(value=Money(1_000_000.0, Currency("EUR")))
        .contract_rate(value=1.10)
        .domestic_discount_curve_id(value="USD-OIS")
        .foreign_discount_curve_id(value="EUR-OIS")
        .spot_rate_override(value=1.09)
        .base_calendar_id(value="target")
        .quote_calendar_id(value="nyse")
        .build()
    )
    assert fwd.id == "FXFWD-KW"


def test_fx_option_builder_setters_accept_keyword_value() -> None:
    opt = (
        FxOption
        .builder()
        .id(value="FXOPT-KW")
        .base_currency(value=Currency("EUR"))
        .quote_currency(value=Currency("USD"))
        .strike(value=1.12)
        .option_type(value="call")
        .delta_convention(
            kind="forward",
            premium_currency=Currency("USD"),
            venue="generic_interbank",
        )
        .expiry(value=datetime.date(2025, 12, 15))
        .notional(value=Money(1_000_000.0, Currency("EUR")))
        .domestic_discount_curve_id(value="USD-OIS")
        .foreign_discount_curve_id(value="EUR-OIS")
        .vol_surface_id(value="EURUSD-VOL")
        .build()
    )
    assert opt.id == "FXOPT-KW"


def test_equity_option_builder_setters_accept_keyword_value() -> None:
    opt = (
        EquityOption
        .builder()
        .id(value="EQOPT-KW")
        .underlying_ticker(value="AAPL")
        .strike(value=200.0)
        .option_type(value="call")
        .exercise_style(value="american")
        .expiry(value=datetime.date(2025, 6, 20))
        .notional(value=Money(100.0, Currency("USD")))
        .discount_curve_id(value="USD-OIS")
        .spot_id(value="AAPL")
        .vol_surface_id(value="AAPL-VOL")
        .div_yield_id(value="AAPL-DIVYIELD")
        .discrete_dividends(value=[(datetime.date(2025, 3, 1), 0.5)])
        .exercise_schedule(value=[datetime.date(2025, 3, 20), datetime.date(2025, 6, 20)])
        .build()
    )
    assert opt.id == "EQOPT-KW"


def test_cds_tranche_builder_setters_accept_keyword_value() -> None:
    tranche = (
        CDSTranche
        .builder()
        .id(value="CDXTR-KW")
        .index_name(value="CDX.NA.IG")
        .series(value=42)
        .attach_pct(value=3.0)
        .detach_pct(value=7.0)
        .notional(value=Money(10_000_000.0, Currency("USD")))
        .maturity(value=datetime.date(2029, 6, 20))
        .running_coupon_bp(value=100.0)
        .frequency(value=Tenor.quarterly())
        .day_count(value=DayCount.ACT_360)
        .calendar_id(value="NYSE")
        .discount_curve_id(value="USD-OIS")
        .credit_index_id(value="CDX-IG-42-CURVE")
        .side(value="buy_protection")
        .effective_date(value=datetime.date(2024, 6, 20))
        .accumulated_loss(value=0.01)
        .standard_imm_dates(value=False)
        .build()
    )
    assert tranche.id == "CDXTR-KW"


def test_convertible_bond_builder_setters_accept_keyword_value() -> None:
    conversion = json.dumps({
        "ratio": 20.0,
        "price": None,
        "policy": "voluntary",
        "anti_dilution": "full_ratchet",
        "dividend_adjustment": "none",
        "dilution_events": [],
    })
    bond = (
        ConvertibleBond
        .builder()
        .id(value="CONV-KW")
        .notional(value=Money(1_000.0, Currency("USD")))
        .issue_date(value=datetime.date(2024, 1, 15))
        .maturity(value=datetime.date(2029, 1, 15))
        .discount_curve_id(value="USD-OIS")
        .credit_curve_id(value="USD-CREDIT-BBB")
        .conversion(value=conversion)
        .underlying_equity_id(value="ACME")
        .settlement_days(value=2)
        .recovery_rate(value=0.4)
        .build()
    )
    assert bond.id == "CONV-KW"


def test_premium_and_protection_leg_spec_accept_keyword_arguments() -> None:
    premium = PremiumLegSpec(
        start=datetime.date(2024, 3, 20),
        end=datetime.date(2029, 6, 20),
        frequency=Tenor.quarterly(),
        day_count=DayCount.ACT_360,
        spread_bp=100.0,
        discount_curve_id="USD-OIS",
        stub="short_front",
        business_day_convention="modified_following",
        calendar_id=None,
        standard_imm_dates=False,
    )
    protection = ProtectionLegSpec(
        credit_curve_id="ACME-CDS",
        recovery_rate=0.4,
        settlement_delay=3,
    )
    assert "spread_bp=100" in repr(premium)
    assert "recovery_rate=0.4" in repr(protection)


def test_rep_line_accepts_keyword_arguments() -> None:
    line = RepLine(
        id="LINE-KW",
        balance=Money(80_000_000.0, Currency("USD")),
        rate=0.07,
        maturity=datetime.date(2031, 1, 15),
        seasoning_months=12,
        day_count=DayCount.ACT_360,
        asset_type={"type": "first_lien_loan", "industry": None},
        spread_bp=None,
        index_id=None,
        cpr=0.10,
        cdr=0.02,
        recovery_rate=0.45,
    )
    assert "LINE-KW" in repr(line)


def test_asset_pool_accepts_keyword_arguments() -> None:
    pool = AssetPool(id="POOL-KW", deal_type="abs", base_currency=Currency("USD"))
    assert "POOL-KW" in repr(pool)
    with_lines = pool.with_rep_lines(rep_lines=[_single_rep_line()])
    assert "POOL-KW" in repr(with_lines)


def test_tranche_builder_setters_accept_keyword_value() -> None:
    tranche = (
        Tranche
        .builder()
        .id(value="TR-KW")
        .attachment_point(value=10.0)
        .detachment_point(value=100.0)
        .seniority(value="senior")
        .original_balance(value=Money(72_000_000.0, Currency("USD")))
        .coupon_fixed(rate=0.05)
        .maturity(value=datetime.date(2031, 1, 15))
        .build()
    )
    assert "TR-KW" in repr(tranche)


def test_tranche_structure_accepts_keyword_argument() -> None:
    structure = TrancheStructure(tranches=[_single_tranche()])
    assert "tranches=1" in repr(structure)


def test_structured_credit_builder_setters_accept_keyword_value() -> None:
    pool = AssetPool("POOL-SC-KW", "abs", Currency("USD")).with_rep_lines([_single_rep_line()])
    tranches = TrancheStructure([_single_tranche()])
    deal = (
        StructuredCredit
        .builder()
        .id(value="SC-KW")
        .deal_type(value="abs")
        .pool(value=pool)
        .tranches(value=tranches)
        .closing_date(value=datetime.date(2024, 1, 15))
        .first_payment_date(value=datetime.date(2024, 2, 15))
        .maturity(value=datetime.date(2031, 1, 15))
        .frequency(value=Tenor.quarterly())
        .discount_curve_id(value="USD-SOFR-DISC")
        .build()
    )
    assert deal.id == "SC-KW"
    assert deal.first_payment_date == datetime.date(2024, 2, 15)


def test_structured_credit_new_abs_accepts_keyword_arguments() -> None:
    pool = AssetPool("POOL-ABS-KW", "abs", Currency("USD")).with_rep_lines([_single_rep_line()])
    tranches = TrancheStructure([_single_tranche()])
    deal = StructuredCredit.new_abs(
        id="ABS-KW",
        pool=pool,
        tranches=tranches,
        closing_date=datetime.date(2024, 1, 15),
        maturity=datetime.date(2031, 1, 15),
        discount_curve_id="USD-SOFR-DISC",
    )
    assert deal.id == "ABS-KW"


def test_result_wrapper_from_json_accepts_keyword_argument() -> None:
    oas_payload = json.dumps({
        "oas": 0.0125,
        "model_price": 99.5,
        "market_price": 98.75,
        "num_paths": 256,
        "price_std_error": 0.05,
    })
    assert OasResult.from_json(json=oas_payload).oas == 0.0125

    scenario_payload = json.dumps({
        "tranche_id": "A",
        "cells": [{"cpr": 0.06, "cdr": 0.02, "severity": 0.6, "price": 98.2, "wal": 4.1, "writedown": 0.0}],
    })
    assert ScenarioTable.from_json(json=scenario_payload).tranche_id == "A"


def test_structured_credit_analytics_function_accepts_keyword_arguments() -> None:
    deal = StructuredCredit.from_json(canonical_structured_credit_json())

    as_of = datetime.date(2024, 1, 1)
    market = (
        MarketContext()
        .insert(DiscountCurve.flat("USD-OIS", as_of, 0.04))
        .insert(ForwardCurve("SOFR-3M", 0.25, as_of, [(0.0, 0.04), (10.0, 0.04)], day_count="act_360"))
    )
    market.insert_series(ScalarTimeSeries("FIXING:SOFR-3M", [(datetime.date(2023, 12, 28), 0.04)]))

    result = structured_credit_tranche_breakeven_cdr(
        instrument=deal,
        tranche_id="CLONOTES-A",
        market=market,
        as_of="2024-01-01",
    )
    assert isinstance(result, float)


def test_tranche_metrics_from_json_uses_json_keyword() -> None:
    # `TrancheMetrics.from_json` is exercised end to end (typed-vs-JSON
    # golden) in `test_typed_structured_credit.py`; this confirms its
    # classmethod accepts the same `json` keyword as the other result
    # wrappers rather than drifting to a different parameter name.
    payload = json.dumps({
        "tranche_id": "A",
        "currency": "USD",
        "pv": 950_000.0,
        "price_pct": 95.0,
        "factor": 1.0,
        "wal": 4.1,
        "z_spread_bp": 150.0,
        "cs01": -380.0,
        "spread_duration": 4.0,
        "modified_duration": 3.8,
        "convexity": 0.2,
        "target_price_pct": 95.0,
    })
    metrics = TrancheMetrics.from_json(json=payload)
    assert metrics.tranche_id == "A"
