use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::math::Compounding;
use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};

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
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("discount curve should build in tests")
}

/// Build a flat forward curve with two knots and a constant rate.
///
/// # Arguments
///
/// * `id` - Curve identifier the market context is keyed by.
/// * `as_of` - Curve base date.
/// * `rate` - Continuous forward rate, as a decimal, applied out to the far knot.
/// * `tenor_years` - Optional standard index tenor (when matching 1M/3M/6M/1Y) and far knot time, in years.
pub fn flat_forward_with_tenor(id: &str, as_of: Date, rate: f64, tenor_years: f64) -> ForwardCurve {
    let index_tenor = if (tenor_years - 0.25).abs() < 1e-6 {
        Some(Tenor::quarterly())
    } else if (tenor_years - 0.5).abs() < 1e-6 {
        Some(Tenor::semi_annual())
    } else if (tenor_years - 1.0).abs() < 1e-6 {
        Some(Tenor::annual())
    } else if (tenor_years - 1.0 / 12.0).abs() < 1e-6 {
        Some(Tenor::monthly())
    } else {
        None
    };
    let far = tenor_years.max(1.0);
    let df_far = Compounding::Continuous
        .df_from_rate(rate, far)
        .expect("finite forward factor");
    ForwardCurve::builder(id)
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .index_tenor_opt(index_tenor)
        .knots([(0.0, 1.0), (far, df_far)])
        .interp(InterpStyle::LogLinear)
        .extrapolation(ExtrapolationPolicy::Extend)
        .build()
        .expect("forward curve should build in tests")
}
