//! WASM bindings for the `finstack-quant-models` crate.
//!
//! Split by model family:
//! - [`analytic`] — closed-form option primitives.
//! - [`fourier`] — COS-method Fourier pricers.
//! - [`volatility`] — volatility models, evaluators, and convention conversion.
//! - [`credit`] — structural-credit model factories.
//! - [`correlation`] — copula, recovery, and joint-probability utilities.
//! - [`monte_carlo`] — stochastic option-pricing convenience functions.
//! - [`liquidity`] — liquidity estimation, risk, and market-impact models.
//! - [`rates`] — interest-rate models and dynamic term-structure engines.

pub mod analytic;
pub mod correlation;
pub mod credit;
pub mod factor;
pub mod fourier;
pub mod liability_management;
pub mod liquidity;
pub mod monte_carlo;
pub mod rates;
pub mod volatility;

/// Convert a JavaScript count or index before the wasm32 ABI can wrap it.
///
/// The explicit 32-bit bound keeps native binding tests faithful to WASM.
///
/// # Arguments
///
/// * `value` - JavaScript number that must be finite, integral, and within the
///   inclusive wasm32 count range `[0, 4294967295]`.
/// * `name` - Caller-visible parameter name included in validation failures.
pub(super) fn parse_usize(value: f64, name: &str) -> Result<usize, wasm_bindgen::JsValue> {
    if !value.is_finite() || value < 0.0 || value.fract() != 0.0 || value > f64::from(u32::MAX) {
        return Err(crate::utils::to_js_err(format!(
            "{name} must be a finite non-negative integer no greater than 4294967295"
        )));
    }
    Ok(value as usize)
}
