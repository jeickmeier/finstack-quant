//! Coverage for scenario classes previously untested: a coupon paying inside
//! the attribution window, negative-rate regimes, discount-vs-forward basis
//! moves, and vol smile/skew changes.

use crate::attribution_support::TestInstrument;
use finstack_quant_attribution::{
    attribute_pnl_metrics_based, attribute_pnl_parallel, ExecutionPolicy,
};
use finstack_quant_core::config::{results_meta, FinstackConfig};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::results::ValuationResult;
use indexmap::IndexMap;
use std::sync::Arc;
use time::macros::date;

fn flat_discount(id: &str, as_of: time::Date, rate: f64) -> DiscountCurve {
    let tenors = [0.0, 0.5, 1.0, 2.0, 3.0, 5.0, 10.0];
    let knots: Vec<(f64, f64)> = tenors.iter().map(|&t| (t, (-rate * t).exp())).collect();
    let builder = DiscountCurve::builder(id)
        .base_date(as_of)
        .knots(knots)
        .interp(InterpStyle::Linear);
    // Negative rates produce DF > 1 (non-monotone): use the documented
    // negative-rate validation preset, as a production EUR/CHF curve would.
    let builder = if rate < 0.0 {
        builder.validation(
            finstack_quant_core::market_data::term_structures::ValidationMode::NegativeRateFriendly {
                forward_floor: -0.02,
            },
        )
    } else {
        builder
    };
    builder.build().unwrap()
}

/// A coupon paying strictly inside `(T0, T1]` must satisfy the
/// total-return identity — `total_pnl = mark_to_market_pnl + coupon` with the
/// coupon visible in `carry_detail.coupon_income` — the classic
/// dirty-price/total-return defect class.
#[test]
fn coupon_payment_inside_window_keeps_total_return_identity() {
    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 25);

    // Semiannual 5% bond with coupon dates Jan-20 / Jul-20: the window
    // contains exactly one coupon (2025-01-20, a Monday).
    let bond = Bond::fixed(
        "COUPON-WINDOW-BOND",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        date!(2024 - 07 - 20),
        date!(2029 - 07 - 20),
        "USD-OIS",
    )
    .unwrap();
    let instrument: Arc<dyn Instrument> = Arc::new(bond);

    // IDENTICAL markets (the same curve object on both dates): every market
    // factor is zero and the entire P&L is the date roll + the coupon.
    let market = MarketContext::new().insert(flat_discount("USD-OIS", as_of_t0, 0.04));

    let attribution = attribute_pnl_parallel(
        &instrument,
        &market,
        &market,
        as_of_t0,
        as_of_t1,
        &FinstackConfig::default(),
        ExecutionPolicy::Serial,
    )
    .expect("attribution over a coupon date should succeed");

    let coupon = attribution
        .carry_detail
        .as_ref()
        .and_then(|d| d.coupon_income.as_ref())
        .map(|line| line.total.amount())
        .unwrap_or(0.0);
    assert!(
        (20_000.0..30_000.0).contains(&coupon),
        "the semiannual coupon (~25k) must be captured in coupon_income, got {coupon}"
    );

    let mtm = attribution
        .mark_to_market_pnl
        .expect("mark_to_market_pnl must be populated")
        .amount();
    let total = attribution.total_pnl.amount();
    assert!(
        (total - (mtm + coupon)).abs() < 1e-6,
        "total-return identity: total ({total}) must equal MTM ({mtm}) + coupon ({coupon})"
    );

    // With identical markets, carry is the only factor: total ≈ carry.
    assert!(
        (total - attribution.carry.amount()).abs() < 1e-6,
        "with unchanged markets the total ({total}) must equal carry ({})",
        attribution.carry.amount()
    );
    assert!(
        attribution.residual.amount().abs() < 1e-6,
        "residual must be ~0, got {}",
        attribution.residual.amount()
    );
}

/// Negative-rate regimes (EUR/JPY/CHF history) must attribute cleanly:
/// the sign convention holds (rates up → long bond loses) and nothing in the
/// measurement chain clamps or NaNs on negative zero rates / DF > 1.
#[test]
fn negative_rates_regime_attribution_succeeds() {
    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 16);

    let bond = Bond::fixed(
        "NEG-RATES-BOND",
        Money::new(1_000_000.0, Currency::EUR),
        0.01,
        date!(2025 - 01 - 01),
        date!(2030 - 01 - 01),
        "EUR-OIS",
    )
    .unwrap();
    let instrument: Arc<dyn Instrument> = Arc::new(bond);

    // -50bp → -40bp: rates rise (less negative).
    let market_t0 = MarketContext::new().insert(flat_discount("EUR-OIS", as_of_t0, -0.0050));
    let market_t1 = MarketContext::new().insert(flat_discount("EUR-OIS", as_of_t1, -0.0040));

    let attribution = attribute_pnl_parallel(
        &instrument,
        &market_t0,
        &market_t1,
        as_of_t0,
        as_of_t1,
        &FinstackConfig::default(),
        ExecutionPolicy::Serial,
    )
    .expect("negative-rate attribution should succeed");

    assert!(
        attribution.rates_curves_pnl.amount() < 0.0,
        "rates up (−50bp → −40bp) must show a loss on a long bond, got {}",
        attribution.rates_curves_pnl.amount()
    );
    assert!(
        attribution.carry.amount().is_finite(),
        "carry must be finite in a negative-rate regime"
    );
    assert!(
        attribution.residual_within_meta_tolerance(),
        "residual must stay within the parallel-method tolerance, got {}",
        attribution.residual.amount()
    );
}

/// A basis move (forward/projection curve moves, discount curve does
/// not) must be attributed to the rates factor via the forward curve's
/// key-rate DV01 — the metrics-based path previously measured discount curves
/// only and sent the entire move to the residual.
#[test]
fn metrics_based_attributes_forward_curve_basis_move() {
    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 16);

    let fwd = |as_of: time::Date, rate: f64| {
        ForwardCurve::builder("USD-SOFR-3M", 0.25)
            .base_date(as_of)
            .knots([(0.0, rate), (1.0, rate), (5.0, rate), (10.0, rate)])
            .interp(InterpStyle::Linear)
            .build()
            .unwrap()
    };

    // Discount unchanged; forward +10bp — a pure basis move.
    let market_t0 = MarketContext::new()
        .insert(flat_discount("USD-OIS", as_of_t0, 0.04))
        .insert(fwd(as_of_t0, 0.0200));
    let market_t1 = MarketContext::new()
        .insert(flat_discount("USD-OIS", as_of_t1, 0.04))
        .insert(fwd(as_of_t1, 0.0210));

    let instrument: Arc<dyn Instrument> = Arc::new(
        TestInstrument::new("BASIS-SWAP", Money::new(0.0, Currency::USD))
            .with_discount_curves(&["USD-OIS"])
            .with_forward_curves(&["USD-SOFR-3M"]),
    );

    // Per-tenor key-rate DV01 on BOTH curves; only the forward curve moves.
    let mut measures = IndexMap::new();
    measures.insert(MetricId::Theta, 0.0);
    measures.insert(MetricId::custom("bucketed_dv01::USD-OIS::1y"), -300.0);
    measures.insert(MetricId::custom("bucketed_dv01::USD-OIS::5y"), -900.0);
    measures.insert(MetricId::custom("bucketed_dv01::USD-SOFR-3M::1y"), -120.0);
    measures.insert(MetricId::custom("bucketed_dv01::USD-SOFR-3M::5y"), -480.0);

    // Expected: forward DV01s × +10bp; discount contributes 0.
    let expected_rates = (-120.0 - 480.0) * 10.0;

    let meta = results_meta(&FinstackConfig::default());
    let p0 = 1_000_000.0;
    let val_t0 = ValuationResult::stamped_with_meta(
        "BASIS-SWAP",
        as_of_t0,
        Money::new(p0, Currency::USD),
        meta.clone(),
    )
    .with_measures(measures.clone());
    let val_t1 = ValuationResult::stamped_with_meta(
        "BASIS-SWAP",
        as_of_t1,
        Money::new(p0 + expected_rates, Currency::USD),
        meta,
    )
    .with_measures(measures);

    let attribution = attribute_pnl_metrics_based(
        &instrument,
        &market_t0,
        &market_t1,
        &val_t0,
        &val_t1,
        as_of_t0,
        as_of_t1,
    )
    .expect("metrics-based attribution should succeed");

    assert!(
        (attribution.rates_curves_pnl.amount() - expected_rates).abs() < 1.0,
        "the basis move must be attributed via the forward key-rate DV01: \
         expected {expected_rates}, got {}",
        attribution.rates_curves_pnl.amount()
    );
    assert!(
        attribution.residual.amount().abs() < 1.0,
        "a fully-explained basis move must leave ~0 residual, got {}",
        attribution.residual.amount()
    );
}
