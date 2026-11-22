//! Monte Carlo path-count validation, override resolution, and diagnostics helpers.

use crate::metrics::MetricId;

/// Hard ceiling on the number of MC paths a single pricer call may allocate.
pub const MAX_MC_PATHS: usize = 5_000_000;

/// Resolve the effective Monte Carlo path count for a pricer call.
///
/// - If `override_paths` is `Some(0)`, returns a validation error matching
///   `ModelConfig::validate`.
/// - If `override_paths` is `Some(n)` with `0 < n <= MAX_MC_PATHS`, returns `n`.
/// - If `override_paths` is `None`, returns `default` (subject to `<= MAX_MC_PATHS`).
///
/// # Arguments
///
/// * `override_paths` - Optional instrument-level override (`mc_paths`).
/// * `default` - Model fallback path count used when `override_paths` is `None`.
///
/// # Errors
///
/// Returns `Validation` when an explicit override is `0` or the resolved count
/// exceeds the workspace cap.
#[inline]
pub fn resolve_mc_paths(
    override_paths: Option<usize>,
    default: usize,
) -> finstack_quant_core::Result<usize> {
    let n = match override_paths {
        Some(0) => {
            return Err(finstack_quant_core::Error::Validation(
                "Monte Carlo path count must be positive (got 0)".to_string(),
            ));
        }
        Some(n) => n,
        None => default,
    };
    if n > MAX_MC_PATHS {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Monte Carlo path count {n} exceeds workspace cap MAX_MC_PATHS = {MAX_MC_PATHS}; \
             reduce `instrument_pricing_overrides.model_config.mc_paths` or raise the cap."
        )));
    }
    Ok(n)
}

/// Apply per-instrument path count and antithetic overrides to a base
/// `PathDependentPricerConfig`, enforcing the workspace path-count cap.
///
/// # Arguments
///
/// * `base` - Default pricer configuration cloned as the starting point.
/// * `overrides` - Instrument pricing overrides providing optional `mc_paths`
///   and `mc_antithetic`.
///
/// # Errors
///
/// Returns `Validation` when the override is `0` or exceeds the workspace cap.
#[inline]
pub fn merged_path_config(
    base: &finstack_quant_models::monte_carlo::pricer::path_dependent::PathDependentPricerConfig,
    overrides: &crate::instruments::pricing_overrides::InstrumentPricingOverrides,
) -> finstack_quant_core::Result<
    finstack_quant_models::monte_carlo::pricer::path_dependent::PathDependentPricerConfig,
> {
    let mut c = base.clone();
    c.num_paths = resolve_mc_paths(overrides.model_config.mc_paths, c.num_paths)?;
    if let Some(antithetic) = overrides.model_config.mc_antithetic {
        c.antithetic = antithetic;
    }
    Ok(c)
}

/// Publish a Monte Carlo estimate's standard error, 95% confidence bounds and
/// path counts as `mc_*` custom measures on `result`.
///
/// # Arguments
///
/// * `result` - Target valuation result whose `measures` map receives the `mc_*` keys.
/// * `estimate` - Monte Carlo estimate carrying standard error, confidence bounds, and path counts.
pub fn attach_mc_diagnostics(
    result: &mut crate::results::ValuationResult,
    estimate: &finstack_quant_models::monte_carlo::results::MoneyEstimate,
) {
    for (id, value) in [
        ("mc_stderr", estimate.stderr),
        ("mc_ci_95_low", estimate.ci_95.0.amount()),
        ("mc_ci_95_high", estimate.ci_95.1.amount()),
        ("mc_num_paths", estimate.num_paths as f64),
        (
            "mc_num_simulated_paths",
            estimate.num_simulated_paths as f64,
        ),
        ("mc_relative_stderr", estimate.relative_stderr()),
    ] {
        result.measures.insert(MetricId::custom(id), value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_mc_paths_uses_default_when_override_missing() {
        let n = resolve_mc_paths(None, 50_000).expect("default returned");
        assert_eq!(n, 50_000);
    }

    #[test]
    fn resolve_mc_paths_rejects_zero_override() {
        let err = resolve_mc_paths(Some(0), 50_000).expect_err("zero override must error");
        assert!(err.to_string().contains("positive"), "{err}");
    }

    #[test]
    fn resolve_mc_paths_honours_positive_override() {
        let n = resolve_mc_paths(Some(123_456), 50_000).expect("override honoured");
        assert_eq!(n, 123_456);
    }

    #[test]
    fn resolve_mc_paths_rejects_override_above_cap() {
        let too_many = MAX_MC_PATHS + 1;
        let err =
            resolve_mc_paths(Some(too_many), 50_000).expect_err("override above cap must error");
        let msg = err.to_string();
        assert!(msg.contains("MAX_MC_PATHS"));
        assert!(msg.contains(&too_many.to_string()));
    }

    #[test]
    fn resolve_mc_paths_accepts_override_at_cap() {
        let at_cap = MAX_MC_PATHS;
        let n = resolve_mc_paths(Some(at_cap), 50_000).expect("exact cap is allowed");
        assert_eq!(n, at_cap);
    }

    #[test]
    fn resolve_mc_paths_rejects_default_above_cap() {
        let too_many = MAX_MC_PATHS + 1;
        let err = resolve_mc_paths(None, too_many).expect_err("default above cap must error");
        assert!(err.to_string().contains("MAX_MC_PATHS"));
    }
}
