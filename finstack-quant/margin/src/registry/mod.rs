//! Margin-registry loading and override helpers.
//!
//! This module exposes the resolved configuration surface that powers schedule
//! IM, SIMM parameters, collateral schedules, and CCP fallback settings.
//! Values are loaded from embedded JSON assets and may be overlaid through
//! [`finstack_quant_core::config::FinstackConfig`] using
//! [`crate::registry::MARGIN_REGISTRY_EXTENSION_KEY`].
//!
//! # Units And Conventions
//!
//! - Rates and haircuts are stored as decimal fractions, not basis points.
//! - Thresholds, MTAs, and independent amounts are stored as raw currency
//!   amounts before conversion into [`finstack_quant_core::money::Money`].
//! - `mpor_days` and settlement lags are stored in calendar days.
//! - Schedule maturities are stored as year fractions.
use std::sync::OnceLock;

use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::money::Money;
use finstack_quant_core::{Error, HashMap, Result};
use serde_json::Value;
use tracing::{debug, info};

mod validation;
use validation::{
    to_validation, validate_haircut, validate_non_negative, validate_probability, validate_rate,
};

use crate::calculators::im::schedule::{MaturityBucket, ScheduleAssetClass};
use crate::calculators::im::simm::SimmVersion;
use crate::types::{
    ordered_credit_sector_pair, ordered_risk_class_pair, ordered_tenor_pair, CollateralAssetClass,
    CollateralEligibility, EligibleCollateralSchedule, ImMethodology, ImParameters,
    MarginCallTiming, MarginTenor, MaturityConstraints, VmParameters,
};
use crate::{SimmCreditSector, SimmRiskClass};

mod embedded;
mod merge;
mod wire;

/// Fully resolved, ready-to-use margin registry.
///
/// This is the public, parsed view of the embedded margin registry after any
/// optional JSON overlay has been applied.
#[derive(Debug, Clone)]
pub struct MarginRegistry {
    /// Workspace-wide default VM, IM, timing, and settlement settings.
    pub defaults: MarginDefaults,
    /// Regulatory schedule IM grids keyed by registry id such as `"bcbs_iosco"`.
    pub schedule_im: HashMap<String, ScheduleImSchedule>,
    /// Default collateral haircuts keyed by collateral asset class.
    pub collateral_asset_class_defaults: HashMap<CollateralAssetClass, AssetClassDefault>,
    /// Eligible collateral schedules keyed by registry id.
    pub collateral_schedules: HashMap<String, EligibleCollateralSchedule>,
    /// CCP conservative fallback parameters keyed by CCP identifier.
    pub ccp: HashMap<String, CcpParams>,
    /// Resolved default CCP conservative fallback parameters.
    pub ccp_default_params: CcpParams,
    /// Resolved generic VaR fallback metadata for unknown CCP names.
    pub ccp_generic_var_defaults: GenericVarDefaults,
    /// XVA default configuration assumptions.
    pub xva: XvaDefaults,
    /// SIMM parameter sets keyed by registry id such as `"v2_6"`.
    pub simm: HashMap<String, SimmParams>,
}

/// Top-level default settings shared across margin methodologies.
#[derive(Debug, Clone)]
pub struct MarginDefaults {
    /// Variation-margin defaults.
    pub vm: VmDefaults,
    /// Initial-margin defaults split by methodology.
    pub im: ImDefaults,
    /// Margin call timing defaults for different agreement types.
    pub timing: TimingDefaults,
    /// Settlement defaults for cleared margin flows.
    pub cleared_settlement: ClearedSettlementDefaults,
}

/// Default variation-margin parameters stored in raw numeric form.
#[derive(Debug, Clone)]
pub struct VmDefaults {
    /// VM threshold amount in base-currency units before conversion to [`finstack_quant_core::money::Money`].
    pub threshold: f64,
    /// Minimum transfer amount in base-currency units before conversion to [`finstack_quant_core::money::Money`].
    pub mta: f64,
    /// Rounding amount in base-currency units before conversion to [`finstack_quant_core::money::Money`].
    pub rounding: f64,
    /// Independent amount in base-currency units before conversion to [`finstack_quant_core::money::Money`].
    pub independent_amount: f64,
    /// Margin-call frequency.
    pub frequency: MarginTenor,
    /// Settlement lag in calendar days.
    pub settlement_lag: u32,
}

/// Default initial-margin settings grouped by methodology.
#[derive(Debug, Clone)]
pub struct ImDefaults {
    /// Defaults for SIMM-based bilateral IM.
    pub simm: ImMethodDefaults,
    /// Defaults for schedule-based IM.
    pub schedule: ImMethodDefaults,
    /// Defaults for cleared-derivative CCP IM.
    pub cleared: ImMethodDefaults,
    /// Defaults for repo haircut IM.
    pub repo_haircut: ImMethodDefaults,
}

/// Raw default parameters for a single IM methodology.
#[derive(Debug, Clone)]
pub struct ImMethodDefaults {
    /// Margin period of risk in calendar days.
    pub mpor_days: u32,
    /// Threshold amount in base-currency units before conversion to [`finstack_quant_core::money::Money`].
    pub threshold: f64,
    /// Minimum transfer amount in base-currency units before conversion to [`finstack_quant_core::money::Money`].
    pub mta: f64,
    /// Whether the methodology assumes segregated collateral.
    pub segregated: bool,
}

/// Default timing conventions for margin calls and responses.
#[derive(Debug, Clone)]
pub struct TimingDefaults {
    /// Standard bilateral timing defaults.
    pub standard: MarginCallTiming,
    /// Regulatory VM timing defaults.
    pub regulatory_vm: MarginCallTiming,
    /// Cleared or CCP timing defaults.
    pub ccp: MarginCallTiming,
}

/// Default settlement handling for cleared collateral flows.
#[derive(Debug, Clone)]
pub struct ClearedSettlementDefaults {
    /// Settlement rounding amount in base-currency units before conversion to [`finstack_quant_core::money::Money`].
    pub rounding: f64,
    /// Settlement lag in calendar days.
    pub settlement_lag: u32,
}

/// Parsed regulatory schedule IM entry from the registry.
#[derive(Debug, Clone)]
pub struct ScheduleImSchedule {
    /// Year-fraction boundaries separating short, medium, and long buckets.
    pub boundaries: ScheduleBucketBoundaries,
    /// Fallback decimal rate used when a bucket-specific entry is absent.
    pub default_rate: f64,
    /// Default asset class used by the schedule calculator's trait-based fallback path.
    pub default_asset_class: ScheduleAssetClass,
    /// Default maturity expressed as a year fraction.
    pub default_maturity_years: f64,
    /// Margin period of risk in calendar days.
    pub mpor_days: u32,
    /// Decimal rates keyed by `(asset_class, maturity_bucket)`.
    pub rates: HashMap<(ScheduleAssetClass, MaturityBucket), f64>,
}

/// Boundaries, in years, for the schedule IM maturity buckets.
#[derive(Debug, Clone)]
pub struct ScheduleBucketBoundaries {
    /// Cutoff between short and medium maturity buckets, in years.
    pub short_to_medium: f64,
    /// Cutoff between medium and long maturity buckets, in years.
    pub medium_to_long: f64,
}

/// Default collateral haircut settings for an asset class.
#[derive(Debug, Clone)]
pub struct AssetClassDefault {
    /// Standard collateral haircut as a decimal fraction.
    pub standard_haircut: f64,
    /// Additional FX haircut as a decimal fraction when collateral currency differs.
    pub fx_addon: f64,
    /// Optional concentration limit as a fraction of total collateral.
    pub concentration_limit: Option<f64>,
}

/// Conservative fallback CCP parameters.
#[derive(Debug, Clone)]
pub struct CcpParams {
    /// Margin period of risk in calendar days.
    pub mpor_days: u32,
    /// Conservative fallback margin rate as a decimal fraction.
    pub conservative_rate: f64,
}

/// Generic VaR metadata used for unknown CCP-name fallbacks.
#[derive(Debug, Clone)]
pub struct GenericVarDefaults {
    /// VaR confidence level as a decimal probability.
    pub confidence: f64,
    /// Historical lookback window in calendar days.
    pub lookback_days: u32,
}

struct ParsedCcpRegistry {
    ccp: HashMap<String, CcpParams>,
    ccp_default_params: CcpParams,
    ccp_generic_var_defaults: GenericVarDefaults,
}

/// Registry-backed XVA defaults.
#[derive(Debug, Clone)]
pub struct XvaDefaults {
    /// Defaults for deterministic exposure profile generation and CVA inputs.
    pub deterministic_exposure: XvaDeterministicExposureDefaults,
    /// Defaults for stochastic exposure profile generation.
    pub stochastic_exposure: XvaStochasticExposureDefaults,
}

/// Registry-backed deterministic XVA exposure defaults.
#[derive(Debug, Clone)]
pub struct XvaDeterministicExposureDefaults {
    /// Number of points in the default exposure time grid.
    pub time_grid_points: usize,
    /// Step between default exposure time-grid points, in years.
    pub time_grid_step_years: f64,
    /// Counterparty recovery rate as a decimal probability.
    pub recovery_rate: f64,
    /// Optional own recovery rate as a decimal probability.
    pub own_recovery_rate: Option<f64>,
}

/// Registry-backed stochastic XVA exposure defaults.
#[derive(Debug, Clone)]
pub struct XvaStochasticExposureDefaults {
    /// Number of Monte Carlo paths to simulate.
    pub num_paths: usize,
    /// Deterministic RNG seed for reproducible exposure profiles.
    pub seed: u64,
    /// Tail quantile used for PFE.
    pub pfe_quantile: f64,
}

/// Registry-backed SIMM parameter set.
#[derive(Debug, Clone)]
pub struct SimmParams {
    /// SIMM version identifier for this parameter set.
    pub version: SimmVersion,
    /// Margin period of risk in calendar days.
    pub mpor_days: u32,
    /// Interest-rate delta risk weights keyed by tenor label.
    pub ir_delta_weights: HashMap<String, f64>,
    /// Credit-qualifying delta risk weights keyed by sector or bucket label.
    pub cq_delta_weights: HashMap<String, f64>,
    /// Credit-non-qualifying delta risk weight.
    pub cnq_delta_weight: f64,
    /// Equity delta risk weight.
    pub equity_delta_weight: f64,
    /// FX delta risk weight.
    pub fx_delta_weight: f64,
    /// FX delta intra-bucket correlation between distinct currency risk factors.
    ///
    /// ISDA SIMM v2.6 uses a uniform 0.5 correlation for FX delta factors.
    pub fx_intra_bucket_correlation: f64,
    /// Cross-risk-class correlations keyed by risk-class pair.
    pub risk_class_correlations: HashMap<(SimmRiskClass, SimmRiskClass), f64>,
    /// Commodity delta risk weights keyed by bucket label.
    pub commodity_bucket_weights: HashMap<String, f64>,
    /// Interest-rate tenor correlations keyed by ordered tenor pair.
    pub ir_tenor_correlations: HashMap<(String, String), f64>,
    /// Inter-currency correlation γ for IR delta aggregation across currencies.
    /// Per ISDA SIMM specification (typically 0.27 for v2.5/v2.6).
    pub ir_inter_currency_correlation: f64,
    /// Interest-rate vega risk weight.
    pub ir_vega_weight: f64,
    /// Credit-qualifying vega risk weight.
    pub cq_vega_weight: f64,
    /// Credit-non-qualifying vega risk weight.
    pub cnq_vega_weight: f64,
    /// Equity vega risk weight.
    pub equity_vega_weight: f64,
    /// FX vega risk weight.
    pub fx_vega_weight: f64,
    /// Commodity vega risk weight.
    pub commodity_vega_weight: f64,
    /// Curvature scale factor applied before aggregation.
    pub curvature_scale_factor: f64,
    /// Concentration thresholds keyed by SIMM risk class.
    pub concentration_thresholds: HashMap<SimmRiskClass, f64>,
    /// Credit qualifying bucket risk weights keyed by [`SimmCreditSector`].
    ///
    /// Maps each ISDA SIMM credit qualifying sector bucket to its risk weight.
    /// When populated, enables bucket-level credit delta aggregation with
    /// intra/inter-bucket diversification per ISDA SIMM v2.6.
    pub cq_bucket_weights: HashMap<SimmCreditSector, f64>,
    /// Intra-bucket name correlation for credit qualifying delta.
    ///
    /// Per ISDA SIMM v2.6, the correlation between distinct names within the
    /// same credit qualifying sector bucket (typically 0.42).
    pub cq_intra_bucket_correlation: f64,
    /// Inter-bucket correlations for credit qualifying delta.
    ///
    /// Correlations between different ISDA SIMM credit qualifying sector
    /// buckets, keyed by ordered sector pair.
    pub cq_inter_bucket_correlations: HashMap<(SimmCreditSector, SimmCreditSector), f64>,
    /// Per-bucket concentration thresholds for credit qualifying delta.
    ///
    /// When the net weighted sensitivity in a bucket exceeds its threshold,
    /// a sqrt(|WS| / threshold) concentration factor is applied.
    pub cq_concentration_thresholds: HashMap<SimmCreditSector, f64>,
    /// Commodity inter-bucket correlation matrix (17×17, row-major).
    ///
    /// Per ISDA SIMM v2.6 Table 11. Populated at registry load from the
    /// default SIMM commodity correlation table and PSD-validated via
    /// `validate_simm_correlations_psd`. Bucket
    /// 16 ("Other" / residual) is zero-correlated with every other
    /// bucket per the specification.
    pub commodity_inter_bucket_correlations: Vec<f64>,
}

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
pub fn embedded_registry() -> Result<&'static MarginRegistry> {
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
pub fn embedded_registry_or_panic() -> &'static MarginRegistry {
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
pub fn build_registry(overlay: Option<&Value>) -> Result<MarginRegistry> {
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
    let xva = parse_xva_defaults(root.get("xva_defaults"))?;
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
        ccp_generic_var_defaults: parsed_ccp.ccp_generic_var_defaults,
        xva,
        simm,
    })
}

// -----------------------------------------------------------------------------//
// Parse helpers
// -----------------------------------------------------------------------------//

fn parse_schedule_im(value: Option<&Value>) -> Result<HashMap<String, ScheduleImSchedule>> {
    let Some(val) = value else {
        return Err(Error::Validation("schedule_im section missing".to_string()));
    };
    let file: wire::ScheduleImFile = serde_json::from_value(val.clone()).map_err(to_validation)?;
    let mut map = HashMap::default();
    for entry in file.entries {
        let record = entry.record;
        validate_non_negative("default_rate", record.default_rate)?;
        let boundaries = ScheduleBucketBoundaries {
            short_to_medium: record.bucket_boundaries_years.short_to_medium,
            medium_to_long: record.bucket_boundaries_years.medium_to_long,
        };
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
        // Fallback maturity must be finite and plausible.
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
        let default_asset_class = parse_schedule_asset_class(&record.default_asset_class)?;
        let mut rates = HashMap::default();
        for rate in record.rates {
            validate_rate("schedule_im.rate", rate.rate)?;
            let asset_class = parse_schedule_asset_class(&rate.asset_class)?;
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

fn parse_collateral_schedules(
    value: Option<&Value>,
) -> Result<(
    HashMap<CollateralAssetClass, AssetClassDefault>,
    HashMap<String, EligibleCollateralSchedule>,
)> {
    let Some(val) = value else {
        return Err(Error::Validation(
            "collateral_schedules section missing".to_string(),
        ));
    };
    let file: wire::CollateralSchedulesFile =
        serde_json::from_value(val.clone()).map_err(to_validation)?;

    let mut defaults = HashMap::default();
    for def in file.asset_class_defaults {
        let asset = parse_collateral_asset_class(&def.asset_class)?;
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

    let mut schedules = HashMap::default();
    for entry in file.entries {
        let mut eligible = Vec::new();
        for elig in entry.record.eligible {
            validate_haircut("haircut", elig.haircut)?;
            validate_haircut("fx_haircut_addon", elig.fx_haircut_addon)?;
            let asset_class = parse_collateral_asset_class(&elig.asset_class)?;
            let constraints = elig
                .maturity_constraints
                .as_ref()
                .map(|c| MaturityConstraints {
                    min_remaining_years: c.min_remaining_years,
                    max_remaining_years: c.max_remaining_years,
                });
            eligible.push(CollateralEligibility {
                asset_class,
                min_rating: elig.min_rating.clone(),
                maturity_constraints: constraints,
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

fn parse_defaults(value: Option<&Value>) -> Result<MarginDefaults> {
    let Some(val) = value else {
        return Err(Error::Validation("defaults section missing".to_string()));
    };
    let file: wire::DefaultsFile = serde_json::from_value(val.clone()).map_err(to_validation)?;

    let vm_freq = parse_margin_tenor(&file.defaults.vm.frequency)?;
    let vm = VmDefaults {
        threshold: file.defaults.vm.threshold,
        mta: file.defaults.vm.mta,
        rounding: file.defaults.vm.rounding,
        independent_amount: file.defaults.vm.independent_amount,
        frequency: vm_freq,
        settlement_lag: file.defaults.vm.settlement_lag,
    };

    let im = ImDefaults {
        simm: to_im_method(&file.defaults.im.simm),
        schedule: to_im_method(&file.defaults.im.schedule),
        cleared: to_im_method(&file.defaults.im.cleared),
        repo_haircut: to_im_method(&file.defaults.im.repo_haircut),
    };

    let timing = TimingDefaults {
        standard: to_timing(&file.defaults.timing.standard),
        regulatory_vm: to_timing(&file.defaults.timing.regulatory_vm),
        ccp: to_timing(&file.defaults.timing.ccp),
    };

    let cleared_settlement = ClearedSettlementDefaults {
        rounding: file.defaults.cleared_settlement.rounding,
        settlement_lag: file.defaults.cleared_settlement.settlement_lag,
    };

    Ok(MarginDefaults {
        vm,
        im,
        timing,
        cleared_settlement,
    })
}

fn parse_ccp(value: Option<&Value>) -> Result<ParsedCcpRegistry> {
    let Some(val) = value else {
        return Err(Error::Validation("ccp section missing".to_string()));
    };
    let file: wire::CcpFile = serde_json::from_value(val.clone()).map_err(to_validation)?;
    let mut map = HashMap::default();
    let mut default_params: Option<CcpParams> = None;
    let mut generic_var_defaults: Option<GenericVarDefaults> = None;
    for entry in file.entries {
        validate_rate("ccp.conservative_rate", entry.record.conservative_rate)?;
        let generic_confidence = entry.record.generic_var_confidence;
        let generic_lookback_days = entry.record.generic_var_lookback_days;
        match (generic_confidence, generic_lookback_days) {
            (Some(confidence), Some(lookback_days)) => {
                validate_probability("ccp.generic_var_confidence", confidence)?;
                if lookback_days == 0 {
                    return Err(Error::Validation(
                        "ccp.generic_var_lookback_days must be positive".to_string(),
                    ));
                }
                if generic_var_defaults
                    .replace(GenericVarDefaults {
                        confidence,
                        lookback_days,
                    })
                    .is_some()
                {
                    return Err(Error::Validation(
                        "duplicate ccp generic_var defaults".to_string(),
                    ));
                }
            }
            (None, None) => {}
            _ => {
                return Err(Error::Validation(
                    "ccp generic_var defaults require both confidence and lookback_days"
                        .to_string(),
                ));
            }
        }
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
    let Some(generic_var_defaults) = generic_var_defaults else {
        return Err(Error::Validation(
            "ccp generic_var defaults missing".to_string(),
        ));
    };
    Ok(ParsedCcpRegistry {
        ccp: map,
        ccp_default_params: default_params,
        ccp_generic_var_defaults: generic_var_defaults,
    })
}

fn parse_xva_defaults(value: Option<&Value>) -> Result<XvaDefaults> {
    let Some(val) = value else {
        return Err(Error::Validation(
            "xva_defaults section missing".to_string(),
        ));
    };
    let file: wire::XvaDefaultsFile = serde_json::from_value(val.clone()).map_err(to_validation)?;
    let deterministic = file.defaults.deterministic_exposure;
    let stochastic = file.defaults.stochastic_exposure;

    if deterministic.time_grid_points == 0 {
        return Err(Error::Validation(
            "xva_defaults deterministic_exposure.time_grid_points must be positive".to_string(),
        ));
    }
    validate_non_negative(
        "xva_defaults.deterministic_exposure.time_grid_step_years",
        deterministic.time_grid_step_years,
    )?;
    if deterministic.time_grid_step_years == 0.0 {
        return Err(Error::Validation(
            "xva_defaults deterministic_exposure.time_grid_step_years must be positive".to_string(),
        ));
    }
    validate_probability(
        "xva_defaults.deterministic_exposure.recovery_rate",
        deterministic.recovery_rate,
    )?;
    if let Some(own_recovery_rate) = deterministic.own_recovery_rate {
        validate_probability(
            "xva_defaults.deterministic_exposure.own_recovery_rate",
            own_recovery_rate,
        )?;
    }
    if stochastic.num_paths == 0 {
        return Err(Error::Validation(
            "xva_defaults stochastic_exposure.num_paths must be positive".to_string(),
        ));
    }
    if !stochastic.pfe_quantile.is_finite()
        || stochastic.pfe_quantile <= 0.0
        || stochastic.pfe_quantile >= 1.0
    {
        return Err(Error::Validation(
            "xva_defaults stochastic_exposure.pfe_quantile must be in (0,1)".to_string(),
        ));
    }

    Ok(XvaDefaults {
        deterministic_exposure: XvaDeterministicExposureDefaults {
            time_grid_points: deterministic.time_grid_points,
            time_grid_step_years: deterministic.time_grid_step_years,
            recovery_rate: deterministic.recovery_rate,
            own_recovery_rate: deterministic.own_recovery_rate,
        },
        stochastic_exposure: XvaStochasticExposureDefaults {
            num_paths: stochastic.num_paths,
            seed: stochastic.seed,
            pfe_quantile: stochastic.pfe_quantile,
        },
    })
}

fn parse_simm(value: Option<&Value>) -> Result<HashMap<String, SimmParams>> {
    let Some(val) = value else {
        return Err(Error::Validation("simm section missing".to_string()));
    };
    let file: wire::SimmFile = serde_json::from_value(val.clone()).map_err(to_validation)?;
    let mut map = HashMap::default();
    for entry in file.entries {
        let record = entry.record;
        let version = parse_simm_version(entry.ids.first().map(String::as_str))?;
        let ir_delta_weights = parse_number_map(&record.ir_delta_weights, "simm.ir_delta_weights")?;
        let cq_delta_weights = parse_number_map(&record.cq_delta_weights, "simm.cq_delta_weights")?;
        let commodity_bucket_weights = parse_number_map(
            &record.commodity_bucket_weights,
            "simm.commodity_bucket_weights",
        )?;

        validate_rate("simm.cnq_delta_weight", record.cnq_delta_weight)?;
        validate_rate("simm.equity_delta_weight", record.equity_delta_weight)?;
        validate_rate("simm.fx_delta_weight", record.fx_delta_weight)?;
        let fx_intra_bucket_correlation = record.fx_intra_bucket_correlation.unwrap_or(0.5);
        if !(-1.0..=1.0).contains(&fx_intra_bucket_correlation) {
            return Err(Error::Validation(
                "simm.fx_intra_bucket_correlation must be in [-1,1]".to_string(),
            ));
        }

        let mut correlations: HashMap<(SimmRiskClass, SimmRiskClass), f64> = HashMap::default();
        for cor in record.risk_class_correlations {
            let a = parse_simm_risk_class(&cor.a)?;
            let b = parse_simm_risk_class(&cor.b)?;
            if !(cor.rho >= -1.0 && cor.rho <= 1.0) {
                return Err(Error::Validation(format!(
                    "simm correlation for ({a:?},{b:?}) must be in [-1,1]"
                )));
            }
            let key = ordered_risk_class_pair(a, b);
            correlations.insert(key, cor.rho);
        }

        let ir_tenor_correlations = parse_ir_tenor_correlations(&record.ir_tenor_correlations)?;
        // ISDA SIMM v2.6 default vega weights and curvature scale factor
        // (used when an overlay omits the field). Values are validated
        // through `validate_non_negative` immediately after assignment so
        // a future typo in either the default or an explicit overlay
        // value is caught at parse time, before the SimmParams escapes
        // this function.
        let ir_vega_weight = record.ir_vega_weight.unwrap_or(0.21);
        let cq_vega_weight = record.cq_vega_weight.unwrap_or(0.27);
        let cnq_vega_weight = record.cnq_vega_weight.unwrap_or(0.27);
        let equity_vega_weight = record.equity_vega_weight.unwrap_or(0.26);
        let fx_vega_weight = record.fx_vega_weight.unwrap_or(0.30);
        let commodity_vega_weight = record.commodity_vega_weight.unwrap_or(0.36);
        let curvature_scale_factor = record.curvature_scale_factor.unwrap_or(1.5);
        validate_non_negative("simm.ir_vega_weight", ir_vega_weight)?;
        validate_non_negative("simm.cq_vega_weight", cq_vega_weight)?;
        validate_non_negative("simm.cnq_vega_weight", cnq_vega_weight)?;
        validate_non_negative("simm.equity_vega_weight", equity_vega_weight)?;
        validate_non_negative("simm.fx_vega_weight", fx_vega_weight)?;
        validate_non_negative("simm.commodity_vega_weight", commodity_vega_weight)?;
        validate_non_negative("simm.curvature_scale_factor", curvature_scale_factor)?;
        let concentration_thresholds =
            parse_concentration_thresholds(&record.concentration_thresholds)?;

        let require_cq_tables = version == SimmVersion::V2_6;
        let cq_bucket_weights =
            parse_cq_bucket_weights(&record.cq_bucket_weights, require_cq_tables)?
                .unwrap_or_else(|| default_cq_bucket_weights(&cq_delta_weights));
        let cq_intra_bucket_correlation =
            record
                .cq_intra_bucket_correlation
                .unwrap_or(if version == SimmVersion::V2_6 {
                    0.46
                } else {
                    0.42
                });
        if !(-1.0..=1.0).contains(&cq_intra_bucket_correlation) {
            return Err(Error::Validation(
                "simm.cq_intra_bucket_correlation must be in [-1,1]".to_string(),
            ));
        }
        let cq_inter_bucket_correlations = parse_cq_inter_bucket_correlations(
            &record.cq_inter_bucket_correlations,
            require_cq_tables,
        )?
        .unwrap_or_else(default_cq_inter_bucket_correlations);
        let cq_concentration_thresholds = parse_cq_concentration_thresholds(
            &record.cq_concentration_thresholds,
            require_cq_tables,
        )?
        .unwrap_or_else(|| {
            default_cq_concentration_thresholds(
                concentration_thresholds
                    .get(&SimmRiskClass::CreditQualifying)
                    .copied()
                    .unwrap_or(9_500_000.0),
            )
        });
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
            cq_delta_weights,
            cnq_delta_weight: record.cnq_delta_weight,
            equity_delta_weight: record.equity_delta_weight,
            fx_delta_weight: record.fx_delta_weight,
            fx_intra_bucket_correlation,
            risk_class_correlations: correlations,
            commodity_bucket_weights,
            ir_tenor_correlations,
            ir_inter_currency_correlation: record.ir_inter_currency_correlation.unwrap_or(0.27),
            ir_vega_weight,
            cq_vega_weight,
            cnq_vega_weight,
            equity_vega_weight,
            fx_vega_weight,
            commodity_vega_weight,
            curvature_scale_factor,
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

// -----------------------------------------------------------------------------//
// Public helper for overrides via FinstackConfig
// -----------------------------------------------------------------------------//

/// Legacy-compatible extension key used by [`finstack_quant_core::config::FinstackConfig`]
/// for margin-registry JSON overlays.
pub const MARGIN_REGISTRY_EXTENSION_KEY: &str = "valuations.margin_registry.v1";

/// Build a margin registry from a [`finstack_quant_core::config::FinstackConfig`] extension overlay.
///
/// # Arguments
///
/// * `cfg` - Config whose `extensions` map may contain [`crate::registry::MARGIN_REGISTRY_EXTENSION_KEY`]
///
/// # Returns
///
/// A new [`MarginRegistry`] built from embedded defaults plus any configured overlay.
///
/// # Errors
///
/// Returns an error if the embedded registry or the configured overlay fails validation.
pub fn margin_registry_from_config(cfg: &FinstackConfig) -> Result<MarginRegistry> {
    let overlay = cfg.extensions.get(MARGIN_REGISTRY_EXTENSION_KEY);
    build_registry(overlay)
}

// -----------------------------------------------------------------------------//
// Conversions and validations
// -----------------------------------------------------------------------------//

fn parse_schedule_asset_class(value: &str) -> Result<ScheduleAssetClass> {
    value
        .parse::<ScheduleAssetClass>()
        .map_err(|e| Error::Validation(format!("invalid schedule asset class '{value}': {e}")))
}

fn parse_maturity_bucket(value: &str) -> Result<MaturityBucket> {
    match value.to_ascii_lowercase().as_str() {
        "short" => Ok(MaturityBucket::Short),
        "medium" => Ok(MaturityBucket::Medium),
        "long" => Ok(MaturityBucket::Long),
        other => Err(Error::Validation(format!(
            "unknown maturity bucket '{other}'"
        ))),
    }
}

fn parse_collateral_asset_class(value: &str) -> Result<CollateralAssetClass> {
    value
        .parse::<CollateralAssetClass>()
        .map_err(|e| Error::Validation(format!("invalid collateral asset class '{value}': {e}")))
}

fn parse_margin_tenor(value: &str) -> Result<MarginTenor> {
    value
        .parse::<MarginTenor>()
        .map_err(|e| Error::Validation(format!("invalid margin frequency '{value}': {e}")))
}

fn parse_simm_version(id: Option<&str>) -> Result<SimmVersion> {
    match id.unwrap_or_default().to_ascii_lowercase().as_str() {
        "v2_5" => Ok(SimmVersion::V2_5),
        "v2_6" | "default" => Ok(SimmVersion::V2_6),
        other => Err(Error::Validation(format!(
            "unknown SIMM version id '{other}'"
        ))),
    }
}

fn parse_simm_risk_class(value: &str) -> Result<SimmRiskClass> {
    match value.to_ascii_lowercase().as_str() {
        "interest_rate" | "ir" => Ok(SimmRiskClass::InterestRate),
        "credit_qualifying" | "cq" => Ok(SimmRiskClass::CreditQualifying),
        "credit_non_qualifying" | "cnq" => Ok(SimmRiskClass::CreditNonQualifying),
        "equity" => Ok(SimmRiskClass::Equity),
        "commodity" => Ok(SimmRiskClass::Commodity),
        "fx" => Ok(SimmRiskClass::Fx),
        other => Err(Error::Validation(format!(
            "unknown SIMM risk class '{other}'"
        ))),
    }
}

fn parse_simm_credit_sector(value: &str) -> Result<SimmCreditSector> {
    match value.to_ascii_lowercase().as_str() {
        "ig_sovereign" | "sovereign" | "bucket_1" | "1" => Ok(SimmCreditSector::Sovereign),
        "ig_financial" | "financial" | "bucket_2" | "2" => Ok(SimmCreditSector::Financial),
        "ig_basic_materials" | "basic_materials" | "bucket_3" | "3" => {
            Ok(SimmCreditSector::BasicMaterials)
        }
        "ig_consumer" | "ig_consumer_goods" | "consumer" | "consumer_goods" | "bucket_4" | "4" => {
            Ok(SimmCreditSector::ConsumerGoods)
        }
        "ig_technology_media"
        | "technology_media"
        | "technology_telecommunications"
        | "bucket_5"
        | "5" => Ok(SimmCreditSector::TechnologyMedia),
        "ig_health_care" | "health_care" | "healthcare_utilities" | "bucket_6" | "6" => {
            Ok(SimmCreditSector::HealthCare)
        }
        "hy_sovereign" | "high_yield_sovereign" | "bucket_7" | "7" => {
            Ok(SimmCreditSector::HighYieldSovereign)
        }
        "hy_financial" | "high_yield_financial" | "bucket_8" | "8" => {
            Ok(SimmCreditSector::HighYieldFinancial)
        }
        "hy_basic_materials" | "high_yield_basic_materials" | "bucket_9" | "9" => {
            Ok(SimmCreditSector::HighYieldBasicMaterials)
        }
        "hy_consumer" | "hy_consumer_goods" | "high_yield_consumer" | "bucket_10" | "10" => {
            Ok(SimmCreditSector::HighYieldConsumerGoods)
        }
        "hy_technology_media" | "high_yield_technology_media" | "bucket_11" | "11" => {
            Ok(SimmCreditSector::HighYieldTechnologyMedia)
        }
        "hy_health_care" | "high_yield_health_care" | "bucket_12" | "12" => {
            Ok(SimmCreditSector::HighYieldHealthCare)
        }
        "index" => Ok(SimmCreditSector::Index),
        "securitized" | "securitised" => Ok(SimmCreditSector::Securitized),
        "residual" => Ok(SimmCreditSector::Residual),
        other => Err(Error::Validation(format!(
            "unknown SIMM credit qualifying sector '{other}'"
        ))),
    }
}

fn parse_number_map(value: &Value, context: &str) -> Result<HashMap<String, f64>> {
    let obj = value
        .as_object()
        .ok_or_else(|| Error::Validation(format!("{context} must be an object")))?;
    let mut out = HashMap::default();
    for (k, v) in obj {
        let num = v.as_f64().ok_or_else(|| {
            Error::Validation(format!("{context} value for key '{k}' must be a number"))
        })?;
        // Reject NaN / ±infinity at the trust boundary so a malformed
        // overlay cannot inject non-finite values into a HashMap whose
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

fn parse_cq_bucket_weights(
    value: &Value,
    required: bool,
) -> Result<Option<HashMap<SimmCreditSector, f64>>> {
    parse_credit_sector_number_map(value, "simm.cq_bucket_weights", required)
}

fn parse_cq_concentration_thresholds(
    value: &Value,
    required: bool,
) -> Result<Option<HashMap<SimmCreditSector, f64>>> {
    parse_credit_sector_number_map(value, "simm.cq_concentration_thresholds", required)
}

fn parse_credit_sector_number_map(
    value: &Value,
    context: &str,
    required: bool,
) -> Result<Option<HashMap<SimmCreditSector, f64>>> {
    if value.is_null() {
        return if required {
            Err(Error::Validation(format!("{context} missing")))
        } else {
            Ok(None)
        };
    }
    let obj = value
        .as_object()
        .ok_or_else(|| Error::Validation(format!("{context} must be an object")))?;
    let mut out = HashMap::default();
    for (k, v) in obj {
        let sector = parse_simm_credit_sector(k)?;
        let num = v.as_f64().ok_or_else(|| {
            Error::Validation(format!("{context} value for key '{k}' must be a number"))
        })?;
        // Reject NaN / ±infinity / negatives at the trust boundary;
        // bucket weights and concentration thresholds are always
        // non-negative finite quantities.
        validate_non_negative(&format!("{context}[{k}]"), num)?;
        out.insert(sector, num);
    }
    Ok(Some(out))
}

fn parse_cq_inter_bucket_correlations(
    value: &Value,
    required: bool,
) -> Result<Option<HashMap<(SimmCreditSector, SimmCreditSector), f64>>> {
    if value.is_null() {
        return if required {
            Err(Error::Validation(
                "simm.cq_inter_bucket_correlations missing".to_string(),
            ))
        } else {
            Ok(None)
        };
    }
    let obj = value.as_object().ok_or_else(|| {
        Error::Validation("simm.cq_inter_bucket_correlations must be an object".to_string())
    })?;
    let mut out = HashMap::default();
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
            ordered_credit_sector_pair(parse_simm_credit_sector(a)?, parse_simm_credit_sector(b)?),
            rho,
        );
    }
    Ok(Some(out))
}

/// Parse the `simm.ir_tenor_correlations` overlay map.
///
/// Keys use a single underscore as the pair separator
/// (`"tenor_a_tenor_b"`); tenor labels themselves must therefore not
/// contain `_`. The embedded registry uses ISDA-standard labels (`2w`,
/// `1m`, `3m`, ..., `30y`) which all satisfy this invariant.
fn parse_ir_tenor_correlations(value: &Value) -> Result<HashMap<(String, String), f64>> {
    if value.is_null() {
        return Ok(HashMap::default());
    }
    let obj = value
        .as_object()
        .ok_or_else(|| Error::Validation("ir_tenor_correlations must be an object".to_string()))?;
    let mut out = HashMap::default();
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

fn parse_concentration_thresholds(value: &Value) -> Result<HashMap<SimmRiskClass, f64>> {
    if value.is_null() {
        return Ok(HashMap::default());
    }
    let obj = value.as_object().ok_or_else(|| {
        Error::Validation("concentration_thresholds must be an object".to_string())
    })?;
    let mut out = HashMap::default();
    for (k, v) in obj {
        let rc = parse_simm_risk_class(k)?;
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

fn to_im_method(record: &wire::ImMethodDefaultsRecord) -> ImMethodDefaults {
    ImMethodDefaults {
        mpor_days: record.mpor_days,
        threshold: record.threshold,
        mta: record.mta,
        segregated: record.segregated,
    }
}

fn to_timing(record: &wire::MarginCallTimingRecord) -> MarginCallTiming {
    MarginCallTiming {
        notification_deadline_hours: record.notification_deadline_hours,
        response_deadline_hours: record.response_deadline_hours,
        dispute_resolution_days: record.dispute_resolution_days,
        delivery_grace_days: record.delivery_grace_days,
    }
}

/// Build legacy per-sector bucket weights from the existing broad `cq_delta_weights` map.
///
/// Used only for non-v2.6 parameter sets or overlays that pre-date the explicit
/// CQ bucket tables. SIMM v2.6 requires registry-backed tables and does not use
/// this fallback.
fn default_cq_bucket_weights(
    cq_delta_weights: &HashMap<String, f64>,
) -> HashMap<SimmCreditSector, f64> {
    let sov = cq_delta_weights.get("sovereigns").copied().unwrap_or(85.0);
    let fin = cq_delta_weights.get("financials").copied().unwrap_or(85.0);
    let corp = cq_delta_weights.get("corporates").copied().unwrap_or(73.0);

    [
        (SimmCreditSector::Sovereign, sov),
        (SimmCreditSector::Financial, fin),
        (SimmCreditSector::BasicMaterials, corp),
        (SimmCreditSector::ConsumerGoods, corp),
        (SimmCreditSector::TechnologyMedia, corp),
        (SimmCreditSector::HealthCare, corp),
        (SimmCreditSector::HighYieldSovereign, sov),
        (SimmCreditSector::HighYieldFinancial, fin),
        (SimmCreditSector::HighYieldBasicMaterials, corp),
        (SimmCreditSector::HighYieldConsumerGoods, corp),
        (SimmCreditSector::HighYieldTechnologyMedia, corp),
        (SimmCreditSector::HighYieldHealthCare, corp),
        (SimmCreditSector::Residual, 500.0),
    ]
    .into_iter()
    .collect()
}

/// Build legacy inter-bucket correlations for credit qualifying sectors.
///
/// Uses a simplified single correlation value of 0.27 across all sector pairs.
/// SIMM v2.6 requires explicit registry tables and does not use this fallback.
fn default_cq_inter_bucket_correlations() -> HashMap<(SimmCreditSector, SimmCreditSector), f64> {
    let mut map = HashMap::default();
    let sectors = simm_cq_validation_sectors();
    for (i, &a) in sectors.iter().enumerate() {
        for &b in sectors.iter().skip(i + 1) {
            let rho = if a == SimmCreditSector::Residual || b == SimmCreditSector::Residual {
                0.0
            } else {
                0.27
            };
            let key = ordered_credit_sector_pair(a, b);
            map.insert(key, rho);
        }
    }
    map
}

/// Build default per-bucket concentration thresholds for credit qualifying.
///
/// Uses the aggregate CQ concentration threshold for each bucket as a legacy
/// fallback. SIMM v2.6 requires explicit registry tables and does not use this
/// fallback.
fn default_cq_concentration_thresholds(aggregate_threshold: f64) -> HashMap<SimmCreditSector, f64> {
    simm_cq_validation_sectors()
        .into_iter()
        .map(|sector| (sector, aggregate_threshold))
        .collect()
}

fn simm_cq_validation_sectors() -> [SimmCreditSector; 13] {
    use SimmCreditSector::*;
    [
        Sovereign,
        Financial,
        BasicMaterials,
        ConsumerGoods,
        TechnologyMedia,
        HealthCare,
        HighYieldSovereign,
        HighYieldFinancial,
        HighYieldBasicMaterials,
        HighYieldConsumerGoods,
        HighYieldTechnologyMedia,
        HighYieldHealthCare,
        Residual,
    ]
}

fn validate_simm_params(p: &SimmParams) -> Result<()> {
    if p.mpor_days == 0 {
        return Err(Error::Validation(
            "simm mpor_days must be greater than zero".to_string(),
        ));
    }
    for (k, v) in &p.ir_delta_weights {
        validate_non_negative(&format!("simm.ir_delta_weights[{k}]"), *v)?;
    }
    for (k, v) in &p.cq_delta_weights {
        validate_non_negative(&format!("simm.cq_delta_weights[{k}]"), *v)?;
    }
    for (k, v) in &p.commodity_bucket_weights {
        validate_non_negative(&format!("simm.commodity_bucket_weights[{k}]"), *v)?;
    }
    validate_non_negative("simm.cnq_delta_weight", p.cnq_delta_weight)?;
    validate_non_negative("simm.equity_delta_weight", p.equity_delta_weight)?;
    validate_non_negative("simm.fx_delta_weight", p.fx_delta_weight)?;
    if !(-1.0..=1.0).contains(&p.fx_intra_bucket_correlation) {
        return Err(Error::Validation(
            "simm.fx_intra_bucket_correlation must be in [-1,1]".to_string(),
        ));
    }
    validate_non_negative("simm.ir_vega_weight", p.ir_vega_weight)?;
    validate_non_negative("simm.cq_vega_weight", p.cq_vega_weight)?;
    validate_non_negative("simm.cnq_vega_weight", p.cnq_vega_weight)?;
    validate_non_negative("simm.equity_vega_weight", p.equity_vega_weight)?;
    validate_non_negative("simm.fx_vega_weight", p.fx_vega_weight)?;
    validate_non_negative("simm.commodity_vega_weight", p.commodity_vega_weight)?;
    validate_non_negative("simm.curvature_scale_factor", p.curvature_scale_factor)?;
    for (rc, v) in &p.concentration_thresholds {
        validate_non_negative(&format!("simm.concentration_thresholds[{rc:?}]"), *v)?;
    }
    validate_cq_tables(p)?;
    validate_simm_correlations_psd(p)?;
    Ok(())
}

fn validate_cq_tables(p: &SimmParams) -> Result<()> {
    let sectors = simm_cq_validation_sectors();
    for sector in sectors {
        let require_v26 = p.version == SimmVersion::V2_6;
        let weight = p.cq_bucket_weights.get(&sector).copied();
        if require_v26 && weight.is_none() {
            return Err(Error::Validation(format!(
                "SIMM registry {:?}: cq_bucket_weights missing {sector:?}",
                p.version
            )));
        }
        if let Some(v) = weight {
            validate_non_negative(&format!("simm.cq_bucket_weights[{sector:?}]"), v)?;
        }

        let threshold = p.cq_concentration_thresholds.get(&sector).copied();
        if require_v26 && threshold.is_none() {
            return Err(Error::Validation(format!(
                "SIMM registry {:?}: cq_concentration_thresholds missing {sector:?}",
                p.version
            )));
        }
        if let Some(v) = threshold {
            validate_non_negative(&format!("simm.cq_concentration_thresholds[{sector:?}]"), v)?;
        }
    }

    for (i, &a) in sectors.iter().enumerate() {
        for &b in sectors.iter().skip(i + 1) {
            let key = ordered_credit_sector_pair(a, b);
            let rho = p.cq_inter_bucket_correlations.get(&key).copied();
            if p.version == SimmVersion::V2_6 && rho.is_none() {
                return Err(Error::Validation(format!(
                    "SIMM registry {:?}: cq_inter_bucket_correlations missing ({a:?},{b:?})",
                    p.version
                )));
            }
            if let Some(v) = rho {
                if !(-1.0..=1.0).contains(&v) {
                    return Err(Error::Validation(format!(
                        "simm.cq_inter_bucket_correlations[({a:?},{b:?})] must be in [-1,1]"
                    )));
                }
            }
        }
    }
    Ok(())
}

/// Validate that SIMM correlation matrices are positive semi-definite.
///
/// Hand-typed or overlay-supplied correlation matrices can become non-PSD through
/// a single typo (e.g. swapping two adjacent off-diagonals). A non-PSD correlation
/// matrix propagates silently through `K_b = sqrt(Σ WSᵢ² + Σ ρᵢⱼ WSᵢ WSⱼ)` — the
/// sum under the sqrt clamps to zero (or worse, aggregates a negative variance)
/// producing a regulatory miscalculation without an obvious failure signal.
///
/// Checks the three correlation matrices the SIMM calculator consumes:
///
/// 1. `risk_class_correlations` — 6×6 matrix across `SimmRiskClass`.
/// 2. `ir_tenor_correlations` — n×n matrix over the `ir_delta_weights` tenor keys.
/// 3. `cq_inter_bucket_correlations` — full credit-qualifying bucket matrix across
///    `SimmCreditSector`.
///
/// Missing off-diagonal entries are filled with the calculator's own fallback
/// value (see `SimmParams::correlation` / `cq_inter_bucket_correlation`) so the
/// validated matrix is the one the calculator will actually observe at runtime.
fn validate_simm_correlations_psd(p: &SimmParams) -> Result<()> {
    use crate::SimmRiskClass::{
        Commodity, CreditNonQualifying, CreditQualifying, Equity, Fx, InterestRate,
    };

    // 1. Cross-risk-class matrix (6x6). Missing pairs fall back to 1.0 per the
    //    calculator's `SimmParams::correlation` — validate the effective matrix.
    let risk_classes = [
        InterestRate,
        CreditQualifying,
        CreditNonQualifying,
        Equity,
        Commodity,
        Fx,
    ];
    validate_dense_correlation_matrix(
        &risk_classes,
        "risk_class_correlations",
        p.version,
        |a, b| {
            let key = ordered_risk_class_pair(*a, *b);
            p.risk_class_correlations.get(&key).copied().unwrap_or(1.0)
        },
    )?;

    // 2. IR tenor correlations — all pairs guaranteed present by the earlier
    //    completeness check in `simm.rs::validate_simm_params`, but we re-assert
    //    here via the same fallback path (1.0) so the registry-level check is
    //    self-contained.
    let tenors: Vec<&str> = p
        .ir_delta_weights
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let mut tenors_sorted = tenors.clone();
    tenors_sorted.sort_unstable();
    validate_dense_correlation_matrix(
        &tenors_sorted,
        "ir_tenor_correlations",
        p.version,
        |a, b| {
            let key = ordered_tenor_pair(a, b);
            p.ir_tenor_correlations.get(&key).copied().unwrap_or(1.0)
        },
    )?;

    // 3. CQ inter-bucket correlations over the full v2.6 bucket set.
    //    Missing legacy pairs are treated as uncorrelated.
    let sectors = simm_cq_validation_sectors();
    validate_dense_correlation_matrix(
        &sectors,
        "cq_inter_bucket_correlations",
        p.version,
        |a, b| {
            let key = ordered_credit_sector_pair(*a, *b);
            p.cq_inter_bucket_correlations
                .get(&key)
                .copied()
                .unwrap_or(0.0)
        },
    )?;

    // 4. Commodity inter-bucket correlations (17x17). Already dense in
    //    row-major form, loaded from the SIMM registry, and validated directly
    //    without densification.
    let n = crate::calculators::im::simm::COMMODITY_BUCKET_COUNT;
    let expected_len = n * n;
    if p.commodity_inter_bucket_correlations.len() != expected_len {
        return Err(Error::Validation(format!(
            "SIMM registry {:?}: commodity_inter_bucket_correlations has {} entries, expected {n}x{n} = {expected_len}",
            p.version,
            p.commodity_inter_bucket_correlations.len()
        )));
    }
    finstack_quant_core::math::linalg::validate_correlation_matrix(
        &p.commodity_inter_bucket_correlations,
        n,
    )
    .map_err(|_| {
        Error::Validation(format!(
            "SIMM registry {:?}: commodity_inter_bucket_correlations is not a valid {n}x{n} correlation matrix (failed diagonal / range / symmetry / PSD check)",
            p.version
        ))
    })?;

    Ok(())
}

/// Build a dense symmetric correlation matrix from a sparse lookup and validate it.
///
/// The diagonal is fixed at 1.0; off-diagonals are sourced from `lookup`. The resulting
/// matrix is handed to [`finstack_quant_core::math::linalg::validate_correlation_matrix`],
/// which checks diagonal exactness, off-diagonal range, symmetry, and positive
/// semi-definiteness.
///
/// On failure, the error message names the matrix and the SIMM version so a bad
/// registry overlay can be traced back to the exact parameter set at load time.
fn validate_dense_correlation_matrix<T, F>(
    keys: &[T],
    matrix_name: &str,
    version: crate::SimmVersion,
    lookup: F,
) -> Result<()>
where
    F: Fn(&T, &T) -> f64,
{
    let n = keys.len();
    if n < 2 {
        return Ok(());
    }
    let mut matrix = vec![0.0; n * n];
    for (i, a) in keys.iter().enumerate() {
        matrix[i * n + i] = 1.0;
        for (j, b) in keys.iter().enumerate().skip(i + 1) {
            let rho = lookup(a, b);
            matrix[i * n + j] = rho;
            matrix[j * n + i] = rho;
        }
    }
    finstack_quant_core::math::linalg::validate_correlation_matrix(&matrix, n).map_err(|_| {
        Error::Validation(format!(
            "SIMM registry {version:?}: {matrix_name} is not a valid correlation matrix (failed diagonal / range / symmetry / PSD check — see finstack_quant_core::math::linalg::validate_correlation_matrix)"
        ))
    })
}

// Generic numeric range validators live in `registry::validation`.

// -----------------------------------------------------------------------------//
// Convenience helpers for constructing Money amounts from defaults
// -----------------------------------------------------------------------------//

impl VmDefaults {
    /// Convert raw VM defaults into currency-tagged [`VmParameters`].
    ///
    /// # Arguments
    ///
    /// * `currency` - Currency used to wrap raw numeric defaults in [`finstack_quant_core::money::Money`]
    ///
    /// # Returns
    ///
    /// Concrete VM parameters in `currency`.
    pub fn to_vm_params(&self, currency: finstack_quant_core::currency::Currency) -> VmParameters {
        VmParameters {
            threshold: Money::new(self.threshold, currency),
            mta: Money::new(self.mta, currency),
            rounding: Money::new(self.rounding, currency),
            independent_amount: Money::new(self.independent_amount, currency),
            frequency: self.frequency,
            settlement_lag: self.settlement_lag,
        }
    }
}

impl ImMethodDefaults {
    /// Convert raw IM defaults into currency-tagged [`ImParameters`].
    ///
    /// # Arguments
    ///
    /// * `methodology` - IM methodology label for the returned parameter set
    /// * `currency` - Currency used to wrap raw numeric defaults in [`finstack_quant_core::money::Money`]
    ///
    /// # Returns
    ///
    /// Concrete IM parameters in `currency`.
    pub fn to_im_params(
        &self,
        methodology: ImMethodology,
        currency: finstack_quant_core::currency::Currency,
    ) -> ImParameters {
        ImParameters {
            methodology,
            mpor_days: self.mpor_days,
            threshold: Money::new(self.threshold, currency),
            mta: Money::new(self.mta, currency),
            segregated: self.segregated,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_registry_loads_successfully() {
        let registry = embedded_registry().expect("embedded registry should load");

        assert!(
            !registry.schedule_im.is_empty(),
            "schedule_im should have entries"
        );
        assert!(
            registry.schedule_im.contains_key("bcbs_iosco"),
            "bcbs_iosco schedule should be present"
        );

        assert!(
            !registry.collateral_schedules.is_empty(),
            "collateral_schedules should have entries"
        );
        assert!(
            registry.collateral_schedules.contains_key("bcbs_standard"),
            "bcbs_standard collateral schedule should be present"
        );
        assert!(
            registry.collateral_schedules.contains_key("cash_only"),
            "cash_only collateral schedule should be present"
        );

        assert!(
            !registry.collateral_asset_class_defaults.is_empty(),
            "collateral_asset_class_defaults should have entries"
        );
        assert!(
            registry
                .collateral_asset_class_defaults
                .contains_key(&CollateralAssetClass::Cash),
            "Cash default should be present"
        );

        assert!(
            registry.simm.contains_key("v2_6"),
            "v2_6 SIMM params should be present"
        );

        assert!(!registry.ccp.is_empty(), "ccp should have entries");
        assert!(
            registry.ccp_default_params.mpor_days > 0,
            "ccp default params should be resolved"
        );
        assert!(
            registry.ccp_generic_var_defaults.confidence > 0.0,
            "generic VaR confidence should be resolved"
        );
        assert!(
            registry.ccp_generic_var_defaults.lookback_days > 0,
            "generic VaR lookback should be resolved"
        );
        assert!(
            registry.xva.deterministic_exposure.time_grid_points > 0,
            "xva time-grid points should be resolved"
        );
        assert!(
            registry.xva.stochastic_exposure.num_paths > 0,
            "xva stochastic path count should be resolved"
        );

        assert!(
            registry.defaults.im.simm.mpor_days > 0,
            "simm mpor_days should be positive"
        );
    }

    #[test]
    fn simm_params_have_required_weights() {
        let registry = embedded_registry().expect("embedded registry should load");
        let params = registry.simm.get("v2_6").expect("v2_6 simm params");

        assert!(
            params.ir_delta_weights.contains_key("5y"),
            "5y IR weight should be present"
        );
        assert!(
            params.cq_delta_weights.contains_key("corporates"),
            "corporates CQ weight should be present"
        );
        assert!(params.cnq_delta_weight > 0.0, "CNQ weight should be > 0");
        assert!(
            params.equity_delta_weight > 0.0,
            "equity weight should be > 0"
        );
        assert!(params.fx_delta_weight > 0.0, "FX weight should be > 0");
        assert!(
            !params.risk_class_correlations.is_empty(),
            "risk class correlations should be populated"
        );
    }

    // ---- PSD validation at registry load --------------------------------------

    /// Clone the embedded v2_6 SIMM params for mutation in PSD tests.
    fn base_simm_params() -> SimmParams {
        let registry = embedded_registry().expect("embedded registry should load");
        registry.simm.get("v2_6").expect("v2_6 simm params").clone()
    }

    #[test]
    fn psd_check_accepts_embedded_registry() {
        // Regression guard: shipped ISDA correlation matrices must continue to
        // satisfy PSD after the new check is wired in.
        let params = base_simm_params();
        validate_simm_correlations_psd(&params).expect("embedded SIMM correlations must be PSD");
    }

    #[test]
    fn simm_v26_credit_qualifying_tables_are_registry_backed() {
        let registry = embedded_registry().expect("embedded registry should load");
        let params = registry.simm.get("v2_6").expect("v2_6 params");

        assert_eq!(
            params
                .cq_bucket_weights
                .get(&SimmCreditSector::Sovereign)
                .copied(),
            Some(75.0)
        );
        assert_eq!(
            params
                .cq_inter_bucket_correlations
                .get(&ordered_credit_sector_pair(
                    SimmCreditSector::Sovereign,
                    SimmCreditSector::Financial
                ))
                .copied(),
            Some(0.38)
        );
        assert_eq!(
            params
                .cq_concentration_thresholds
                .get(&SimmCreditSector::Sovereign)
                .copied(),
            Some(1_000_000.0)
        );
    }

    #[test]
    fn psd_check_rejects_non_psd_risk_class_matrix() {
        // Non-PSD 3-way pattern: ρ(IR,CQ)=0.9, ρ(IR,EQ)=0.9, ρ(CQ,EQ)=-0.9.
        // Determinant of the embedded 3x3 submatrix is -2.888 < 0 → one negative
        // eigenvalue → NOT positive semi-definite, even though every entry is in
        // [-1, 1]. Exactly the silent-propagation failure mode the PSD
        // check targets.
        let mut params = base_simm_params();
        params.risk_class_correlations.insert(
            ordered_risk_class_pair(SimmRiskClass::InterestRate, SimmRiskClass::CreditQualifying),
            0.9,
        );
        params.risk_class_correlations.insert(
            ordered_risk_class_pair(SimmRiskClass::InterestRate, SimmRiskClass::Equity),
            0.9,
        );
        params.risk_class_correlations.insert(
            ordered_risk_class_pair(SimmRiskClass::CreditQualifying, SimmRiskClass::Equity),
            -0.9,
        );

        let err = validate_simm_correlations_psd(&params)
            .expect_err("non-PSD risk-class matrix must be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("risk_class_correlations"),
            "error should name the offending matrix: {msg}"
        );
    }

    #[test]
    fn psd_check_rejects_non_psd_cq_inter_bucket_matrix() {
        // Same 3-way non-PSD pattern applied to the CQ inter-bucket matrix.
        let mut params = base_simm_params();
        params.cq_inter_bucket_correlations.insert(
            ordered_credit_sector_pair(SimmCreditSector::Sovereign, SimmCreditSector::Financial),
            0.9,
        );
        params.cq_inter_bucket_correlations.insert(
            ordered_credit_sector_pair(
                SimmCreditSector::Sovereign,
                SimmCreditSector::BasicMaterials,
            ),
            0.9,
        );
        params.cq_inter_bucket_correlations.insert(
            ordered_credit_sector_pair(
                SimmCreditSector::Financial,
                SimmCreditSector::BasicMaterials,
            ),
            -0.9,
        );

        let err = validate_simm_correlations_psd(&params)
            .expect_err("non-PSD CQ inter-bucket matrix must be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("cq_inter_bucket_correlations"),
            "error should name the offending matrix: {msg}"
        );
    }

    #[test]
    fn psd_check_accepts_registry_commodity_matrix() {
        // Regression guard: the ISDA SIMM commodity matrix loaded from the
        // registry must continue to pass PSD validation after future edits.
        let params = base_simm_params();
        validate_simm_correlations_psd(&params)
            .expect("embedded commodity 17x17 matrix must be PSD");
    }

    #[test]
    fn psd_check_rejects_non_psd_commodity_matrix() {
        // Same 3-way non-PSD pattern applied to the commodity matrix.
        // Write (i,j) and (j,i) symmetrically so only the PSD branch fires.
        let mut params = base_simm_params();
        let n = crate::calculators::im::simm::COMMODITY_BUCKET_COUNT;
        let set = |m: &mut Vec<f64>, i: usize, j: usize, v: f64| {
            m[i * n + j] = v;
            m[j * n + i] = v;
        };
        set(&mut params.commodity_inter_bucket_correlations, 0, 1, 0.9);
        set(&mut params.commodity_inter_bucket_correlations, 0, 2, 0.9);
        set(&mut params.commodity_inter_bucket_correlations, 1, 2, -0.9);

        let err = validate_simm_correlations_psd(&params)
            .expect_err("non-PSD commodity matrix must be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("commodity_inter_bucket_correlations"),
            "error should name the offending matrix: {msg}"
        );
    }

    #[test]
    fn psd_check_rejects_wrong_shape_commodity_matrix() {
        let mut params = base_simm_params();
        params.commodity_inter_bucket_correlations.truncate(16 * 16);
        let err = validate_simm_correlations_psd(&params)
            .expect_err("wrong-shape commodity matrix must be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("commodity_inter_bucket_correlations") && msg.contains("expected"),
            "error should name the matrix and expected shape: {msg}"
        );
    }

    #[test]
    fn psd_check_rejects_out_of_range_correlation() {
        // |ρ| > 1 is a range-check failure (validate_correlation_matrix catches
        // it before the Cholesky stage). Still routed through the PSD validator
        // so the registry rejects at load time rather than producing complex
        // sqrt downstream.
        let mut params = base_simm_params();
        params.risk_class_correlations.insert(
            ordered_risk_class_pair(SimmRiskClass::InterestRate, SimmRiskClass::Equity),
            1.5,
        );
        let err = validate_simm_correlations_psd(&params).expect_err("|ρ| > 1 must be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("risk_class_correlations"),
            "error should name the offending matrix: {msg}"
        );
    }

    #[test]
    fn parse_credit_sector_number_map_rejects_negative_values() {
        // Use serde_json::json! which permits regular floats; -5.0
        // exercises the validate_non_negative call, not the finiteness
        // check.
        let val = serde_json::json!({
            "sovereign": -5.0,
        });
        let err = parse_credit_sector_number_map(&val, "simm.cq_bucket_weights", false)
            .expect_err("negative bucket weight must be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("simm.cq_bucket_weights"),
            "error should name the offending field: {msg}"
        );
    }

    #[test]
    fn parse_concentration_thresholds_rejects_negative() {
        let val = serde_json::json!({
            "interest_rate": -10.0,
        });
        let err = parse_concentration_thresholds(&val)
            .expect_err("negative concentration threshold must be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("concentration_thresholds"),
            "error should name the offending field: {msg}"
        );
    }

    #[test]
    fn parse_ir_tenor_correlations_rejects_out_of_range() {
        let val = serde_json::json!({
            "1y_5y": 1.5,
        });
        let err = parse_ir_tenor_correlations(&val).expect_err("|ρ| > 1 must be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("ir_tenor_correlations") && msg.contains("[-1,1]"),
            "error should name the offending field and range: {msg}"
        );
    }

    #[test]
    fn parse_ir_tenor_correlations_rejects_malformed_key() {
        // No underscore separator → parse error mentioning the
        // tenor-label-no-underscore convention.
        let val = serde_json::json!({
            "5y": 0.5,
        });
        let err = parse_ir_tenor_correlations(&val).expect_err("malformed key must be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("expected format 'tenor1_tenor2'"),
            "error should describe the expected format: {msg}"
        );
    }

    #[test]
    fn schedule_im_rejects_implausible_default_maturity() {
        // Synthesise a minimal ScheduleImFile with a bogus
        // default_maturity_years = 1000.0 and verify the parser bails.
        let val = serde_json::json!({
            "schema": "schedule_im.v1",
            "version": 1,
            "entries": [{
                "ids": ["test"],
                "record": {
                    "default_rate": 0.04,
                    "default_asset_class": "interest_rate",
                    "default_maturity_years": 1000.0,
                    "mpor_days": 10,
                    "bucket_boundaries_years": {
                        "short_to_medium": 2.0,
                        "medium_to_long": 5.0,
                    },
                    "rates": [],
                }
            }]
        });
        let err = parse_schedule_im(Some(&val))
            .expect_err("implausible default_maturity_years must be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("default_maturity_years"),
            "error should name the offending field: {msg}"
        );
    }

    #[test]
    fn v26_overlay_missing_cq_bucket_weights_is_rejected() {
        // Build a minimal v2.6-tagged record that omits cq_bucket_weights
        // entirely; the parser must reject because v2.6 requires the
        // explicit table (the legacy fallback is only valid for v2.5).
        let val = serde_json::json!({
            "schema": "simm.v1",
            "version": 1,
            "entries": [{
                "ids": ["v2_6"],
                "record": {
                    "ir_delta_weights": { "5y": 50.0 },
                    "cq_delta_weights": { "corporates": 73.0 },
                    "cnq_delta_weight": 169.0,
                    "equity_delta_weight": 25.0,
                    "fx_delta_weight": 8.1,
                    "fx_intra_bucket_correlation": 0.5,
                    "commodity_bucket_weights": {},
                    "risk_class_correlations": [],
                    "ir_tenor_correlations": {},
                    "concentration_thresholds": {},
                    "mpor_days": 10,
                    "cq_bucket_weights": null
                }
            }]
        });
        let result = parse_simm(Some(&val));
        assert!(
            result.is_err(),
            "v2.6 without cq_bucket_weights must be rejected, got {result:?}"
        );
    }
}
