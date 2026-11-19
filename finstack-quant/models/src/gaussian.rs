//! Normal evaluation for scalar model kernels whose specified failure value is NaN.
//! Fallible pricing and calibration paths use the checked core functions directly.

use finstack_quant_core::math::special_functions;

#[inline]
pub(crate) fn norm_cdf_or_nan(x: f64) -> f64 {
    if x.is_nan() {
        f64::NAN
    } else {
        special_functions::norm_cdf_unchecked(x)
    }
}

#[inline]
pub(crate) fn norm_pdf_or_nan(x: f64) -> f64 {
    if x.is_nan() {
        f64::NAN
    } else {
        special_functions::norm_pdf_unchecked(x)
    }
}

#[inline]
pub(crate) fn log_norm_cdf_or_nan(x: f64) -> f64 {
    special_functions::log_norm_cdf(x).unwrap_or(f64::NAN)
}
