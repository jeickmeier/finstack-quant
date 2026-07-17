//! Tests for the surrounding crate component and its documented behavior.
//!
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::diff::{
    measure_discount_curve_shift, measure_fx_shift, measure_hazard_curve_shift,
    measure_inflation_curve_shift, measure_scalar_absolute_shift, measure_scalar_shift,
    measure_vol_surface_shift, TenorSamplingMethod, STANDARD_TENORS,
};
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::market_data::term_structures::HazardCurve;
use finstack_quant_core::market_data::term_structures::InflationCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::{
    fx::{FxConversionPolicy, FxMatrix, FxProvider},
    Money,
};
use std::sync::Arc;
use time::Month;

// ===================================================================
// Test Helpers
// ===================================================================

fn sample_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date")
}

struct MockFxProvider;
impl FxProvider for MockFxProvider {
    fn rate(
        &self,
        from: Currency,
        to: Currency,
        _on: Date,
        _policy: FxConversionPolicy,
    ) -> finstack_quant_core::Result<f64> {
        match (from, to) {
            (Currency::USD, Currency::EUR) => Ok(0.90),
            (Currency::EUR, Currency::USD) => Ok(1.0 / 0.90),
            _ if from == to => Ok(1.0),
            _ => Ok(1.0),
        }
    }
}

fn sample_fx_matrix() -> FxMatrix {
    FxMatrix::new(Arc::new(MockFxProvider))
}

fn market_with_discount(curve: DiscountCurve) -> MarketContext {
    MarketContext::new().insert(curve)
}

fn market_with_hazard(curve: HazardCurve) -> MarketContext {
    MarketContext::new().insert(curve)
}

fn market_with_inflation(curve: InflationCurve) -> MarketContext {
    MarketContext::new().insert(curve)
}

fn market_with_surface(surface: VolSurface) -> MarketContext {
    MarketContext::new().insert_surface(surface)
}

fn market_with_fx(fx: FxMatrix) -> MarketContext {
    MarketContext::new().insert_fx(fx)
}

fn market_with_price(id: &'static str, scalar: MarketScalar) -> MarketContext {
    MarketContext::new().insert_price(id, scalar)
}

// ===================================================================
// Discount Curve Shift Tests
// ===================================================================

#[test]
fn test_discount_curve_parallel_shift() {
    let base_date = sample_date();

    let curve_t0 = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (1.0, 0.96), (5.0, 0.82), (10.0, 0.67)])
        .interp(InterpStyle::LogLinear)
        .build()
        .expect("Should build curve");

    // +50bp shift: multiply discount factors by exp(-0.005*t)
    let curve_t1 = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .day_count(DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (1.0, 0.96 * (-0.005_f64 * 1.0).exp()),
            (5.0, 0.82 * (-0.005_f64 * 5.0).exp()),
            (10.0, 0.67 * (-0.005_f64 * 10.0).exp()),
        ])
        .interp(InterpStyle::LogLinear)
        .build()
        .expect("Should build curve");

    let market_t0 = market_with_discount(curve_t0);
    let market_t1 = market_with_discount(curve_t1);

    let shift = measure_discount_curve_shift(
        "USD-OIS",
        &market_t0,
        &market_t1,
        TenorSamplingMethod::Standard,
    )
    .expect("Should measure shift");

    assert!((shift - 50.0).abs() < 5.0, "Expected ~50bp, got {}", shift);
}

#[test]
fn test_discount_curve_steepening() {
    let base_date = sample_date();

    let curve_t0 = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots([(0.0, 1.0), (1.0, 0.97), (5.0, 0.85), (10.0, 0.74)])
        .interp(InterpStyle::Linear)
        .build()
        .expect("Should build curve");

    // Steepening: long end moves more than short end
    let curve_t1 = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots([
            (0.0, 1.0),
            (1.0, 0.968),  // -30bp
            (5.0, 0.835),  // -50bp
            (10.0, 0.704), // -100bp
        ])
        .interp(InterpStyle::Linear)
        .build()
        .expect("Should build curve");

    let market_t0 = market_with_discount(curve_t0);
    let market_t1 = market_with_discount(curve_t1);

    let shift = measure_discount_curve_shift(
        "USD-OIS",
        &market_t0,
        &market_t1,
        TenorSamplingMethod::Standard,
    )
    .expect("Should measure shift");

    // Shift should be mixed between short and long moves
    assert!(shift > 0.0, "Should detect positive shift on average");
}

#[test]
fn test_discount_curve_zero_shift() {
    let base_date = sample_date();

    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots([(0.0, 1.0), (1.0, 0.98), (5.0, 0.90)])
        .interp(InterpStyle::Linear)
        .build()
        .expect("Should build curve");

    let market = market_with_discount(curve);

    let shift =
        measure_discount_curve_shift("USD-OIS", &market, &market, TenorSamplingMethod::Standard)
            .expect("Should measure shift");

    assert_eq!(shift, 0.0, "Same market should produce zero shift");
}

#[test]
fn test_discount_curve_missing_error() {
    let market_t0 = MarketContext::new();
    let market_t1 = MarketContext::new();

    let result = measure_discount_curve_shift(
        "MISSING",
        &market_t0,
        &market_t1,
        TenorSamplingMethod::Standard,
    );

    assert!(result.is_err(), "Should error on missing curve");
}

#[test]
fn test_discount_curve_dynamic_sampling() {
    let base_date = sample_date();

    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots([(0.0, 1.0), (1.5, 0.97), (3.5, 0.92), (7.5, 0.80)])
        .interp(InterpStyle::Linear)
        .build()
        .expect("Should build curve");

    let market = market_with_discount(curve);

    let shift =
        measure_discount_curve_shift("USD-OIS", &market, &market, TenorSamplingMethod::Dynamic)
            .expect("Should measure with dynamic sampling");

    assert_eq!(shift, 0.0, "Same market should produce zero shift");
}

#[test]
fn test_discount_curve_custom_sampling() {
    let base_date = sample_date();

    let curve_t0 = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots([(0.0, 1.0), (1.0, 0.96), (5.0, 0.82), (10.0, 0.67)])
        .interp(InterpStyle::LogLinear)
        .build()
        .expect("Should build curve");

    let curve_t1 = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots([
            (0.0, 1.0),
            (1.0, 0.96 * (-0.005_f64 * 1.0).exp()),
            (5.0, 0.82 * (-0.005_f64 * 5.0).exp()),
            (10.0, 0.67 * (-0.005_f64 * 10.0).exp()),
        ])
        .interp(InterpStyle::LogLinear)
        .build()
        .expect("Should build curve");

    let market_t0 = market_with_discount(curve_t0);
    let market_t1 = market_with_discount(curve_t1);

    let shift = measure_discount_curve_shift(
        "USD-OIS",
        &market_t0,
        &market_t1,
        TenorSamplingMethod::Custom(vec![1.0, 5.0, 10.0]),
    )
    .expect("Should measure with custom sampling");

    assert!((shift - 50.0).abs() < 5.0, "Expected ~50bp, got {}", shift);
}

// ===================================================================
// Hazard Curve Shift Tests
// ===================================================================

#[test]
fn test_hazard_curve_parallel_shift() {
    let base_date = sample_date();

    let curve_t0 = HazardCurve::builder("CORP-01")
        .base_date(base_date)
        .recovery_rate(0.4)
        .knots(vec![(1.0, 0.01), (5.0, 0.02), (10.0, 0.025)])
        .build()
        .expect("Should build curve");

    // +25bp shift
    let curve_t1 = HazardCurve::builder("CORP-01")
        .base_date(base_date)
        .recovery_rate(0.4)
        .knots(vec![(1.0, 0.0125), (5.0, 0.0225), (10.0, 0.0275)])
        .build()
        .expect("Should build curve");

    let market_t0 = market_with_hazard(curve_t0);
    let market_t1 = market_with_hazard(curve_t1);

    let shift = measure_hazard_curve_shift(
        "CORP-01",
        &market_t0,
        &market_t1,
        TenorSamplingMethod::Standard,
    )
    .expect("Should measure shift");

    assert!((shift - 25.0).abs() < 1.0, "Expected ~25bp, got {}", shift);
}

#[test]
fn test_hazard_curve_widening() {
    let base_date = sample_date();

    let curve_t0 = HazardCurve::builder("CORP-01")
        .base_date(base_date)
        .recovery_rate(0.4)
        .knots(vec![(1.0, 0.010), (5.0, 0.012), (10.0, 0.015)])
        .build()
        .expect("Should build curve");

    // Widening: longer spreads move more
    let curve_t1 = HazardCurve::builder("CORP-01")
        .base_date(base_date)
        .recovery_rate(0.4)
        .knots(vec![(1.0, 0.0110), (5.0, 0.0145), (10.0, 0.0250)])
        .build()
        .expect("Should build curve");

    let market_t0 = market_with_hazard(curve_t0);
    let market_t1 = market_with_hazard(curve_t1);

    let shift = measure_hazard_curve_shift(
        "CORP-01",
        &market_t0,
        &market_t1,
        TenorSamplingMethod::Standard,
    )
    .expect("Should measure shift");

    assert!(shift > 0.0, "Should detect positive widening");
}

#[test]
fn test_hazard_curve_missing_error() {
    let market_t0 = MarketContext::new();
    let market_t1 = MarketContext::new();

    let result = measure_hazard_curve_shift(
        "MISSING",
        &market_t0,
        &market_t1,
        TenorSamplingMethod::Standard,
    );

    assert!(result.is_err(), "Should error on missing curve");
}

#[test]
fn test_hazard_curve_dynamic_sampling() {
    let base_date = sample_date();

    let curve = HazardCurve::builder("CORP-01")
        .base_date(base_date)
        .recovery_rate(0.4)
        .knots(vec![(2.0, 0.01), (4.0, 0.015), (8.0, 0.02)])
        .build()
        .expect("Should build curve");

    let market = market_with_hazard(curve);

    let shift =
        measure_hazard_curve_shift("CORP-01", &market, &market, TenorSamplingMethod::Dynamic)
            .expect("Should measure with dynamic sampling");

    assert_eq!(shift, 0.0, "Same market should produce zero shift");
}

// ===================================================================
// Inflation Curve Shift Tests
// ===================================================================

#[test]
fn test_inflation_curve_shift() {
    let curve_t0 = InflationCurve::builder("CPI-USD")
        .base_cpi(100.0)
        .base_date(sample_date())
        .knots([(0.0, 100.0), (1.0, 102.0), (5.0, 110.0), (10.0, 120.0)])
        .build()
        .expect("Should build curve");

    // 2% higher CPI at all tenors
    let curve_t1 = InflationCurve::builder("CPI-USD")
        .base_cpi(100.0)
        .base_date(sample_date())
        .knots([(0.0, 100.0), (1.0, 104.04), (5.0, 112.2), (10.0, 122.4)])
        .build()
        .expect("Should build curve");

    let market_t0 = market_with_inflation(curve_t0);
    let market_t1 = market_with_inflation(curve_t1);

    let shift = measure_inflation_curve_shift("CPI-USD", &market_t0, &market_t1)
        .expect("Should measure shift");

    assert!(shift > 0.0, "Should detect positive inflation shift");
}

#[test]
fn test_inflation_curve_missing_error() {
    let market_t0 = MarketContext::new();
    let market_t1 = MarketContext::new();

    let result = measure_inflation_curve_shift("MISSING", &market_t0, &market_t1);

    assert!(result.is_err(), "Should error on missing curve");
}

// ===================================================================
// Volatility Surface Shift Tests
// ===================================================================

#[test]
fn test_vol_surface_point_shift() {
    let _base_date = sample_date();
    let surface_t0 = VolSurface::builder("EQ-VOL")
        .expiries(&[0.25, 1.0, 2.0])
        .strikes(&[0.9, 1.0, 1.1])
        .row(&[0.30, 0.25, 0.22])
        .row(&[0.28, 0.23, 0.20])
        .row(&[0.26, 0.21, 0.18])
        .build()
        .expect("Should build surface");

    let surface_t1 = VolSurface::builder("EQ-VOL")
        .expiries(&[0.25, 1.0, 2.0])
        .strikes(&[0.9, 1.0, 1.1])
        .row(&[0.32, 0.27, 0.24])
        .row(&[0.30, 0.25, 0.22])
        .row(&[0.28, 0.23, 0.20])
        .build()
        .expect("Should build surface");

    let market_t0 = market_with_surface(surface_t0);
    let market_t1 = market_with_surface(surface_t1);

    let shift = measure_vol_surface_shift("EQ-VOL", &market_t0, &market_t1, Some(1.0), Some(1.0))
        .expect("Should measure shift");

    // At expiry=1.0 and strike=1.0 (ATM), the constructed surfaces have:
    // - surface_t0: 0.23
    // - surface_t1: 0.25
    // so the expected shift is +0.02 = +2 vol points (percentage points).

    assert!((shift - 2.0).abs() < 0.5, "Expected ~2pct pts vol shift");
}

#[test]
fn test_vol_surface_average_shift() {
    let surface_t0 = VolSurface::builder("EQ-VOL")
        .expiries(&[0.25, 1.0])
        .strikes(&[0.9, 1.0, 1.1])
        .row(&[0.25, 0.24, 0.23])
        .row(&[0.22, 0.21, 0.20])
        .build()
        .expect("Should build surface");

    let surface_t1 = VolSurface::builder("EQ-VOL")
        .expiries(&[0.25, 1.0])
        .strikes(&[0.9, 1.0, 1.1])
        .row(&[0.27, 0.26, 0.25])
        .row(&[0.24, 0.23, 0.22])
        .build()
        .expect("Should build surface");

    let market_t0 = market_with_surface(surface_t0);
    let market_t1 = market_with_surface(surface_t1);

    let shift = measure_vol_surface_shift("EQ-VOL", &market_t0, &market_t1, None, None)
        .expect("Should measure average shift");

    assert!(shift > 0.0, "Should detect positive vol shift");
}

#[test]
fn test_vol_surface_missing_error() {
    let market_t0 = MarketContext::new();
    let market_t1 = MarketContext::new();

    let result = measure_vol_surface_shift("MISSING", &market_t0, &market_t1, None, None);

    assert!(result.is_err(), "Should error on missing surface");
}

#[test]
fn test_vol_surface_zero_shift() {
    let surface = VolSurface::builder("EQ-VOL")
        .expiries(&[0.25, 1.0])
        .strikes(&[0.9, 1.0, 1.1])
        .row(&[0.25, 0.24, 0.23])
        .row(&[0.22, 0.21, 0.20])
        .build()
        .expect("Should build surface");

    let market = market_with_surface(surface);

    let shift = measure_vol_surface_shift("EQ-VOL", &market, &market, None, None)
        .expect("Should measure shift");

    assert_eq!(shift, 0.0, "Same surface should produce zero shift");
}

// ===================================================================
// FX Shift Tests
// ===================================================================

#[test]
fn test_fx_shift_strengthening() {
    let market_t0 = market_with_fx(sample_fx_matrix());
    let market_t1 = market_with_fx(sample_fx_matrix());
    let as_of_t0 = sample_date();
    let as_of_t1 = sample_date();

    // Since MockFxProvider always returns same rates, shift should be zero
    let shift = measure_fx_shift(
        Currency::USD,
        Currency::EUR,
        &market_t0,
        &market_t1,
        as_of_t0,
        as_of_t1,
    )
    .expect("Should measure shift");

    assert_eq!(shift, 0.0, "Same FX matrix should produce zero shift");
}

#[test]
fn test_fx_missing_error() {
    let market_t0 = MarketContext::new();
    let market_t1 = MarketContext::new();
    let as_of_t0 = sample_date();
    let as_of_t1 = sample_date();

    let result = measure_fx_shift(
        Currency::USD,
        Currency::EUR,
        &market_t0,
        &market_t1,
        as_of_t0,
        as_of_t1,
    );

    assert!(result.is_err(), "Should error on missing FX matrix");
}

#[test]
fn test_fx_shift_uses_valuation_dates() {
    struct DateAwareFx;
    impl FxProvider for DateAwareFx {
        fn rate(
            &self,
            from: Currency,
            to: Currency,
            on: Date,
            _policy: FxConversionPolicy,
        ) -> finstack_quant_core::Result<f64> {
            if from == Currency::EUR && to == Currency::USD {
                let base = 1.10_f64;
                let days = (on - sample_date()).whole_days();
                Ok(base + 0.001 * days as f64)
            } else {
                Ok(1.0)
            }
        }
    }

    let t0 = sample_date();
    let t1 = t0.next_day().expect("next day should exist");

    let market_t0 = market_with_fx(FxMatrix::new(Arc::new(DateAwareFx)));
    let market_t1 = market_with_fx(FxMatrix::new(Arc::new(DateAwareFx)));

    let shift =
        measure_fx_shift(Currency::EUR, Currency::USD, &market_t0, &market_t1, t0, t1).unwrap();

    // rate_t0 = 1.10, rate_t1 = 1.101 -> (1.101/1.10 - 1) * 100 ≈ 0.0909%
    assert!(
        (shift - 0.0909).abs() < 0.01,
        "Expected ~0.09% shift, got {}",
        shift
    );
}

#[test]
fn test_fx_shift_zero_baseline_errors() {
    struct ZeroFx;
    impl FxProvider for ZeroFx {
        fn rate(
            &self,
            _from: Currency,
            _to: Currency,
            _on: Date,
            _policy: FxConversionPolicy,
        ) -> finstack_quant_core::Result<f64> {
            Ok(0.0)
        }
    }

    let market_t0 = market_with_fx(FxMatrix::new(Arc::new(ZeroFx)));
    let market_t1 = market_with_fx(sample_fx_matrix());
    let result = measure_fx_shift(
        Currency::EUR,
        Currency::USD,
        &market_t0,
        &market_t1,
        sample_date(),
        sample_date(),
    );

    assert!(result.is_err(), "zero t0 FX rate should be rejected");
}

// ===================================================================
// Scalar Shift Tests
// ===================================================================

#[test]
fn test_scalar_price_shift() {
    let price_t0 = Money::new(100.0, Currency::USD);
    let price_t1 = Money::new(110.0, Currency::USD);

    let market_t0 = market_with_price("EQUITY-SPX", MarketScalar::Price(price_t0));
    let market_t1 = market_with_price("EQUITY-SPX", MarketScalar::Price(price_t1));

    let shift =
        measure_scalar_shift("EQUITY-SPX", &market_t0, &market_t1).expect("Should measure shift");

    // (110 / 100 - 1) * 100 = 10%
    assert!((shift - 10.0).abs() < 0.01, "Expected 10%, got {}", shift);
}

#[test]
fn test_scalar_unitless_shift() {
    let market_t0 = market_with_price("COMMODITY-GOLD", MarketScalar::Unitless(1800.0));
    let market_t1 = market_with_price("COMMODITY-GOLD", MarketScalar::Unitless(1900.0));

    let shift = measure_scalar_shift("COMMODITY-GOLD", &market_t0, &market_t1)
        .expect("Should measure shift");

    // (1900 / 1800 - 1) * 100 = 5.56%
    assert!(
        (shift - 5.56).abs() < 0.01,
        "Expected ~5.56%, got {}",
        shift
    );
}

#[test]
fn test_scalar_shift_zero_baseline_errors() {
    let market_t0 = market_with_price("ZERO", MarketScalar::Unitless(0.0));
    let market_t1 = market_with_price("ZERO", MarketScalar::Unitless(100.0));

    let result = measure_scalar_shift("ZERO", &market_t0, &market_t1);
    assert!(
        result.is_err(),
        "Zero baseline should produce validation error"
    );
}

#[test]
fn test_scalar_missing_error() {
    let market_t0 = MarketContext::new();
    let market_t1 = MarketContext::new();

    let result = measure_scalar_shift("MISSING", &market_t0, &market_t1);

    assert!(result.is_err(), "Should error on missing scalar");
}

#[test]
fn test_scalar_zero_shift() {
    let price = Money::new(100.0, Currency::USD);
    let market = market_with_price("TEST", MarketScalar::Price(price));

    let shift = measure_scalar_shift("TEST", &market, &market).expect("Should measure shift");

    assert_eq!(shift, 0.0, "Same scalar should produce zero shift");
}

#[test]
fn test_scalar_absolute_shift_returns_native_unit_difference() {
    let market_t0 = market_with_price("EQUITY-SPX", MarketScalar::Unitless(100.0));
    let market_t1 = market_with_price("EQUITY-SPX", MarketScalar::Unitless(112.5));

    let shift = measure_scalar_absolute_shift("EQUITY-SPX", &market_t0, &market_t1)
        .expect("Should measure absolute shift");

    assert_eq!(shift, 12.5);
}

#[test]
fn test_scalar_shift_non_finite_results_error() {
    let market_t0 = market_with_price("BAD", MarketScalar::Unitless(1.0));
    let market_t1 = market_with_price("BAD", MarketScalar::Unitless(f64::INFINITY));

    assert!(measure_scalar_shift("BAD", &market_t0, &market_t1).is_err());
    assert!(measure_scalar_absolute_shift("BAD", &market_t0, &market_t1).is_err());
}

// ===================================================================
// Tenor Sampling Method Tests
// ===================================================================

#[test]
fn test_standard_tenors_constant() {
    // Verify STANDARD_TENORS contains expected values
    assert_eq!(STANDARD_TENORS.len(), 9);
    assert_eq!(STANDARD_TENORS[0], 0.25);
    assert_eq!(STANDARD_TENORS[4], 3.0);
    assert_eq!(STANDARD_TENORS[8], 30.0);
}

#[test]
fn test_tenor_sampling_with_all_methods() {
    let base_date = sample_date();

    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots([
            (0.0, 1.0),
            (0.25, 0.9925),
            (1.0, 0.97),
            (2.0, 0.94),
            (3.0, 0.91),
            (5.0, 0.85),
            (10.0, 0.74),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .expect("Should build curve");

    let market = market_with_discount(curve);

    // Standard sampling
    let shift_std =
        measure_discount_curve_shift("USD-OIS", &market, &market, TenorSamplingMethod::Standard)
            .expect("Standard sampling should work");

    // Dynamic sampling
    let shift_dyn =
        measure_discount_curve_shift("USD-OIS", &market, &market, TenorSamplingMethod::Dynamic)
            .expect("Dynamic sampling should work");

    // Custom sampling
    let shift_custom = measure_discount_curve_shift(
        "USD-OIS",
        &market,
        &market,
        TenorSamplingMethod::Custom(vec![1.0, 2.0, 5.0]),
    )
    .expect("Custom sampling should work");

    // All should be zero for identical market
    assert_eq!(shift_std, 0.0);
    assert_eq!(shift_dyn, 0.0);
    assert_eq!(shift_custom, 0.0);
}

// ===================================================================
// Edge Cases and Boundary Conditions
// ===================================================================

#[test]
fn test_discount_shift_negative_shift() {
    let base_date = sample_date();

    let curve_t0 = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots([(0.0, 1.0), (1.0, 0.97), (5.0, 0.85)])
        .interp(InterpStyle::Linear)
        .build()
        .expect("Should build curve");

    // -50bp shift: multiply discount factors by exp(0.005*t)
    let curve_t1 = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots([
            (0.0, 1.0),
            (1.0, 0.97 * (0.005_f64 * 1.0).exp()),
            (5.0, 0.85 * (0.005_f64 * 5.0).exp()),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .expect("Should build curve");

    let market_t0 = market_with_discount(curve_t0);
    let market_t1 = market_with_discount(curve_t1);

    let shift = measure_discount_curve_shift(
        "USD-OIS",
        &market_t0,
        &market_t1,
        TenorSamplingMethod::Standard,
    )
    .expect("Should measure shift");

    // Should detect negative (downward) shift
    assert!(
        shift < -40.0,
        "Should detect negative shift of ~-50bp, got {}",
        shift
    );
}

#[test]
fn test_hazard_curve_zero_shift_consistency() {
    let base_date = sample_date();

    let curve = HazardCurve::builder("CORP-01")
        .base_date(base_date)
        .recovery_rate(0.4)
        .knots(vec![(1.0, 0.01), (5.0, 0.02), (10.0, 0.03)])
        .build()
        .expect("Should build curve");

    let market = market_with_hazard(curve);

    // All sampling methods should yield same result for identical curves
    let shift_std =
        measure_hazard_curve_shift("CORP-01", &market, &market, TenorSamplingMethod::Standard)
            .expect("Should measure");

    let shift_dyn =
        measure_hazard_curve_shift("CORP-01", &market, &market, TenorSamplingMethod::Dynamic)
            .expect("Should measure");

    assert_eq!(shift_std, 0.0);
    assert_eq!(shift_dyn, 0.0);
}

#[test]
fn test_vol_surface_single_expiry() {
    let surface_t0 = VolSurface::builder("TEST-VOL")
        .expiries(&[1.0])
        .strikes(&[0.9, 1.0, 1.1])
        .row(&[0.25, 0.24, 0.23])
        .build()
        .expect("Should build surface");

    let surface_t1 = VolSurface::builder("TEST-VOL")
        .expiries(&[1.0])
        .strikes(&[0.9, 1.0, 1.1])
        .row(&[0.27, 0.26, 0.25])
        .build()
        .expect("Should build surface");

    let market_t0 = market_with_surface(surface_t0);
    let market_t1 = market_with_surface(surface_t1);

    let shift = measure_vol_surface_shift("TEST-VOL", &market_t0, &market_t1, None, None)
        .expect("Should measure shift");

    assert!(shift > 0.0, "Should detect positive shift");
}

#[test]
fn test_scalar_neutral_shift() {
    let price = Money::new(100.0, Currency::USD);
    let market_t0 = market_with_price("TEST", MarketScalar::Price(price));
    let market_t1 = market_with_price("TEST", MarketScalar::Price(price));

    let shift = measure_scalar_shift("TEST", &market_t0, &market_t1).expect("Should measure shift");

    assert_eq!(shift, 0.0, "Should be zero for identical prices");
}
