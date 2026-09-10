"""Shared canonical-example factories for typed-instrument tests.

Each ``build_*`` function constructs the canonical typed instance for one
instrument family, reused by the per-family test modules (Tasks 2-7) and by
the cross-instrument round-trip matrix
(``test_typed_instruments_roundtrip.py``, Task 9) so every instrument's
example is defined exactly once.

Not a pytest module: the filename does not match ``python_files`` in
``pyproject.toml`` (``test_*.py`` / ``*_test.py``), so pytest never collects
it directly; it is only ever imported.
"""

from __future__ import annotations

import datetime
import json
from pathlib import Path

from finstack_quant.core.currency import Currency
from finstack_quant.core.dates import DayCount, Tenor
from finstack_quant.core.money import Money
from finstack_quant.valuations.instruments import (
    AssetPool,
    CapFloor,
    CDSIndex,
    CDSTranche,
    ConvertibleBond,
    CreditDefaultSwap,
    EquityOption,
    FixedLegSpec,
    FloatLegSpec,
    FxForward,
    FxOption,
    InterestRateSwap,
    PremiumLegSpec,
    ProtectionLegSpec,
    RepLine,
    StructuredCredit,
    Swaption,
    Tranche,
    TrancheStructure,
)

_STRUCTURED_CREDIT_FIXTURE = (
    Path(__file__).resolve().parents[2]
    / "finstack-quant"
    / "valuations"
    / "tests"
    / "instruments"
    / "json_examples"
    / "structured_credit.json"
)


def canonical_structured_credit_json(*, payment_calendar_id: str = "nyse") -> str:
    """Load the registry-generated, priceable structured-credit envelope."""
    envelope = json.loads(_STRUCTURED_CREDIT_FIXTURE.read_text(encoding="utf-8"))
    envelope["instrument"]["spec"]["payment_calendar_id"] = payment_calendar_id
    return json.dumps(envelope)


def irs_legs() -> tuple[FixedLegSpec, FloatLegSpec]:
    start = datetime.date(2024, 1, 15)
    end = datetime.date(2029, 1, 15)
    fixed = FixedLegSpec(
        "USD-OIS", 0.04, Tenor.semi_annual(), DayCount.THIRTY_360, start, end, compounding_simple=False
    )
    float_leg = FloatLegSpec("USD-OIS", "USD-SOFR-3M", 0.0, Tenor.quarterly(), DayCount.ACT_360, start, end)
    return fixed, float_leg


def build_irs() -> InterestRateSwap:
    fixed, float_leg = irs_legs()
    return (
        InterestRateSwap
        .builder()
        .id("IRS-1")
        .notional(Money(10_000_000.0, Currency("USD")))
        .side("pay")
        .fixed(fixed)
        .float(float_leg)
        .build()
    )


def swaption_legs() -> tuple[FixedLegSpec, FloatLegSpec]:
    start = datetime.date(2025, 1, 15)
    end = datetime.date(2030, 1, 15)
    return (
        FixedLegSpec("USD-OIS", 0.04, Tenor.semi_annual(), DayCount.THIRTY_360, start, end, compounding_simple=False),
        FloatLegSpec("USD-OIS", "USD-SOFR-3M", 0.0, Tenor.quarterly(), DayCount.ACT_360, start, end),
    )


def build_swaption(
    *,
    option_type: str = "call",
    exercise_style: str = "european",
    settlement: str = "cash",
    cash_settlement_method: str = "collateralized_cash_price",
    vol_model: str = "normal",
) -> Swaption:
    fixed, float_leg = swaption_legs()
    return (
        Swaption
        .builder()
        .id("SWPT-1")
        .option_type(option_type)
        .notional(Money(10_000_000.0, Currency("USD")))
        .expiry(datetime.date(2025, 1, 13))
        .exercise_style(exercise_style)
        .settlement(settlement)
        .cash_settlement_method(cash_settlement_method)
        .vol_model(vol_model)
        .vol_surface_id("USD-SWPT-VOL")
        .underlying_fixed_leg(fixed)
        .underlying_float_leg(float_leg)
        .build()
    )


def build_capfloor(
    *,
    rate_option_type: str = "cap",
    vol_type: str = "normal",
) -> CapFloor:
    return (
        CapFloor
        .builder()
        .id("CAP-1")
        .rate_option_type(rate_option_type)
        .notional(Money(5_000_000.0, Currency("USD")))
        .strike(0.05)
        .start_date(datetime.date(2024, 1, 15))
        .maturity(datetime.date(2027, 1, 15))
        .frequency(Tenor.quarterly())
        .day_count(DayCount.ACT_360)
        .discount_curve_id("USD-OIS")
        .forward_curve_id("USD-SOFR-3M")
        .vol_surface_id("USD-CAP-VOL")
        .vol_type(vol_type)
        .build()
    )


def cds_legs() -> tuple[PremiumLegSpec, ProtectionLegSpec]:
    return (
        PremiumLegSpec(
            datetime.date(2024, 3, 20),
            datetime.date(2029, 6, 20),
            Tenor.quarterly(),
            DayCount.ACT_360,
            100.0,
            "USD-OIS",
            standard_imm_dates=True,
        ),
        ProtectionLegSpec("ACME-CDS", 0.4, 3),
    )


def build_cds() -> CreditDefaultSwap:
    premium, protection = cds_legs()
    return (
        CreditDefaultSwap
        .builder()
        .id("CDS-1")
        .notional(Money(10_000_000.0, Currency("USD")))
        .side("pay")
        .convention("isda_na")
        .premium(premium)
        .protection(protection)
        .build()
    )


def build_cds_index() -> CDSIndex:
    premium, protection = cds_legs()
    return (
        CDSIndex
        .builder()
        .id("CDX-IG-42")
        .index_name("CDX.NA.IG")
        .series(42)
        .version(1)
        .notional(Money(10_000_000.0, Currency("USD")))
        .index_factor(1.0)
        .side("pay")
        .convention("isda_na")
        .premium(premium)
        .protection(protection)
        .pricing("single_curve")
        .num_constituents(125)
        .build()
    )


def build_fx_forward() -> FxForward:
    return (
        FxForward
        .builder()
        .id("EURUSD-FWD-6M")
        .base_currency(Currency("EUR"))
        .quote_currency(Currency("USD"))
        .maturity(datetime.date(2025, 6, 15))
        .notional(Money(1_000_000.0, Currency("EUR")))
        .contract_rate(1.10)
        .domestic_discount_curve_id("USD-OIS")
        .foreign_discount_curve_id("EUR-OIS")
        .build()
    )


def build_fx_option() -> FxOption:
    return (
        FxOption
        .builder()
        .id("EURUSD-CALL-1Y")
        .base_currency(Currency("EUR"))
        .quote_currency(Currency("USD"))
        .strike(1.12)
        .option_type("call")
        .delta_convention("forward", Currency("USD"), "generic_interbank")
        .expiry(datetime.date(2025, 12, 15))
        .notional(Money(1_000_000.0, Currency("EUR")))
        .domestic_discount_curve_id("USD-OIS")
        .foreign_discount_curve_id("EUR-OIS")
        .vol_surface_id("EURUSD-VOL")
        .build()
    )


def build_equity_option() -> EquityOption:
    return (
        EquityOption
        .builder()
        .id("AAPL-C-200")
        .underlying_ticker("AAPL")
        .strike(200.0)
        .option_type("call")
        .expiry(datetime.date(2025, 6, 20))
        .notional(Money(100.0, Currency("USD")))
        .discount_curve_id("USD-OIS")
        .spot_id("AAPL")
        .vol_surface_id("AAPL-VOL")
        .build()
    )


def build_cds_tranche() -> CDSTranche:
    return (
        CDSTranche
        .builder()
        .id("CDX-IG-42-3-7")
        .index_name("CDX.NA.IG")
        .series(42)
        .attach_pct(3.0)
        .detach_pct(7.0)
        .notional(Money(10_000_000.0, Currency("USD")))
        .maturity(datetime.date(2029, 6, 20))
        .running_coupon_bp(100.0)
        .frequency(Tenor.quarterly())
        .day_count(DayCount.ACT_360)
        .discount_curve_id("USD-OIS")
        .credit_index_id("CDX-IG-42-CURVE")
        .side("buy_protection")
        .build()
    )


def build_convertible() -> ConvertibleBond:
    conversion = json.dumps({
        "ratio": 20.0,
        "price": None,
        "policy": "voluntary",
        "anti_dilution": "full_ratchet",
        "dividend_adjustment": "none",
        "dilution_events": [],
    })
    return (
        ConvertibleBond
        .builder()
        .id("CONV-1")
        .notional(Money(1_000.0, Currency("USD")))
        .issue_date(datetime.date(2024, 1, 15))
        .maturity(datetime.date(2029, 1, 15))
        .discount_curve_id("USD-OIS")
        .conversion(conversion)
        .underlying_equity_id("ACME")
        .build()
    )


def structured_credit_pool() -> AssetPool:
    pool = AssetPool("POOL-1", "abs", Currency("USD"))
    return pool.with_rep_lines([
        RepLine(
            "LINE-1",
            Money(80_000_000.0, Currency("USD")),
            0.07,
            datetime.date(2031, 1, 15),
            12,
            DayCount.ACT_360,
            asset_type={"type": "first_lien_loan", "industry": None},
            cpr=0.10,
            cdr=0.02,
            recovery_rate=0.45,
        )
    ])


def structured_credit_tranches() -> TrancheStructure:
    senior = (
        Tranche
        .builder()
        .id("A")
        .attachment_point(10.0)
        .detachment_point(100.0)
        .seniority("senior")
        .original_balance(Money(72_000_000.0, Currency("USD")))
        .coupon_fixed(0.05)
        .maturity(datetime.date(2031, 1, 15))
        .build()
    )
    equity = (
        Tranche
        .builder()
        .id("E")
        .attachment_point(0.0)
        .detachment_point(10.0)
        .seniority("equity")
        .original_balance(Money(8_000_000.0, Currency("USD")))
        .coupon_fixed(0.0)
        .maturity(datetime.date(2031, 1, 15))
        .build()
    )
    return TrancheStructure([senior, equity])


def build_structured_credit() -> StructuredCredit:
    return StructuredCredit.new_abs(
        "ABS-1",
        structured_credit_pool(),
        structured_credit_tranches(),
        datetime.date(2024, 1, 15),
        datetime.date(2031, 1, 15),
        "USD-SOFR-DISC",
    )
