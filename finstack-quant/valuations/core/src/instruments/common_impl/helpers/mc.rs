//! Monte Carlo path-count validation and override resolution helpers.

/// Hard ceiling on the number of MC paths a single pricer call may allocate.
pub(super) const MAX_MC_PATHS: usize = 5_000_000;

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
