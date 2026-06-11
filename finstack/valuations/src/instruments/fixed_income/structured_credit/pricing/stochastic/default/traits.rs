//! Stochastic default trait definition.
//!
//! The [`StochasticDefault`] trait provides a common interface for all
//! default models that incorporate systematic risk factors and correlation.

/// Macroeconomic credit factors affecting default rates.
///
/// These are economy-wide factors that influence default behavior,
/// distinct from individual loan-level `CreditFactors` in the types module.
#[derive(Debug, Clone, Default)]
pub struct MacroCreditFactors {
    /// Unemployment rate (e.g., 0.05 for 5%)
    pub unemployment: f64,
    /// GDP growth rate (e.g., 0.02 for 2%)
    pub gdp_growth: f64,
    /// House price appreciation (e.g., 0.03 for 3%)
    pub hpa: f64,
    /// Credit spread level (e.g., 0.01 for 100bp)
    pub credit_spread: f64,
}

/// Stochastic default model interface.
///
/// Implementations provide conditional default rates given:
/// - Loan seasoning (months since origination)
/// - Systematic factor realizations
/// - Macroeconomic credit factors
///
/// # Mathematical Framework
///
/// General form:
/// ```text
/// MDR(t, Z) = f(base_mdr, Z, credit_factors)
/// ```
///
/// where:
/// - Z is the systematic factor realization(s)
/// - credit_factors include macroeconomic conditions
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
    /// - `macro_factors`: Macroeconomic conditions
    fn conditional_mdr(
        &self,
        seasoning: u32,
        factors: &[f64],
        macro_factors: &MacroCreditFactors,
    ) -> f64;

    /// Asset correlation parameter.
    fn correlation(&self) -> f64;

    /// Model name for diagnostics.
    fn model_name(&self) -> &'static str;

    /// Number of factors used by the model.
    fn num_factors(&self) -> usize {
        1
    }

    /// Expected (unconditional) MDR at given seasoning.
    fn expected_mdr(&self, seasoning: u32) -> f64;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_macro_credit_factors_default() {
        let factors = MacroCreditFactors::default();
        assert_eq!(factors.unemployment, 0.0);
        assert_eq!(factors.gdp_growth, 0.0);
    }
}
