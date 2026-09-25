//! Embedded Monte Carlo defaults registry.
//!
//! Runtime defaults are versioned JSON data so path counts, seeds, parallel
//! defaults, and convenience-layer defaults can be reviewed and changed
//! without hunting through constructor bodies.

use finstack_quant_core::embedded_registry::EmbeddedJsonRegistry;
use finstack_quant_core::{ContractDescriptor, Error, Result};
use serde::Deserialize;

const PRICER_DEFAULTS: &str = include_str!("../../data/defaults/pricer_defaults.v1.json");
const MONTE_CARLO_DEFAULTS_CONTRACT: ContractDescriptor =
    ContractDescriptor::new("finstack_quant.models.monte_carlo_defaults");

static EMBEDDED_DEFAULTS: EmbeddedJsonRegistry<MonteCarloDefaults, DefaultsFile> =
    EmbeddedJsonRegistry::new(PRICER_DEFAULTS, None, "Monte Carlo defaults");

/// Resolved Monte Carlo defaults.
#[derive(Debug, Clone, Deserialize)]
pub struct MonteCarloDefaults {
    /// Rust API defaults.
    pub rust: RustDefaults,
    /// Convenience-layer defaults shared by hosts and Rust convenience pricers.
    pub convenience: ConvenienceDefaults,
}

/// Defaults used by Rust Monte Carlo APIs.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RustDefaults {
    /// Generic engine defaults.
    pub engine: EngineDefaults,
    /// European pricer defaults.
    pub european_pricer: PricerRuntimeDefaults,
    /// Path-dependent pricer defaults.
    pub path_dependent_pricer: PathDependentPricerDefaults,
    /// LSMC configuration defaults.
    pub lsmc: LsmcRuntimeDefaults,
    /// Shared rate-exotic Monte Carlo defaults.
    pub rate_exotics: RateExoticDefaults,
    /// Swaption LSMC defaults.
    pub swaption_lsmc: SwaptionLsmcDefaults,
    /// LMM Bermudan swaption defaults.
    pub lmm_bermudan: LmmBermudanDefaults,
    /// Cheyette rough-vol Bermudan swaption defaults.
    pub cheyette_rough: CheyetteRoughDefaults,
    /// Merton PIK-bond Monte Carlo defaults.
    pub merton_pik_bond: MertonPikBondDefaults,
    /// Stochastic revolving-credit Monte Carlo defaults.
    pub revolving_credit: RevolvingCreditDefaults,
}

/// Defaults shared by the host bindings and the Rust convenience pricers.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConvenienceDefaults {
    /// Default currency code for convenience entry points.
    pub default_currency: String,
    /// European pricer defaults.
    pub european_pricer: ConveniencePricerDefaults,
    /// Path-dependent pricer defaults.
    pub path_dependent_pricer: ConveniencePricerDefaults,
    /// LSMC pricer defaults.
    pub lsmc: ConvenienceLsmcDefaults,
    /// Greek estimator defaults.
    pub greeks: ConvenienceGreekDefaults,
}

/// Common path-count, seed, and parallel-execution defaults.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PricerRuntimeDefaults {
    /// Number of Monte Carlo paths.
    pub num_paths: usize,
    /// Root RNG seed.
    pub seed: u64,
    /// Whether parallel execution is requested by default.
    pub use_parallel: bool,
}

/// Generic engine runtime defaults.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineDefaults {
    /// Whether parallel execution is requested by default.
    pub use_parallel: bool,
    /// Whether antithetic variance reduction is enabled by default.
    pub antithetic: bool,
}

/// Path-dependent pricer runtime defaults.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PathDependentPricerDefaults {
    /// Number of Monte Carlo paths.
    pub num_paths: usize,
    /// Root RNG seed.
    pub seed: u64,
    /// Whether parallel execution is requested by default.
    pub use_parallel: bool,
    /// Parallel chunk size.
    pub chunk_size: usize,
    /// Steps per year for automatic time-grid construction.
    pub steps_per_year: f64,
    /// Minimum number of time-grid steps.
    pub min_steps: usize,
    /// Whether Sobol QMC is enabled by default.
    pub use_sobol: bool,
    /// Whether antithetic variance reduction is enabled by default.
    pub antithetic: bool,
    /// Whether Brownian-bridge ordering is enabled by default.
    pub use_brownian_bridge: bool,
}

/// LSMC runtime defaults.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LsmcRuntimeDefaults {
    /// Root RNG seed.
    pub seed: u64,
    /// Whether parallel execution is requested by default.
    pub use_parallel: bool,
    /// Whether antithetic variance reduction is enabled by default.
    pub antithetic: bool,
}

/// Shared rate-exotic Monte Carlo defaults.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RateExoticDefaults {
    /// Number of Monte Carlo paths.
    pub num_paths: usize,
    /// Root RNG seed.
    pub seed: u64,
    /// Whether antithetic variance reduction is enabled by default.
    pub antithetic: bool,
    /// Minimum number of simulation sub-steps between events.
    pub min_steps_between_events: usize,
    /// Polynomial basis degree for LSMC regression.
    pub basis_degree: usize,
}

/// Swaption LSMC defaults.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SwaptionLsmcDefaults {
    /// Number of Monte Carlo paths.
    pub num_paths: usize,
    /// Root RNG seed.
    pub seed: u64,
    /// Polynomial basis degree for LSMC regression.
    pub basis_degree: usize,
    /// Whether antithetic variance reduction is enabled by default.
    pub antithetic: bool,
}

/// LMM Bermudan swaption defaults.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LmmBermudanDefaults {
    /// Number of Monte Carlo paths.
    pub num_paths: usize,
    /// Root RNG seed.
    pub seed: u64,
    /// Polynomial basis degree for LSMC regression.
    pub basis_degree: usize,
    /// Whether antithetic variance reduction is enabled by default.
    pub antithetic: bool,
    /// Minimum simulation steps between exercise dates.
    pub min_steps_between_exercises: usize,
}

/// Cheyette rough-vol Bermudan swaption defaults.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheyetteRoughDefaults {
    /// Number of Monte Carlo paths.
    pub num_paths: usize,
    /// Number of simulation time steps.
    pub num_steps: usize,
    /// Polynomial basis degree for LSMC regression.
    pub basis_degree: usize,
}

/// Merton PIK-bond Monte Carlo defaults.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MertonPikBondDefaults {
    /// Number of independent Monte Carlo estimators (antithetic sampling
    /// simulates two paths per estimator). The seed is derived per instrument
    /// from `model_config.mc_seed_scenario`.
    pub num_paths: usize,
    /// Whether antithetic variance reduction is enabled by default.
    pub antithetic: bool,
    /// Simulation time steps per year.
    pub steps_per_year: usize,
}

/// Stochastic revolving-credit Monte Carlo defaults.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevolvingCreditDefaults {
    /// Number of independent Monte Carlo estimators (antithetic sampling
    /// simulates two paths per estimator). The seed is derived per facility
    /// from `model_config.mc_seed_scenario`.
    pub num_paths: usize,
    /// Whether antithetic variance reduction is enabled by default. Must stay
    /// `false` while Sobol facilities (`use_sobol_qmc`) rely on the default,
    /// because the two are mutually exclusive.
    pub antithetic: bool,
}

/// Pricer defaults with default time-grid step count.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConveniencePricerDefaults {
    /// Number of Monte Carlo paths.
    pub num_paths: usize,
    /// Root RNG seed.
    pub seed: u64,
    /// Whether parallel execution is requested by default.
    pub use_parallel: bool,
    /// Default number of time steps for GBM convenience pricing methods.
    pub num_steps: usize,
}

/// LSMC pricer defaults.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConvenienceLsmcDefaults {
    /// Number of Monte Carlo paths.
    pub num_paths: usize,
    /// Root RNG seed.
    pub seed: u64,
    /// Whether parallel execution is requested by default.
    pub use_parallel: bool,
    /// Default regression basis name.
    pub basis: String,
    /// Default regression basis degree.
    pub basis_degree: usize,
    /// Default number of time steps for American option methods.
    pub num_steps: usize,
    /// Whether antithetic variance reduction is enabled by default.
    pub antithetic: bool,
}

/// Finite-difference Greek estimator defaults.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConvenienceGreekDefaults {
    /// Number of Monte Carlo paths.
    pub num_paths: usize,
    /// Root RNG seed.
    pub seed: u64,
    /// Default number of time steps.
    pub num_steps: usize,
    /// Relative spot bump used as an MC finite-difference shock (e.g. `0.01`
    /// for a 1% of spot bump). This is a Monte Carlo differentiation step, not
    /// a closed-form local Greek step and not a desk reporting shock.
    pub bump_size: f64,
    /// Whether parallel execution is requested by default.
    pub use_parallel: bool,
    /// Parallel chunk size.
    pub chunk_size: usize,
    /// Whether antithetic variance reduction is enabled by default.
    pub antithetic: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DefaultsFile {
    schema: String,
    version: u32,
    rust: RustDefaults,
    convenience: ConvenienceDefaults,
}

/// Return the validated, process-wide Monte Carlo defaults embedded in the crate.
///
/// The defaults are parsed from the compile-time `PRICER_DEFAULTS` asset on the
/// first call and then cached. The returned reference is shared and must be
/// treated as read-only; callers that need to modify settings should clone it.
///
/// # Errors
///
/// Returns an error when the embedded JSON cannot be deserialized or when its
/// numeric and structural defaults fail the same validation applied to external
/// configuration. A cached initialization error is cloned for later calls.
pub fn embedded_defaults() -> Result<&'static MonteCarloDefaults> {
    EMBEDDED_DEFAULTS.load(defaults_from_file)
}

/// Panic-on-failure access for `Default` implementations backed by embedded data.
#[must_use]
#[allow(clippy::expect_used)]
pub fn embedded_defaults_or_panic() -> &'static MonteCarloDefaults {
    embedded_defaults().expect("embedded Monte Carlo defaults are compile-time assets")
}

fn defaults_from_file(file: DefaultsFile) -> Result<MonteCarloDefaults> {
    validate_file(&file)?;
    Ok(MonteCarloDefaults {
        rust: file.rust,
        convenience: file.convenience,
    })
}

fn validate_file(file: &DefaultsFile) -> Result<()> {
    let schema_version = MONTE_CARLO_DEFAULTS_CONTRACT
        .parse_schema(&file.schema)
        .map_err(|error| Error::Validation(error.to_string()))?;
    let version = MONTE_CARLO_DEFAULTS_CONTRACT
        .resolve(Some(file.version))
        .map_err(|error| Error::Validation(error.to_string()))?;
    if schema_version != version {
        return Err(Error::Validation(format!(
            "Monte Carlo defaults schema version {schema_version} does not match version {version}"
        )));
    }
    validate_runtime("rust.european_pricer", &file.rust.european_pricer)?;
    validate_runtime(
        "rust.path_dependent_pricer",
        &PricerRuntimeDefaults {
            num_paths: file.rust.path_dependent_pricer.num_paths,
            seed: file.rust.path_dependent_pricer.seed,
            use_parallel: file.rust.path_dependent_pricer.use_parallel,
        },
    )?;
    validate_engine("rust.engine", &file.rust.engine)?;
    validate_positive_usize(
        "rust.path_dependent_pricer.chunk_size",
        file.rust.path_dependent_pricer.chunk_size,
    )?;
    validate_positive_f64(
        "rust.path_dependent_pricer.steps_per_year",
        file.rust.path_dependent_pricer.steps_per_year,
    )?;
    validate_positive_usize(
        "rust.path_dependent_pricer.min_steps",
        file.rust.path_dependent_pricer.min_steps,
    )?;
    validate_convenience_pricer(
        "convenience.european_pricer",
        &file.convenience.european_pricer,
    )?;
    validate_nonblank(
        "convenience.default_currency",
        &file.convenience.default_currency,
    )?;
    validate_convenience_pricer(
        "convenience.path_dependent_pricer",
        &file.convenience.path_dependent_pricer,
    )?;
    validate_rate_exotics("rust.rate_exotics", &file.rust.rate_exotics)?;
    validate_swaption_lsmc("rust.swaption_lsmc", &file.rust.swaption_lsmc)?;
    validate_lmm_bermudan("rust.lmm_bermudan", &file.rust.lmm_bermudan)?;
    validate_cheyette_rough("rust.cheyette_rough", &file.rust.cheyette_rough)?;
    validate_merton_pik_bond("rust.merton_pik_bond", &file.rust.merton_pik_bond)?;
    validate_revolving_credit("rust.revolving_credit", &file.rust.revolving_credit)?;
    validate_python_lsmc("convenience.lsmc", &file.convenience.lsmc)?;
    validate_python_greeks("convenience.greeks", &file.convenience.greeks)?;
    Ok(())
}

fn validate_runtime(label: &str, defaults: &PricerRuntimeDefaults) -> Result<()> {
    validate_positive_usize(&format!("{label}.num_paths"), defaults.num_paths)?;
    let _seed = defaults.seed;
    let _parallel = defaults.use_parallel;
    Ok(())
}

fn validate_engine(_label: &str, defaults: &EngineDefaults) -> Result<()> {
    let _parallel = defaults.use_parallel;
    let _antithetic = defaults.antithetic;
    Ok(())
}

fn validate_convenience_pricer(label: &str, defaults: &ConveniencePricerDefaults) -> Result<()> {
    validate_positive_usize(&format!("{label}.num_paths"), defaults.num_paths)?;
    validate_positive_usize(&format!("{label}.num_steps"), defaults.num_steps)?;
    let _seed = defaults.seed;
    let _parallel = defaults.use_parallel;
    Ok(())
}

fn validate_python_lsmc(label: &str, defaults: &ConvenienceLsmcDefaults) -> Result<()> {
    validate_positive_usize(&format!("{label}.num_paths"), defaults.num_paths)?;
    validate_positive_usize(&format!("{label}.basis_degree"), defaults.basis_degree)?;
    validate_positive_usize(&format!("{label}.num_steps"), defaults.num_steps)?;
    if defaults.basis.trim().is_empty() {
        return Err(Error::Validation(format!(
            "{label}.basis must not be blank"
        )));
    }
    let _seed = defaults.seed;
    let _parallel = defaults.use_parallel;
    let _antithetic = defaults.antithetic;
    Ok(())
}

fn validate_python_greeks(label: &str, defaults: &ConvenienceGreekDefaults) -> Result<()> {
    validate_positive_usize(&format!("{label}.num_paths"), defaults.num_paths)?;
    validate_positive_usize(&format!("{label}.num_steps"), defaults.num_steps)?;
    validate_positive_f64(&format!("{label}.bump_size"), defaults.bump_size)?;
    validate_positive_usize(&format!("{label}.chunk_size"), defaults.chunk_size)?;
    let _seed = defaults.seed;
    let _parallel = defaults.use_parallel;
    let _antithetic = defaults.antithetic;
    Ok(())
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

fn validate_swaption_lsmc(label: &str, defaults: &SwaptionLsmcDefaults) -> Result<()> {
    validate_positive_usize(&format!("{label}.num_paths"), defaults.num_paths)?;
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

fn validate_cheyette_rough(label: &str, defaults: &CheyetteRoughDefaults) -> Result<()> {
    validate_positive_usize(&format!("{label}.num_paths"), defaults.num_paths)?;
    validate_positive_usize(&format!("{label}.num_steps"), defaults.num_steps)?;
    validate_positive_usize(&format!("{label}.basis_degree"), defaults.basis_degree)
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

fn validate_positive_f64(label: &str, value: f64) -> Result<()> {
    if !value.is_finite() || value <= 0.0 {
        return Err(Error::Validation(format!("{label} must be positive")));
    }
    Ok(())
}

fn validate_nonblank(label: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(Error::Validation(format!("{label} must not be blank")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_contract_marker_is_required_and_versioned() {
        let mut value: serde_json::Value =
            serde_json::from_str(PRICER_DEFAULTS).expect("embedded defaults JSON");
        value["schema"] = serde_json::json!("finstack_quant.models.monte_carlo_defaults/2");
        let file: DefaultsFile =
            serde_json::from_value(value).expect("structurally valid defaults");
        assert!(defaults_from_file(file).is_err());

        let mut missing: serde_json::Value =
            serde_json::from_str(PRICER_DEFAULTS).expect("embedded defaults JSON");
        missing.as_object_mut().expect("object").remove("schema");
        assert!(serde_json::from_value::<DefaultsFile>(missing).is_err());
    }
}
