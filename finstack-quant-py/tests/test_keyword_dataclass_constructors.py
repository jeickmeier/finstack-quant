"""Keyword-only data-class constructors read their Rust serde struct.

Each constructor takes its fields as keywords and Rust reads them as one
mapping, so the Rust struct owns defaults and rejects unknown keywords. The
pinned ``to_json()`` values in ``data/keyword_dataclass_golden.json`` were
captured from the earlier positional constructors with the same inputs: the
keyword path must reproduce them byte for byte.
"""

from __future__ import annotations

import datetime as dt
from decimal import Decimal
import json
from pathlib import Path
from typing import Any

import pytest

from finstack_quant.cashflows.builder import (
    FeeAccrualBasis,
    FeeBase,
    FeeSpec,
    FloatingLegCompounding,
    FloatingRateFallback,
    FloatingRateSpec,
    OvernightIndexConstraintApplication,
    RollRule,
    ScheduleParams,
)
from finstack_quant.core.currency import Currency
from finstack_quant.core.dates import DayCount, StubKind, Tenor
from finstack_quant.core.money import Money
from finstack_quant.margin import SaCcrTrade
from finstack_quant.models.credit.recovery_waterfall import RecoveryClaim
from finstack_quant.models.credit.scoring import AltmanZScoreInput, OhlsonOScoreInput
from finstack_quant.models.liquidity import LiquidityProfile
from finstack_quant.valuations.instruments import (
    AssetPool,
    CdsTrancheParams,
    FixedLegSpec,
    FloatLegSpec,
    PoolAsset,
    PremiumLegSpec,
    RepLine,
    StandardDealParams,
    Tranche,
    TrancheStructure,
)

GOLDEN: dict[str, str] = json.loads((Path(__file__).parent / "data" / "keyword_dataclass_golden.json").read_text())
USD = Currency("USD")
START, END = dt.date(2024, 1, 15), dt.date(2029, 1, 15)
MAT = dt.date(2029, 1, 15)
TRANCHE_GETTERS = [
    "index_name",
    "series",
    "attach_pct",
    "detach_pct",
    "notional",
    "maturity",
    "coupon_bp",
    "realized_loss",
]


def _pool() -> AssetPool:
    line = RepLine(
        id="LINE-1",
        balance=Money(80_000_000.0, USD),
        rate=0.07,
        maturity=dt.date(2031, 1, 15),
        seasoning_months=12,
        day_count=DayCount.ACT_360,
        asset_type={"type": "first_lien_loan"},
        cpr=0.10,
        cdr=0.02,
        recovery_rate=0.45,
    )
    return AssetPool("POOL-1", "abs", USD).with_rep_lines([line])


def _tranches() -> TrancheStructure:
    senior = (
        Tranche
        .builder()
        .id("A")
        .attach_pct(0.0)
        .detach_pct(100.0)
        .seniority("senior")
        .original_balance(Money(72_000_000.0, USD))
        .coupon_fixed(0.05)
        .maturity(dt.date(2031, 1, 15))
        .build()
    )
    return TrancheStructure([senior])


CASES: dict[str, Any] = {
    "FloatingRateSpec.min": lambda: FloatingRateSpec(
        forward_curve_id="USD-SOFR-3M", spread_bp=Decimal("200"), reset_frequency=Tenor.parse("3M")
    ),
    "FloatingRateSpec.full": lambda: FloatingRateSpec(
        forward_curve_id="USD-SOFR",
        spread_bp="12.5",
        reset_frequency=Tenor.parse("1M"),
        gearing=Decimal("1.5"),
        gearing_includes_spread=False,
        index_floor_bp="0",
        all_in_floor_bp="10",
        all_in_cap_bp="900",
        index_cap_bp="800",
        overnight_index_constraints=OvernightIndexConstraintApplication.PERIOD,
        index_tenor=Tenor.parse("3M"),
        reset_lag_days=0,
        fixing_calendar_id="weekends_only",
        compounding=FloatingLegCompounding.compounded_in_arrears(2),
        overnight_basis=DayCount.ACT_360,
        fallback=FloatingRateFallback.SPREAD_ONLY,
    ),
    "ScheduleParams.min": lambda: ScheduleParams(
        frequency=Tenor.parse("3M"), day_count=DayCount.ACT_360, calendar_id="weekends_only"
    ),
    "ScheduleParams.full": lambda: ScheduleParams(
        frequency=Tenor.parse("6M"),
        day_count=DayCount.THIRTY_360,
        calendar_id="usny",
        business_day_convention="following",
        stub=StubKind.SHORT_BACK,
        end_of_month=True,
        payment_lag_days=2,
        adjust_accrual_dates=True,
        roll_rule=RollRule.CDS_IMM,
    ),
    "FeeSpec.periodic_bp.min": lambda: FeeSpec.periodic_bp(
        base=FeeBase.DRAWN,
        bp="25",
        frequency=Tenor.parse("3M"),
        day_count=DayCount.ACT_360,
        calendar_id="weekends_only",
    ),
    "FeeSpec.periodic_bp.full": lambda: FeeSpec.periodic_bp(
        base=FeeBase.undrawn(commitment=Money(10_000_000.0, USD)),
        bp=Decimal("37.5"),
        frequency=Tenor.quarterly(),
        day_count=DayCount.ACT_360,
        calendar_id="usny",
        business_day_convention="following",
        stub=StubKind.LONG_FRONT,
        accrual_basis=FeeAccrualBasis.TIME_WEIGHTED_AVERAGE,
    ),
    "SaCcrTrade.linear": lambda: SaCcrTrade(
        trade_id="IRS-1",
        asset_class="interest_rate",
        notional=1_000_000.0,
        start_date=dt.date(2025, 1, 15),
        end_date="2030-01-15",
        underlier="USD-SOFR",
        hedging_set="USD-IR",
        direction=1.0,
        supervisory_delta=1.0,
        mtm=10_000.0,
        is_option=False,
    ),
    "SaCcrTrade.option": lambda: SaCcrTrade(
        trade_id="OPT",
        asset_class="equity",
        notional=1e6,
        start_date="2025-01-15",
        end_date="2030-01-15",
        underlier="ACME",
        hedging_set="EQ",
        direction=1.0,
        supervisory_delta=0.6,
        mtm=0.0,
        is_option=True,
        option_type="call_long",
        supervisory_category="equity_single_name",
        option_maturity_date="2026-01-15",
    ),
    "LiquidityProfile.min": lambda: LiquidityProfile(
        instrument_id="ACME",
        mid=100.0,
        bid=99.95,
        ask=100.05,
        avg_daily_volume=1_000_000.0,
        avg_trade_size=500.0,
        spread_volatility=0.0002,
    ),
    "LiquidityProfile.full": lambda: LiquidityProfile(
        instrument_id="ACME",
        mid=100.0,
        bid=99.95,
        ask=100.05,
        avg_daily_volume=1_000_000.0,
        avg_trade_size=500.0,
        spread_volatility=0.0002,
        spread_volatility_kind="absolute",
        observation_days=60,
    ),
    "RecoveryClaim.min": lambda: RecoveryClaim(
        id="SEN", seniority="secured", priority=1, principal=100.0, accrued=0.0, penalties=0.0, collateral_haircut=0.0
    ),
    "RecoveryClaim.full": lambda: RecoveryClaim(
        id="SEN",
        seniority="secured",
        priority=1,
        principal=100.0,
        accrued=5.0,
        penalties=1.0,
        collateral_haircut=0.25,
        collateral_value=60.0,
    ),
    "OhlsonOScoreInput": lambda: OhlsonOScoreInput(
        log_total_assets_adjusted=1.0,
        total_liabilities_to_total_assets=0.5,
        working_capital_to_total_assets=0.1,
        current_liabilities_to_current_assets=0.8,
        liabilities_exceed_assets=0.0,
        net_income_to_total_assets=0.05,
        funds_from_operations_to_total_liabilities=0.2,
        negative_net_income_two_years=0.0,
        net_income_change=0.1,
    ),
    "AltmanZScoreInput": lambda: AltmanZScoreInput(
        working_capital_to_total_assets=0.1,
        retained_earnings_to_total_assets=0.2,
        ebit_to_total_assets=0.05,
        market_equity_to_total_liabilities=1.5,
        sales_to_total_assets=0.9,
    ),
    "FixedLegSpec.min": lambda: FixedLegSpec(
        discount_curve_id="USD-OIS",
        rate="0.04",
        frequency=Tenor.semi_annual(),
        day_count=DayCount.THIRTY_360,
        start=START,
        end=END,
    ),
    "FixedLegSpec.min045": lambda: FixedLegSpec(
        discount_curve_id="USD-OIS",
        rate=Decimal("0.045"),
        frequency=Tenor.semi_annual(),
        day_count=DayCount.THIRTY_360,
        start="2025-04-15",
        end="2028-04-15",
        calendar_id="weekends_only",
    ),
    "FixedLegSpec.full": lambda: FixedLegSpec(
        discount_curve_id="USD-OIS",
        rate="0.04",
        frequency=Tenor.semi_annual(),
        day_count=DayCount.THIRTY_360,
        start=START,
        end=END,
        business_day_convention="following",
        calendar_id="usny",
        stub="long_back",
        payment_lag_days=2,
        end_of_month=True,
    ),
    "FloatLegSpec.min": lambda: FloatLegSpec(
        discount_curve_id="USD-OIS",
        forward_curve_id="USD-SOFR-3M",
        spread_bp="0",
        frequency=Tenor.quarterly(),
        day_count=DayCount.ACT_360,
        start=START,
        end=END,
    ),
    "FloatLegSpec.full": lambda: FloatLegSpec(
        discount_curve_id="USD-OIS",
        forward_curve_id="USD-SOFR",
        spread_bp="12.5",
        frequency=Tenor.annual(),
        day_count=DayCount.ACT_360,
        start=START,
        end=END,
        business_day_convention="following",
        calendar_id="usny",
        stub="short_back",
        reset_lag_days=2,
        fixing_calendar_id="usny",
        compounding={"compounded_in_arrears": {"lookback_days": 0}},
        payment_lag_days=2,
        end_of_month=True,
    ),
    "PremiumLegSpec.min": lambda: PremiumLegSpec(
        start=dt.date(2024, 3, 20),
        end=dt.date(2029, 6, 20),
        frequency=Tenor.quarterly(),
        day_count=DayCount.ACT_360,
        coupon_bp="100",
        discount_curve_id="USD-OIS",
        roll_rule=RollRule.CDS_IMM,
    ),
    "PremiumLegSpec.full": lambda: PremiumLegSpec(
        start="2025-03-20",
        end="2030-03-20",
        frequency=Tenor.quarterly(),
        day_count=DayCount.ACT_360,
        coupon_bp="100",
        discount_curve_id="USD-OIS",
        calendar_id="usny",
        business_day_convention="following",
        roll_rule=RollRule.CDS_IMM,
        stub="long_front",
    ),
    "CdsTrancheParams.min": lambda: CdsTrancheParams(
        index_name="CDX.NA.IG",
        series=42,
        attach_pct=7.0,
        detach_pct=15.0,
        notional=Money(5_000_000.0, USD),
        maturity="2029-12-20",
        coupon_bp=100.0,
    ),
    "CdsTrancheParams.full": lambda: CdsTrancheParams(
        index_name="CDX.NA.IG",
        series=42,
        attach_pct=0.0,
        detach_pct=3.0,
        notional=Money(1.0, USD),
        maturity="2029-12-20",
        coupon_bp=100.0,
        realized_loss=0.02,
    ),
    "PoolAsset.min": lambda: PoolAsset(
        id="L1",
        asset_type={"type": "first_lien_loan"},
        balance=Money(1_000_000.0, USD),
        rate=0.08,
        maturity=MAT,
        day_count=DayCount.ACT_360,
    ),
    "PoolAsset.full": lambda: PoolAsset(
        id="L0",
        asset_type={"type": "first_lien_loan"},
        balance=Money(10_000_000.0, USD),
        rate=0.08,
        maturity=MAT,
        day_count=DayCount.THIRTY_360,
        spread_bp=300.0,
        forward_curve_id="USD-SOFR-3M",
        index_floor_bp=50.0,
        rating="B",
        industry="Software",
        obligor_id="OBL-1",
        defaulted=True,
        recovery_amount=Money(1.0, USD),
        default_date=dt.date(2025, 1, 1),
        purchase_price=Money(9_800_000.0, USD),
        acquisition_date=dt.date(2024, 2, 1),
        origination_date=dt.date(2023, 1, 1),
        smm_override=0.01,
        mdr_override=0.002,
        recovery_rate=0.4,
        commitment=Money(11_000_000.0, USD),
        contractual_payment=Money(50_000.0, USD),
        amortization_term_months=120,
        io_months=12,
        market_price_pct=98.0,
        delinquency_buckets=[Money(1.0, USD), Money(2.0, USD)],
        balloon={"extension_prob": 0.3, "extension_months": 24, "extension_rate": 0.07},
        noi=Money(5.0, USD),
    ),
    "RepLine.min": lambda: RepLine(
        id="LINE-1",
        balance=Money(80_000_000.0, USD),
        rate=0.07,
        maturity=dt.date(2031, 1, 15),
        seasoning_months=12,
        day_count=DayCount.ACT_360,
        asset_type={"type": "first_lien_loan"},
    ),
    "RepLine.full": lambda: RepLine(
        id="LINE-1",
        balance=Money(80.0, USD),
        rate=0.07,
        maturity="2031-01-15",
        seasoning_months=12,
        day_count=DayCount.ACT_360,
        asset_type={"type": "first_lien_loan"},
        spread_bp=150.0,
        forward_curve_id="USD-SOFR-3M",
        index_floor_bp=25.0,
        cpr=0.10,
        cdr=0.02,
        recovery_rate=0.45,
        contractual_payment=Money(1.0, USD),
        amortization_term_months=240,
        io_months=6,
    ),
    "StandardDealParams.min": lambda: StandardDealParams(
        id="ABS-1",
        pool=_pool(),
        tranches=_tranches(),
        closing_date=dt.date(2024, 1, 15),
        maturity=dt.date(2031, 1, 15),
        discount_curve_id="USD-SOFR-DISC",
    ),
    "StandardDealParams.full": lambda: StandardDealParams(
        id="ABS-1",
        pool=_pool(),
        tranches=_tranches(),
        closing_date=dt.date(2024, 1, 15),
        maturity=dt.date(2031, 1, 15),
        discount_curve_id="USD-SOFR-DISC",
        calendar_id="nyse",
    ),
    "AssetPool.with_accounts.some": lambda: _pool().with_accounts(
        collection_account=Money(125_000.0, USD), original_balance=Money(3_000_000.0, USD)
    ),
    "AssetPool.with_accounts.all": lambda: _pool().with_accounts(
        cumulative_defaults=Money(1.0, USD),
        cumulative_recoveries=Money(2.0, USD),
        cumulative_prepayments=Money(3.0, USD),
        cumulative_scheduled_amortization=Money(4.0, USD),
        collection_account=Money(5.0, USD),
        excess_spread_account=Money(6.0, USD),
        original_balance=Money(7.0, USD),
    ),
}


def _wire(value: Any) -> str:
    if hasattr(value, "to_json"):
        return value.to_json()
    # ``CdsTrancheParams`` carries getters only; pin each getter's text form.
    return repr([(name, str(getattr(value, name))) for name in TRANCHE_GETTERS])


@pytest.mark.parametrize("key", sorted(CASES))
def test_keyword_constructor_matches_the_positional_golden(key: str) -> None:
    assert _wire(CASES[key]()) == GOLDEN[key]


def test_every_golden_entry_is_pinned() -> None:
    assert set(GOLDEN) - {"_pool", "_tranches"} == set(CASES)


@pytest.mark.parametrize(
    "build",
    [
        lambda: FloatingRateSpec("USD-SOFR", "0", Tenor.parse("3M")),
        lambda: RecoveryClaim("SEN", "secured", 1, 100.0, 0.0, 0.0, 0.0),
        lambda: FixedLegSpec("USD-OIS", "0.04", Tenor.semi_annual(), DayCount.THIRTY_360, START, END),
    ],
)
def test_positional_fields_are_rejected(build: Any) -> None:
    with pytest.raises(TypeError):
        build()


@pytest.mark.parametrize(
    "build",
    [
        lambda: ScheduleParams(frequency=Tenor.parse("3M"), day_count=DayCount.ACT_360, calendar_id="x", lag=1),
        lambda: RecoveryClaim(
            id="SEN",
            seniority="secured",
            priority=1,
            principal=1.0,
            accrued=0.0,
            penalties=0.0,
            collateral_haircut=0.0,
            total_claim=1.0,
        ),
        lambda: _pool().with_accounts(assets=[]),
    ],
)
def test_unknown_keywords_are_rejected_by_rust(build: Any) -> None:
    with pytest.raises(ValueError, match="unknown field"):
        build()


def test_decimal_fields_take_the_exact_wire_string_not_a_float() -> None:
    with pytest.raises(ValueError, match="floating point"):
        FixedLegSpec(
            discount_curve_id="USD-OIS",
            rate=0.04,
            frequency=Tenor.semi_annual(),
            day_count=DayCount.THIRTY_360,
            start=START,
            end=END,
        )
