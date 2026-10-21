//! Stochastic-process definitions used by the Monte Carlo engine.
//!
//! Start with [`gbm`] for vanilla equity / FX-style simulations and
//! [`brownian`] for additive Gaussian dynamics. This module also exposes
//! Heston, Dupire local volatility, CIR, Hull-White / Vasicek, and
//! Schwartz-Smith models.
//!
//! Important assumptions such as time units, rate / volatility quoting, and
//! state-vector layout are documented in each process module. Use
//! [`metadata::ProcessMetadata`] when captured paths need a stable schema for
//! downstream consumers.

pub mod brownian;
pub mod cheyette_rough;
pub mod cir;
pub mod credit_line;
pub mod gbm;
pub mod gbm_dividends;
pub mod heston;
pub mod lmm;
pub mod local_vol;
pub(crate) mod metadata;
pub mod multi_ou;
pub mod ou;
pub mod piecewise_gbm;
pub mod rough_bergomi;
pub mod rough_heston;
pub mod schwartz_smith;

pub use metadata::ProcessMetadata;

/// Validate a piecewise knot schedule: equal non-empty lengths, finite
/// entries, strictly increasing times and, when `require_non_negative_times`
/// is set, no negative time. `label` names the schedule in error messages.
pub(crate) fn validate_piecewise_knots(
    times: &[f64],
    values: &[f64],
    require_non_negative_times: bool,
    label: &str,
) -> finstack_quant_core::Result<()> {
    if times.is_empty() || times.len() != values.len() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{label} curve and times must be non-empty and equally sized, got {} and {}",
            values.len(),
            times.len()
        )));
    }
    for (index, (&time, &value)) in times.iter().zip(values).enumerate() {
        if !time.is_finite() || (require_non_negative_times && time < 0.0) || !value.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "invalid {label} knot at index {index}: time={time}, value={value}"
            )));
        }
        if index > 0 && time <= times[index - 1] {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{label} times must increase strictly, got {time} after {} at index {index}",
                times[index - 1]
            )));
        }
    }
    Ok(())
}

/// Evaluate a validated left-continuous piecewise-constant curve at `t`.
#[inline]
pub(crate) fn piecewise_constant_at(times: &[f64], values: &[f64], t: f64) -> f64 {
    values[times.partition_point(|&knot| knot <= t).saturating_sub(1)]
}
