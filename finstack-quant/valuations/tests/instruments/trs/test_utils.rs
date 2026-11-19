//! Common test utilities for TRS tests.
//!
//! Provides shared fixtures, builders, and helpers for TRS unit tests.

use finstack_quant_cashflows::builder::ScheduleParams;
use finstack_quant_core::{
    currency::Currency,
    dates::{Date, DayCount},
    market_data::{
        context::MarketContext,
        scalars::{MarketScalar, ScalarTimeSeries},
        term_structures::DiscountCurve,
        term_structures::ForwardCurve,
    },
    math::interp::InterpStyle,
    money::Money,
    types::PriceId,
};
pub use finstack_quant_test_utils::assert::approx_eq;
use finstack_quant_valuations::instruments::EquityUnderlyingParams;
use finstack_quant_valuations::instruments::FinancingLegSpec;
use finstack_quant_valuations::instruments::IndexUnderlyingParams;
use finstack_quant_valuations::instruments::equity::equity_trs::EquityTotalReturnSwap;
use finstack_quant_valuations::instruments::fixed_income::fi_trs::FiIndexTotalReturnSwap;
use finstack_quant_valuations::instruments::{PayReceive, TrsScheduleSpec};
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
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();
    context = context.try_insert(disc_curve).expect("insert market curve");

    // USD forward curve (SOFR 3M)
    let fwd_curve = ForwardCurve::builder("USD-SOFR-3M")
        .base_date(as_of_date())
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (0.125, 0.997471950911942),
            (0.25, 0.994888110405994),
            (0.375, 0.992248945555301),
            (0.5, 0.989554932567899),
            (0.625, 0.98682197561651),
            (0.75, 0.984065814019865),
            (0.875, 0.981286684604636),
            (1.0, 0.978484825682528),
            (1.125, 0.975668099393593),
            (1.25, 0.972844280694656),
            (1.375, 0.970013478228807),
            (1.5, 0.967175800650738),
            (1.625, 0.964331356619928),
            (1.75, 0.961480254793845),
            (1.875, 0.958622603821178),
            (2.0, 0.955758512335073),
            (2.1875, 0.951461686598083),
            (2.375, 0.947173078450782),
            (2.5625, 0.942892751140432),
            (2.75, 0.938620767338384),
            (2.9375, 0.934357189139748),
            (3.125, 0.9301020780631),
            (3.3125, 0.925855495050225),
            (3.5, 0.921617500465892),
            (3.6875, 0.917388154097666),
            (3.875, 0.913167515155755),
            (4.0625, 0.908955642272887),
            (4.25, 0.904752593504231),
            (4.4375, 0.900558426327339),
            (4.625, 0.896373197642133),
            (4.8125, 0.892196963770921),
            (5.0, 0.888029780458446),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();
    context = context.try_insert(fwd_curve).expect("insert market curve");
    context = context.insert_series(
        ScalarTimeSeries::new(
            "FIXING:USD-SOFR-3M",
            vec![(d(2025, 1, 1), 0.02), (d(2025, 1, 2), 0.02)],
            None,
            finstack_quant_core::market_data::scalars::SeriesInterpolation::Step,
            finstack_quant_core::market_data::scalars::SeriesExtrapolation::Error,
        )
        .expect("valid SOFR fixing series"),
    );

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
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();
    context = context.try_insert(eur_disc).expect("insert market curve");

    let eur_fwd = ForwardCurve::builder("EUR-EURIBOR-3M")
        .base_date(as_of_date())
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (0.125, 0.998095565740596),
            (0.25, 0.996132498124296),
            (0.375, 0.994111157606445),
            (0.5, 0.992031914837061),
            (0.625, 0.989910617780149),
            (0.75, 0.987762988692325),
            (0.875, 0.985589218780527),
            (1.0, 0.983389501281869),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();
    context = context.try_insert(eur_fwd).expect("insert market curve");

    // Equity market data
    context = context.insert_price(
        "SPX-SPOT",
        MarketScalar::unitless(5000.0).expect("valid scalar"),
    );
    context = context.insert_price(
        "SPX-DIV-YIELD",
        MarketScalar::unitless(0.015).expect("valid scalar"),
    ); // 1.5%
    context = context.insert_price(
        "NDX-SPOT",
        MarketScalar::unitless(18000.0).expect("valid scalar"),
    );
    context = context.insert_price(
        "NDX-DIV-YIELD",
        MarketScalar::unitless(0.005).expect("valid scalar"),
    ); // 0.5%

    // Fixed income index market data
    context = context.insert_price(
        "HY-INDEX-YIELD",
        MarketScalar::unitless(0.055).expect("valid scalar"),
    ); // 5.5%
    context = context.insert_price(
        "HY-INDEX-DURATION",
        MarketScalar::unitless(4.5).expect("valid scalar"),
    ); // 4.5 years
    context = context.insert_price(
        "IG-INDEX-YIELD",
        MarketScalar::unitless(0.035).expect("valid scalar"),
    ); // 3.5%
    context = context.insert_price(
        "IG-INDEX-DURATION",
        MarketScalar::unitless(7.0).expect("valid scalar"),
    ); // 7 years

    context
}

/// Builder for test equity TRS with sensible defaults.
pub struct TestEquityTrsBuilder {
    id: String,
    notional: Money,
    spot_id: String,
    div_yield_id: Option<PriceId>,
    discount_curve_id: String,
    forward_curve_id: String,
    spread_bp: f64,
    start: Date,
    end: Date,
    side: PayReceive,
    initial_level: Option<f64>,
}

impl Default for TestEquityTrsBuilder {
    fn default() -> Self {
        Self {
            id: "TEST-EQ-TRS-001".into(),
            notional: Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"),
            spot_id: "SPX-SPOT".into(),
            div_yield_id: Some(finstack_quant_core::types::PriceId::new("SPX-DIV-YIELD")),
            discount_curve_id: "USD-OIS".into(),
            forward_curve_id: "USD-SOFR-3M".into(),
            spread_bp: 25.0,
            // Keep `start` strictly after `as_of` so theta (which rolls the
            // valuation date forward) doesn't cross the start date.
            start: as_of_date() + time::Duration::days(2),
            end: d(2026, 1, 2),
            side: PayReceive::Receive,
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

    pub fn div_yield_id(mut self, id: Option<PriceId>) -> Self {
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

    pub fn side(mut self, side: PayReceive) -> Self {
        self.side = side;
        self
    }

    pub fn initial_level(mut self, level: f64) -> Self {
        self.initial_level = Some(level);
        self
    }

    pub fn build(self) -> EquityTotalReturnSwap {
        let mut underlying =
            EquityUnderlyingParams::new("TEST-EQ", self.spot_id, self.notional.currency());
        if let Some(div_id) = self.div_yield_id {
            underlying = underlying.with_div_yield_id(div_id);
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
            .financing_leg(financing)
            .schedule(schedule)
            .side(self.side)
            .dividend_settlement(
                finstack_quant_valuations::instruments::equity::equity_trs::TrsDividendSettlement::OnDividendDate,
            );

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
    discount_curve_id: String,
    forward_curve_id: String,
    spread_bp: f64,
    start: Date,
    end: Date,
    side: PayReceive,
    initial_level: Option<f64>,
}

impl Default for TestFIIndexTrsBuilder {
    fn default() -> Self {
        Self {
            id: "TEST-FI-TRS-001".into(),
            notional: Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"),
            index_id: "HY-INDEX".into(),
            yield_id: Some("HY-INDEX-YIELD".into()),
            duration_id: Some("HY-INDEX-DURATION".into()),
            discount_curve_id: "USD-OIS".into(),
            forward_curve_id: "USD-SOFR-3M".into(),
            spread_bp: 100.0,
            // Keep `start` strictly after `as_of` so theta (which rolls the
            // valuation date forward) doesn't cross the start date.
            start: as_of_date() + time::Duration::days(2),
            end: d(2026, 1, 2),
            side: PayReceive::Receive,
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

    pub fn side(mut self, side: PayReceive) -> Self {
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

    pub fn build(self) -> FiIndexTotalReturnSwap {
        let mut underlying =
            IndexUnderlyingParams::new(self.index_id.clone(), self.notional.currency());

        if let Some(y_id) = self.yield_id {
            underlying = underlying.with_yield_id(y_id);
        }
        if let Some(d_id) = self.duration_id {
            underlying = underlying.with_duration_id(d_id);
        }

        let financing = FinancingLegSpec::new(
            self.discount_curve_id,
            self.forward_curve_id,
            Decimal::try_from(self.spread_bp).expect("valid spread_bp"),
            DayCount::Act360,
        );

        let schedule =
            TrsScheduleSpec::from_params(self.start, self.end, ScheduleParams::quarterly_act360());

        let mut builder = FiIndexTotalReturnSwap::builder()
            .id(self.id.into())
            .notional(self.notional)
            .underlying(underlying)
            .financing_leg(financing)
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

/// Helper to assert two Money values are approximately equal.
pub fn assert_money_approx_eq(actual: Money, expected: Money, tolerance: f64, msg: &str) {
    assert_eq!(
        actual.currency(),
        expected.currency(),
        "{}: currency mismatch",
        msg
    );
    approx_eq(actual.amount(), expected.amount(), tolerance, msg);
}
