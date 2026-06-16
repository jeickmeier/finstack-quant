//! Common test utilities and fixtures for ILB tests

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DateExt, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::{
    InflationIndex, InflationInterpolation, InflationLag,
};
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::market_data::term_structures::InflationCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::fixed_income::inflation_linked_bond::{
    DeflationProtection, IndexationMethod, InflationLinkedBond,
};
use finstack_quant_valuations::instruments::Attributes;
use rust_decimal::Decimal;
use time::Month;

/// Shorthand for date creation
pub fn d(y: i32, m: u8, day: u8) -> Date {
    Date::from_calendar_date(y, Month::try_from(m).unwrap(), day).unwrap()
}

fn monthly_observations(
    start: Date,
    months: usize,
    start_value: f64,
    monthly_rate: f64,
) -> Vec<(Date, f64)> {
    (0..months)
        .map(|i| {
            let date = start.add_months(i as i32);
            let value = start_value * (1.0 + monthly_rate).powi(i as i32);
            (date, value)
        })
        .collect()
}

/// Create a standard TIPS bond for testing
pub fn sample_tips() -> InflationLinkedBond {
    let mut bond = InflationLinkedBond::builder()
        .id(InstrumentId::new("TIPS-TEST"))
        .notional(Money::new(1_000_000.0, Currency::USD))
        .real_coupon(Decimal::try_from(0.0125).expect("valid decimal")) // 1.25% real coupon
        .frequency(Tenor::semi_annual())
        .day_count(DayCount::ActAct)
        .issue_date(d(2020, 1, 15))
        .maturity(d(2030, 1, 15))
        .base_index(250.0)
        .base_date(d(2019, 10, 1))
        .indexation_method(IndexationMethod::TIPS)
        .lag(InflationLag::Months(3))
        .deflation_protection(DeflationProtection::MaturityOnly)
        .bdc(BusinessDayConvention::Following)
        .stub(StubKind::ShortBack)
        .discount_curve_id(CurveId::new("USD-REAL"))
        .inflation_index_id(CurveId::new("US-CPI-U"))
        .attributes(Attributes::new())
        .build()
        .unwrap();

    bond.quoted_clean = Some(100.0);
    bond
}

/// Create a UK Index-Linked Gilt for testing
pub fn sample_uk_linker() -> InflationLinkedBond {
    let mut bond = InflationLinkedBond::builder()
        .id(InstrumentId::new("UK-GILT-TEST"))
        .notional(Money::new(1_000_000.0, Currency::GBP))
        .real_coupon(Decimal::try_from(0.00625).expect("valid decimal")) // 0.625% real coupon
        .frequency(Tenor::semi_annual())
        .day_count(DayCount::ActAct)
        .issue_date(d(2020, 3, 22))
        .maturity(d(2040, 3, 22))
        .base_index(280.0)
        .base_date(d(2019, 7, 1))
        .indexation_method(IndexationMethod::UK)
        .lag(InflationLag::Months(8))
        .deflation_protection(DeflationProtection::None)
        .bdc(BusinessDayConvention::Following)
        .stub(StubKind::ShortBack)
        .discount_curve_id(CurveId::new("GBP-NOMINAL"))
        .inflation_index_id(CurveId::new("UK-RPI"))
        .attributes(Attributes::new())
        .build()
        .unwrap();

    bond.quoted_clean = Some(100.0);
    bond
}

/// Create a market context with inflation index (linear interpolation for TIPS)
pub fn market_context_with_index() -> (MarketContext, InflationIndex) {
    let disc = DiscountCurve::builder("USD-REAL")
        .base_date(d(2025, 1, 2))
        .knots([
            (0.0, 1.0),
            (0.5, 0.99),
            (1.0, 0.98),
            (2.0, 0.96),
            (5.0, 0.90),
            (10.0, 0.82),
        ])
        .build()
        .unwrap();

    // Cover the bond's full lagged history so schedule generation can look up
    // early coupon fixings instead of failing on missing 2020-era observations.
    let observations = monthly_observations(d(2019, 10, 1), 364, 250.0, 0.002);
    let index = InflationIndex::new("US-CPI-U", observations, Currency::USD)
        .unwrap()
        .with_interpolation(InflationInterpolation::Linear);

    let ctx = MarketContext::new()
        .insert(disc)
        .insert_inflation_index("US-CPI-U", index.clone());

    (ctx, index)
}

/// Create a market context with inflation curve (for forward projections)
pub fn market_context_with_curve() -> (MarketContext, InflationCurve) {
    let disc = DiscountCurve::builder("USD-REAL")
        .base_date(d(2025, 1, 2))
        .knots([
            (0.0, 1.0),
            (0.5, 0.99),
            (1.0, 0.98),
            (2.0, 0.96),
            (5.0, 0.90),
            (10.0, 0.82),
        ])
        .build()
        .unwrap();

    let inflation_base = d(2025, 1, 2);
    let curve = InflationCurve::builder("US-CPI-U")
        .base_date(inflation_base)
        .base_cpi(300.0)
        .knots([
            (0.0, 300.0),
            (0.5, 303.0),
            (1.0, 306.0),
            (2.0, 312.0),
            (5.0, 330.0),
            (10.0, 366.0),
        ])
        .build()
        .unwrap();

    let ctx = MarketContext::new().insert(disc).insert(curve);

    (
        ctx,
        InflationCurve::builder("US-CPI-U")
            .base_date(inflation_base)
            .base_cpi(300.0)
            .knots([
                (0.0, 300.0),
                (0.5, 303.0),
                (1.0, 306.0),
                (2.0, 312.0),
                (5.0, 330.0),
                (10.0, 366.0),
            ])
            .build()
            .unwrap(),
    )
}

/// Create a UK market context with step interpolation
pub fn uk_market_context() -> (MarketContext, InflationIndex) {
    let disc = DiscountCurve::builder("GBP-NOMINAL")
        .base_date(d(2025, 1, 2))
        .knots([
            (0.0, 1.0),
            (0.5, 0.985),
            (1.0, 0.97),
            (2.0, 0.94),
            (5.0, 0.87),
            (15.0, 0.65),
        ])
        .build()
        .unwrap();

    let observations = monthly_observations(d(2019, 7, 1), 260, 280.0, 0.0025);
    let index = InflationIndex::new("UK-RPI", observations, Currency::GBP)
        .unwrap()
        .with_interpolation(InflationInterpolation::Step);

    let ctx = MarketContext::new()
        .insert(disc)
        .insert_inflation_index("UK-RPI", index.clone());

    (ctx, index)
}

/// Tolerance for floating point comparisons
pub const EPSILON: f64 = 1e-9;

/// Relative tolerance for financial calculations
pub const REL_TOL: f64 = 1e-6;

/// Helper to check relative difference
pub fn relative_diff(a: f64, b: f64) -> f64 {
    if b.abs() < EPSILON {
        (a - b).abs()
    } else {
        ((a - b) / b).abs()
    }
}

/// Assert two values are approximately equal
#[track_caller]
pub fn assert_approx_eq(a: f64, b: f64, tol: f64, msg: &str) {
    let diff = relative_diff(a, b);
    assert!(
        diff < tol,
        "{}: expected {}, got {} (rel diff: {})",
        msg,
        b,
        a,
        diff
    );
}
