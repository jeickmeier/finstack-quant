//! Capital structure builder integration tests.
//!
//! Instruments are built through their canonical Valuations APIs and attached
//! with `add_debt`; Statements has no bond or swap construction helpers.
#![allow(clippy::expect_used, clippy::panic)]

use finstack_quant_cashflows::CashflowScheduleSource;
use finstack_quant_cashflows::builder::FloatingLegCompounding;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId, Rate};
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::types::{FinancialModelSpec, FinancialStatementInstrument};
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::fixed_income::revolving_credit::RevolvingCredit;
use finstack_quant_valuations::instruments::rates::irs::InterestRateSwap;
use finstack_quant_valuations::instruments::{
    BondConvention, FixedLegSpec, FloatLegSpec, PayReceive,
};
use rust_decimal::Decimal;
use time::macros::date;

fn usd(amount: f64) -> Money {
    Money::new(amount, Currency::USD).expect("valid money fixture")
}

/// A bond on a deliberately selected dated convention.
fn treasury_bond(id: &str) -> Bond {
    Bond::with_convention(
        InstrumentId::new(id),
        usd(1_000_000.0),
        Rate::from_decimal(0.03).expect("valid coupon"),
        date!(2025 - 01 - 15),
        date!(2030 - 01 - 15),
        BondConvention::UsTreasury,
        CurveId::new("USD-OIS"),
    )
    .expect("valid bond")
}

/// A pay-fixed swap with every leg term authored.
fn authored_swap(id: &str) -> InterestRateSwap {
    let (start, end) = (date!(2025 - 01 - 01), date!(2030 - 01 - 01));
    let fixed = FixedLegSpec {
        discount_curve_id: CurveId::new("USD-OIS"),
        rate: Decimal::new(4, 2),
        frequency: Tenor::annual(),
        day_count: DayCount::Act360,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id: Some("usny".into()),
        stub: StubKind::None,
        start,
        end,
        payment_lag_days: 0,
        end_of_month: false,
    };
    let float = FloatLegSpec {
        discount_curve_id: CurveId::new("USD-OIS"),
        index_id: "USD-SOFR-3M".into(),
        forward_curve_id: CurveId::new("USD-SOFR-3M"),
        spread_bp: Decimal::ZERO,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id: Some("usny".into()),
        stub: StubKind::None,
        reset_lag_days: 0,
        fixing_calendar_id: None,
        start,
        end,
        compounding: FloatingLegCompounding::Simple,
        payment_lag_days: 0,
        end_of_month: false,
    };
    InterestRateSwap::builder()
        .id(InstrumentId::new(id))
        .notional(usd(5_000_000.0))
        .side(PayReceive::Pay)
        .fixed_leg(fixed)
        .float_leg(float)
        .build()
        .expect("valid swap")
}

fn model_with(instruments: Vec<(&str, FinancialStatementInstrument)>) -> FinancialModelSpec {
    let mut builder = ModelBuilder::new("test");
    builder
        .periods("2025Q1..2025Q2", None)
        .expect("valid period range");
    for (id, instrument) in instruments {
        builder.add_debt(id, instrument);
    }
    builder.build().expect("valid model")
}

#[test]
fn add_debt_attaches_canonical_instruments_in_order() {
    let model = model_with(vec![
        (
            "BOND-001",
            FinancialStatementInstrument::Bond(treasury_bond("BOND-001")),
        ),
        (
            "SWAP-001",
            FinancialStatementInstrument::InterestRateSwap(authored_swap("SWAP-001")),
        ),
        (
            "RCF-A",
            FinancialStatementInstrument::RevolvingCredit(
                RevolvingCredit::example().expect("valid revolving credit example"),
            ),
        ),
    ]);
    let debt = model
        .capital_structure
        .expect("capital structure")
        .debt_instruments;
    let ids: Vec<&str> = debt.iter().map(|entry| entry.id.as_str()).collect();
    assert_eq!(ids, ["BOND-001", "SWAP-001", "RCF-A"]);
    assert!(matches!(
        debt[0].spec,
        FinancialStatementInstrument::Bond(_)
    ));
    assert!(matches!(
        debt[1].spec,
        FinancialStatementInstrument::InterestRateSwap(_)
    ));
}

/// The instrument the model holds is the canonical instrument, unchanged:
/// its resolved terms, elected convention and contractual schedule survive
/// attachment and a replay through the model's wire form.
#[test]
fn resolved_terms_and_schedule_survive_attachment_and_replay() {
    let bond = treasury_bond("BOND-001");
    let swap = authored_swap("SWAP-001");
    let model = model_with(vec![
        ("BOND-001", FinancialStatementInstrument::Bond(bond.clone())),
        (
            "SWAP-001",
            FinancialStatementInstrument::InterestRateSwap(swap.clone()),
        ),
    ]);
    let wire = serde_json::to_string(&model).expect("serialize model");
    let replayed: FinancialModelSpec = serde_json::from_str(&wire).expect("replay model");

    let terms = |instrument: &FinancialStatementInstrument| {
        serde_json::to_value(instrument).expect("serialize instrument")
    };
    let expected = [
        terms(&FinancialStatementInstrument::Bond(bond.clone())),
        terms(&FinancialStatementInstrument::InterestRateSwap(swap)),
    ];
    for candidate in [&model, &replayed] {
        let debt = &candidate
            .capital_structure
            .as_ref()
            .expect("capital structure")
            .debt_instruments;
        for (entry, expected) in debt.iter().zip(&expected) {
            assert_eq!(&terms(&entry.spec), expected, "{}", entry.id);
        }
    }
    assert_eq!(
        replayed.content_hash().unwrap(),
        model.content_hash().unwrap()
    );

    // The contractual schedule is the canonical bond's own, before and after
    // the replay: the same dated flows.
    let schedule = |instrument: &FinancialStatementInstrument| {
        let FinancialStatementInstrument::Bond(bond) = instrument else {
            panic!("expected a bond");
        };
        let schedule = bond
            .raw_cashflow_schedule(&MarketContext::new(), date!(2025 - 01 - 15))
            .expect("schedule");
        assert!(
            schedule.get_flows().len() >= 11,
            "ten coupons and the redemption"
        );
        format!("{:?}", schedule.get_flows())
    };
    let replayed_bond = &replayed
        .capital_structure
        .as_ref()
        .expect("capital structure")
        .debt_instruments[0]
        .spec;
    assert_eq!(
        schedule(replayed_bond),
        schedule(&FinancialStatementInstrument::Bond(bond))
    );
}
