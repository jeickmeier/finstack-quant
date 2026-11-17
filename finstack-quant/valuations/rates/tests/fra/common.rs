//! Shared test utilities for FRA tests.
//!
//! Provides common fixtures, builders, and assertion helpers to reduce
//! duplication across the test suite and ensure consistent test setup.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DateExt, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
use finstack_quant_core::money::Money;
pub use finstack_quant_test_utils::assert::{approx_eq, in_range};
use finstack_quant_valuations_core::instruments::PayReceive;
use finstack_quant_valuations_rates::fra::ForwardRateAgreement;
use rust_decimal::Decimal;
use time::macros::date;

/// Standard test base date
pub const BASE_DATE: Date = date!(2024 - 01 - 01);

/// Standard FRA periods (3M x 6M means 3M forward, 3M tenor)
pub fn standard_fra_dates() -> (Date, Date, Date) {
    let fixing = date!(2024 - 04 - 01); // 3M forward
    let start = date!(2024 - 04 - 01);
    let end = date!(2024 - 07 - 01); // 3M tenor
    (fixing, start, end)
}

/// Creates a flat forward curve at the given rate
pub fn build_flat_forward_curve(rate: f64, base_date: Date, curve_id: &str) -> ForwardCurve {
    let mut points = vec![(base_date, 1.0)];
    let mut prev = base_date;
    let mut factor = 1.0;
    for quarter in 1..=40 {
        let next = base_date.add_months(quarter * 3).expect("valid date shift");
        let yf = DayCount::Act360
            .year_fraction(prev, next, DayCountContext::default())
            .expect("valid quarterly accrual");
        factor /= 1.0 + rate * yf;
        points.push((next, factor));
        prev = next;
    }
    ForwardCurve::from_dates(
        curve_id,
        base_date,
        &points,
        DayCount::Act360,
        InterpStyle::LogLinear,
        ExtrapolationPolicy::Extend,
    )
    .unwrap()
}

/// Creates a flat discount curve at the given rate
pub fn build_flat_discount_curve(rate: f64, base_date: Date, curve_id: &str) -> DiscountCurve {
    // DF = exp(-rate * time)
    let builder = DiscountCurve::builder(curve_id)
        .base_date(base_date)
        .day_count(DayCount::Act360)
        .knots([
            (0.0, 1.0),
            (1.0, (-rate).exp()),
            (5.0, (-rate * 5.0).exp()),
            (10.0, (-rate * 10.0).exp()),
        ])
        .interp(InterpStyle::Linear)
        // Long-tenor risk buckets must see positive DFs beyond the last pillar.
        .extrapolation(ExtrapolationPolicy::Flat);
    builder.build().unwrap()
}

/// Creates an upward sloping forward curve (normal term structure)
#[allow(dead_code)]
pub fn build_upward_forward_curve(base_date: Date, curve_id: &str) -> ForwardCurve {
    ForwardCurve::builder(curve_id)
        .base_date(base_date)
        .day_count(DayCount::Act360)
        .knots([
            (0.0, 1.0),
            (0.125, 0.996218106939305),
            (0.25, 0.992372984425763),
            (0.375, 0.98846547612484),
            (0.5, 0.984496437005408),
            (0.625, 0.980466733028608),
            (0.75, 0.976377240833651),
            (0.875, 0.972228847420751),
            (1.0, 0.968022449831306),
            (1.125, 0.963758954825533),
            (1.25, 0.959439278557683),
            (1.375, 0.955064346249016),
            (1.5, 0.950635091858698),
            (1.625, 0.946152457752782),
            (1.75, 0.941617394371427),
            (1.875, 0.937030859894532),
            (2.0, 0.932393819905948),
            (2.1875, 0.925399912673928),
            (2.375, 0.918404652549806),
            (2.5625, 0.911408866395109),
            (2.75, 0.904413374889132),
            (2.9375, 0.897418992384432),
            (3.125, 0.890426526765256),
            (3.3125, 0.883436779308968),
            (3.5, 0.876450544550457),
            (3.6875, 0.869468610149596),
            (3.875, 0.862491756761729),
            (4.0625, 0.855520757911249),
            (4.25, 0.848556379868256),
            (4.4375, 0.841599381528326),
            (4.625, 0.834650514295413),
            (4.8125, 0.827710521967885),
            (5.0, 0.820780140627725),
            (5.3125, 0.809279180503689),
            (5.625, 0.797861454516039),
            (5.9375, 0.786528002540189),
            (6.25, 0.775279825217673),
            (6.5625, 0.764117884125373),
            (6.875, 0.753043101961267),
            (7.1875, 0.742056362746364),
            (7.5, 0.731158512042463),
            (7.8125, 0.720350357185388),
            (8.125, 0.709632667533337),
            (8.4375, 0.699006174729978),
            (8.75, 0.688471572981913),
            (9.0625, 0.678029519350152),
            (9.375, 0.667680634055198),
            (9.6875, 0.657425500795382),
            (10.0, 0.647264667078035),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap()
}

/// Creates an inverted forward curve (stressed scenario)
#[allow(dead_code)]
pub fn build_inverted_forward_curve(base_date: Date, curve_id: &str) -> ForwardCurve {
    ForwardCurve::builder(curve_id)
        .base_date(base_date)
        .day_count(DayCount::Act360)
        .knots([
            (0.0, 1.0),
            (0.125, 0.992605599102455),
            (0.25, 0.985419835190368),
            (0.375, 0.978438960281303),
            (0.5, 0.971659348942016),
            (0.625, 0.965077495162132),
            (0.75, 0.958690009342707),
            (0.875, 0.952493615396068),
            (1.0, 0.946485147953484),
            (1.125, 0.940661549677314),
            (1.25, 0.935019868674477),
            (1.375, 0.929557256008159),
            (1.5, 0.924270963304852),
            (1.625, 0.919158340453891),
            (1.75, 0.91421683339682),
            (1.875, 0.909443982003995),
            (2.0, 0.90483741803596),
            (2.1875, 0.89812914566372),
            (2.375, 0.891575082330825),
            (2.5625, 0.885172572109236),
            (2.75, 0.878919031389447),
            (2.9375, 0.872811947154315),
            (3.125, 0.866848875306405),
            (3.3125, 0.861027439047309),
            (3.5, 0.855345327307423),
            (3.6875, 0.849800293224752),
            (3.875, 0.844390152671351),
            (4.0625, 0.83911278282603),
            (4.25, 0.833966120792056),
            (4.4375, 0.828948162258568),
            (4.625, 0.824056960204514),
            (4.8125, 0.819290623643926),
            (5.0, 0.814647316411415),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap()
}

/// Standard market context with flat 5% curves
pub fn standard_market() -> MarketContext {
    let disc = build_flat_discount_curve(0.05, BASE_DATE, "USD_OIS");
    let fwd = build_flat_forward_curve(0.05, BASE_DATE, "USD_LIBOR_3M");
    MarketContext::new().insert(disc).insert(fwd)
}

/// Creates a standard 3M x 6M FRA for testing
pub fn create_standard_fra() -> ForwardRateAgreement {
    let (fixing, start, end) = standard_fra_dates();
    ForwardRateAgreement {
        id: "FRA_TEST".into(),
        notional: Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        fixing_date: Some(fixing),
        start_date: start,
        maturity: end,
        fixed_rate: Decimal::try_from(0.05).expect("valid decimal"), // At-market: 5% = forward rate
        day_count: DayCount::Act360,
        reset_lag_days: 2,
        fixing_calendar_id: None,
        fixing_business_day_convention: None,
        observed_fixing: None,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
        side: PayReceive::Receive, // receive fixed rate, pay floating
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    }
}

/// Fluent builder for test FRAs with sensible defaults
#[derive(Clone)]
pub struct TestFraBuilder {
    id: String,
    notional: Money,
    fixing_date: Option<Date>,
    start_date: Date,
    maturity: Date,
    fixed_rate: f64,
    day_count: DayCount,
    reset_lag_days: i32,
    discount_curve_id: String,
    forward_curve_id: String,
    side: PayReceive,
}

impl Default for TestFraBuilder {
    fn default() -> Self {
        let (fixing, start, end) = standard_fra_dates();
        Self {
            id: "FRA_TEST".to_string(),
            notional: Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
            fixing_date: Some(fixing),
            start_date: start,
            maturity: end,
            fixed_rate: 0.05, // At-market: 5% = forward rate
            day_count: DayCount::Act360,
            reset_lag_days: 2,
            discount_curve_id: "USD_OIS".to_string(),
            forward_curve_id: "USD_LIBOR_3M".to_string(),
            side: PayReceive::Receive, // receive fixed rate
        }
    }
}

impl TestFraBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn id(mut self, id: &str) -> Self {
        self.id = id.to_string();
        self
    }

    pub fn notional(mut self, amount: f64, currency: Currency) -> Self {
        self.notional = Money::new(amount, currency).expect("valid money fixture");
        self
    }

    pub fn dates(mut self, fixing: Date, start: Date, end: Date) -> Self {
        self.fixing_date = Some(fixing);
        self.start_date = start;
        self.maturity = end;
        self
    }

    pub fn fixed_rate(mut self, rate: f64) -> Self {
        self.fixed_rate = rate;
        self
    }

    pub fn day_count(mut self, day_count: DayCount) -> Self {
        self.day_count = day_count;
        self
    }

    pub fn curves(mut self, disc: &str, fwd: &str) -> Self {
        self.discount_curve_id = disc.to_string();
        self.forward_curve_id = fwd.to_string();
        self
    }

    /// Set the FRA direction: true = receive fixed rate, false = pay fixed rate.
    pub fn receive_fixed(mut self, receive: bool) -> Self {
        self.side = if receive {
            PayReceive::Receive
        } else {
            PayReceive::Pay
        };
        self
    }

    pub fn build(self) -> ForwardRateAgreement {
        ForwardRateAgreement {
            id: self.id.into(),
            notional: self.notional,
            fixing_date: self.fixing_date,
            start_date: self.start_date,
            maturity: self.maturity,
            fixed_rate: Decimal::try_from(self.fixed_rate).expect("valid decimal"),
            day_count: self.day_count,
            reset_lag_days: self.reset_lag_days,
            fixing_calendar_id: None,
            fixing_business_day_convention: None,
            observed_fixing: None,
            discount_curve_id: self.discount_curve_id.into(),
            forward_curve_id: self.forward_curve_id.into(),
            side: self.side,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Default::default(),
        }
    }
}

/// Assert value is negative
pub fn assert_negative(value: f64, msg: &str) {
    assert!(
        value < 0.0,
        "{}: value should be negative, got {}",
        msg,
        value
    );
}
