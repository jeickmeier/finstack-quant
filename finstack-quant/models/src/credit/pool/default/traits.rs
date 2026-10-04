//! Stochastic default trait definition.
//!
//! The [`StochasticDefault`] trait provides a common interface for all
//! default models that incorporate systematic risk factors and correlation.

/// Stochastic default model interface.
///
/// Implementations provide conditional default rates given:
/// - Loan seasoning (months since origination)
/// - Systematic factor realizations
///
/// # Mathematical Framework
///
/// General form:
/// ```text
/// MDR(t, Z) = f(base_mdr, Z)
/// ```
///
/// where Z is the systematic factor realization(s).
///
/// # Sign Convention
///
/// All implementations follow the canonical copula convention: a LOW
/// systematic factor realization (`Z < 0`) is the stress state, i.e.
/// `conditional_mdr` is non-increasing in `Z`. Recovery models share the
/// same factor, so a positive recovery `factor_correlation` makes
/// recoveries fall in stress — defaults and recoveries co-move negatively
/// in every engine.
pub trait StochasticDefault: Send + Sync + std::fmt::Debug {
    /// Conditional MDR (monthly default rate) given factor realizations.
    ///
    /// Returns the monthly default rate conditional on:
    /// - `seasoning`: Months since origination
    /// - `factors`: Systematic factor values [credit_factor, ...]
    fn conditional_mdr(&self, seasoning: u32, factors: &[f64]) -> f64;

    /// Asset correlation parameter.
    fn correlation(&self) -> f64;

    /// Expected (unconditional) MDR at given seasoning.
    fn expected_mdr(&self, seasoning: u32) -> f64;
}
