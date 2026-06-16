//! Smoke tests for direct `CashflowProvider` coverage.

use finstack_quant_cashflows::CashflowProvider;
use finstack_quant_valuations::instruments::commodity::commodity_forward::CommodityForward;
use finstack_quant_valuations::instruments::commodity::commodity_swap::CommoditySwap;
use finstack_quant_valuations::instruments::credit_derivatives::cds::CreditDefaultSwap;
use finstack_quant_valuations::instruments::credit_derivatives::{CDSIndex, CDSTranche};
use finstack_quant_valuations::instruments::fixed_income::bond_future::BondFuture;
use finstack_quant_valuations::instruments::fixed_income::convertible::ConvertibleBond;
use finstack_quant_valuations::instruments::fixed_income::dollar_roll::DollarRoll;
use finstack_quant_valuations::instruments::fixed_income::revolving_credit::RevolvingCredit;
use finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan;
use finstack_quant_valuations::instruments::fixed_income::{
    AgencyCmo, AgencyMbsPassthrough, AgencyTba,
};
use finstack_quant_valuations::instruments::fx::fx_forward::FxForward;
use finstack_quant_valuations::instruments::fx::fx_swap::FxSwap;
use finstack_quant_valuations::instruments::fx::ndf::Ndf;
fn assert_provider<T: CashflowProvider>(_instrument: &T) {}
use finstack_quant_valuations::instruments::rates::basis_swap::BasisSwap;
use finstack_quant_valuations::instruments::rates::cms_swap::CmsSwap;
use finstack_quant_valuations::instruments::rates::inflation_swap::{
    InflationSwap, YoYInflationSwap,
};
use finstack_quant_valuations::instruments::rates::xccy_swap::XccySwap;

#[test]
fn term_loan_exposes_cashflow_provider_bridge() {
    let loan = TermLoan::example().expect("term loan example");

    assert_provider(&loan);
}

#[test]
fn revolving_credit_exposes_cashflow_provider_bridge() {
    let facility = RevolvingCredit::example().expect("revolving credit example");

    assert_provider(&facility);
}

#[test]
fn cds_exposes_cashflow_provider_bridge() {
    let cds = CreditDefaultSwap::example();

    assert_provider(&cds);
}

#[test]
fn commodity_forward_exposes_cashflow_provider_bridge() {
    let forward = CommodityForward::example();

    assert_provider(&forward);
}

#[test]
fn fx_forward_exposes_cashflow_provider_bridge() {
    let forward = FxForward::example().expect("fx forward example");

    assert_provider(&forward);
}

#[test]
fn ndf_exposes_cashflow_provider_bridge() {
    let ndf = Ndf::example();

    assert_provider(&ndf);
}

#[test]
fn dollar_roll_exposes_cashflow_provider_bridge() {
    let roll = DollarRoll::example().expect("dollar roll example");

    assert_provider(&roll);
}

#[test]
fn bond_future_exposes_cashflow_provider_bridge() {
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use finstack_quant_valuations::instruments::fixed_income::bond_future::{
        BondFutureSpecs, DeliverableBond, Position,
    };
    use time::Month;

    let future = BondFuture::builder()
        .id(InstrumentId::new("TYH5"))
        .notional(Money::new(1_000_000.0, Currency::USD))
        .expiry(Date::from_calendar_date(2025, Month::March, 20).expect("valid date"))
        .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("valid date"))
        .delivery_end(Date::from_calendar_date(2025, Month::March, 31).expect("valid date"))
        .quoted_price(125.50)
        .position(Position::Long)
        .contract_specs(BondFutureSpecs::default())
        .deliverable_basket(vec![DeliverableBond {
            bond_id: InstrumentId::new("US912828XG33"),
            conversion_factor: 0.8234,
        }])
        .discount_curve_id(CurveId::new("USD-TREASURY"))
        .attributes(Default::default())
        .build_validated()
        .expect("bond future fixture");

    assert_provider(&future);
}

#[test]
fn convertible_bond_exposes_cashflow_provider_bridge() {
    let bond = ConvertibleBond::example().expect("convertible bond example");

    assert_provider(&bond);
}

#[test]
fn inflation_swap_exposes_cashflow_provider_bridge() {
    let swap = InflationSwap::example();

    assert_provider(&swap);
}

#[test]
fn basis_swap_exposes_cashflow_provider_bridge() {
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::CurveId;
    use rust_decimal::Decimal;
    use time::Month;

    let start = Date::from_calendar_date(2024, Month::January, 3).expect("valid date");
    let end = Date::from_calendar_date(2025, Month::January, 3).expect("valid date");
    let primary_leg = finstack_quant_valuations::instruments::rates::basis_swap::BasisSwapLeg {
        forward_curve_id: CurveId::new("3M-SOFR"),
        discount_curve_id: CurveId::new("OIS"),
        start,
        end,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        stub: StubKind::ShortFront,
        spread_bp: Decimal::ZERO,
        payment_lag_days: 0,
        reset_lag_days: 0,
    };
    let reference_leg = finstack_quant_valuations::instruments::rates::basis_swap::BasisSwapLeg {
        forward_curve_id: CurveId::new("6M-SOFR"),
        ..primary_leg.clone()
    };
    let swap = BasisSwap::new(
        "BASIS-BRIDGE",
        Money::new(1_000_000.0, Currency::USD),
        primary_leg,
        reference_leg,
    )
    .expect("basis swap fixture");

    assert_provider(&swap);
}

#[test]
fn xccy_swap_exposes_cashflow_provider_bridge() {
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::CurveId;
    use rust_decimal::Decimal;
    use time::Month;

    let start = Date::from_calendar_date(2025, Month::January, 2).expect("valid date");
    let end = Date::from_calendar_date(2026, Month::January, 2).expect("valid date");
    let leg1 = finstack_quant_valuations::instruments::rates::xccy_swap::XccySwapLeg {
        currency: Currency::USD,
        notional: Money::new(1_000_000.0, Currency::USD),
        side: finstack_quant_valuations::instruments::rates::xccy_swap::LegSide::Receive,
        forward_curve_id: CurveId::new("USD-SOFR-3M"),
        discount_curve_id: CurveId::new("USD-OIS"),
        start,
        end,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        bdc: BusinessDayConvention::ModifiedFollowing,
        stub: StubKind::ShortFront,
        spread_bp: Decimal::ZERO,
        payment_lag_days: 0,
        calendar_id: None,
        reset_lag_days: None,
        allow_calendar_fallback: true,
    };
    let leg2 = finstack_quant_valuations::instruments::rates::xccy_swap::XccySwapLeg {
        currency: Currency::EUR,
        notional: Money::new(900_000.0, Currency::EUR),
        side: finstack_quant_valuations::instruments::rates::xccy_swap::LegSide::Pay,
        forward_curve_id: CurveId::new("EUR-EURIBOR-3M"),
        discount_curve_id: CurveId::new("EUR-OIS"),
        start,
        end,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        bdc: BusinessDayConvention::ModifiedFollowing,
        stub: StubKind::ShortFront,
        spread_bp: Decimal::ZERO,
        payment_lag_days: 0,
        calendar_id: None,
        reset_lag_days: None,
        allow_calendar_fallback: true,
    };
    let swap = XccySwap::new("XCCY-BRIDGE", leg1, leg2, Currency::USD);

    assert_provider(&swap);
}

#[test]
fn cms_swap_exposes_cashflow_provider_bridge() {
    let swap = CmsSwap::example();
    assert_provider(&swap);
}

#[test]
fn yoy_inflation_swap_exposes_cashflow_provider_bridge() {
    let swap = YoYInflationSwap::builder()
        .id("YOY-BRIDGE".into())
        .notional(finstack_quant_core::money::Money::new(
            1_000_000.0,
            finstack_quant_core::currency::Currency::USD,
        ))
        .start_date(
            finstack_quant_core::dates::Date::from_calendar_date(2025, time::Month::January, 1)
                .expect("valid date"),
        )
        .maturity(
            finstack_quant_core::dates::Date::from_calendar_date(2027, time::Month::January, 1)
                .expect("valid date"),
        )
        .fixed_rate(rust_decimal::Decimal::try_from(0.02).expect("valid"))
        .frequency(finstack_quant_core::dates::Tenor::annual())
        .inflation_index_id("US-CPI".into())
        .discount_curve_id("USD-OIS".into())
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .side(finstack_quant_valuations::instruments::PayReceive::Pay)
        .attributes(Default::default())
        .build()
        .expect("yoy fixture");

    assert_provider(&swap);
}

#[test]
fn commodity_swap_exposes_cashflow_provider_bridge() {
    let swap = CommoditySwap::example();
    assert_provider(&swap);
}

#[test]
fn fx_swap_exposes_cashflow_provider_bridge() {
    let swap = FxSwap::example();
    assert_provider(&swap);
}

#[test]
fn agency_mbs_passthrough_exposes_cashflow_provider_bridge() {
    let mbs = AgencyMbsPassthrough::example().expect("agency mbs example");
    assert_provider(&mbs);
}

#[test]
fn agency_tba_exposes_cashflow_provider_bridge() {
    let tba = AgencyTba::example().expect("agency tba example");
    assert_provider(&tba);
}

#[test]
fn agency_cmo_exposes_cashflow_provider_bridge() {
    let cmo = AgencyCmo::example().expect("agency cmo example");
    assert_provider(&cmo);
}

#[test]
fn cds_index_exposes_cashflow_provider_bridge() {
    let index = CDSIndex::example();
    assert_provider(&index);
}

#[test]
fn cds_tranche_exposes_cashflow_provider_bridge() {
    let tranche = CDSTranche::example();
    assert_provider(&tranche);
}
