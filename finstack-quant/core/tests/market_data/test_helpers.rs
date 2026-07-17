//! Tests for the surrounding crate component and its documented behavior.
//!
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::BaseCorrelationCurve;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::market_data::term_structures::ForwardCurve;
use finstack_quant_core::market_data::term_structures::HazardCurve;
use finstack_quant_core::market_data::term_structures::InflationCurve;
use finstack_quant_core::math::interp::InterpStyle;
use time::Month;

// ===================================================================
// Test Tolerance Constants (Market Standards Review)
// ===================================================================

/// Tolerance for mathematical roundtrip verification.
/// IEEE 754 double precision has ~15-17 significant decimal digits.
/// 1e-12 provides 3 orders of magnitude safety margin.
#[allow(dead_code)]
pub const MATH_TOLERANCE: f64 = 1e-12;

/// Tolerance for serde roundtrip verification.
#[allow(dead_code)]
pub const SERDE_TOLERANCE: f64 = 1e-12;

/// Tolerance for forward rate continuity checks at knot points.
/// Looser than MATH_TOLERANCE due to numerical differentiation.
#[allow(dead_code)]
pub const CONTINUITY_TOLERANCE: f64 = 1e-4;

// ===================================================================
// Test Fixtures
// ===================================================================

pub(crate) fn sample_base_date() -> Date {
    Date::from_calendar_date(2024, Month::January, 1).unwrap()
}

pub(crate) fn sample_discount_curve(id: &str) -> DiscountCurve {
    DiscountCurve::builder(id)
        .base_date(sample_base_date())
        .day_count(DayCount::Act365F) // Explicit day count convention
        .knots([(0.0, 1.0), (1.0, 0.98), (2.0, 0.96)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap()
}

pub(crate) fn sample_forward_curve(id: &str) -> ForwardCurve {
    ForwardCurve::builder(id, 0.25)
        .base_date(sample_base_date())
        .day_count(DayCount::Act360) // Explicit day count (LIBOR/SOFR convention)
        .knots([(0.0, 0.02), (1.0, 0.021), (2.0, 0.022)])
        .build()
        .unwrap()
}

pub(crate) fn sample_hazard_curve(id: &str) -> HazardCurve {
    HazardCurve::builder(id)
        .base_date(sample_base_date())
        .knots([(1.0, 0.01), (3.0, 0.015), (5.0, 0.02)])
        .build()
        .unwrap()
}

pub(crate) fn sample_inflation_curve(id: &str) -> InflationCurve {
    InflationCurve::builder(id)
        .base_cpi(100.0)
        .base_date(sample_base_date())
        .knots([(0.0, 100.0), (1.0, 102.0), (2.0, 104.0)])
        .build()
        .unwrap()
}

pub(crate) fn sample_base_correlation_curve(id: &str) -> BaseCorrelationCurve {
    BaseCorrelationCurve::builder(id)
        .knots([(3.0, 0.25), (7.0, 0.4), (10.0, 0.55)])
        .build()
        .unwrap()
}

pub(crate) fn sample_vol_surface() -> VolSurface {
    VolSurface::builder("EQ-VOL")
        .expiries(&[0.25, 0.5, 1.0])
        .strikes(&[0.9, 1.0, 1.1])
        .row(&[0.25, 0.24, 0.23])
        .row(&[0.22, 0.21, 0.2])
        .row(&[0.2, 0.19, 0.18])
        .build()
        .unwrap()
}
