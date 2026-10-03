//! Embedded product-specific Monte Carlo defaults owned by valuations.
use finstack_quant_core::embedded_registry::EmbeddedJsonRegistry;
use finstack_quant_core::{ContractDescriptor, Error, Result};
use serde::Deserialize;
const DATA: &str = include_str!("../../data/defaults/product_mc_defaults.v1.json");
const CONTRACT: ContractDescriptor =
    ContractDescriptor::new("finstack_quant.valuations.product_mc_defaults");
static DEFAULTS: EmbeddedJsonRegistry<ProductMcDefaults, DefaultsFile> =
    EmbeddedJsonRegistry::new(DATA, None, "product Monte Carlo defaults");
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProductMcDefaults {
    /// Shared rate-exotic Monte Carlo defaults.
    pub(crate) rate_exotics: RateExoticDefaults,
    /// LMM Bermudan swaption defaults.
    pub(crate) lmm_bermudan: LmmBermudanDefaults,
    /// Merton PIK-bond Monte Carlo defaults.
    pub(crate) merton_pik_bond: MertonPikBondDefaults,
    /// Stochastic revolving-credit Monte Carlo defaults.
    pub(crate) revolving_credit: RevolvingCreditDefaults,
}
/// Shared rate-exotic Monte Carlo defaults.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RateExoticDefaults {
    /// Number of Monte Carlo paths.
    pub(crate) num_paths: usize,
    /// Root RNG seed.
    pub(crate) seed: u64,
    /// Whether antithetic variance reduction is enabled by default.
    pub(crate) antithetic: bool,
    /// Minimum number of simulation sub-steps between events.
    pub(crate) min_steps_between_events: usize,
    /// Polynomial basis degree for LSMC regression.
    pub(crate) basis_degree: usize,
}

/// LMM Bermudan swaption defaults.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LmmBermudanDefaults {
    /// Number of Monte Carlo paths.
    pub(crate) num_paths: usize,
    /// Root RNG seed.
    pub(crate) seed: u64,
    /// Polynomial basis degree for LSMC regression.
    pub(crate) basis_degree: usize,
    /// Whether antithetic variance reduction is enabled by default.
    pub(crate) antithetic: bool,
    /// Minimum simulation steps between exercise dates.
    pub(crate) min_steps_between_exercises: usize,
}

/// Merton PIK-bond Monte Carlo defaults.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MertonPikBondDefaults {
    /// Number of independent Monte Carlo estimators (antithetic sampling
    /// simulates two paths per estimator). The seed is derived per instrument
    /// from `model_config.mc_seed_scenario`.
    pub(crate) num_paths: usize,
    /// Whether antithetic variance reduction is enabled by default.
    pub(crate) antithetic: bool,
    /// Simulation time steps per year.
    pub(crate) steps_per_year: usize,
}

/// Stochastic revolving-credit Monte Carlo defaults.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RevolvingCreditDefaults {
    /// Number of independent Monte Carlo estimators (antithetic sampling
    /// simulates two paths per estimator). The seed is derived per facility
    /// from `model_config.mc_seed_scenario`.
    pub(crate) num_paths: usize,
    /// Whether antithetic variance reduction is enabled by default. Must stay
    /// `false` while Sobol facilities (`use_sobol_qmc`) rely on the default,
    /// because the two are mutually exclusive.
    pub(crate) antithetic: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DefaultsFile {
    schema: String,
    version: u32,
    defaults: ProductMcDefaults,
}
pub(crate) fn embedded_defaults() -> Result<&'static ProductMcDefaults> {
    DEFAULTS.load(|file| {
        let schema_version = CONTRACT
            .parse_schema(&file.schema)
            .map_err(|e| Error::Validation(e.to_string()))?;
        let version = CONTRACT
            .resolve(Some(file.version))
            .map_err(|e| Error::Validation(e.to_string()))?;
        if schema_version != version {
            return Err(Error::Validation(
                "product Monte Carlo defaults schema and version disagree".into(),
            ));
        }
        validate_rate_exotics("rate_exotics", &file.defaults.rate_exotics)?;
        validate_lmm_bermudan("lmm_bermudan", &file.defaults.lmm_bermudan)?;
        validate_merton_pik_bond("merton_pik_bond", &file.defaults.merton_pik_bond)?;
        validate_revolving_credit("revolving_credit", &file.defaults.revolving_credit)?;

        Ok(file.defaults)
    })
}
#[allow(clippy::expect_used)]
pub(crate) fn embedded_defaults_or_panic() -> &'static ProductMcDefaults {
    embedded_defaults().expect("embedded product Monte Carlo defaults are compile-time assets")
}
fn validate_rate_exotics(label: &str, defaults: &RateExoticDefaults) -> Result<()> {
    validate_positive_usize(&format!("{label}.num_paths"), defaults.num_paths)?;
    validate_positive_usize(
        &format!("{label}.min_steps_between_events"),
        defaults.min_steps_between_events,
    )?;
    validate_positive_usize(&format!("{label}.basis_degree"), defaults.basis_degree)?;
    let _seed = defaults.seed;
    let _antithetic = defaults.antithetic;
    Ok(())
}

fn validate_lmm_bermudan(label: &str, defaults: &LmmBermudanDefaults) -> Result<()> {
    validate_positive_usize(&format!("{label}.num_paths"), defaults.num_paths)?;
    validate_positive_usize(&format!("{label}.basis_degree"), defaults.basis_degree)?;
    validate_positive_usize(
        &format!("{label}.min_steps_between_exercises"),
        defaults.min_steps_between_exercises,
    )?;
    let _seed = defaults.seed;
    let _antithetic = defaults.antithetic;
    Ok(())
}

fn validate_merton_pik_bond(label: &str, defaults: &MertonPikBondDefaults) -> Result<()> {
    validate_positive_usize(&format!("{label}.num_paths"), defaults.num_paths)?;
    validate_positive_usize(&format!("{label}.steps_per_year"), defaults.steps_per_year)?;
    let _antithetic = defaults.antithetic;
    Ok(())
}

fn validate_revolving_credit(label: &str, defaults: &RevolvingCreditDefaults) -> Result<()> {
    if defaults.num_paths < 2 {
        return Err(Error::Validation(format!(
            "{label}.num_paths must be at least 2, got {}",
            defaults.num_paths
        )));
    }
    let _antithetic = defaults.antithetic;
    Ok(())
}

fn validate_positive_usize(label: &str, value: usize) -> Result<()> {
    if value == 0 {
        return Err(Error::Validation(format!("{label} must be positive")));
    }
    Ok(())
}
