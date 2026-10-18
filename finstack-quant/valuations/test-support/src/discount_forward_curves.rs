use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};

/// Build a flat discount curve with two knots: (0, 1.0) and (1y, exp(-rate)).
///
/// # Arguments
///
/// * `id` - Curve identifier the market context is keyed by.
/// * `as_of` - Curve base date.
/// * `rate` - Continuously compounded zero rate, as a decimal, applied over the one-year knot.
pub fn flat_discount(id: &str, as_of: Date, rate: f64) -> DiscountCurve {
    flat_discount_with_tenor(id, as_of, rate, 1.0)
}

/// Build a flat discount curve with a configurable far-tenor knot.
///
/// # Arguments
///
/// * `id` - Curve identifier the market context is keyed by.
/// * `as_of` - Curve base date.
/// * `rate` - Continuously compounded zero rate, as a decimal; the far knot is `exp(-rate * tenor_years)`.
/// * `tenor_years` - Time of the far knot in years from `as_of`.
pub fn flat_discount_with_tenor(
    id: &str,
    as_of: Date,
    rate: f64,
    tenor_years: f64,
) -> DiscountCurve {
    DiscountCurve::builder(id)
        .base_date(as_of)
        .knots([(0.0, 1.0), (tenor_years, (-rate * tenor_years).exp())])
        .build()
        .expect("discount curve should build in tests")
}

/// Build a flat forward curve with two knots and a constant rate.
///
/// # Arguments
///
/// * `id` - Curve identifier the market context is keyed by.
/// * `as_of` - Curve base date.
/// * `rate` - Simple forward rate, as a decimal, written at both knots.
/// * `tenor_years` - Both the index tenor of the forward curve and the time of the far knot, in years.
pub fn flat_forward_with_tenor(id: &str, as_of: Date, rate: f64, tenor_years: f64) -> ForwardCurve {
    ForwardCurve::builder(id, tenor_years)
        .base_date(as_of)
        .knots([(0.0, rate), (tenor_years, rate)])
        .build()
        .expect("forward curve should build in tests")
}
