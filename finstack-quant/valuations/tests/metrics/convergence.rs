//! Convergence tests comparing finite difference vs analytical greeks.
//!
//! For instruments with analytical greeks (e.g., EquityOption, FxOption), verifies that
//! finite difference implementations converge to analytical values. Also validates that
//! bucketed metrics sum to total metrics (DV01, CS01, Vega).
//!
//! Tests:
//! - Analytical vs FD greeks for EquityOption (all greeks)
//! - Analytical vs FD greeks for FxOption (delta, vega, rho)
//! - Bucketed DV01 sums to total DV01
//! - Bucketed CS01 sums to total CS01
//! - Bucketed Vega sums to total Vega (for instruments with vol surfaces)

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use finstack_quant_valuations::instruments::equity::equity_option::EquityOption;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::SettlementType;
use finstack_quant_valuations::instruments::{ExerciseStyle, OptionType};
use finstack_quant_valuations::metrics::{standard_registry, MetricContext, MetricId};
use std::sync::Arc;
use time::macros::date;

fn create_option_market(as_of: Date, spot: f64, vol: f64, rate: f64) -> MarketContext {
    let disc_curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([
            (0.0f64, 1.0f64),
            (1.0f64, (-rate).exp()),
            (2.0f64, (-rate * 2.0f64).exp()),
        ])
        .build()
        .unwrap();

    let vol_surface = VolSurface::builder("AAPL_VOL")
        .expiries(&[0.5, 1.0, 2.0])
        .strikes(&[80.0, 90.0, 100.0, 110.0, 120.0])
        .row(&[vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol])
        .build()
        .unwrap();

    MarketContext::new()
        .insert(disc_curve)
        .insert_surface(vol_surface)
        .insert_price("AAPL", MarketScalar::Price(Money::new(spot, Currency::USD)))
}

const SPOT_BUMP_PCT: f64 = 0.01;
const VOL_BUMP_ABS: f64 = 0.01; // 1 vol point

fn assert_close(label: &str, actual: f64, expected: f64, rel_tol: f64) {
    let denom = expected.abs().max(1.0);
    let rel_error = (actual - expected).abs() / denom;
    assert!(
        rel_error < rel_tol,
        "{} should match: actual={}, expected={}, rel_error={:.4}%",
        label,
        actual,
        expected,
        rel_error * 100.0
    );
}

fn equity_option_fd_delta(
    option: &EquityOption,
    as_of: Date,
    spot: f64,
    vol: f64,
    rate: f64,
) -> f64 {
    let market_up = create_option_market(as_of, spot * (1.0 + SPOT_BUMP_PCT), vol, rate);
    let market_dn = create_option_market(as_of, spot * (1.0 - SPOT_BUMP_PCT), vol, rate);
    let pv_up = option.value(&market_up, as_of).unwrap().amount();
    let pv_dn = option.value(&market_dn, as_of).unwrap().amount();
    let h = spot * SPOT_BUMP_PCT;
    (pv_up - pv_dn) / (2.0 * h)
}

fn equity_option_fd_gamma(
    option: &EquityOption,
    as_of: Date,
    spot: f64,
    vol: f64,
    rate: f64,
) -> f64 {
    let market_up = create_option_market(as_of, spot * (1.0 + SPOT_BUMP_PCT), vol, rate);
    let market_dn = create_option_market(as_of, spot * (1.0 - SPOT_BUMP_PCT), vol, rate);
    let market_0 = create_option_market(as_of, spot, vol, rate);
    let pv_up = option.value(&market_up, as_of).unwrap().amount();
    let pv_dn = option.value(&market_dn, as_of).unwrap().amount();
    let pv_0 = option.value(&market_0, as_of).unwrap().amount();
    let h = spot * SPOT_BUMP_PCT;
    (pv_up - 2.0 * pv_0 + pv_dn) / (h * h)
}

fn equity_option_fd_vega(
    option: &EquityOption,
    as_of: Date,
    spot: f64,
    vol: f64,
    rate: f64,
) -> f64 {
    let market_up = create_option_market(as_of, spot, vol + VOL_BUMP_ABS, rate);
    let market_0 = create_option_market(as_of, spot, vol, rate);
    let pv_up = option.value(&market_up, as_of).unwrap().amount();
    let pv_0 = option.value(&market_0, as_of).unwrap().amount();
    // Match the library's vega convention: PV change for a +1 vol point bump.
    pv_up - pv_0
}

/// Helper to test analytical vs registry greek for EquityOption
fn test_equity_option_greek(
    option: &EquityOption,
    market: &MarketContext,
    as_of: Date,
    metric_id: MetricId,
    analytical_fn: fn(&EquityOption, &MarketContext, Date) -> Result<f64>,
) {
    let registry = standard_registry();
    let pv = option.value(market, as_of).unwrap();

    // Compute analytical greek directly
    let mut analytical_value = analytical_fn(option, market, as_of).unwrap();
    // Registry exposes Rho per 1bp; direct equity option rho is per 1%.
    if metric_id == MetricId::Rho {
        analytical_value /= 100.0;
    }

    let mut context = MetricContext::new(
        Arc::new(option.clone()),
        Arc::new(market.clone()),
        as_of,
        pv,
        MetricContext::default_config(),
    );

    // Compute greek via registry (uses analytical formula for EquityOption)
    let results = registry
        .compute(std::slice::from_ref(&metric_id), &mut context)
        .unwrap();
    let registry_value = *results.get(&metric_id).unwrap();

    // Should match exactly (both use analytical formulas)
    let diff = (analytical_value - registry_value).abs();
    assert!(
        diff < 1e-10,
        "Analytical {:?} from registry ({}) should match direct call ({}), diff: {}",
        metric_id,
        registry_value,
        analytical_value,
        diff
    );
}

#[test]
fn test_equity_option_instantaneous_analytical_greeks() {
    // Instantaneous analytical Greeks match between direct calls and registry.
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);

    let option = EquityOption {
        id: "ANALYTICAL_GREEKS_TEST".into(),
        underlying_ticker: "AAPL".to_string(),
        strike: 100.0,
        option_type: OptionType::Call,
        exercise_style: ExerciseStyle::European,
        expiry,
        notional: Money::new(100.0, Currency::USD),
        day_count: DayCount::Act365F,
        theta_day_basis: Default::default(),
        settlement: SettlementType::Cash,
        exercise: None,
        discount_curve_id: "USD-OIS".into(),
        spot_id: "AAPL".into(),
        vol_surface_id: "AAPL_VOL".into(),
        div_yield_id: None,
        discrete_dividends: Vec::new(),
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        exercise_schedule: None,
        attributes: Default::default(),
    };

    let market = create_option_market(as_of, 100.0, 0.25, 0.05);

    // Test all analytical greeks
    test_equity_option_greek(&option, &market, as_of, MetricId::Delta, |opt, mkt, dt| {
        opt.delta(mkt, dt)
    });
    test_equity_option_greek(&option, &market, as_of, MetricId::Gamma, |opt, mkt, dt| {
        opt.gamma(mkt, dt)
    });
    test_equity_option_greek(&option, &market, as_of, MetricId::Vega, |opt, mkt, dt| {
        opt.vega(mkt, dt)
    });
    test_equity_option_greek(&option, &market, as_of, MetricId::Rho, |opt, mkt, dt| {
        opt.rho(mkt, dt)
    });
}

#[test]
fn test_equity_option_fd_matches_analytical_greeks() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let spot = 100.0;
    let vol = 0.25;
    let rate = 0.05;

    let option = EquityOption {
        id: "FD_GREEKS_TEST".into(),
        underlying_ticker: "AAPL".to_string(),
        strike: 100.0,
        option_type: OptionType::Call,
        exercise_style: ExerciseStyle::European,
        expiry,
        notional: Money::new(100.0, Currency::USD),
        day_count: DayCount::Act365F,
        theta_day_basis: Default::default(),
        settlement: SettlementType::Cash,
        exercise: None,
        discount_curve_id: "USD-OIS".into(),
        spot_id: "AAPL".into(),
        vol_surface_id: "AAPL_VOL".into(),
        div_yield_id: None,
        discrete_dividends: Vec::new(),
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        exercise_schedule: None,
        attributes: Default::default(),
    };

    let market = create_option_market(as_of, spot, vol, rate);
    let delta_analytic = option.delta(&market, as_of).unwrap();
    let gamma_analytic = option.gamma(&market, as_of).unwrap();
    let vega_analytic = option.vega(&market, as_of).unwrap();

    let delta_fd = equity_option_fd_delta(&option, as_of, spot, vol, rate);
    let gamma_fd = equity_option_fd_gamma(&option, as_of, spot, vol, rate);
    let vega_fd = equity_option_fd_vega(&option, as_of, spot, vol, rate);

    assert_close("delta", delta_fd, delta_analytic, 0.002);
    assert_close("gamma", gamma_fd, gamma_analytic, 0.01);
    assert_close("vega", vega_fd, vega_analytic, 0.01);
}

#[test]
fn test_bucketed_dv01_sums_to_parallel() {
    // Test that bucketed DV01 sums to approximately parallel DV01 using the
    // triangular key-rate implementation.
    //
    // NOTE: Due to Money type rounding to cents (2 decimal places), small bucket
    // sensitivities may be lost. This test uses a large notional ($10M) to ensure
    // bucket DV01s are above the precision threshold.
    //
    // The triangular weights partition unity across the bucket grid, ensuring:
    //   sum(bucketed DV01) ≈ parallel DV01
    let as_of = date!(2025 - 01 - 01);

    use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
    // Use large notional ($10M) to ensure bucket DV01s are above Money precision threshold
    let bond = Bond::fixed(
        "BUCKETED_TEST",
        Money::new(10_000_000.0, Currency::USD), // $10M notional
        finstack_quant_core::types::Rate::from_decimal(0.05), // 5% coupon
        as_of,
        date!(2035 - 01 - 01), // 10 year bond
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .unwrap();

    // Create curve with dense knots (semi-annual) to properly capture cashflow sensitivity.
    let rate: f64 = 0.05;
    let mut knots: Vec<(f64, f64)> = vec![(0.0, 1.0)];
    for i in 1..=60 {
        let t = i as f64 * 0.5;
        knots.push((t, (-rate * t).exp()));
    }
    let disc_curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots(knots)
        .build()
        .unwrap();

    let market = MarketContext::new().insert(disc_curve);
    let registry = standard_registry();
    let pv = bond.value(&market, as_of).unwrap();
    let mut context = MetricContext::new(
        Arc::new(bond),
        Arc::new(market),
        as_of,
        pv,
        MetricContext::default_config(),
    );

    // Compute both total DV01 and bucketed DV01
    let results = registry
        .compute(&[MetricId::Dv01, MetricId::BucketedDv01], &mut context)
        .unwrap();

    let total_dv01 = *results.get(&MetricId::Dv01).unwrap();
    let curve_key = MetricId::custom("bucketed_dv01::USD-OIS");
    let bucketed_series = context.computed_series.get(&curve_key);

    if let Some(series) = bucketed_series {
        let sum_bucketed: f64 = series.iter().map(|(_, v)| v).sum();

        // Debug output
        eprintln!("Parallel DV01: {:.2}", total_dv01);
        eprintln!("Sum of bucketed: {:.2}", sum_bucketed);
        eprintln!("Buckets:");
        for (label, value) in series.iter() {
            eprintln!("  {}: {:.2}", label, value);
        }

        // Verify basic properties
        assert!(
            sum_bucketed.is_finite(),
            "Bucketed DV01 sum should be finite"
        );
        assert!(total_dv01.abs() > 1e-6, "Total DV01 should be non-trivial");
        assert!(
            total_dv01 < 0.0,
            "Parallel DV01 should be negative for long bond"
        );
        assert!(
            sum_bucketed < 0.0,
            "Sum of bucketed DV01 should be negative"
        );

        // Sum of bucketed DV01 should equal parallel DV01 within 0.01%
        // Triangular weights partition unity across the bucket grid, so this should be near-exact.
        // Using value_raw() for high-precision calculations enables tight tolerance.
        let diff_pct = ((sum_bucketed - total_dv01) / total_dv01).abs();
        assert!(
            diff_pct < 0.0001,
            "Sum of bucketed DV01 ({:.4}) should be within 0.01% of parallel DV01 ({:.4}), got {:.3}%",
            sum_bucketed, total_dv01, diff_pct * 100.0
        );

        // The 10y bucket should capture most of the sensitivity
        let ten_year_dv01 = series
            .iter()
            .find(|(k, _)| k == "10y")
            .map(|(_, v)| *v)
            .unwrap_or(0.0);
        assert!(ten_year_dv01 < 0.0, "10Y bucket DV01 should be negative");

        // At least some intermediate buckets should have non-zero DV01
        let nonzero_buckets = series.iter().filter(|(_, v)| v.abs() > 0.01).count();
        assert!(
            nonzero_buckets >= 3,
            "At least 3 buckets should have significant DV01, got {}",
            nonzero_buckets
        );
    } else {
        panic!("Bucketed DV01 series should be populated");
    }
}

#[test]
fn test_bucketed_vega_reports_raw_total_and_residual() {
    // Point buckets remain their own finite-difference derivatives.
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);

    let option = EquityOption {
        id: "BUCKETED_VEGA_TEST".into(),
        underlying_ticker: "AAPL".to_string(),
        strike: 100.0,
        option_type: OptionType::Call,
        exercise_style: ExerciseStyle::European,
        expiry,
        notional: Money::new(100.0, Currency::USD),
        day_count: DayCount::Act365F,
        theta_day_basis: Default::default(),
        settlement: SettlementType::Cash,
        exercise: None,
        discount_curve_id: "USD-OIS".into(),
        spot_id: "AAPL".into(),
        vol_surface_id: "AAPL_VOL".into(),
        div_yield_id: None,
        discrete_dividends: Vec::new(),
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        exercise_schedule: None,
        attributes: Default::default(),
    };

    let market = create_option_market(as_of, 100.0, 0.25, 0.05);
    let registry = standard_registry();
    let pv = option.value(&market, as_of).unwrap();
    let mut context = MetricContext::new(
        Arc::new(option),
        Arc::new(market),
        as_of,
        pv,
        MetricContext::default_config(),
    );

    // Compute both total Vega and bucketed Vega
    let results = registry
        .compute(&[MetricId::Vega, MetricId::BucketedVega], &mut context)
        .unwrap();

    let total_vega = *results.get(&MetricId::Vega).unwrap();
    let reported_bucket_total = *results.get(&MetricId::BucketedVega).unwrap();

    // BucketedVega must be wired end-to-end: the KeyRateVega calculator is
    // registered for EquityOption and must populate the 2D matrix store.
    let matrix = context
        .computed_matrix
        .get(&MetricId::BucketedVega)
        .expect("BucketedVega must be registered for EquityOption and store a 2D matrix");

    // The matrix grid must be non-empty (expiry rows x strike cols).
    assert!(
        !matrix.values.is_empty() && matrix.values.iter().all(|row| !row.is_empty()),
        "BucketedVega matrix must have a populated expiry/strike grid"
    );
    assert_eq!(
        matrix.values.len(),
        matrix.rows.len(),
        "BucketedVega matrix row count must match row labels"
    );

    let sum_bucketed: f64 = matrix.values.iter().flatten().sum();

    assert!((sum_bucketed - reported_bucket_total).abs() < 1e-10);
    let residual = context.computed[&MetricId::custom("bucketed_vega_residual")];
    assert!(
        (sum_bucketed + residual - total_vega).abs() < 1e-10,
        "raw buckets plus uncovered residual must reconcile to parallel vega"
    );
}
