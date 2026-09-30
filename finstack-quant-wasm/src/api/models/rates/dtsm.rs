//! WASM bindings for dynamic term-structure models.

use crate::utils::input::js_f64_seq;
use crate::utils::to_js_err;
use finstack_quant_core::Error;
use wasm_bindgen::prelude::*;

/// Evaluate the static Nelson-Siegel (1987) yield curve for one factor triple.
///
/// This is the Diebold-Li cross-sectional equation for a single date:
/// `y(tau) = b1 + b2 * s(tau) + b3 * (s(tau) - exp(-lambda * tau))` with
/// `s(tau) = (1 - exp(-lambda * tau)) / (lambda * tau)`. Returns one yield per
/// tenor, in decimal units and in input order.
/// @param lambda - Exponential decay parameter for tenors in years; must be finite and greater than zero (0.7308 is the years-equivalent of Diebold-Li's 0.0609 months value).
/// @param factors - Nelson-Siegel `[level, slope, curvature]` (beta1, beta2, beta3) in decimal
/// yield units such as `[0.06, -0.02, 0.01]`; exactly three numbers.
/// @param tenors - Maturities in years, each finite and non-negative; output order matches this array.
/// @returns One decimal yield per tenor, in the same order as `tenors`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) if `factors` or `tenors` is
/// not an array of numbers, and a `FinstackError` (`kind: "validation"`) if
/// `factors` does not hold exactly three entries, `lambda` is non-finite or
/// non-positive, any factor loading is non-finite, or any tenor is non-finite
/// or negative.
#[wasm_bindgen(js_name = nelsonSiegelYields)]
pub fn nelson_siegel_yields(
    lambda: f64,
    factors: JsValue,
    tenors: JsValue,
) -> Result<Box<[f64]>, JsValue> {
    let factors = js_f64_seq(&factors, "factors")?;
    let tenors = js_f64_seq(&tenors, "tenors")?;
    // Shape conversion to the Rust `[f64; 3]`, as PyO3 does for Python.
    let factors = <[f64; 3]>::try_from(factors.as_slice()).map_err(|_| {
        to_js_err(Error::Validation(format!(
            "factors must hold exactly 3 entries [level, slope, curvature], got {}",
            factors.len()
        )))
    })?;
    finstack_quant_models::rates::dtsm::nelson_siegel_yields(lambda, factors, &tenors)
        .map(Vec::into_boxed_slice)
        .map_err(to_js_err)
}
