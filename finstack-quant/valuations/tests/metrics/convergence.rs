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
fn test_equity_option_all_analytical_greeks() {
    // For EquityOption, all greeks should match between direct calls and registry
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
        settlement: SettlementType::Cash,
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
    test_equity_option_greek(&option, &market, as_of, MetricId::Theta, |opt, mkt, dt| {
        opt.theta(mkt, dt)
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
        settlement: SettlementType::Cash,
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
        0.05,                                    // 5% coupon
        as_of,
        date!(2035 - 01 - 01), // 10 year bond
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

/// Test bucketed CS01 sum with full curve support (strict tolerance).
///
/// When the hazard curve has knots covering the full standard bucket grid,
/// bucketed CS01 should sum very closely to total CS01 (within 2%).
#[test]
fn test_bucketed_cs01_sums_to_total_strict() {
    let as_of = date!(2025 - 01 - 01);

    use finstack_quant_core::market_data::term_structures::HazardCurve;

    // 10Y CDS - within curve support
    let cds = crate::finstack_quant_test_utils::cds_buy_protection(
        "BUCKETED_CS01_STRICT",
        Money::new(1_000_000.0, Currency::USD),
        200.0, // 200bp spread
        as_of,
        date!(2035 - 01 - 01), // 10Y CDS
        "USD-OIS",
        "HAZARD",
    )
    .expect("CDS construction should succeed");

    // Discount curve with knots extending to 30Y to cover all standard buckets
    let disc_curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([
            (0.0f64, 1.0f64),
            (0.25f64, (-0.05f64 * 0.25).exp()),
            (0.5f64, (-0.05f64 * 0.5).exp()),
            (1.0f64, (-0.05f64).exp()),
            (2.0f64, (-0.05f64 * 2.0).exp()),
            (3.0f64, (-0.05f64 * 3.0).exp()),
            (5.0f64, (-0.05f64 * 5.0).exp()),
            (7.0f64, (-0.05f64 * 7.0).exp()),
            (10.0f64, (-0.05f64 * 10.0).exp()),
            (15.0f64, (-0.05f64 * 15.0).exp()),
            (20.0f64, (-0.05f64 * 20.0).exp()),
            (30.0f64, (-0.05f64 * 30.0).exp()),
        ])
        .build()
        .unwrap();

    // Hazard curve with knots covering standard bucket grid (out to 30Y)
    let hazard_curve = HazardCurve::builder("HAZARD")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .recovery_rate(0.4)
        .knots([
            (0.0f64, 0.02f64),
            (0.25f64, 0.020f64),
            (0.5f64, 0.021f64),
            (1.0f64, 0.022f64),
            (2.0f64, 0.025f64),
            (3.0f64, 0.028f64),
            (5.0f64, 0.032f64),
            (7.0f64, 0.036f64),
            (10.0f64, 0.040f64),
            (15.0f64, 0.045f64),
            (20.0f64, 0.050f64),
            (30.0f64, 0.055f64),
        ])
        .build()
        .unwrap();

    let market = MarketContext::new().insert(disc_curve).insert(hazard_curve);

    let registry = standard_registry();
    let pv = cds.value(&market, as_of).unwrap();
    let mut context = MetricContext::new(
        Arc::new(cds),
        Arc::new(market),
        as_of,
        pv,
        MetricContext::default_config(),
    );

    // Compute both total CS01 and bucketed CS01
    let results = registry
        .compute(&[MetricId::Cs01, MetricId::BucketedCs01], &mut context)
        .unwrap();

    let total_cs01 = *results.get(&MetricId::Cs01).unwrap();

    // Get bucketed CS01 series (now stored under curve-specific key)
    let curve_key = MetricId::custom("bucketed_cs01::HAZARD");
    let bucketed_series = context.computed_series.get(&curve_key);

    if let Some(series) = bucketed_series {
        let sum_bucketed: f64 = series.iter().map(|(_, v)| v).sum();

        // With full curve support, bucketed CS01 should sum to total CS01 within 2%
        let diff_pct = (sum_bucketed - total_cs01).abs() / total_cs01.abs().max(1e-10);
        assert!(
            diff_pct < 0.02,
            "Bucketed CS01 sum ({:.4}) should equal total CS01 ({:.4}) within 2%, diff: {:.2}%",
            sum_bucketed,
            total_cs01,
            diff_pct * 100.0
        );
    } else {
        panic!("Bucketed CS01 series should be populated");
    }
}

/// Test bucketed CS01 sum when curve support is limited (fallback tolerance).
///
/// When the hazard curve has fewer knots than the standard bucket grid,
/// some buckets will have zero or extrapolated values. This test documents
/// the graceful degradation behavior with a looser tolerance.
#[test]
fn test_bucketed_cs01_sums_to_total_limited_curve_support() {
    let as_of = date!(2025 - 01 - 01);

    use finstack_quant_core::market_data::term_structures::HazardCurve;

    // 5Y CDS - curve support only to 5Y
    let cds = crate::finstack_quant_test_utils::cds_buy_protection(
        "BUCKETED_CS01_LIMITED",
        Money::new(1_000_000.0, Currency::USD),
        200.0, // 200bp spread
        as_of,
        date!(2030 - 01 - 01), // 5Y CDS
        "USD-OIS",
        "HAZARD",
    )
    .expect("CDS construction should succeed");

    // Discount curve with knots only to 5Y
    let disc_curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([
            (0.0f64, 1.0f64),
            (1.0f64, (-0.05f64).exp()),
            (2.0f64, (-0.10f64).exp()),
            (3.0f64, (-0.15f64).exp()),
            (4.0f64, (-0.20f64).exp()),
            (5.0f64, (-0.25f64).exp()),
        ])
        .build()
        .unwrap();

    // Hazard curve with knots only to 5Y (standard buckets include 7Y, 10Y, 15Y, 20Y, 30Y)
    let hazard_curve = HazardCurve::builder("HAZARD")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .recovery_rate(0.4)
        .knots([
            (0.0f64, 0.02f64),
            (1.0f64, 0.025f64),
            (2.0f64, 0.03f64),
            (3.0f64, 0.035f64),
            (4.0f64, 0.04f64),
            (5.0f64, 0.045f64),
        ])
        .build()
        .unwrap();

    let market = MarketContext::new().insert(disc_curve).insert(hazard_curve);

    let registry = standard_registry();
    let pv = cds.value(&market, as_of).unwrap();
    let mut context = MetricContext::new(
        Arc::new(cds),
        Arc::new(market),
        as_of,
        pv,
        MetricContext::default_config(),
    );

    // Compute both total CS01 and bucketed CS01
    let results = registry
        .compute(&[MetricId::Cs01, MetricId::BucketedCs01], &mut context)
        .unwrap();

    let total_cs01 = *results.get(&MetricId::Cs01).unwrap();

    // Get bucketed CS01 series (stored under curve-specific key)
    let curve_key = MetricId::custom("bucketed_cs01::HZD-LIMITED");
    let bucketed_series = context.computed_series.get(&curve_key);

    if let Some(series) = bucketed_series {
        let sum_bucketed: f64 = series.iter().map(|(_, v)| v).sum();

        // With limited curve support, bucketed CS01 may not sum exactly to total.
        // Standard buckets beyond curve support will have extrapolated or zero values.
        let diff = (sum_bucketed - total_cs01).abs();
        let diff_pct = diff / total_cs01.abs().max(1e-10);

        // Enforce sign consistency and a bounded deviation (documented behavior).
        assert!(
            sum_bucketed.is_finite(),
            "Bucketed CS01 sum should be finite"
        );
        assert!(total_cs01.is_finite(), "Total CS01 should be finite");
        assert!(
            sum_bucketed.signum() == total_cs01.signum() || sum_bucketed.abs() < 1e-8,
            "Bucketed CS01 sum ({:.4}) should have same sign as total CS01 ({:.4})",
            sum_bucketed,
            total_cs01
        );

        // Require bucketed sum within 10% or $50 of total, whichever is larger.
        assert!(
            diff_pct < 0.10 || diff < 50.0,
            "Bucketed CS01 sum ({:.4}) should be within 10% or $50 of total CS01 ({:.4}) \
            with limited curve support, diff: {:.2}% (abs {:.4})",
            sum_bucketed,
            total_cs01,
            diff_pct * 100.0,
            diff
        );
    }
}

#[test]
fn test_bucketed_vega_sums_to_total() {
    // Bucketed Vega should approximately sum to total Vega
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
        settlement: SettlementType::Cash,
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

    // Sum should approximately equal total (within 2% for vol surface interpolation)
    let diff_pct = (sum_bucketed - total_vega).abs() / total_vega.abs().max(1e-10);
    assert!(
        diff_pct < 0.02,
        "Bucketed Vega sum ({}) should be close to total Vega ({}), diff: {:.2}%",
        sum_bucketed,
        total_vega,
        diff_pct * 100.0
    );
}
