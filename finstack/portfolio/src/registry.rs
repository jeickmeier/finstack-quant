//! Embedded portfolio defaults registries.

use std::sync::OnceLock;

use serde::Deserialize;

use crate::liquidity::LiquidityConfig;
use finstack_core::{Error, Result};

const LIQUIDITY_DEFAULTS: &str = include_str!("../data/defaults/liquidity_defaults.v1.json");

static EMBEDDED_LIQUIDITY_DEFAULTS: OnceLock<Result<LiquidityDefaults>> = OnceLock::new();

/// Registry-backed portfolio liquidity defaults.
#[derive(Debug, Clone)]
pub struct LiquidityDefaults {
    /// Default liquidity configuration.
    pub default_config: LiquidityConfig,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LiquidityDefaultsFile {
    schema: Option<String>,
    version: Option<u32>,
    default_config: LiquidityConfig,
}

/// Return the embedded liquidity defaults registry.
///
/// The defaults are parsed once and cached in a process-wide `OnceLock`. If
/// the first caller's parse fails, that error is cached: every subsequent
/// call returns a clone of the *same* error rather than re-parsing. The parse
/// is therefore deterministic — embedded JSON is a compile-time asset — so a
/// failure indicates a build-time defect, not a transient condition.
pub fn embedded_liquidity_defaults() -> Result<&'static LiquidityDefaults> {
    match EMBEDDED_LIQUIDITY_DEFAULTS.get_or_init(parse_liquidity_defaults) {
        Ok(defaults) => Ok(defaults),
        Err(err) => Err(err.clone()),
    }
}

/// Panic-on-failure access for `Default` implementations backed by embedded data.
///
/// Panics only if the embedded liquidity defaults fail to parse. Because the
/// parse result is cached in a `OnceLock`, this is deterministic: it panics
/// on the first call or never. The embedded JSON is a compile-time asset, so
/// a panic here means the binary itself is malformed.
#[must_use]
#[allow(clippy::expect_used)]
pub fn embedded_liquidity_defaults_or_panic() -> &'static LiquidityDefaults {
    embedded_liquidity_defaults()
        .expect("embedded portfolio liquidity defaults are compile-time assets")
}

fn parse_liquidity_defaults() -> Result<LiquidityDefaults> {
    let file: LiquidityDefaultsFile = serde_json::from_str(LIQUIDITY_DEFAULTS).map_err(|err| {
        Error::Validation(format!(
            "failed to parse embedded liquidity defaults: {err}"
        ))
    })?;
    liquidity_defaults_from_file(file)
}

fn liquidity_defaults_from_file(file: LiquidityDefaultsFile) -> Result<LiquidityDefaults> {
    let _schema = &file.schema;
    let _version = file.version;
    validate_liquidity_config(&file.default_config)?;
    Ok(LiquidityDefaults {
        default_config: file.default_config,
    })
}

fn validate_liquidity_config(config: &LiquidityConfig) -> Result<()> {
    validate_positive("liquidity.participation_rate", config.participation_rate)?;
    validate_positive("liquidity.risk_aversion", config.risk_aversion)?;
    validate_positive("liquidity.holding_period", config.holding_period)?;
    validate_unit_interval("liquidity.confidence_level", config.confidence_level)?;
    validate_non_negative(
        "liquidity.endogenous_spread_coef",
        config.endogenous_spread_coef,
    )?;
    let mut prev = 0.0;
    for (idx, threshold) in config.tier_thresholds.iter().copied().enumerate() {
        validate_positive(&format!("liquidity.tier_thresholds[{idx}]"), threshold)?;
        if threshold <= prev {
            return Err(Error::Validation(format!(
                "liquidity tier threshold {idx} must be greater than prior threshold"
            )));
        }
        prev = threshold;
    }
    Ok(())
}

fn validate_positive(label: &str, value: f64) -> Result<()> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(Error::Validation(format!("{label} must be positive")))
    }
}

fn validate_non_negative(label: &str, value: f64) -> Result<()> {
    if value.is_finite() && value >= 0.0 {
        Ok(())
    } else {
        Err(Error::Validation(format!("{label} must be non-negative")))
    }
}

fn validate_unit_interval(label: &str, value: f64) -> Result<()> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(Error::Validation(format!("{label} must be in [0, 1]")))
    }
}
