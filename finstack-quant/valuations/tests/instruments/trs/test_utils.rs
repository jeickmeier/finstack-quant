//! Common test utilities for TRS tests.
//!
//! Provides shared fixtures, builders, and helpers for TRS unit tests.

use finstack_quant_cashflows::builder::ScheduleParams;
use finstack_quant_core::{
    currency::Currency,
    dates::{Date, DayCount},
    market_data::{
        context::MarketContext, scalars::MarketScalar, term_structures::DiscountCurve,
        term_structures::ForwardCurve,
    },
    math::interp::InterpStyle,
    money::Money,
    types::CurveId,
};
use finstack_quant_valuations::instruments::equity::equity_trs::EquityTotalReturnSwap;
use finstack_quant_valuations::instruments::fixed_income::fi_trs::FIIndexTotalReturnSwap;
use finstack_quant_valuations::instruments::EquityUnderlyingParams;
use finstack_quant_valuations::instruments::FinancingLegSpec;
use finstack_quant_valuations::instruments::IndexUnderlyingParams;
use finstack_quant_valuations::instruments::{TrsScheduleSpec, TrsSide};
use rust_decimal::Decimal;
use time::Month;

/// Creates a standard test date.
pub fn d(y: i32, m: u8, day: u8) -> Date {
    Date::from_calendar_date(y, Month::try_from(m).unwrap(), day).unwrap()
}

/// Standard valuation date for tests.
pub fn as_of_date() -> Date {
    d(2025, 1, 2)
}

/// Creates a fully populated market context for TRS testing.
///
/// Includes:
/// - USD-OIS discount curve (flat to 5Y)
/// - USD-SOFR-3M forward curve
/// - SPX spot and dividend yield
/// - HY index yield and duration
/// - EUR curves for multi-currency tests
pub fn create_market_context() -> MarketContext {
    let mut context = MarketContext::new();

    // USD discount curve (OIS)
    let disc_curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of_date())
        .knots(vec![
            (0.0, 1.0),
            (0.25, 0.995),
            (0.5, 0.990),
            (1.0, 0.980),
            (2.0, 0.960),
            (5.0, 0.900),
        ])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();
    context = context.insert(disc_curve);

    // USD forward curve (SOFR 3M)
    let fwd_curve = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(as_of_date())
        .knots(vec![
            (0.0, 0.02),
            (0.25, 0.021),
            (0.5, 0.022),
            (1.0, 0.023),
            (2.0, 0.024),
            (5.0, 0.025),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();
    context = context.insert(fwd_curve);

    // EUR curves for multi-currency testing
    let eur_disc = DiscountCurve::builder("EUR-ESTR")
        .base_date(as_of_date())
        .knots(vec![
            (0.0, 1.0),
            (0.25, 0.9975),
            (0.5, 0.995),
            (1.0, 0.990),
            (2.0, 0.980),
        ])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();
    context = context.insert(eur_disc);

    let eur_fwd = ForwardCurve::builder("EUR-EURIBOR-3M", 0.25)
        .base_date(as_of_date())
        .knots(vec![
            (0.0, 0.015),
            (0.25, 0.016),
            (0.5, 0.017),
            (1.0, 0.018),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();
    context = context.insert(eur_fwd);

    // Equity market data
    context = context.insert_price("SPX-SPOT", MarketScalar::Unitless(5000.0));
    context = context.insert_price("SPX-DIV-YIELD", MarketScalar::Unitless(0.015)); // 1.5%
    context = context.insert_price("NDX-SPOT", MarketScalar::Unitless(18000.0));
    context = context.insert_price("NDX-DIV-YIELD", MarketScalar::Unitless(0.005)); // 0.5%

    // Fixed income index market data
    context = context.insert_price("HY-INDEX-YIELD", MarketScalar::Unitless(0.055)); // 5.5%
    context = context.insert_price("HY-INDEX-DURATION", MarketScalar::Unitless(4.5)); // 4.5 years
    context = context.insert_price("IG-INDEX-YIELD", MarketScalar::Unitless(0.035)); // 3.5%
    context = context.insert_price("IG-INDEX-DURATION", MarketScalar::Unitless(7.0)); // 7 years

    context
}

/// Builder for test equity TRS with sensible defaults.
pub struct TestEquityTrsBuilder {
    id: String,
    notional: Money,
    spot_id: String,
    div_yield_id: Option<CurveId>,
    contract_size: f64,
    discount_curve_id: String,
    forward_curve_id: String,
    spread_bp: f64,
    start: Date,
    end: Date,
    side: TrsSide,
    initial_level: Option<f64>,
}

impl Default for TestEquityTrsBuilder {
    fn default() -> Self {
        Self {
            id: "TEST-EQ-TRS-001".into(),
            notional: Money::new(10_000_000.0, Currency::USD),
            spot_id: "SPX-SPOT".into(),
            div_yield_id: Some(CurveId::new("SPX-DIV-YIELD")),
            contract_size: 1.0,
            discount_curve_id: "USD-OIS".into(),
            forward_curve_id: "USD-SOFR-3M".into(),
            spread_bp: 25.0,
            // Keep `start` strictly after `as_of` so theta (which rolls the
            // valuation date forward) doesn't cross the start date.
            start: as_of_date() + time::Duration::days(2),
            end: d(2026, 1, 2),
            side: TrsSide::ReceiveTotalReturn,
            initial_level: None,
        }
    }
}

impl TestEquityTrsBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    pub fn notional(mut self, notional: Money) -> Self {
        self.notional = notional;
        self
    }

    pub fn div_yield_id(mut self, id: Option<CurveId>) -> Self {
        self.div_yield_id = id;
        self
    }

    pub fn spread_bp(mut self, spread: f64) -> Self {
        self.spread_bp = spread;
        self
    }

    pub fn tenor_months(mut self, months: i32) -> Self {
        self.end = as_of_date() + time::Duration::days((months * 30) as i64);
        self
    }

    pub fn side(mut self, side: TrsSide) -> Self {
        self.side = side;
        self
    }

    pub fn initial_level(mut self, level: f64) -> Self {
        self.initial_level = Some(level);
        self
    }

    pub fn build(self) -> EquityTotalReturnSwap {
        let mut underlying =
            EquityUnderlyingParams::new("TEST-EQ", self.spot_id, self.notional.currency())
                .with_contract_size(self.contract_size);
        if let Some(div_id) = self.div_yield_id {
            underlying = underlying.with_dividend_yield(div_id);
        }

        let financing = FinancingLegSpec::new(
            self.discount_curve_id,
            self.forward_curve_id,
            Decimal::try_from(self.spread_bp).expect("valid spread_bp"),
            DayCount::Act360,
        );

        let schedule =
            TrsScheduleSpec::from_params(self.start, self.end, ScheduleParams::quarterly_act360());

        let mut builder = EquityTotalReturnSwap::builder()
            .id(self.id.into())
            .notional(self.notional)
            .underlying(underlying)
            .financing(financing)
            .schedule(schedule)
            .side(self.side);

        if let Some(level) = self.initial_level {
            builder = builder.initial_level(level);
        }

        builder.build().unwrap()
    }
}

/// Builder for test FI index TRS with sensible defaults.
pub struct TestFIIndexTrsBuilder {
    id: String,
    notional: Money,
    index_id: String,
    yield_id: Option<String>,
    duration_id: Option<String>,
    contract_size: f64,
    discount_curve_id: String,
    forward_curve_id: String,
    spread_bp: f64,
    start: Date,
    end: Date,
    side: TrsSide,
    initial_level: Option<f64>,
}

impl Default for TestFIIndexTrsBuilder {
    fn default() -> Self {
        Self {
            id: "TEST-FI-TRS-001".into(),
            notional: Money::new(10_000_000.0, Currency::USD),
            index_id: "HY-INDEX".into(),
            yield_id: Some("HY-INDEX-YIELD".into()),
            duration_id: Some("HY-INDEX-DURATION".into()),
            contract_size: 1.0,
            discount_curve_id: "USD-OIS".into(),
            forward_curve_id: "USD-SOFR-3M".into(),
            spread_bp: 100.0,
            // Keep `start` strictly after `as_of` so theta (which rolls the
            // valuation date forward) doesn't cross the start date.
            start: as_of_date() + time::Duration::days(2),
            end: d(2026, 1, 2),
            side: TrsSide::ReceiveTotalReturn,
            initial_level: None,
        }
    }
}

impl TestFIIndexTrsBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    pub fn notional(mut self, notional: Money) -> Self {
        self.notional = notional;
        self
    }

    pub fn spread_bp(mut self, spread: f64) -> Self {
        self.spread_bp = spread;
        self
    }

    pub fn tenor_months(mut self, months: i32) -> Self {
        self.end = as_of_date() + time::Duration::days((months * 30) as i64);
        self
    }

    pub fn side(mut self, side: TrsSide) -> Self {
        self.side = side;
        self
    }

    pub fn yield_id(mut self, id: Option<String>) -> Self {
        self.yield_id = id;
        self
    }

    pub fn duration_id(mut self, id: Option<String>) -> Self {
        self.duration_id = id;
        self
    }

    pub fn build(self) -> FIIndexTotalReturnSwap {
        let mut underlying =
            IndexUnderlyingParams::new(self.index_id.clone(), self.notional.currency())
                .with_contract_size(self.contract_size);

        if let Some(y_id) = self.yield_id {
            underlying = underlying.with_yield(y_id);
        }
        if let Some(d_id) = self.duration_id {
            underlying = underlying.with_duration(d_id);
        }

        let financing = FinancingLegSpec::new(
            self.discount_curve_id,
            self.forward_curve_id,
            Decimal::try_from(self.spread_bp).expect("valid spread_bp"),
            DayCount::Act360,
        );

        let schedule =
            TrsScheduleSpec::from_params(self.start, self.end, ScheduleParams::quarterly_act360());

        let mut builder = FIIndexTotalReturnSwap::builder()
            .id(self.id.into())
            .notional(self.notional)
            .underlying(underlying)
            .financing(financing)
            .schedule(schedule)
            .side(self.side);

        if let Some(level) = self.initial_level {
            builder = builder.initial_level(level);
        }

        builder.build().unwrap()
    }
}

/// Tolerance for floating point comparisons (1 cent).
pub const TOLERANCE_CENTS: f64 = 0.01;

/// Helper to assert two floats are approximately equal.
pub fn assert_approx_eq(actual: f64, expected: f64, tolerance: f64, msg: &str) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{}: expected {}, got {} (diff: {})",
        msg,
        expected,
        actual,
        (actual - expected).abs()
    );
}

/// Helper to assert two Money values are approximately equal.
pub fn assert_money_approx_eq(actual: Money, expected: Money, tolerance: f64, msg: &str) {
    assert_eq!(
        actual.currency(),
        expected.currency(),
        "{}: currency mismatch",
        msg
    );
    assert_approx_eq(actual.amount(), expected.amount(), tolerance, msg);
}
