"""Typed collateral and facility sub-specs.

Construction, wire round-trips and use as ``PoolAsset`` / ``AssetBackedFacility``
inputs without JSON.
"""

from __future__ import annotations

import datetime
import pickle

import pytest

from finstack_quant.cashflows.builder import DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec
from finstack_quant.core.currency import Currency
from finstack_quant.core.dates import DayCount, Tenor
from finstack_quant.core.market_data import DiscountCurve, MarketContext
from finstack_quant.core.money import Money
from finstack_quant.valuations.instruments import (
    AdvanceRate,
    AmortizationEvent,
    AssetBackedFacility,
    AssetPool,
    BalloonSpec,
    BorrowingBaseRules,
    ConcentrationLimit,
    EligibilityRule,
    LiquidationSpec,
    PoolAsset,
    PrepaymentPenalty,
    SpecialServicingSpec,
    StructuredCredit,
    TermOutSpec,
    Tranche,
    TrancheStructure,
)

USD = Currency("USD")
CLOSE = datetime.date(2024, 1, 15)
MATURITY = datetime.date(2029, 1, 15)


def usd(amount: float) -> Money:
    return Money(amount, USD)


@pytest.mark.parametrize(
    "value",
    [
        BalloonSpec(0.3, 24, extension_rate=0.07, loss_prob=0.1, severity_pct=40.0, workout_months=18),
        PrepaymentPenalty.lockout(datetime.date(2025, 12, 31)),
        PrepaymentPenalty.fixed(3.0, through=datetime.date(2026, 1, 1)),
        PrepaymentPenalty.step_down([(datetime.date(2025, 12, 31), 5.0), (datetime.date(2026, 12, 31), 3.0)]),
        PrepaymentPenalty.yield_maintenance(reinvestment_rate=0.05, discount_curve_id="USD-OIS", floor_pct=1.0),
        SpecialServicingSpec(40.0),
        LiquidationSpec(18, 65.0, carry_cost_pct=5.0, reperformance_prob=0.2, modified_rate=0.04),
        EligibilityRule(max_days_past_due=60, max_maturity=datetime.date(2030, 1, 1)),
        AdvanceRate("commercial_mortgage", 0.8, eligibility=EligibilityRule(exclude_defaulted=False)),
        ConcentrationLimit("industry", 20.0),
        BorrowingBaseRules(
            [AdvanceRate("commercial_mortgage", 0.8)], concentration_limits=[ConcentrationLimit("obligor", 5.0)]
        ),
        TermOutSpec(24),
        AmortizationEvent.date(datetime.date(2025, 6, 1)),
        AmortizationEvent.cumulative_loss(4.0),
        AmortizationEvent.excess_spread(0.01),
    ],
)
def test_typed_specs_round_trip_through_json_dict_and_pickle(value: object) -> None:
    cls = type(value)
    again = cls.from_json(value.to_json())
    assert again.to_dict() == value.to_dict()
    assert pickle.loads(pickle.dumps(value)).to_dict() == value.to_dict()  # noqa: S301 - own payload
    assert cls.__name__ in repr(value)


def test_typed_specs_validate_their_inputs() -> None:
    with pytest.raises(ValueError, match="extension_prob"):
        BalloonSpec(1.5, 24)
    with pytest.raises(ValueError, match="reinvestment_rate"):
        PrepaymentPenalty.yield_maintenance()
    with pytest.raises(ValueError, match="at least one step"):
        PrepaymentPenalty.step_down([])
    with pytest.raises(ValueError, match="appraisal_reduction_pct"):
        SpecialServicingSpec(140.0)
    with pytest.raises(ValueError, match="concentration scope"):
        ConcentrationLimit("country", 20.0)
    with pytest.raises(ValueError, match="advance rate"):
        AdvanceRate("commercial_mortgage", 1.5)
    with pytest.raises(ValueError, match="advance"):
        BorrowingBaseRules([])
    with pytest.raises(ValueError, match="max_pct"):
        AmortizationEvent.cumulative_loss(-1.0)


def test_penalty_and_event_accessors_follow_the_kind() -> None:
    steps = PrepaymentPenalty.step_down([(datetime.date(2025, 12, 31), 5.0)])
    assert steps.kind == "step_down"
    assert steps.schedule == [(datetime.date(2025, 12, 31), 5.0)]
    assert steps.through is None
    assert steps.pct is None
    ym = PrepaymentPenalty.yield_maintenance(discount_curve_id="USD-OIS", floor_pct=1.0)
    assert ym.reinvestment_rate is None
    assert ym.discount_curve_id == "USD-OIS"
    assert ym.to_dict()["kind"] == "yield_maintenance"
    event = AmortizationEvent.excess_spread(0.01)
    assert event.kind == "excess_spread"
    assert event.max_pct is None
    assert event.date_value is None
    assert event.min_3m == 0.01


def test_typed_specs_are_accepted_by_pool_asset_and_the_deal_prices() -> None:
    """A conduit CMBS assembled from typed terms only."""
    loan = PoolAsset(
        "L1",
        {"type": "commercial_mortgage", "ltv": None},
        usd(10_000_000.0),
        0.06,
        MATURITY,
        day_count=DayCount.THIRTY_360,
        io_months=60,
        balloon=BalloonSpec(0.3, 24, extension_rate=0.07, loss_prob=0.1, severity_pct=40.0, workout_months=12),
        prepayment_penalty=PrepaymentPenalty.lockout(datetime.date(2026, 1, 15)),
        special_servicing=SpecialServicingSpec(0.0),
    )
    assert loan.balloon["extension_months"] == 24
    assert loan.prepayment_penalty == {"kind": "lockout", "through": "2026-01-15"}
    npl = PoolAsset(
        "N1",
        {"type": "commercial_mortgage", "ltv": None},
        usd(2_000_000.0),
        0.0,
        MATURITY,
        liquidation=LiquidationSpec(18, 65.0, carry_cost_pct=5.0),
    )
    assert npl.liquidation["months_to_resolution"] == 18
    pool = AssetPool("P", "cmbs", USD).with_assets([loan, npl])
    tranches = TrancheStructure([
        Tranche
        .builder()
        .id("A")
        .attachment_point(30.0)
        .detachment_point(100.0)
        .seniority("senior")
        .original_balance(usd(8_400_000.0))
        .coupon_fixed(0.05)
        .maturity(MATURITY)
        .build(),
        Tranche
        .builder()
        .id("E")
        .attachment_point(0.0)
        .detachment_point(30.0)
        .seniority("equity")
        .original_balance(usd(3_600_000.0))
        .coupon_fixed(0.0)
        .maturity(MATURITY)
        .build(),
    ])
    deal = (
        StructuredCredit
        .builder()
        .id("CMBS-TYPED")
        .deal_type("cmbs")
        .pool(pool)
        .tranches(tranches)
        .closing_date(CLOSE)
        .first_payment_date(datetime.date(2024, 2, 15))
        .maturity(MATURITY)
        .frequency(Tenor.monthly())
        .payment_calendar_id("nyse")
        .discount_curve_id("USD-OIS")
        .prepayment_spec(PrepaymentModelSpec.constant_cpr(0.20))
        .default_spec(DefaultModelSpec.constant_cdr(0.0))
        .recovery_spec(RecoveryModelSpec(0.5, 0))
        .build()
    )
    market = MarketContext().insert(DiscountCurve.flat("USD-OIS", CLOSE, 0.04))
    diagnostics = deal.run_simulation_with_diagnostics(market, CLOSE)
    frame = diagnostics.to_dataframe()
    assert len(frame) > 12
    assert (frame["pool_balance"].iloc[:12] >= 10_000_000.0 - 1e-6).all(), "the lockout stops prepayment"


def test_typed_facility_terms_are_accepted_by_the_facility_builder() -> None:
    collateral = AssetPool("WH", "abs", USD).with_assets([
        PoolAsset.fixed_rate_bond("L1", usd(50_000_000.0), 0.08, MATURITY, DayCount.ACT_360)
    ])
    facility = (
        AssetBackedFacility
        .builder()
        .id("WH-TYPED")
        .collateral(collateral)
        .borrowing_base_rules(BorrowingBaseRules([AdvanceRate("high_yield_bond", 0.8)]))
        .commitment(usd(45_000_000.0))
        .drawn(usd(30_000_000.0))
        .margin_bp(500.0)
        .closing_date(CLOSE)
        .revolving_end(datetime.date(2026, 1, 15))
        .maturity(MATURITY)
        .frequency("3M")
        .payment_calendar_id("nyse")
        .term_out(TermOutSpec(24).months)
        .amortization_events([AmortizationEvent.cumulative_loss(4.0), AmortizationEvent.excess_spread(0.01)])
        .discount_curve_id("USD-OIS")
        .build()
    )
    assert [event["kind"] for event in facility.amortization_events] == ["cumulative_loss", "excess_spread"]
    assert facility.borrowing_base_rules["advance_rates"][0]["rate"] == 0.8
    market = MarketContext().insert(DiscountCurve.flat("USD-OIS", CLOSE, 0.04))
    projection = facility.project(market, CLOSE)
    assert projection.facility.total_principal.amount > 0.0
