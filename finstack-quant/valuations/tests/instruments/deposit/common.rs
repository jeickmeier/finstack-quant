//! Common test fixtures and utilities for deposit tests.
//!
//! Provides reusable components to minimize duplication and ensure consistency
//! across the test suite following DRY principles.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::rates::deposit::Deposit;
pub use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::{MetricContext, MetricId, MetricRegistry};
use rust_decimal::Decimal;
use std::sync::Arc;

/// Tolerance for floating point comparisons in financial calculations.
pub const PRICE_TOLERANCE: f64 = 1e-10;
pub const RATE_TOLERANCE: f64 = 1e-12;
pub const DF_TOLERANCE: f64 = 1e-12;

// Re-export the canonical date helper from shared test utilities.
pub use crate::finstack_quant_test_utils::date;

/// Creates a market context with a flat discount curve.
///
/// # Arguments
/// * `base` - Base date for the curve
/// * `id` - Curve identifier
/// * `rate` - Annual continuously compounded rate (e.g., 0.02 for 2%)
///
/// Uses continuously compounded discounting: DF(t) = exp(-rate * t)
/// This produces smooth interpolation at any tenor.
/// For deposits priced at their quoted rate, the PV will be approximately zero
/// (small differences due to simple vs continuous compounding conventions).
pub fn ctx_with_flat_rate(base: Date, id: &str, rate: f64) -> MarketContext {
    let disc = DiscountCurve::builder(id)
        .base_date(base)
        .knots([
            (0.0, 1.0),
            (0.25, (-rate * 0.25).exp()),
            (0.5, (-rate * 0.5).exp()),
            (1.0, (-rate).exp()),
            (2.0, (-rate * 2.0).exp()),
            (5.0, (-rate * 5.0).exp()),
        ])
        .build()
        .unwrap();
    MarketContext::new().insert(disc)
}

/// Creates a market context with a standard discount curve for testing.
///
/// Uses a realistic discount curve with decreasing discount factors:
/// - T=0: DF=1.0
/// - T=1: DF=0.98 (≈2% rate)
/// - T=2: DF=0.96 (≈2% rate)
pub fn ctx_with_standard_disc(base: Date, id: &str) -> MarketContext {
    let disc = DiscountCurve::builder(id)
        .base_date(base)
        .knots([(0.0, 1.0), (1.0, 0.98), (2.0, 0.96)])
        .build()
        .unwrap();
    MarketContext::new().insert(disc)
}

/// Creates a market context with a steep discount curve for sensitivity testing.
pub fn ctx_with_steep_curve(base: Date, id: &str) -> MarketContext {
    let disc = DiscountCurve::builder(id)
        .base_date(base)
        .knots([(0.0, 1.0), (0.5, 0.95), (1.0, 0.90), (2.0, 0.80)])
        .build()
        .unwrap();
    MarketContext::new().insert(disc)
}

/// Creates a standard 6-month USD deposit for testing.
///
/// - Notional: 1,000,000 USD
/// - Period: 6 months from base date
/// - Day count: Act/360
/// - Curve: USD-OIS
pub fn standard_deposit(base: Date) -> Deposit {
    Deposit::builder()
        .id(InstrumentId::new("DEP-STD"))
        .notional(Money::new(1_000_000.0, Currency::USD))
        .start_date(base)
        .maturity(date(
            base.year(),
            (base.month() as u8 + 6).min(12),
            base.day(),
        ))
        .day_count(DayCount::Act360)
        .quote_rate_opt(Some(Decimal::try_from(0.0).expect("valid decimal")))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .build()
        .unwrap()
}

/// Creates a deposit with custom parameters.
pub struct DepositBuilder {
    id: String,
    notional: Money,
    start_date: Date,
    maturity: Date,
    day_count: DayCount,
    quote_rate: Option<Decimal>,
    discount_curve_id: String,
}

impl DepositBuilder {
    pub fn new(base: Date) -> Self {
        Self {
            id: "DEP-TEST".to_string(),
            notional: Money::new(1_000_000.0, Currency::USD),
            start_date: base,
            maturity: date(base.year(), (base.month() as u8 + 6).min(12), base.day()),
            day_count: DayCount::Act360,
            quote_rate: Some(Decimal::try_from(0.0).expect("valid decimal")),
            discount_curve_id: "USD-OIS".to_string(),
        }
    }

    pub fn id(mut self, id: &str) -> Self {
        self.id = id.to_string();
        self
    }

    pub fn notional(mut self, notional: Money) -> Self {
        self.notional = notional;
        self
    }

    pub fn start_date(mut self, start_date: Date) -> Self {
        self.start_date = start_date;
        self
    }

    pub fn maturity(mut self, maturity: Date) -> Self {
        self.maturity = maturity;
        self
    }

    pub fn day_count(mut self, day_count: DayCount) -> Self {
        self.day_count = day_count;
        self
    }

    pub fn quote_rate(mut self, rate: f64) -> Self {
        self.quote_rate = Some(Decimal::try_from(rate).expect("valid decimal"));
        self
    }

    pub fn discount_curve_id(mut self, id: &str) -> Self {
        self.discount_curve_id = id.to_string();
        self
    }

    pub fn build(self) -> Deposit {
        let mut dep = Deposit::builder()
            .id(InstrumentId::new(&self.id))
            .notional(self.notional)
            .start_date(self.start_date)
            .maturity(self.maturity)
            .day_count(self.day_count)
            .discount_curve_id(CurveId::new(&self.discount_curve_id))
            .build()
            .unwrap();
        dep.quote_rate = self.quote_rate;
        dep
    }
}

/// Sets up a metric context for testing metric calculators.
///
/// # Returns
/// Tuple of (deposit, market context, metric context, registry)
#[allow(dead_code)]
pub fn setup_metric_context(base: Date) -> (Deposit, MarketContext, MetricContext, MetricRegistry) {
    let ctx = ctx_with_standard_disc(base, "USD-OIS");
    let dep = standard_deposit(base);
    let base_val = dep.value(&ctx, base).unwrap();

    let instrument_arc: Arc<dyn Instrument> = Arc::new(dep.clone());
    let metric_ctx = MetricContext::new(
        instrument_arc,
        Arc::new(ctx.clone()),
        base,
        base_val,
        MetricContext::default_config(),
    );

    let registry = finstack_quant_valuations::metrics::standard_registry().clone();

    (dep, ctx, metric_ctx, registry)
}

/// Computes a specific metric for a deposit.
pub fn compute_metric(
    deposit: &Deposit,
    ctx: &MarketContext,
    base: Date,
    metric_id: MetricId,
) -> f64 {
    let registry = finstack_quant_valuations::metrics::standard_registry();

    let base_val = deposit.value(ctx, base).unwrap();
    let instrument_arc: Arc<dyn Instrument> = Arc::new(deposit.clone());
    let mut metric_ctx = MetricContext::new(
        instrument_arc,
        Arc::new(ctx.clone()),
        base,
        base_val,
        MetricContext::default_config(),
    );

    let results = registry
        .compute(std::slice::from_ref(&metric_id), &mut metric_ctx)
        .unwrap();
    *results.get(&metric_id).unwrap()
}

/// Computes multiple metrics for a deposit.
pub fn compute_metrics(
    deposit: &Deposit,
    ctx: &MarketContext,
    base: Date,
    metric_ids: &[MetricId],
) -> finstack_quant_core::HashMap<MetricId, f64> {
    let registry = finstack_quant_valuations::metrics::standard_registry();

    let base_val = deposit.value(ctx, base).unwrap();
    let instrument_arc: Arc<dyn Instrument> = Arc::new(deposit.clone());
    let mut metric_ctx = MetricContext::new(
        instrument_arc,
        Arc::new(ctx.clone()),
        base,
        base_val,
        MetricContext::default_config(),
    );

    registry.compute(metric_ids, &mut metric_ctx).unwrap()
}
