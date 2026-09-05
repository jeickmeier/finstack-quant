//! Integration harness for the carry/credit audit fixes owned by this
//! session. Kept separate from `tests/attribution.rs` (owned by a different
//! test harness).
//!
//! Covers the spec-level `rate_bump_bp` wiring: `AttributionConfig`'s
//! `rate_bump_bp` is written into the `valuations.sensitivities.v1` config
//! extension by `build_finstack_config`, and `execute()` must pass that
//! config into `price_with_metrics` (via `PricingOptions::with_config`) so
//! the sensitivity calculators actually see it. Before the fix the
//! metrics-based path priced with `PricingOptions::default()` (config =
//! None) and the knob was inert.

use finstack_quant_attribution::{AttributionConfig, AttributionMethod, AttributionSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{create_date, Date};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::{Bond, InstrumentJson};
use time::Month;

/// Flat continuously-compounded discount curve at rate `r`.
fn flat_discount(base: Date, r: f64) -> DiscountCurve {
    DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([(0.0, 1.0), (1.0, (-r).exp()), (30.0, (-r * 30.0).exp())])
        .interp(InterpStyle::LogLinear)
        .build()
        .expect("discount curve")
}

fn spec_with_config(config: Option<AttributionConfig>) -> AttributionSpec {
    let t0 = create_date(2025, Month::January, 1).expect("t0");
    let t1 = create_date(2025, Month::January, 2).expect("t1");
    let bond = Bond::fixed(
        "RATE-BUMP-BOND",
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        create_date(2024, Month::January, 1).expect("issue"),
        create_date(2034, Month::January, 1).expect("maturity"),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond construction");

    let market_t0 = MarketContext::new().insert(flat_discount(t0, 0.03));
    // +10bp parallel move so the rates leg is materially nonzero.
    let market_t1 = MarketContext::new().insert(flat_discount(t1, 0.031));

    AttributionSpec {
        instrument: InstrumentJson::Bond(bond),
        market_t0: (&market_t0).into(),
        market_t1: (&market_t1).into(),
        as_of_t0: t0,
        as_of_t1: t1,
        method: AttributionMethod::MetricsBased,
        model_params_t0: None,
        config,
        credit_factor_model: None,
        credit_factor_detail_options: Default::default(),
        full_cross_attribution: false,
    }
}

/// Audit (spec): `rate_bump_bp` must reach the pricer. Observable: DV01 for
/// an instrument with convexity depends on the finite-difference bump size
/// (the O(b²) higher-order terms do not cancel), so the rates leg of a
/// metrics-based attribution computed with a 200bp bump must differ from the
/// default 1bp bump. With the knob inert both runs are bit-identical.
#[test]
fn rate_bump_bp_config_reaches_metrics_based_pricing() {
    let default_run = spec_with_config(None).execute().expect("default run");

    let config = AttributionConfig {
        tolerance_abs: None,
        tolerance_pct: None,
        metrics: None,
        strict_validation: None,
        rounding_scale: None,
        rate_bump_bp: Some(200.0),
        target_currency: None,
        execution_policy: None,
    };
    let bumped_run = spec_with_config(Some(config))
        .execute()
        .expect("bumped run");

    let default_rates = default_run.attribution.rates_curves_pnl.amount();
    let bumped_rates = bumped_run.attribution.rates_curves_pnl.amount();

    assert!(
        default_rates.abs() > 1.0,
        "fixture must produce a material rates P&L, got {default_rates}"
    );
    assert!(
        (default_rates - bumped_rates).abs() > 1e-6,
        "rate_bump_bp override must change the FD bump the pricer uses: \
         rates P&L identical ({default_rates} vs {bumped_rates}) — the \
         config knob is inert"
    );
}
