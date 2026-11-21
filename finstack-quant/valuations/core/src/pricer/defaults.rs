//! Embedded product-specific Monte Carlo defaults compiled into the binary.

use finstack_quant_core::contract::ContractDescriptor;
use finstack_quant_core::embedded_registry::EmbeddedJsonRegistry;
use finstack_quant_core::{Error, Result};
use serde::Deserialize;

const DATA: &str = include_str!("../../data/defaults/product_mc_defaults.v1.json");
const CONTRACT: ContractDescriptor =
    ContractDescriptor::new("finstack_quant.valuations.product_mc_defaults");
static DEFAULTS: EmbeddedJsonRegistry<ProductMcDefaults, DefaultsFile> =
    EmbeddedJsonRegistry::new(DATA, None, "product Monte Carlo defaults");

/// Root struct for embedded product-specific Monte Carlo defaults.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductMcDefaults {
    /// Sampling defaults for Merton PIK bonds.
    pub merton_pik_bond: MertonPikBondDefaults,
}

/// Embedded Monte Carlo sampling defaults for Merton PIK bonds.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MertonPikBondDefaults {
    /// Default number of simulated paths.
    pub num_paths: usize,
    /// Whether antithetic variates are enabled by default.
    pub antithetic: bool,
    /// Default time steps per year along the simulation horizon.
    pub steps_per_year: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct DefaultsFile {
    schema: String,
    version: u32,
    defaults: ProductMcDefaults,
}

/// Return the compiled-in product Monte Carlo defaults.
///
/// # Errors
///
/// Returns an error if the embedded JSON asset fails schema parsing or validation.
pub fn embedded_defaults() -> Result<&'static ProductMcDefaults> {
    DEFAULTS.load(|file| {
        CONTRACT
            .parse_schema(&file.schema)
            .map_err(|e| Error::Validation(e.to_string()))?;
        CONTRACT
            .resolve(Some(file.version))
            .map_err(|e| Error::Validation(e.to_string()))?;
        validate_merton_pik_bond("merton_pik_bond", &file.defaults.merton_pik_bond)?;
        Ok(file.defaults)
    })
}

fn validate_merton_pik_bond(label: &str, defaults: &MertonPikBondDefaults) -> Result<()> {
    validate_positive_usize(&format!("{label}.num_paths"), defaults.num_paths)?;
    validate_positive_usize(&format!("{label}.steps_per_year"), defaults.steps_per_year)?;
    Ok(())
}

fn validate_positive_usize(label: &str, value: usize) -> Result<()> {
    if value == 0 {
        return Err(Error::Validation(format!(
            "product Monte Carlo default '{label}' must be positive"
        )));
    }
    Ok(())
}
