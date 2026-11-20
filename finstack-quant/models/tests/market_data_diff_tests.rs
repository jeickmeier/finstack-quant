//! Tests for the surrounding crate component and its documented behavior.
//!
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::diff::{
    FxQuoteDiffParams, ProportionalDiffScale, measure_fx_quote_diff, measure_price_scalar_diff,
    measure_scalar_proportional_diff, measure_unitless_scalar_diff,
};
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::money::{
    Money,
    fx::{FxConversionPolicy, FxMatrix, FxProvider},
};
use finstack_quant_models::volatility::measure_vol_surface_shift;
use std::sync::Arc;
use time::Month;

// Test Helpers

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
    FxMatrix::new(Arc::new(MockFxProvider), None)
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

// Volatility Surface Shift Tests

#[test]
fn test_vol_surface_point_shift() {
    use finstack_quant_core::market_data::surfaces::{
        VolInterpolationMode, VolQuoteType, VolSurfaceAxis,
    };
    let _base_date = sample_date();
    let surface_t0 = VolSurface::builder("EQ-VOL")
        .expiries(&[0.25, 1.0, 2.0])
        .secondary_values(&[0.9, 1.0, 1.1])
        .secondary_axis(VolSurfaceAxis::Strike)
        .quote_type(VolQuoteType::BlackLognormal)
        .interpolation_mode(VolInterpolationMode::Vol)
        .row(&[0.30, 0.25, 0.22])
        .row(&[0.28, 0.23, 0.20])
        .row(&[0.26, 0.21, 0.18])
        .build()
        .expect("Should build surface");

    let surface_t1 = VolSurface::builder("EQ-VOL")
        .expiries(&[0.25, 1.0, 2.0])
        .secondary_values(&[0.9, 1.0, 1.1])
        .secondary_axis(VolSurfaceAxis::Strike)
        .quote_type(VolQuoteType::BlackLognormal)
        .interpolation_mode(VolInterpolationMode::Vol)
        .row(&[0.32, 0.27, 0.24])
        .row(&[0.30, 0.25, 0.22])
        .row(&[0.28, 0.23, 0.20])
        .build()
        .expect("Should build surface");

    let market_t0 = market_with_surface(surface_t0);
    let market_t1 = market_with_surface(surface_t1);

    let shift = measure_vol_surface_shift("EQ-VOL", &market_t0, &market_t1, 1.0, 1.0, 1.0)
        .expect("Should measure shift");

    // At expiry=1.0 and strike=1.0 (ATM), the constructed surfaces have:
    // - surface_t0: 0.23
    // - surface_t1: 0.25
    // so the expected shift is +0.02 = +2 vol points (percentage points).

    assert!(
        (shift - 0.02).abs() < 1e-14,
        "Expected 0.02 native volatility shift"
    );
}

#[test]
fn surface_shifts_preserve_native_normal_units_and_reject_incompatible_inputs() {
    use finstack_quant_core::dates::{DayCountContextState, VolatilityClock};
    use finstack_quant_core::market_data::surfaces::{
        VolGridOpts, VolInterpolationMode, VolQuoteType, VolSurfaceAxis,
    };
    let grid = |quote_type, secondary_axis, sigma, shift: Option<&[f64]>| {
        VolSurface::from_grid(
            "VOL",
            &[1.0],
            &[1.0],
            &[sigma],
            VolGridOpts {
                secondary_axis,
                quote_type,
                interpolation_mode: VolInterpolationMode::Vol,
            },
            shift,
        )
        .unwrap()
    };
    let normal = grid(VolQuoteType::Normal, VolSurfaceAxis::Strike, 0.008, None);
    let t0 = market_with_surface(normal.clone());
    let t1 = market_with_surface(grid(
        VolQuoteType::Normal,
        VolSurfaceAxis::Strike,
        0.009,
        None,
    ));
    let measured = measure_vol_surface_shift("VOL", &t0, &t1, 1.0, 1.0, 1.0).unwrap();
    assert!((measured - 0.001).abs() < 1e-14);
    let clock = VolatilityClock::try_new(
        sample_date(),
        DayCount::Act365F,
        DayCountContextState::default(),
    )
    .unwrap();
    for changed in [
        grid(
            VolQuoteType::BlackLognormal,
            VolSurfaceAxis::Strike,
            0.20,
            None,
        ),
        grid(VolQuoteType::Normal, VolSurfaceAxis::Tenor, 0.008, None),
        normal.with_expiry_clock(clock),
    ] {
        assert!(
            measure_vol_surface_shift("VOL", &t0, &market_with_surface(changed), 1.0, 1.0, 1.0)
                .is_err()
        );
    }
    let shifted = |shift| {
        market_with_surface(grid(
            VolQuoteType::ShiftedBlackLognormal,
            VolSurfaceAxis::Strike,
            0.2,
            Some(&[shift]),
        ))
    };
    assert!(
        measure_vol_surface_shift("VOL", &shifted(0.02), &shifted(0.03), 1.0, 1.0, 1.0).is_err()
    );
}

#[test]
fn surface_shifts_compare_only_active_displacements_and_use_each_expiry_clock() {
    use finstack_quant_core::dates::{DayCountContextState, VolatilityClock};
    use finstack_quant_core::market_data::surfaces::{
        VolGridOpts, VolInterpolationMode, VolQuoteType, VolSurfaceAxis,
    };
    let build = |anchor, shifts: &[f64]| {
        let surface = VolSurface::from_grid(
            "VOL",
            &[0.5, 1.0],
            &[1.0],
            &[0.12, 0.13],
            VolGridOpts {
                secondary_axis: VolSurfaceAxis::Strike,
                quote_type: VolQuoteType::ShiftedBlackLognormal,
                interpolation_mode: VolInterpolationMode::Vol,
            },
            Some(shifts),
        )
        .unwrap();
        let clock =
            VolatilityClock::try_new(anchor, DayCount::Act365F, DayCountContextState::default())
                .unwrap();
        market_with_surface(surface.with_expiry_clock(clock))
    };
    let opening = build(sample_date(), &[0.01, 0.02]);
    let closing = build(sample_date().next_day().unwrap(), &[0.02, 0.03]);
    let matched = measure_vol_surface_shift("VOL", &opening, &closing, 1.0, 0.5, 1.0).unwrap();
    assert!((matched + 0.01).abs() < 1e-14);
    assert!(measure_vol_surface_shift("VOL", &opening, &closing, 0.75, 0.5, 1.0).is_err());
}

#[test]
fn test_vol_surface_shift_across_expiry_coordinates() {
    use finstack_quant_core::market_data::surfaces::{
        VolInterpolationMode, VolQuoteType, VolSurfaceAxis,
    };
    let surface_t0 = VolSurface::builder("EQ-VOL")
        .expiries(&[0.25, 1.0])
        .secondary_values(&[0.9, 1.0, 1.1])
        .secondary_axis(VolSurfaceAxis::Strike)
        .quote_type(VolQuoteType::BlackLognormal)
        .interpolation_mode(VolInterpolationMode::Vol)
        .row(&[0.25, 0.24, 0.23])
        .row(&[0.22, 0.21, 0.20])
        .build()
        .expect("Should build surface");

    let surface_t1 = VolSurface::builder("EQ-VOL")
        .expiries(&[0.25, 1.0])
        .secondary_values(&[0.9, 1.0, 1.1])
        .secondary_axis(VolSurfaceAxis::Strike)
        .quote_type(VolQuoteType::BlackLognormal)
        .interpolation_mode(VolInterpolationMode::Vol)
        .row(&[0.27, 0.26, 0.25])
        .row(&[0.24, 0.23, 0.22])
        .build()
        .expect("Should build surface");

    let market_t0 = market_with_surface(surface_t0);
    let market_t1 = market_with_surface(surface_t1);

    let shift = measure_vol_surface_shift("EQ-VOL", &market_t0, &market_t1, 0.25, 1.0, 1.0)
        .expect("Should measure the two requested expiries");

    assert!((shift + 0.01).abs() < 1e-14);
}

#[test]
fn test_vol_surface_missing_error() {
    let market_t0 = MarketContext::new();
    let market_t1 = MarketContext::new();

    let result = measure_vol_surface_shift("MISSING", &market_t0, &market_t1, 1.0, 1.0, 1.0);

    assert!(result.is_err(), "Should error on missing surface");
}

#[test]
fn test_vol_surface_zero_shift() {
    use finstack_quant_core::market_data::surfaces::{
        VolInterpolationMode, VolQuoteType, VolSurfaceAxis,
    };
    let surface = VolSurface::builder("EQ-VOL")
        .expiries(&[0.25, 1.0])
        .secondary_values(&[0.9, 1.0, 1.1])
        .secondary_axis(VolSurfaceAxis::Strike)
        .quote_type(VolQuoteType::BlackLognormal)
        .interpolation_mode(VolInterpolationMode::Vol)
        .row(&[0.25, 0.24, 0.23])
        .row(&[0.22, 0.21, 0.20])
        .build()
        .expect("Should build surface");

    let market = market_with_surface(surface);

    let shift = measure_vol_surface_shift("EQ-VOL", &market, &market, 1.0, 1.0, 1.0)
        .expect("Should measure shift");

    assert_eq!(shift, 0.0, "Same surface should produce zero shift");
}

// FX Shift Tests

#[test]
fn test_fx_shift_strengthening() {
    let market_t0 = market_with_fx(sample_fx_matrix());
    let market_t1 = market_with_fx(sample_fx_matrix());
    let as_of_t0 = sample_date();
    let as_of_t1 = sample_date();

    // Since MockFxProvider always returns same rates, shift should be zero
    let shift = measure_fx_quote_diff(
        &FxQuoteDiffParams {
            base: Currency::USD,
            quote: Currency::EUR,
            date_t0: as_of_t0,
            policy_t0: FxConversionPolicy::CashflowDate,
            date_t1: as_of_t1,
            policy_t1: FxConversionPolicy::CashflowDate,
            scale: ProportionalDiffScale::Percent,
        },
        &market_t0,
        &market_t1,
    )
    .expect("Should measure shift");

    assert_eq!(shift.diff, 0.0, "Same FX matrix should produce zero shift");
}

#[test]
fn test_fx_missing_error() {
    let market_t0 = MarketContext::new();
    let market_t1 = MarketContext::new();
    let as_of_t0 = sample_date();
    let as_of_t1 = sample_date();

    let result = measure_fx_quote_diff(
        &FxQuoteDiffParams {
            base: Currency::USD,
            quote: Currency::EUR,
            date_t0: as_of_t0,
            policy_t0: FxConversionPolicy::CashflowDate,
            date_t1: as_of_t1,
            policy_t1: FxConversionPolicy::CashflowDate,
            scale: ProportionalDiffScale::Percent,
        },
        &market_t0,
        &market_t1,
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

    let market_t0 = market_with_fx(FxMatrix::new(Arc::new(DateAwareFx), None));
    let market_t1 = market_with_fx(FxMatrix::new(Arc::new(DateAwareFx), None));

    let meas = measure_fx_quote_diff(
        &FxQuoteDiffParams {
            base: Currency::EUR,
            quote: Currency::USD,
            date_t0: t0,
            policy_t0: FxConversionPolicy::CashflowDate,
            date_t1: t1,
            policy_t1: FxConversionPolicy::CashflowDate,
            scale: ProportionalDiffScale::Percent,
        },
        &market_t0,
        &market_t1,
    )
    .unwrap();

    // rate_t0 = 1.10, rate_t1 = 1.101 -> (1.101/1.10 - 1) * 100 ≈ 0.0909%
    assert!(
        (meas.diff - 0.0909).abs() < 0.01,
        "Expected ~0.09% shift, got {}",
        meas.diff
    );
    assert_eq!(meas.rate_t0, 1.10);
    assert!((meas.rate_t1 - 1.101).abs() < 1e-12);
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

    let market_t0 = market_with_fx(FxMatrix::new(Arc::new(ZeroFx), None));
    let market_t1 = market_with_fx(sample_fx_matrix());
    let result = measure_fx_quote_diff(
        &FxQuoteDiffParams {
            base: Currency::EUR,
            quote: Currency::USD,
            date_t0: sample_date(),
            policy_t0: FxConversionPolicy::CashflowDate,
            date_t1: sample_date(),
            policy_t1: FxConversionPolicy::CashflowDate,
            scale: ProportionalDiffScale::Percent,
        },
        &market_t0,
        &market_t1,
    );

    assert!(result.is_err(), "zero t0 FX rate should be rejected");
}

// Scalar Shift Tests

#[test]
fn test_scalar_price_shift() {
    let price_t0 = Money::new(100.0, Currency::USD).expect("valid money fixture");
    let price_t1 = Money::new(110.0, Currency::USD).expect("valid money fixture");

    let market_t0 = market_with_price("EQUITY-SPX", MarketScalar::price(price_t0));
    let market_t1 = market_with_price("EQUITY-SPX", MarketScalar::price(price_t1));

    let shift = measure_scalar_proportional_diff(
        "EQUITY-SPX",
        "EQUITY-SPX",
        &market_t0,
        &market_t1,
        ProportionalDiffScale::Percent,
    )
    .expect("Should measure shift");

    // (110 / 100 - 1) * 100 = 10%
    assert!((shift - 10.0).abs() < 0.01, "Expected 10%, got {}", shift);

    let native = measure_price_scalar_diff("EQUITY-SPX", "EQUITY-SPX", &market_t0, &market_t1)
        .expect("Should measure exact money diff");
    assert_eq!(
        native,
        Money::new(10.0, Currency::USD).expect("expected delta")
    );
}

#[test]
fn test_scalar_unitless_shift() {
    let market_t0 = market_with_price("COMMODITY-GOLD", MarketScalar::unitless(1800.0).unwrap());
    let market_t1 = market_with_price("COMMODITY-GOLD", MarketScalar::unitless(1900.0).unwrap());

    let shift = measure_scalar_proportional_diff(
        "COMMODITY-GOLD",
        "COMMODITY-GOLD",
        &market_t0,
        &market_t1,
        ProportionalDiffScale::Percent,
    )
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
    let market_t0 = market_with_price("ZERO", MarketScalar::unitless(0.0).unwrap());
    let market_t1 = market_with_price("ZERO", MarketScalar::unitless(100.0).unwrap());

    let result = measure_scalar_proportional_diff(
        "ZERO",
        "ZERO",
        &market_t0,
        &market_t1,
        ProportionalDiffScale::Percent,
    );
    assert!(
        result.is_err(),
        "Zero baseline should produce validation error"
    );
}

#[test]
fn test_scalar_missing_error() {
    let market_t0 = MarketContext::new();
    let market_t1 = MarketContext::new();

    let result = measure_scalar_proportional_diff(
        "MISSING",
        "MISSING",
        &market_t0,
        &market_t1,
        ProportionalDiffScale::Percent,
    );

    assert!(result.is_err(), "Should error on missing scalar");
}

#[test]
fn test_scalar_zero_shift() {
    let price = Money::new(100.0, Currency::USD).expect("valid money fixture");
    let market = market_with_price("TEST", MarketScalar::price(price));

    let shift = measure_scalar_proportional_diff(
        "TEST",
        "TEST",
        &market,
        &market,
        ProportionalDiffScale::Percent,
    )
    .expect("Should measure shift");

    assert_eq!(shift, 0.0, "Same scalar should produce zero shift");
}

#[test]
fn test_scalar_absolute_shift_returns_native_unit_difference() {
    let market_t0 = market_with_price("EQUITY-SPX", MarketScalar::unitless(100.0).unwrap());
    let market_t1 = market_with_price("EQUITY-SPX", MarketScalar::unitless(112.5).unwrap());

    let shift = measure_unitless_scalar_diff("EQUITY-SPX", "EQUITY-SPX", &market_t0, &market_t1)
        .expect("Should measure unitless diff");

    assert_eq!(shift, 12.5);
}

#[test]
fn test_scalar_shift_non_finite_results_error() {
    assert!(MarketScalar::unitless(f64::INFINITY).is_err());

    let rel_t0 = market_with_price("BAD", MarketScalar::unitless(1e-14).unwrap());
    let rel_t1 = market_with_price("BAD", MarketScalar::unitless(f64::MAX).unwrap());
    assert!(
        measure_scalar_proportional_diff(
            "BAD",
            "BAD",
            &rel_t0,
            &rel_t1,
            ProportionalDiffScale::Percent,
        )
        .is_err()
    );

    let abs_t0 = market_with_price("BAD", MarketScalar::unitless(-f64::MAX).unwrap());
    let abs_t1 = market_with_price("BAD", MarketScalar::unitless(f64::MAX).unwrap());
    assert!(measure_unitless_scalar_diff("BAD", "BAD", &abs_t0, &abs_t1).is_err());
}

#[test]
fn test_vol_surface_single_expiry() {
    use finstack_quant_core::market_data::surfaces::{
        VolInterpolationMode, VolQuoteType, VolSurfaceAxis,
    };
    let surface_t0 = VolSurface::builder("TEST-VOL")
        .expiries(&[1.0])
        .secondary_values(&[0.9, 1.0, 1.1])
        .secondary_axis(VolSurfaceAxis::Strike)
        .quote_type(VolQuoteType::BlackLognormal)
        .interpolation_mode(VolInterpolationMode::Vol)
        .row(&[0.25, 0.24, 0.23])
        .build()
        .expect("Should build surface");

    let surface_t1 = VolSurface::builder("TEST-VOL")
        .expiries(&[1.0])
        .secondary_values(&[0.9, 1.0, 1.1])
        .secondary_axis(VolSurfaceAxis::Strike)
        .quote_type(VolQuoteType::BlackLognormal)
        .interpolation_mode(VolInterpolationMode::Vol)
        .row(&[0.27, 0.26, 0.25])
        .build()
        .expect("Should build surface");

    let market_t0 = market_with_surface(surface_t0);
    let market_t1 = market_with_surface(surface_t1);

    let shift = measure_vol_surface_shift("TEST-VOL", &market_t0, &market_t1, 1.0, 1.0, 1.0)
        .expect("Should measure shift");

    assert!(shift > 0.0, "Should detect positive shift");
}

#[test]
fn test_scalar_neutral_shift() {
    let price = Money::new(100.0, Currency::USD).expect("valid money fixture");
    let market_t0 = market_with_price("TEST", MarketScalar::price(price));
    let market_t1 = market_with_price("TEST", MarketScalar::price(price));

    let shift = measure_scalar_proportional_diff(
        "TEST",
        "TEST",
        &market_t0,
        &market_t1,
        ProportionalDiffScale::Percent,
    )
    .expect("Should measure shift");

    assert_eq!(shift, 0.0, "Should be zero for identical prices");
}
