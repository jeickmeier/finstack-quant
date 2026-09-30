//! Embedded models liquidity defaults registry.

use serde::Deserialize;

use super::LiquidityConfig;
use finstack_quant_core::embedded_registry::EmbeddedJsonRegistry;
use finstack_quant_core::{ContractDescriptor, Error, Result};

const LIQUIDITY_DEFAULTS: &str = include_str!("../../data/defaults/liquidity_defaults.v1.json");
pub(crate) const LIQUIDITY_DEFAULTS_CONTRACT: ContractDescriptor =
    ContractDescriptor::new("finstack_quant.models.liquidity_defaults");

static EMBEDDED_LIQUIDITY_DEFAULTS: EmbeddedJsonRegistry<LiquidityDefaults, LiquidityDefaultsFile> =
    EmbeddedJsonRegistry::new(LIQUIDITY_DEFAULTS, None, "liquidity defaults");

/// Registry-backed models liquidity defaults.
#[derive(Debug, Clone)]
pub struct LiquidityDefaults {
    /// Default liquidity configuration.
    pub default_config: LiquidityConfig,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LiquidityDefaultsFile {
    schema: String,
    version: u32,
    default_config: LiquidityConfig,
}

/// Return the embedded liquidity defaults registry.
///
/// The defaults are parsed once and cached process-wide. If the first caller's
/// parse fails, that error is cached and returned by clone on every subsequent
/// call. Embedded JSON is a compile-time asset, so a failure indicates a
/// build-time defect, not a transient condition.
///
/// # Errors
///
/// Returns [`Error::Validation`] when the embedded JSON cannot be decoded or
/// its contract marker, version, or tier thresholds fail validation.
pub(crate) fn embedded_liquidity_defaults() -> Result<&'static LiquidityDefaults> {
    EMBEDDED_LIQUIDITY_DEFAULTS.load(liquidity_defaults_from_file)
}

fn liquidity_defaults_from_file(file: LiquidityDefaultsFile) -> Result<LiquidityDefaults> {
    let schema_version = LIQUIDITY_DEFAULTS_CONTRACT
        .parse_schema(&file.schema)
        .map_err(|error| {
            Error::Validation(format!(
                "invalid liquidity defaults schema {:?}; expected {:?}: {error}",
                file.schema,
                LIQUIDITY_DEFAULTS_CONTRACT.schema_string(),
            ))
        })?;
    let version = LIQUIDITY_DEFAULTS_CONTRACT
        .resolve(Some(file.version))
        .map_err(|error| Error::Validation(error.to_string()))?;
    if schema_version != version {
        return Err(Error::Validation(format!(
            "liquidity defaults schema version {schema_version} does not match version field \
             {version}"
        )));
    }
    file.default_config.validate()?;
    Ok(LiquidityDefaults {
        default_config: file.default_config,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_value(value: serde_json::Value) -> Result<LiquidityDefaults> {
        let file: LiquidityDefaultsFile =
            serde_json::from_value(value).map_err(|error| Error::Validation(error.to_string()))?;
        liquidity_defaults_from_file(file)
    }

    #[test]
    fn liquidity_defaults_reject_missing_zero_future_and_malformed_contract_markers() {
        let base: serde_json::Value =
            serde_json::from_str(LIQUIDITY_DEFAULTS).expect("embedded fixture is JSON");

        let mut missing_schema = base.clone();
        missing_schema
            .as_object_mut()
            .expect("object")
            .remove("schema");
        assert!(load_value(missing_schema).is_err());

        let mut missing_version = base.clone();
        missing_version
            .as_object_mut()
            .expect("object")
            .remove("version");
        assert!(load_value(missing_version).is_err());

        for version in [0, 2] {
            let mut value = base.clone();
            value["version"] = serde_json::json!(version);
            assert!(load_value(value).is_err(), "version {version} must fail");
        }

        for schema in [
            "finstack_quant.models.liquidity_defaults/0",
            "finstack_quant.models.liquidity_defaults/2",
            "finstack_quant.models.liquidity_defaults/not-a-version",
            "finstack_quant.portfolio.liquidity_defaults/1",
            "other.liquidity_defaults/1",
        ] {
            let mut value = base.clone();
            value["schema"] = serde_json::json!(schema);
            assert!(load_value(value).is_err(), "schema {schema} must fail");
        }
    }
}
