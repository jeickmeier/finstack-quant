//! Parity tests that lock behavior across the new canonical APIs introduced
//! by the simplicity audit.
//!
//! These tests prove that:
//! 1. `Money::format_with` + defaults == `format_with_separators`, and that
//!    `format(decimals, show_currency)` matches the corresponding `FormatOpts`.
//! 2. `DiscountCurveBuilder::validation(ValidationMode::*)` covers the
//!    market-standard, negative-rate-friendly, and raw validation policies.
//! 3. `VolSurface::from_grid_opts` preserves the explicit axis and
//!    interpolation contract.
//! 4. `VolSurface::value_checked` and `value_in_bounds` agree on interior
//!    evaluation points (the shared bilinear kernel regression test).
//! 5. `Rate::try_from(f64)` / `Percentage::try_from(f64)` / `Bps::try_from(f64)`
//!    are fallible analogs of the panicking `From<f64>` conversions.
//! 6. `[(Date, f64)]::irr(None)` equals
//!    `xirr_with_daycount_ctx(.., Act365F, DayCountContext::default(), None)`.
//!
//! These tests lock the canonical APIs produced by the simplicity audit.

use finstack_core::cashflow::{xirr, xirr_with_daycount_ctx};
use finstack_core::currency::Currency;
use finstack_core::dates::{Date, DayCount, DayCountContext};
use finstack_core::market_data::surfaces::{
    VolGridOpts, VolInterpolationMode, VolQuoteType, VolSurface, VolSurfaceAxis,
};
use finstack_core::market_data::term_structures::{DiscountCurve, ValidationMode};
use finstack_core::math::interp::InterpStyle;
use finstack_core::money::{FormatOpts, Money};
use finstack_core::types::{Bps, Percentage, Rate};
use time::Month;

fn d(year: i32, month: u8, day: u8) -> Date {
    Date::from_calendar_date(year, Month::try_from(month).unwrap(), day).unwrap()
}

// ---------------------------------------------------------------------------
// Money formatting parity
// ---------------------------------------------------------------------------

#[test]
fn format_with_default_matches_format_with_separators() {
    let amt = Money::new(1_042_315.67, Currency::USD);
    assert_eq!(
        amt.format_with(FormatOpts::default()),
        amt.format_with_separators(2)
    );
}

#[test]
fn format_with_no_group_matches_format_two_args() {
    let amt = Money::new(1_042_315.67, Currency::USD);
    let via_opts = amt.format_with(FormatOpts {
        decimals: Some(2),
        show_currency: true,
        group: None,
        rounding: Default::default(),
    });
    assert_eq!(via_opts, amt.format(2, true));

    let no_currency = amt.format_with(FormatOpts {
        decimals: Some(2),
        show_currency: false,
        group: None,
        rounding: Default::default(),
    });
    assert_eq!(no_currency, amt.format(2, false));
}

#[test]
fn format_with_handles_negative_amounts() {
    let amt = Money::new(-1_234.5, Currency::USD);
    assert_eq!(amt.format_with(FormatOpts::default()), "USD -1,234.50");
    assert_eq!(amt.format_with_separators(2), "USD -1,234.50");
}

// ---------------------------------------------------------------------------
// DiscountCurveBuilder::validation parity
// ---------------------------------------------------------------------------

#[test]
fn validation_market_standard_builds_monotonic_curve() {
    let base = d(2025, 1, 1);
    let knots = [(0.0, 1.0), (1.0, 0.95), (5.0, 0.80)];

    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots(knots)
        .validation(ValidationMode::MarketStandard)
        .build()
        .expect("market-standard validation build");

    assert!(curve.df(0.25) < 1.0);
    assert!(curve.df(4.5) < curve.df(0.25));
}

#[test]
fn validation_negative_rate_friendly_builds_increasing_discount_factors() {
    let base = d(2025, 1, 1);
    let knots = [(0.0, 1.0), (1.0, 1.002), (5.0, 0.99)];

    let curve = DiscountCurve::builder("EUR-OIS")
        .base_date(base)
        .knots(knots)
        .interp(InterpStyle::LogLinear)
        .validation(ValidationMode::NegativeRateFriendly {
            forward_floor: -0.05,
        })
        .build()
        .expect("negative-rate validation build");

    assert!(curve.df(0.5) > 1.0);
    assert!(curve.df(4.5) < curve.df(1.0));
}

#[test]
fn validation_raw_allows_non_monotonic_without_forward_floor() {
    let base = d(2025, 1, 1);
    let knots = [(0.0, 1.0), (1.0, 1.001), (5.0, 0.98)];

    let curve = DiscountCurve::builder("RAW")
        .base_date(base)
        .knots(knots)
        .interp(InterpStyle::LogLinear)
        .validation(ValidationMode::Raw {
            allow_non_monotonic: true,
            forward_floor: None,
        })
        .build()
        .expect("raw validation build");

    assert!(curve.df(0.5) > 1.0);
    assert!(curve.df(4.5) < curve.df(1.0));
}

// ---------------------------------------------------------------------------
// VolSurface construction parity
// ---------------------------------------------------------------------------

fn sample_grid() -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let expiries = vec![0.25, 0.5, 1.0, 2.0];
    let strikes = vec![80.0, 90.0, 100.0, 110.0, 120.0];
    let n = expiries.len() * strikes.len();
    let vols: Vec<f64> = (0..n).map(|i| 0.18 + 0.001 * i as f64).collect();
    (expiries, strikes, vols)
}

#[test]
fn from_grid_opts_default_matches_from_grid() {
    let (exp, strikes, vols) = sample_grid();
    let a = VolSurface::from_grid("S", &exp, &strikes, &vols).unwrap();
    let b = VolSurface::from_grid_opts("S", &exp, &strikes, &vols, VolGridOpts::default()).unwrap();

    for &e in &exp {
        for &k in &strikes {
            assert!(
                (a.value_checked(e, k).unwrap() - b.value_checked(e, k).unwrap()).abs() < 1e-15
            );
        }
    }
}

#[test]
fn from_grid_opts_preserves_axis_and_interpolation_mode() {
    let (exp, strikes, vols) = sample_grid();
    let surface = VolSurface::from_grid_opts(
        "S",
        &exp,
        &strikes,
        &vols,
        VolGridOpts {
            secondary_axis: VolSurfaceAxis::Tenor,
            quote_type: VolQuoteType::Normal,
            interpolation_mode: VolInterpolationMode::TotalVariance,
        },
    )
    .unwrap();

    assert_eq!(surface.secondary_axis(), VolSurfaceAxis::Tenor);
    assert_eq!(surface.quote_type(), VolQuoteType::Normal);
    assert_eq!(
        surface.interpolation_mode(),
        VolInterpolationMode::TotalVariance
    );
    for &e in &exp {
        for &k in &strikes {
            assert!(surface.value_checked(e, k).unwrap().is_finite());
        }
    }
}

// ---------------------------------------------------------------------------
// VolSurface bilinear evaluation parity (value_checked vs value_clamped)
// ---------------------------------------------------------------------------

#[test]
fn value_checked_matches_value_clamped_on_interior() {
    let (exp, strikes, vols) = sample_grid();
    let surface = VolSurface::from_grid("S", &exp, &strikes, &vols).unwrap();
    // Strictly interior samples (not on grid edges) exercise the
    // shared bilinear kernel used by both evaluators.
    let test_pts = [
        (0.35_f64, 85.0_f64),
        (0.75, 95.0),
        (1.5, 100.5),
        (1.75, 115.0),
    ];
    for (e, k) in test_pts {
        let a = surface.value_checked(e, k).unwrap();
        let b = surface.value_clamped(e, k);
        assert!(
            (a - b).abs() < 1e-15,
            "interior bilinear mismatch at (e={e}, k={k}): checked={a}, clamped={b}",
        );
    }
}

// ---------------------------------------------------------------------------
// Rate / Percentage / Bps: TryFrom<f64> parity and NaN rejection
// ---------------------------------------------------------------------------

#[test]
fn rate_try_from_decimal_matches_from_decimal_for_valid_inputs() {
    for &x in &[0.0, 0.05, -0.001, 1.5, -0.999_5] {
        let via_try = Rate::try_from_decimal(x).unwrap();
        let via_from = Rate::from(x);
        assert!(
            (via_try.as_decimal() - via_from.as_decimal()).abs() < 1e-15,
            "Rate parity mismatch at {x}",
        );
    }
}

#[test]
fn rate_try_from_decimal_rejects_non_finite() {
    assert!(Rate::try_from_decimal(f64::NAN).is_err());
    assert!(Rate::try_from_decimal(f64::INFINITY).is_err());
    assert!(Rate::try_from_decimal(f64::NEG_INFINITY).is_err());
}

#[test]
fn percentage_try_new_matches_new_for_valid_inputs() {
    for &x in &[0.0, 5.0, -1.25, 150.0] {
        let via_try = Percentage::try_new(x).unwrap();
        let via_new = Percentage::new(x);
        assert!((via_try.as_percent() - via_new.as_percent()).abs() < 1e-15);
    }
}

#[test]
fn percentage_try_new_rejects_non_finite() {
    assert!(Percentage::try_new(f64::NAN).is_err());
    assert!(Percentage::try_new(f64::INFINITY).is_err());
}

#[test]
fn bps_try_from_f64_matches_try_new() {
    let via_try = Bps::try_from(25.0_f64).unwrap();
    let via_new = Bps::try_new(25.0).unwrap();
    assert_eq!(via_try.as_bps(), via_new.as_bps());
}

#[test]
fn bps_try_from_f64_rejects_non_finite() {
    assert!(Bps::try_from(f64::NAN).is_err());
    assert!(Bps::try_from(f64::INFINITY).is_err());
}

// ---------------------------------------------------------------------------
// XIRR: trait irr() delegation to xirr_with_daycount_ctx(Act365F, default ctx)
// ---------------------------------------------------------------------------

#[test]
fn xirr_trait_matches_ctx_helper_on_act365f_default() {
    let flows = [
        (d(2024, 1, 1), -100_000.0),
        (d(2024, 7, 1), 50_000.0),
        (d(2025, 1, 1), 60_000.0),
    ];
    let a = xirr(&flows, None).expect("xirr converges");
    let b = xirr_with_daycount_ctx(&flows, DayCount::Act365F, DayCountContext::default(), None)
        .expect("ctx helper converges");
    assert_eq!(a, b, "XIRR parity mismatch");
}
