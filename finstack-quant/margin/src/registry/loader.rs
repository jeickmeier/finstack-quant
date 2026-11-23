//! Embedded margin registry loading, JSON overlay merging, and parsing.

use std::fmt::Display;
use std::str::FromStr;
use std::sync::OnceLock;

use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::{Error, FxHashMap, Result};
use serde_json::Value;
use tracing::{debug, info};

use super::simm_validation::validate_simm_params;
use super::types::{
    AssetClassDefault, CcpParams, MarginDefaults, MarginRegistry, ParsedCcpRegistry,
    ScheduleImSchedule, SimmParams, VmDefaults,
};
use super::validation::{to_validation, validate_haircut, validate_non_negative};
use super::{embedded, merge, wire};
use crate::SimmVersion;
use crate::calculators::im::schedule::MaturityBucket;
use crate::types::{
    CollateralAssetClass, CollateralEligibility, EligibleCollateralSchedule, SimmCreditSector,
    SimmRiskClass, ordered_pair, ordered_tenor_pair,
};

static EMBEDDED_REGISTRY: OnceLock<MarginRegistry> = OnceLock::new();

/// Access the embedded registry without any overlays.
///
/// The result is cached for the lifetime of the process.
///
/// # Returns
///
/// A shared reference to the parsed embedded registry.
///
/// # Errors
///
/// Returns an error if the embedded JSON assets cannot be parsed into a valid
/// [`MarginRegistry`].
pub(crate) fn embedded_registry() -> Result<&'static MarginRegistry> {
    if EMBEDDED_REGISTRY.get().is_none() {
        let registry = build_registry(None)?;
        let _ = EMBEDDED_REGISTRY.set(registry);
    }
    EMBEDDED_REGISTRY
        .get()
        .ok_or_else(|| Error::Validation("Failed to load embedded margin registry".to_string()))
}

/// Panic-on-failure access to the embedded registry for infallible constructors.
///
/// Use [`embedded_registry`] when the caller can surface a runtime error.
#[must_use]
#[allow(clippy::expect_used)]
pub(crate) fn embedded_registry_or_panic() -> &'static MarginRegistry {
    embedded_registry().expect("embedded margin registry is a compile-time asset")
}

/// Build a registry applying an optional JSON overlay.
///
/// # Arguments
///
/// * `overlay` - Parsed JSON overlay that follows the embedded registry schema
///
/// # Returns
///
/// A fully parsed [`MarginRegistry`] containing embedded defaults plus any overlay values.
///
/// # Errors
///
/// Returns an error if the embedded assets are invalid, if the overlay is
/// malformed, or if any parsed values violate registry validation rules.
pub(crate) fn build_registry(overlay: Option<&Value>) -> Result<MarginRegistry> {
    let mut root = embedded::load_embedded_root()?;
    if let Some(ov) = overlay {
        debug!("Applying margin registry overlay");
        merge::merge_json(&mut root, ov);
    }

    let schedule_im = parse_schedule_im(root.get("schedule_im"))?;
    let (collateral_defaults, collateral_schedules) =
        parse_collateral_schedules(root.get("collateral_schedules"))?;
    let defaults = parse_defaults(root.get("defaults"))?;
    let parsed_ccp = parse_ccp(root.get("ccp"))?;
    let simm = parse_simm(root.get("simm"))?;

    info!(
        schedules = schedule_im.len(),
        collateral_schedules = collateral_schedules.len(),
        ccps = parsed_ccp.ccp.len(),
        simm_versions = simm.len(),
        has_overlay = overlay.is_some(),
        "Margin registry loaded"
    );

    Ok(MarginRegistry {
        defaults,
        schedule_im,
        collateral_asset_class_defaults: collateral_defaults,
        collateral_schedules,
        ccp: parsed_ccp.ccp,
        ccp_default_params: parsed_ccp.ccp_default_params,
        simm,
    })
}

/// Version of every margin registry file this crate reads.
pub(super) const REGISTRY_FILE_VERSION: u32 = 1;

/// Check a registry file's optional `schema` / `version` envelope.
///
/// Overlays may omit both; when present, `schema` must name the file kind and
/// `version` must be [`REGISTRY_FILE_VERSION`], so a file written for another
/// format version is rejected instead of being read as this one.
pub(super) fn check_envelope(
    schema: Option<&str>,
    version: Option<u32>,
    expected: &str,
) -> Result<()> {
    if let Some(schema) = schema
        && schema != expected
    {
        return Err(Error::Validation(format!(
            "margin registry file has schema {schema:?}, expected {expected:?}"
        )));
    }
    if let Some(version) = version
        && version != REGISTRY_FILE_VERSION
    {
        return Err(Error::Validation(format!(
            "margin registry file {expected} has version {version}, expected {REGISTRY_FILE_VERSION}"
        )));
    }
    Ok(())
}

pub(super) fn parse_schedule_im(
    value: Option<&Value>,
) -> Result<FxHashMap<String, ScheduleImSchedule>> {
    let Some(val) = value else {
        return Err(Error::Validation("schedule_im section missing".to_string()));
    };
    let file: wire::ScheduleImFile = serde_json::from_value(val.clone()).map_err(to_validation)?;
    check_envelope(
        file.schema.as_deref(),
        file.version,
        "finstack_quant.margin.schedule_im.v1",
    )?;
    let mut map = FxHashMap::default();
    for entry in file.entries {
        let record = entry.record;
        validate_non_negative("default_rate", record.default_rate)?;
        let boundaries = record.bucket_boundaries_years;
        if !boundaries.short_to_medium.is_finite() || !boundaries.medium_to_long.is_finite() {
            return Err(Error::Validation(
                "schedule_im bucket boundaries must be finite".to_string(),
            ));
        }
        if boundaries.short_to_medium <= 0.0
            || boundaries.medium_to_long <= boundaries.short_to_medium
        {
            return Err(Error::Validation(
                "schedule_im bucket boundaries must be increasing and > 0".to_string(),
            ));
        }
        if !record.default_maturity_years.is_finite()
            || record.default_maturity_years < 0.0
            || record.default_maturity_years > 100.0
        {
            return Err(Error::Validation(format!(
                "schedule_im default_maturity_years must be a finite value in [0, 100] years, \
                 got {}",
                record.default_maturity_years
            )));
        }
        let default_asset_class = parse_enum("schedule asset class", &record.default_asset_class)?;
        let mut rates = FxHashMap::default();
        for rate in record.rates {
            validate_non_negative("schedule_im.rate", rate.rate)?;
            let asset_class = parse_enum("schedule asset class", &rate.asset_class)?;
            let bucket = parse_maturity_bucket(&rate.bucket)?;
            rates.insert((asset_class, bucket), rate.rate);
        }
        let sched = ScheduleImSchedule {
            boundaries,
            default_rate: record.default_rate,
            default_asset_class,
            default_maturity_years: record.default_maturity_years,
            mpor_days: record.mpor_days,
            rates,
        };
        for id in entry.ids {
            if map.insert(id.clone(), sched.clone()).is_some() {
                return Err(Error::Validation(format!(
                    "duplicate schedule_im id '{id}'"
                )));
            }
        }
    }
    Ok(map)
}

pub(super) fn parse_collateral_schedules(
    value: Option<&Value>,
) -> Result<(
    FxHashMap<CollateralAssetClass, AssetClassDefault>,
    FxHashMap<String, EligibleCollateralSchedule>,
)> {
    let Some(val) = value else {
        return Err(Error::Validation(
            "collateral_schedules section missing".to_string(),
        ));
    };
    let file: wire::CollateralSchedulesFile =
        serde_json::from_value(val.clone()).map_err(to_validation)?;
    check_envelope(
        file.schema.as_deref(),
        file.version,
        "finstack_quant.margin.collateral_schedules.v1",
    )?;

    let mut defaults = FxHashMap::default();
    for def in file.asset_class_defaults {
        let asset = parse_enum("collateral asset class", &def.asset_class)?;
        validate_haircut("standard_haircut", def.standard_haircut)?;
        validate_haircut("fx_addon", def.fx_addon)?;
        if let Some(concentration_limit) = def.concentration_limit {
            validate_haircut("concentration_limit", concentration_limit)?;
        }
        defaults.insert(
            asset,
            AssetClassDefault {
                standard_haircut: def.standard_haircut,
                fx_addon: def.fx_addon,
                concentration_limit: def.concentration_limit,
            },
        );
    }

    let mut schedules = FxHashMap::default();
    for entry in file.entries {
        let mut eligible = Vec::new();
        for elig in entry.record.eligible {
            validate_haircut("haircut", elig.haircut)?;
            validate_haircut("fx_haircut_addon", elig.fx_haircut_addon)?;
            let asset_class = parse_enum("collateral asset class", &elig.asset_class)?;
            eligible.push(CollateralEligibility {
                asset_class,
                min_rating: elig.min_rating,
                maturity_constraints: elig.maturity_constraints,
                haircut: elig.haircut,
                fx_haircut_addon: elig.fx_haircut_addon,
                concentration_limit: elig.concentration_limit,
            });
        }

        let schedule = EligibleCollateralSchedule {
            eligible,
            default_haircut: entry.record.default_haircut,
            rehypothecation_allowed: entry.record.rehypothecation_allowed,
        };
        for id in entry.ids {
            if schedules.insert(id.clone(), schedule.clone()).is_some() {
                return Err(Error::Validation(format!(
                    "duplicate collateral schedule id '{id}'"
                )));
            }
        }
    }

    Ok((defaults, schedules))
}

pub(super) fn parse_defaults(value: Option<&Value>) -> Result<MarginDefaults> {
    let Some(val) = value else {
        return Err(Error::Validation("defaults section missing".to_string()));
    };
    let file: wire::DefaultsFile = serde_json::from_value(val.clone()).map_err(to_validation)?;
    check_envelope(
        file.schema.as_deref(),
        file.version,
        "finstack_quant.margin.defaults.v1",
    )?;

    let vm_frequency = parse_enum("margin frequency", &file.defaults.vm.frequency)?;
    let vm = VmDefaults {
        threshold: file.defaults.vm.threshold,
        mta: file.defaults.vm.mta,
        rounding: file.defaults.vm.rounding,
        independent_amount: file.defaults.vm.independent_amount,
        frequency: vm_frequency,
        settlement_lag: file.defaults.vm.settlement_lag,
    };

    Ok(MarginDefaults {
        vm,
        im: file.defaults.im,
        timing: file.defaults.timing,
        cleared_settlement: file.defaults.cleared_settlement,
    })
}

pub(super) fn parse_ccp(value: Option<&Value>) -> Result<ParsedCcpRegistry> {
    let Some(val) = value else {
        return Err(Error::Validation("ccp section missing".to_string()));
    };
    let file: wire::CcpFile = serde_json::from_value(val.clone()).map_err(to_validation)?;
    check_envelope(
        file.schema.as_deref(),
        file.version,
        "finstack_quant.margin.ccp.v1",
    )?;
    let mut map = FxHashMap::default();
    let mut default_params: Option<CcpParams> = None;
    for entry in file.entries {
        validate_non_negative("ccp.conservative_rate", entry.record.conservative_rate)?;
        let record = CcpParams {
            mpor_days: entry.record.mpor_days,
            conservative_rate: entry.record.conservative_rate,
        };
        if entry.record.is_default {
            if default_params.is_some() {
                return Err(Error::Validation("duplicate ccp default entry".to_string()));
            }
            if entry.ids.is_empty() {
                return Err(Error::Validation(
                    "ccp default entry requires at least one id".to_string(),
                ));
            }
            default_params = Some(record.clone());
        }
        for id in entry.ids {
            if map.insert(id.clone(), record.clone()).is_some() {
                return Err(Error::Validation(format!("duplicate ccp id '{id}'")));
            }
        }
    }
    let Some(default_params) = default_params else {
        return Err(Error::Validation("ccp default entry missing".to_string()));
    };
    Ok(ParsedCcpRegistry {
        ccp: map,
        ccp_default_params: default_params,
    })
}

pub(super) fn parse_simm(value: Option<&Value>) -> Result<FxHashMap<String, SimmParams>> {
    let Some(val) = value else {
        return Err(Error::Validation("simm section missing".to_string()));
    };
    let file: wire::SimmFile = serde_json::from_value(val.clone()).map_err(to_validation)?;
    check_envelope(
        file.schema.as_deref(),
        file.version,
        "finstack_quant.margin.simm.v1",
    )?;
    let mut map = FxHashMap::default();
    for entry in file.entries {
        let record = entry.record;
        let version = parse_simm_version(entry.ids.first().map(String::as_str))?;
        let ir_delta_weights = parse_number_map(&record.ir_delta_weights, "simm.ir_delta_weights")?;
        let commodity_bucket_weights = parse_number_map(
            &record.commodity_bucket_weights,
            "simm.commodity_bucket_weights",
        )?;

        validate_non_negative("simm.cnq_delta_weight", record.cnq_delta_weight)?;
        validate_non_negative("simm.equity_delta_weight", record.equity_delta_weight)?;
        validate_non_negative("simm.fx_delta_weight", record.fx_delta_weight)?;
        let fx_intra_bucket_correlation = record.fx_intra_bucket_correlation;
        if !(-1.0..=1.0).contains(&fx_intra_bucket_correlation) {
            return Err(Error::Validation(
                "simm.fx_intra_bucket_correlation must be in [-1,1]".to_string(),
            ));
        }

        let mut correlations: FxHashMap<(SimmRiskClass, SimmRiskClass), f64> = FxHashMap::default();
        for cor in record.risk_class_correlations {
            let a = parse_enum("SIMM risk class", &cor.a)?;
            let b = parse_enum("SIMM risk class", &cor.b)?;
            if !(cor.rho >= -1.0 && cor.rho <= 1.0) {
                return Err(Error::Validation(format!(
                    "simm correlation for ({a:?},{b:?}) must be in [-1,1]"
                )));
            }
            let key = ordered_pair(a, b);
            correlations.insert(key, cor.rho);
        }

        let ir_tenor_correlations = parse_ir_tenor_correlations(&record.ir_tenor_correlations)?;
        // ISDA SIMM v2.6 default vega weights and curvature scale factor
        // (used when an overlay omits the field). Values are validated
        // through `validate_non_negative` immediately after assignment so
        // a future typo in either the default or an explicit overlay
        // value is caught at parse time, before the SimmParams escapes
        // this function.
        let ir_vega_weight = record.ir_vega_weight;
        let cq_vega_weight = record.cq_vega_weight;
        let cnq_vega_weight = record.cnq_vega_weight;
        let equity_vega_weight = record.equity_vega_weight;
        let fx_vega_weight = record.fx_vega_weight;
        let commodity_vega_weight = record.commodity_vega_weight;
        validate_non_negative("simm.ir_vega_weight", ir_vega_weight)?;
        validate_non_negative("simm.cq_vega_weight", cq_vega_weight)?;
        validate_non_negative("simm.cnq_vega_weight", cnq_vega_weight)?;
        validate_non_negative("simm.equity_vega_weight", equity_vega_weight)?;
        validate_non_negative("simm.fx_vega_weight", fx_vega_weight)?;
        validate_non_negative("simm.commodity_vega_weight", commodity_vega_weight)?;
        let concentration_thresholds =
            parse_concentration_thresholds(&record.concentration_thresholds)?;

        // Credit-qualifying bucket tables are ISDA-published per SIMM version.
        // They are required: the previous fallbacks fabricated them (broad
        // weights via `unwrap_or(85.0)`, a flat 0.27 across every sector pair,
        // per-bucket thresholds collapsed to the aggregate), producing
        // confident margin numbers that match no ISDA calibration.
        let cq_bucket_weights = parse_cq_bucket_weights(&record.cq_bucket_weights)?;
        let cq_intra_bucket_correlation = record.cq_intra_bucket_correlation;
        if !(-1.0..=1.0).contains(&cq_intra_bucket_correlation) {
            return Err(Error::Validation(
                "simm.cq_intra_bucket_correlation must be in [-1,1]".to_string(),
            ));
        }
        let cq_inter_bucket_correlations =
            parse_cq_inter_bucket_correlations(&record.cq_inter_bucket_correlations)?;
        let cq_concentration_thresholds =
            parse_cq_concentration_thresholds(&record.cq_concentration_thresholds)?;
        let commodity_inter_bucket_correlations = record.commodity_inter_bucket_correlations;
        if commodity_inter_bucket_correlations.is_empty() {
            return Err(Error::Validation(format!(
                "SIMM registry {:?}: commodity_inter_bucket_correlations missing",
                version
            )));
        }

        let params = SimmParams {
            version,
            mpor_days: record.mpor_days,
            ir_delta_weights,
            cnq_delta_weight: record.cnq_delta_weight,
            equity_delta_weight: record.equity_delta_weight,
            fx_delta_weight: record.fx_delta_weight,
            fx_intra_bucket_correlation,
            risk_class_correlations: correlations,
            commodity_bucket_weights,
            ir_tenor_correlations,
            ir_inter_currency_correlation: record.ir_inter_currency_correlation,
            ir_vega_weight,
            cq_vega_weight,
            cnq_vega_weight,
            equity_vega_weight,
            fx_vega_weight,
            commodity_vega_weight,
            ir_subcurve_correlation: record.ir_subcurve_correlation,
            commodity_intra_bucket_correlations: parse_number_map(
                &record.commodity_intra_bucket_correlations,
                "simm.commodity_intra_bucket_correlations",
            )?,
            ir_historical_volatility_ratio: record.ir_historical_volatility_ratio,
            equity_historical_volatility_ratio: record.equity_historical_volatility_ratio,
            fx_historical_volatility_ratio: record.fx_historical_volatility_ratio,
            commodity_historical_volatility_ratio: record.commodity_historical_volatility_ratio,
            fx_high_delta_weight: record.fx_high_delta_weight,
            fx_regular_high_correlation: record.fx_regular_high_correlation,
            fx_high_high_correlation: record.fx_high_high_correlation,
            cq_same_issuer_correlation: record.cq_same_issuer_correlation,
            credit_residual_correlation: record.credit_residual_correlation,
            ir_delta_weights_low: parse_number_map(
                &record.ir_delta_weights_low,
                "simm.ir_delta_weights_low",
            )?,
            ir_delta_weights_high: parse_number_map(
                &record.ir_delta_weights_high,
                "simm.ir_delta_weights_high",
            )?,
            ir_delta_concentration_thresholds: parse_number_map(
                &record.ir_delta_concentration_thresholds,
                "simm.ir_delta_concentration_thresholds",
            )?,
            ir_vega_concentration_thresholds: parse_number_map(
                &record.ir_vega_concentration_thresholds,
                "simm.ir_vega_concentration_thresholds",
            )?,
            fx_delta_concentration_thresholds: parse_number_map(
                &record.fx_delta_concentration_thresholds,
                "simm.fx_delta_concentration_thresholds",
            )?,
            fx_vega_concentration_thresholds: parse_number_map(
                &record.fx_vega_concentration_thresholds,
                "simm.fx_vega_concentration_thresholds",
            )?,
            commodity_delta_concentration_thresholds: parse_number_map(
                &record.commodity_delta_concentration_thresholds,
                "simm.commodity_delta_concentration_thresholds",
            )?,
            commodity_vega_concentration_thresholds: parse_number_map(
                &record.commodity_vega_concentration_thresholds,
                "simm.commodity_vega_concentration_thresholds",
            )?,
            vega_concentration_thresholds: parse_concentration_thresholds(
                &record.vega_concentration_thresholds,
            )?,
            concentration_thresholds,
            cq_bucket_weights,
            cq_intra_bucket_correlation,
            cq_inter_bucket_correlations,
            cq_concentration_thresholds,
            commodity_inter_bucket_correlations,
        };

        validate_simm_params(&params)?;

        for id in entry.ids {
            if map.insert(id.clone(), params.clone()).is_some() {
                return Err(Error::Validation(format!("duplicate simm id '{id}'")));
            }
        }
    }
    Ok(map)
}

// Public helper for overrides via FinstackConfig

/// Extension key used by [`finstack_quant_core::config::FinstackConfig`] for
/// margin-registry JSON overlays.
pub(crate) const MARGIN_REGISTRY_EXTENSION_KEY: &str = "margin.registry.v1";

/// Build a margin registry from a [`finstack_quant_core::config::FinstackConfig`] extension overlay.
///
/// # Arguments
///
/// * `cfg` - Config whose `extensions` map may contain [`MARGIN_REGISTRY_EXTENSION_KEY`]
///
/// # Returns
///
/// A new [`MarginRegistry`] built from embedded defaults plus any configured overlay.
///
/// # Errors
///
/// Returns an error if the embedded registry or the configured overlay fails validation.
pub(crate) fn margin_registry_from_config(cfg: &FinstackConfig) -> Result<MarginRegistry> {
    let overlay = cfg.extensions.get(MARGIN_REGISTRY_EXTENSION_KEY);
    build_registry(overlay)
}

// Conversions and validations

pub(super) fn parse_enum<T>(label: &str, value: &str) -> Result<T>
where
    T: FromStr,
    T::Err: Display,
{
    value
        .parse::<T>()
        .map_err(|e| Error::Validation(format!("invalid {label} '{value}': {e}")))
}

pub(super) fn parse_maturity_bucket(value: &str) -> Result<MaturityBucket> {
    match value {
        "short" => Ok(MaturityBucket::Short),
        "medium" => Ok(MaturityBucket::Medium),
        "long" => Ok(MaturityBucket::Long),
        other => Err(Error::Validation(format!(
            "unknown maturity bucket '{other}'"
        ))),
    }
}

pub(super) fn parse_simm_version(id: Option<&str>) -> Result<SimmVersion> {
    let id = id.ok_or_else(|| Error::Validation("SIMM version id is required".to_string()))?;
    parse_enum("SIMM version id", id)
}

pub(super) fn parse_number_map(value: &Value, context: &str) -> Result<FxHashMap<String, f64>> {
    let obj = value
        .as_object()
        .ok_or_else(|| Error::Validation(format!("{context} must be an object")))?;
    let mut out = FxHashMap::default();
    for (k, v) in obj {
        let num = v.as_f64().ok_or_else(|| {
            Error::Validation(format!("{context} value for key '{k}' must be a number"))
        })?;
        // Reject NaN / ±infinity at the trust boundary so a malformed
        // overlay cannot inject non-finite values into a FxHashMap whose
        // downstream validators only check non-negativity.
        if !num.is_finite() {
            return Err(Error::Validation(format!(
                "{context} value for key '{k}' must be finite, got {num}"
            )));
        }
        out.insert(k.clone(), num);
    }
    Ok(out)
}

pub(super) fn parse_cq_bucket_weights(value: &Value) -> Result<FxHashMap<SimmCreditSector, f64>> {
    parse_credit_sector_number_map(value, "simm.cq_bucket_weights")
}

pub(super) fn parse_cq_concentration_thresholds(
    value: &Value,
) -> Result<FxHashMap<SimmCreditSector, f64>> {
    parse_credit_sector_number_map(value, "simm.cq_concentration_thresholds")
}

pub(super) fn parse_credit_sector_number_map(
    value: &Value,
    context: &str,
) -> Result<FxHashMap<SimmCreditSector, f64>> {
    if value.is_null() {
        return Err(Error::Validation(format!("{context} missing")));
    }
    let obj = value
        .as_object()
        .ok_or_else(|| Error::Validation(format!("{context} must be an object")))?;
    let mut out = FxHashMap::default();
    for (k, v) in obj {
        let sector = parse_enum("SIMM credit qualifying sector", k)?;
        let num = v.as_f64().ok_or_else(|| {
            Error::Validation(format!("{context} value for key '{k}' must be a number"))
        })?;
        // Reject NaN / ±infinity / negatives at the trust boundary;
        // bucket weights and concentration thresholds are always
        // non-negative finite quantities.
        validate_non_negative(&format!("{context}[{k}]"), num)?;
        out.insert(sector, num);
    }
    Ok(out)
}

pub(super) fn parse_cq_inter_bucket_correlations(
    value: &Value,
) -> Result<FxHashMap<(SimmCreditSector, SimmCreditSector), f64>> {
    if value.is_null() {
        return Err(Error::Validation(
            "simm.cq_inter_bucket_correlations missing".to_string(),
        ));
    }
    let obj = value.as_object().ok_or_else(|| {
        Error::Validation("simm.cq_inter_bucket_correlations must be an object".to_string())
    })?;
    let mut out = FxHashMap::default();
    for (k, v) in obj {
        let (a, b) = k.split_once("__").ok_or_else(|| {
            Error::Validation(format!(
                "invalid cq_inter_bucket_correlations key '{k}': expected 'sector_a__sector_b'"
            ))
        })?;
        let rho = v.as_f64().ok_or_else(|| {
            Error::Validation(format!(
                "cq_inter_bucket_correlations value for '{k}' must be a number"
            ))
        })?;
        if !rho.is_finite() || !(-1.0..=1.0).contains(&rho) {
            return Err(Error::Validation(format!(
                "cq_inter_bucket_correlations value for '{k}' must be a finite value in [-1,1], got {rho}"
            )));
        }
        out.insert(
            ordered_pair(
                parse_enum("SIMM credit qualifying sector", a)?,
                parse_enum("SIMM credit qualifying sector", b)?,
            ),
            rho,
        );
    }
    Ok(out)
}

/// Parse the `simm.ir_tenor_correlations` overlay map.
///
/// Keys use a single underscore as the pair separator
/// (`"tenor_a_tenor_b"`); tenor labels themselves must therefore not
/// contain `_`. The embedded registry uses ISDA-standard labels (`2w`,
/// `1m`, `3m`, ..., `30y`) which all satisfy this invariant.
pub(super) fn parse_ir_tenor_correlations(
    value: &Value,
) -> Result<FxHashMap<(String, String), f64>> {
    if value.is_null() {
        return Ok(FxHashMap::default());
    }
    let obj = value
        .as_object()
        .ok_or_else(|| Error::Validation("ir_tenor_correlations must be an object".to_string()))?;
    let mut out = FxHashMap::default();
    for (k, v) in obj {
        let (a, b) = k.split_once('_').ok_or_else(|| {
            Error::Validation(format!(
                "invalid ir_tenor_correlations key '{k}': expected format 'tenor1_tenor2' \
                 (tenor labels must not contain '_')"
            ))
        })?;
        let rho = v.as_f64().ok_or_else(|| {
            Error::Validation(format!(
                "ir_tenor_correlations value for '{k}' must be a number"
            ))
        })?;
        if !rho.is_finite() || !(-1.0..=1.0).contains(&rho) {
            return Err(Error::Validation(format!(
                "ir_tenor_correlations value for '{k}' must be a finite value in [-1,1], got {rho}"
            )));
        }
        let (a, b) = ordered_tenor_pair(a, b);
        out.insert((a, b), rho);
    }
    Ok(out)
}

pub(super) fn parse_concentration_thresholds(
    value: &Value,
) -> Result<FxHashMap<SimmRiskClass, f64>> {
    if value.is_null() {
        return Ok(FxHashMap::default());
    }
    let obj = value.as_object().ok_or_else(|| {
        Error::Validation("concentration_thresholds must be an object".to_string())
    })?;
    let mut out = FxHashMap::default();
    for (k, v) in obj {
        let rc = parse_enum("SIMM risk class", k)?;
        let threshold = v.as_f64().ok_or_else(|| {
            Error::Validation(format!(
                "concentration_thresholds value for '{k}' must be a number"
            ))
        })?;
        validate_non_negative(&format!("concentration_thresholds[{k}]"), threshold)?;
        out.insert(rc, threshold);
    }
    Ok(out)
}
