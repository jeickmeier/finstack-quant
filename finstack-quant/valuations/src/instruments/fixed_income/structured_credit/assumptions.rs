//! Embedded structured-credit assumptions registry.

use crate::cashflow::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};
use crate::instruments::fixed_income::structured_credit::pricing::stochastic::calibrations::{
    CloCalibration, CmbsCalibration, RmbsCalibration,
};
use crate::instruments::fixed_income::structured_credit::types::{
    CreditFactors, DealFees, DealType, DefaultAssumptions,
};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Tenor;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CreditRating;
use finstack_quant_core::{Error, HashMap, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::sync::OnceLock;

const EMBEDDED_STRUCTURED_CREDIT_ASSUMPTIONS: &str =
    include_str!("../../../../data/assumptions/structured_credit_assumptions.v1.json");

static EMBEDDED_REGISTRY: OnceLock<Result<StructuredCreditAssumptionRegistry>> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StructuredCreditAssumptionRegistry {
    schema_version: String,
    market_conditions: MarketConditionsRecord,
    credit_model_defaults: CreditModelDefaultsRecord,
    cmo_collateral_defaults: CmoCollateralDefaultsRecord,
    seasonality: SeasonalityRecord,
    scenario_grids: ScenarioGridsRecord,
    simulation: SimulationRecord,
    concentration_limits: ConcentrationLimitsRecord,
    prepayment_models: PrepaymentModelsRecord,
    default_models: DefaultModelsRecord,
    stochastic_calibrations: StochasticCalibrationsRecord,
    coverage_haircuts: Vec<CoverageHaircutRecord>,
    asset_type_defaults: AssetTypeDefaultsRecord,
    deal_profiles: Vec<DealProfileRecord>,
}

impl StructuredCreditAssumptionRegistry {
    pub(crate) fn market_conditions(&self) -> (f64, Option<f64>) {
        (
            self.market_conditions.refi_rate,
            Some(self.market_conditions.seasonal_factor),
        )
    }

    pub(crate) fn default_prepayment_spec(&self) -> PrepaymentModelSpec {
        PrepaymentModelSpec::constant_cpr(self.credit_model_defaults.prepayment_cpr_annual)
    }

    pub(crate) fn default_default_spec(&self) -> DefaultModelSpec {
        DefaultModelSpec::constant_cdr(self.credit_model_defaults.default_cdr_annual)
    }

    pub(crate) fn default_recovery_spec(&self) -> RecoveryModelSpec {
        RecoveryModelSpec::with_lag(
            self.credit_model_defaults.recovery_rate,
            self.credit_model_defaults.recovery_lag_months,
        )
    }

    pub(crate) fn cmo_collateral_defaults(&self) -> CmoCollateralDefaults {
        CmoCollateralDefaults {
            wac: self.cmo_collateral_defaults.wac,
            wam_months: self.cmo_collateral_defaults.wam_months,
            servicing_fee_rate: self.cmo_collateral_defaults.servicing_fee_rate,
            guarantee_fee_rate: self.cmo_collateral_defaults.guarantee_fee_rate,
            psa_multiplier: self.cmo_collateral_defaults.psa_multiplier,
        }
    }

    pub(crate) fn psa_curve(&self) -> PsaCurveDefaults {
        PsaCurveDefaults {
            ramp_months: self.prepayment_models.psa.ramp_months,
            terminal_cpr: self.prepayment_models.psa.terminal_cpr,
        }
    }

    pub(crate) fn sda_curve(&self) -> SdaCurveDefaults {
        SdaCurveDefaults {
            peak_month: self.default_models.sda.peak_month,
            peak_cdr: self.default_models.sda.peak_cdr,
            terminal_cdr: self.default_models.sda.terminal_cdr,
        }
    }

    pub(crate) fn pool_balance_cleanup_threshold(&self) -> f64 {
        self.simulation.pool_balance_cleanup_threshold
    }

    pub(crate) fn mortgage_seasonality(&self) -> [f64; 12] {
        month_array(&self.seasonality.mortgage)
    }

    pub(crate) fn credit_card_seasonality(&self) -> [f64; 12] {
        month_array(&self.seasonality.credit_card)
    }

    pub(crate) fn standard_psa_speeds(&self) -> &[f64] {
        &self.scenario_grids.psa_speeds
    }

    pub(crate) fn standard_cdr_rates(&self) -> &[f64] {
        &self.scenario_grids.cdr_rates
    }

    pub(crate) fn standard_severity_rates(&self) -> &[f64] {
        &self.scenario_grids.severity_rates
    }

    pub(crate) fn simulation_defaults(&self) -> SimulationDefaults {
        SimulationDefaults {
            pool_balance_cleanup_threshold: self.simulation.pool_balance_cleanup_threshold,
            resolution_lag_months: self.simulation.resolution_lag_months,
            burnout_threshold_months: self.simulation.burnout_threshold_months,
            baseline_unemployment_rate: self.simulation.baseline_unemployment_rate,
        }
    }

    pub(crate) fn concentration_limits(&self) -> ConcentrationLimits {
        ConcentrationLimits {
            max_obligor_concentration: self.concentration_limits.max_obligor_concentration,
            max_top5_concentration: self.concentration_limits.max_top5_concentration,
            max_top10_concentration: self.concentration_limits.max_top10_concentration,
            max_second_lien: self.concentration_limits.max_second_lien,
            max_cov_lite: self.concentration_limits.max_cov_lite,
            max_dip: self.concentration_limits.max_dip,
        }
    }

    pub(crate) fn auto_abs_prepayment(&self) -> AutoAbsPrepaymentDefaults {
        AutoAbsPrepaymentDefaults {
            monthly_speed: self.prepayment_models.auto_abs.monthly_speed,
            ramp_months: self.prepayment_models.auto_abs.ramp_months,
        }
    }

    pub(crate) fn rmbs_stochastic_calibration(&self, id: &str) -> Result<RmbsCalibration> {
        let record = self
            .stochastic_calibrations
            .rmbs_profiles
            .iter()
            .find(|record| has_id(&record.ids, id))
            .ok_or_else(|| not_found("RMBS stochastic calibration", id))?;
        Ok(RmbsCalibration {
            base_cdr: record.base_cdr,
            default_correlation: record.default_correlation,
            base_cpr: record.base_cpr,
            prepay_factor_loading: record.prepay_factor_loading,
            cpr_volatility: record.cpr_volatility,
            default_factor_sensitivity: record.default_factor_sensitivity,
            default_mean_reversion: record.default_mean_reversion,
            default_volatility: record.default_volatility,
            refi_sensitivity: record.refi_sensitivity,
            burnout_rate: record.burnout_rate,
        })
    }

    pub(crate) fn clo_stochastic_calibration(&self, id: &str) -> Result<CloCalibration> {
        let record = self
            .stochastic_calibrations
            .clo_profiles
            .iter()
            .find(|record| has_id(&record.ids, id))
            .ok_or_else(|| not_found("CLO stochastic calibration", id))?;
        Ok(CloCalibration {
            base_cdr: record.base_cdr,
            default_correlation: record.default_correlation,
            base_cpr: record.base_cpr,
            prepay_factor_loading: record.prepay_factor_loading,
            cpr_volatility: record.cpr_volatility,
            default_factor_sensitivity: record.default_factor_sensitivity,
            default_mean_reversion: record.default_mean_reversion,
            default_volatility: record.default_volatility,
        })
    }

    pub(crate) fn cmbs_stochastic_calibration(&self, id: &str) -> Result<CmbsCalibration> {
        let record = self
            .stochastic_calibrations
            .cmbs_profiles
            .iter()
            .find(|record| has_id(&record.ids, id))
            .ok_or_else(|| not_found("CMBS stochastic calibration", id))?;
        Ok(CmbsCalibration {
            base_cdr: record.base_cdr,
            default_correlation: record.default_correlation,
            base_cpr: record.base_cpr,
            prepay_factor_loading: record.prepay_factor_loading,
            cpr_volatility: record.cpr_volatility,
        })
    }

    pub(crate) fn coverage_haircuts(&self) -> HashMap<CreditRating, f64> {
        let mut haircuts = HashMap::default();
        for record in &self.coverage_haircuts {
            haircuts.insert(record.rating, record.haircut);
        }
        haircuts
    }

    pub(crate) fn deal_fees(&self, id: &str, base_currency: Currency) -> Result<DealFees> {
        let fees = &self.deal_profile(id)?.fees;
        Ok(DealFees {
            trustee_fee_annual: Money::new(fees.trustee_fee_annual, base_currency),
            senior_mgmt_fee_bps: fees.senior_mgmt_fee_bps,
            subordinated_mgmt_fee_bps: fees.subordinated_mgmt_fee_bps,
            servicing_fee_bps: fees.servicing_fee_bps,
            master_servicer_fee_bps: fees.master_servicer_fee_bps,
            special_servicer_fee_bps: fees.special_servicer_fee_bps,
        })
    }

    pub(crate) fn default_assumptions(&self, id: &str) -> Result<DefaultAssumptions> {
        Ok(default_assumptions_from_record(
            &self.deal_profile(id)?.assumptions,
            HashMap::default(),
            HashMap::default(),
            HashMap::default(),
        ))
    }

    pub(crate) fn generic_default_assumptions(&self) -> DefaultAssumptions {
        default_assumptions_from_record(
            &self.asset_type_defaults.assumptions(),
            assumption_map(&self.asset_type_defaults.cpr_by_asset_type),
            assumption_map(&self.asset_type_defaults.cdr_by_asset_type),
            assumption_map(&self.asset_type_defaults.recovery_by_asset_type),
        )
    }

    pub(crate) fn constructor_defaults(&self, id: &str) -> Result<ConstructorDefaults> {
        let profile = self.deal_profile(id)?;
        Ok(ConstructorDefaults {
            first_payment_month: profile.constructor.first_payment_month,
            frequency: profile.constructor.frequency.tenor(),
            prepayment_spec: profile.constructor.prepayment.spec(),
            default_spec: DefaultModelSpec::constant_cdr(profile.constructor.default_cdr_annual),
            recovery_spec: RecoveryModelSpec::with_lag(
                profile.constructor.recovery_rate,
                profile.constructor.recovery_lag_months,
            ),
            credit_factors: CreditFactors {
                ltv: profile.constructor.ltv,
                ..Default::default()
            },
        })
    }

    pub(crate) fn profile_id_for_deal_type(&self, deal_type: DealType) -> &'static str {
        match deal_type {
            DealType::CLO => "clo_standard",
            DealType::RMBS => "rmbs_standard",
            DealType::CMBS => "cmbs_standard",
            _ => "abs_auto_standard",
        }
    }

    fn deal_profile(&self, id: &str) -> Result<&DealProfileRecord> {
        self.deal_profiles
            .iter()
            .find(|profile| profile.ids.iter().any(|candidate| candidate == id))
            .ok_or_else(|| not_found("structured-credit deal profile", id))
    }

    fn validate(&self) -> Result<()> {
        if self.schema_version != "finstack_quant.structured_credit_assumptions/1" {
            return Err(Error::Validation(format!(
                "unsupported structured-credit assumptions schema version '{}'",
                self.schema_version
            )));
        }
        validate_unit_interval(self.market_conditions.refi_rate, "refi rate")?;
        validate_unit_interval(self.market_conditions.seasonal_factor, "seasonal factor")?;
        validate_unit_interval(
            self.credit_model_defaults.prepayment_cpr_annual,
            "default prepayment CPR",
        )?;
        validate_unit_interval(self.credit_model_defaults.default_cdr_annual, "default CDR")?;
        validate_unit_interval(
            self.credit_model_defaults.recovery_rate,
            "default recovery rate",
        )?;
        validate_unit_interval(self.cmo_collateral_defaults.wac, "CMO collateral WAC")?;
        validate_nonzero_u32(
            self.cmo_collateral_defaults.wam_months,
            "CMO collateral WAM",
        )?;
        validate_unit_interval(
            self.cmo_collateral_defaults.servicing_fee_rate,
            "CMO collateral servicing fee rate",
        )?;
        validate_unit_interval(
            self.cmo_collateral_defaults.guarantee_fee_rate,
            "CMO collateral guarantee fee rate",
        )?;
        validate_nonnegative_finite(
            self.cmo_collateral_defaults.psa_multiplier,
            "CMO collateral PSA multiplier",
        )?;
        validate_seasonality("mortgage", &self.seasonality.mortgage)?;
        validate_seasonality("credit card", &self.seasonality.credit_card)?;
        validate_nonnegative_finite(
            self.simulation.pool_balance_cleanup_threshold,
            "pool balance cleanup threshold",
        )?;
        validate_unit_interval(
            self.simulation.baseline_unemployment_rate,
            "baseline unemployment rate",
        )?;
        validate_concentration_limits(&self.concentration_limits)?;
        validate_nonzero_u32(
            self.simulation.resolution_lag_months,
            "resolution lag months",
        )?;
        validate_nonzero_u32(
            self.simulation.burnout_threshold_months,
            "burnout threshold months",
        )?;
        validate_nonzero_u32(self.prepayment_models.psa.ramp_months, "PSA ramp months")?;
        validate_unit_interval(self.prepayment_models.psa.terminal_cpr, "PSA terminal CPR")?;
        validate_unit_interval(
            self.prepayment_models.auto_abs.monthly_speed,
            "auto ABS monthly speed",
        )?;
        validate_nonzero_u32(
            self.prepayment_models.auto_abs.ramp_months,
            "auto ABS ramp months",
        )?;
        validate_nonzero_u32(self.default_models.sda.peak_month, "SDA peak month")?;
        validate_unit_interval(self.default_models.sda.peak_cdr, "SDA peak CDR")?;
        validate_unit_interval(self.default_models.sda.terminal_cdr, "SDA terminal CDR")?;
        validate_grid("standard PSA speed", &self.scenario_grids.psa_speeds)?;
        validate_grid("standard CDR rate", &self.scenario_grids.cdr_rates)?;
        validate_grid(
            "standard severity rate",
            &self.scenario_grids.severity_rates,
        )?;
        validate_ids(
            "structured-credit deal profile",
            self.deal_profiles
                .iter()
                .map(|profile| profile.ids.as_slice()),
        )?;
        validate_ids(
            "RMBS stochastic calibration",
            self.stochastic_calibrations
                .rmbs_profiles
                .iter()
                .map(|record| record.ids.as_slice()),
        )?;
        validate_ids(
            "CLO stochastic calibration",
            self.stochastic_calibrations
                .clo_profiles
                .iter()
                .map(|record| record.ids.as_slice()),
        )?;
        validate_ids(
            "CMBS stochastic calibration",
            self.stochastic_calibrations
                .cmbs_profiles
                .iter()
                .map(|record| record.ids.as_slice()),
        )?;

        for haircut in &self.coverage_haircuts {
            validate_unit_interval(haircut.haircut, "coverage haircut")?;
        }
        for record in &self.stochastic_calibrations.rmbs_profiles {
            validate_rmbs_stochastic_record(record)?;
        }
        for record in &self.stochastic_calibrations.clo_profiles {
            validate_clo_stochastic_record(record)?;
        }
        for record in &self.stochastic_calibrations.cmbs_profiles {
            validate_cmbs_stochastic_record(record)?;
        }
        validate_assumption_record(&self.asset_type_defaults.assumptions())?;
        for point in self
            .asset_type_defaults
            .cpr_by_asset_type
            .iter()
            .chain(self.asset_type_defaults.cdr_by_asset_type.iter())
            .chain(self.asset_type_defaults.recovery_by_asset_type.iter())
        {
            if point.asset_type.trim().is_empty() {
                return Err(Error::Validation(
                    "structured-credit asset-type assumption has blank asset type".to_string(),
                ));
            }
            validate_unit_interval(point.value, "asset-type assumption")?;
        }
        for profile in &self.deal_profiles {
            validate_assumption_record(&profile.assumptions)?;
            validate_fee_record(&profile.fees)?;
            validate_constructor_record(&profile.constructor)?;
        }
        Ok(())
    }
}

pub(crate) struct ConstructorDefaults {
    pub(crate) first_payment_month: u8,
    pub(crate) frequency: Tenor,
    pub(crate) prepayment_spec: PrepaymentModelSpec,
    pub(crate) default_spec: DefaultModelSpec,
    pub(crate) recovery_spec: RecoveryModelSpec,
    pub(crate) credit_factors: CreditFactors,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PsaCurveDefaults {
    pub(crate) ramp_months: u32,
    pub(crate) terminal_cpr: f64,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SdaCurveDefaults {
    pub(crate) peak_month: u32,
    pub(crate) peak_cdr: f64,
    pub(crate) terminal_cdr: f64,
}

impl SdaCurveDefaults {
    /// Standard PSA/BMA SDA shape as a multiplier of the peak CDR.
    ///
    /// This is THE canonical SDA shape for the crate — every SDA consumer
    /// (pool characteristics, behavior overrides, the copula-based seasoning
    /// curve) must derive from it rather than re-implementing the piecewise
    /// segments. With the standard registry values (`peak_month = 30`):
    ///
    /// - months 1..=30: linear ramp from 0 to 1
    /// - months 31..=60: flat plateau at 1 (the segment historical
    ///   re-implementations omitted)
    /// - months 61..=120: linear decline to `terminal_cdr / peak_cdr`
    /// - months 121+: flat at `terminal_cdr / peak_cdr`
    pub(crate) fn multiplier_at(&self, month: u32) -> f64 {
        let peak = self.peak_month.max(1);
        let plateau_end = peak * 2;
        let terminal_month = peak * 4;
        let terminal_mult = if self.peak_cdr > 0.0 {
            (self.terminal_cdr / self.peak_cdr).clamp(0.0, 1.0)
        } else {
            0.0
        };
        if month == 0 {
            0.0
        } else if month <= peak {
            month as f64 / peak as f64
        } else if month <= plateau_end {
            1.0
        } else if month <= terminal_month {
            let months_past_plateau = (month - plateau_end) as f64;
            let decline_period = (terminal_month - plateau_end) as f64;
            1.0 - (months_past_plateau / decline_period) * (1.0 - terminal_mult)
        } else {
            terminal_mult
        }
    }

    /// Annual CDR at the given seasoning month for 100% SDA.
    pub(crate) fn cdr_at(&self, month: u32) -> f64 {
        self.multiplier_at(month) * self.peak_cdr
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SimulationDefaults {
    pub(crate) pool_balance_cleanup_threshold: f64,
    pub(crate) resolution_lag_months: u32,
    pub(crate) burnout_threshold_months: u32,
    pub(crate) baseline_unemployment_rate: f64,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ConcentrationLimits {
    pub(crate) max_obligor_concentration: f64,
    pub(crate) max_top5_concentration: f64,
    pub(crate) max_top10_concentration: f64,
    pub(crate) max_second_lien: f64,
    pub(crate) max_cov_lite: f64,
    pub(crate) max_dip: f64,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct AutoAbsPrepaymentDefaults {
    pub(crate) monthly_speed: f64,
    pub(crate) ramp_months: u32,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CmoCollateralDefaults {
    pub(crate) wac: f64,
    pub(crate) wam_months: u32,
    pub(crate) servicing_fee_rate: f64,
    pub(crate) guarantee_fee_rate: f64,
    pub(crate) psa_multiplier: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MarketConditionsRecord {
    refi_rate: f64,
    seasonal_factor: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreditModelDefaultsRecord {
    prepayment_cpr_annual: f64,
    default_cdr_annual: f64,
    recovery_rate: f64,
    recovery_lag_months: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CmoCollateralDefaultsRecord {
    wac: f64,
    wam_months: u32,
    servicing_fee_rate: f64,
    guarantee_fee_rate: f64,
    psa_multiplier: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SeasonalityRecord {
    mortgage: Vec<f64>,
    credit_card: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScenarioGridsRecord {
    psa_speeds: Vec<f64>,
    cdr_rates: Vec<f64>,
    severity_rates: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SimulationRecord {
    pool_balance_cleanup_threshold: f64,
    resolution_lag_months: u32,
    burnout_threshold_months: u32,
    baseline_unemployment_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConcentrationLimitsRecord {
    max_obligor_concentration: f64,
    max_top5_concentration: f64,
    max_top10_concentration: f64,
    max_second_lien: f64,
    max_cov_lite: f64,
    max_dip: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PrepaymentModelsRecord {
    psa: PsaRecord,
    auto_abs: AutoAbsPrepaymentRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PsaRecord {
    ramp_months: u32,
    terminal_cpr: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AutoAbsPrepaymentRecord {
    monthly_speed: f64,
    ramp_months: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DefaultModelsRecord {
    sda: SdaRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SdaRecord {
    peak_month: u32,
    peak_cdr: f64,
    terminal_cdr: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StochasticCalibrationsRecord {
    rmbs_profiles: Vec<RmbsStochasticRecord>,
    clo_profiles: Vec<CloStochasticRecord>,
    cmbs_profiles: Vec<CmbsStochasticRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RmbsStochasticRecord {
    ids: Vec<String>,
    base_cdr: f64,
    default_correlation: f64,
    base_cpr: f64,
    prepay_factor_loading: f64,
    cpr_volatility: f64,
    default_factor_sensitivity: f64,
    default_mean_reversion: f64,
    default_volatility: f64,
    refi_sensitivity: f64,
    burnout_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CloStochasticRecord {
    ids: Vec<String>,
    base_cdr: f64,
    default_correlation: f64,
    base_cpr: f64,
    prepay_factor_loading: f64,
    cpr_volatility: f64,
    default_factor_sensitivity: f64,
    default_mean_reversion: f64,
    default_volatility: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CmbsStochasticRecord {
    ids: Vec<String>,
    base_cdr: f64,
    default_correlation: f64,
    base_cpr: f64,
    prepay_factor_loading: f64,
    cpr_volatility: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CoverageHaircutRecord {
    rating: CreditRating,
    haircut: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AssetTypeDefaultsRecord {
    base_cdr_annual: f64,
    base_recovery_rate: f64,
    base_cpr_annual: f64,
    cpr_by_asset_type: Vec<AssetTypeAssumptionRecord>,
    cdr_by_asset_type: Vec<AssetTypeAssumptionRecord>,
    recovery_by_asset_type: Vec<AssetTypeAssumptionRecord>,
}

impl AssetTypeDefaultsRecord {
    fn assumptions(&self) -> AssumptionRecord {
        AssumptionRecord {
            base_cdr_annual: self.base_cdr_annual,
            base_recovery_rate: self.base_recovery_rate,
            base_cpr_annual: self.base_cpr_annual,
            psa_speed: None,
            sda_speed: None,
            abs_speed_monthly: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AssetTypeAssumptionRecord {
    asset_type: String,
    value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DealProfileRecord {
    ids: Vec<String>,
    deal_type: DealType,
    fees: FeeRecord,
    assumptions: AssumptionRecord,
    constructor: ConstructorRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FeeRecord {
    trustee_fee_annual: f64,
    senior_mgmt_fee_bps: f64,
    subordinated_mgmt_fee_bps: f64,
    servicing_fee_bps: f64,
    master_servicer_fee_bps: Option<f64>,
    special_servicer_fee_bps: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AssumptionRecord {
    base_cdr_annual: f64,
    base_recovery_rate: f64,
    base_cpr_annual: f64,
    psa_speed: Option<f64>,
    sda_speed: Option<f64>,
    abs_speed_monthly: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConstructorRecord {
    first_payment_month: u8,
    frequency: ConstructorFrequency,
    prepayment: ConstructorPrepaymentRecord,
    default_cdr_annual: f64,
    recovery_rate: f64,
    recovery_lag_months: u32,
    ltv: Option<f64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ConstructorFrequency {
    Monthly,
    Quarterly,
}

impl ConstructorFrequency {
    fn tenor(self) -> Tenor {
        match self {
            Self::Monthly => Tenor::monthly(),
            Self::Quarterly => Tenor::quarterly(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConstructorPrepaymentRecord {
    kind: ConstructorPrepaymentKind,
    rate: f64,
    lockout_months: Option<u32>,
}

impl ConstructorPrepaymentRecord {
    fn spec(&self) -> PrepaymentModelSpec {
        match self.kind {
            ConstructorPrepaymentKind::ConstantCpr => PrepaymentModelSpec::constant_cpr(self.rate),
            ConstructorPrepaymentKind::Psa => PrepaymentModelSpec::psa(self.rate),
            ConstructorPrepaymentKind::MonthlyAbsSpeed => {
                PrepaymentModelSpec::constant_cpr(self.rate * 12.0)
            }
            ConstructorPrepaymentKind::CmbsLockout => {
                PrepaymentModelSpec::cmbs_with_lockout(self.lockout_months.unwrap_or(0), self.rate)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ConstructorPrepaymentKind {
    ConstantCpr,
    Psa,
    MonthlyAbsSpeed,
    CmbsLockout,
}

pub(crate) fn embedded_registry() -> Result<&'static StructuredCreditAssumptionRegistry> {
    match EMBEDDED_REGISTRY
        .get_or_init(|| parse_registry_json(EMBEDDED_STRUCTURED_CREDIT_ASSUMPTIONS))
    {
        Ok(registry) => Ok(registry),
        Err(err) => Err(err.clone()),
    }
}

#[allow(clippy::expect_used)]
pub(crate) fn embedded_registry_or_panic() -> &'static StructuredCreditAssumptionRegistry {
    embedded_registry().expect("embedded structured-credit assumptions are compile-time assets")
}

fn parse_registry_json(raw: &str) -> Result<StructuredCreditAssumptionRegistry> {
    let registry = serde_json::from_str(raw).map_err(|err| {
        Error::Validation(format!(
            "failed to parse embedded structured-credit assumptions registry: {err}"
        ))
    })?;
    validate_registry(registry)
}

fn validate_registry(
    registry: StructuredCreditAssumptionRegistry,
) -> Result<StructuredCreditAssumptionRegistry> {
    registry.validate()?;
    Ok(registry)
}

fn default_assumptions_from_record(
    record: &AssumptionRecord,
    cpr_by_asset_type: HashMap<String, f64>,
    cdr_by_asset_type: HashMap<String, f64>,
    recovery_by_asset_type: HashMap<String, f64>,
) -> DefaultAssumptions {
    DefaultAssumptions {
        base_cdr_annual: record.base_cdr_annual,
        base_recovery_rate: record.base_recovery_rate,
        base_cpr_annual: record.base_cpr_annual,
        psa_speed: record.psa_speed,
        sda_speed: record.sda_speed,
        abs_speed_monthly: record.abs_speed_monthly,
        cpr_by_asset_type,
        cdr_by_asset_type,
        recovery_by_asset_type,
    }
}

fn assumption_map(records: &[AssetTypeAssumptionRecord]) -> HashMap<String, f64> {
    let mut assumptions = HashMap::default();
    for record in records {
        assumptions.insert(record.asset_type.clone(), record.value);
    }
    assumptions
}

fn month_array(values: &[f64]) -> [f64; 12] {
    let mut months = [0.0; 12];
    months.copy_from_slice(values);
    months
}

fn validate_assumption_record(record: &AssumptionRecord) -> Result<()> {
    validate_unit_interval(record.base_cdr_annual, "base CDR")?;
    validate_unit_interval(record.base_recovery_rate, "base recovery rate")?;
    validate_unit_interval(record.base_cpr_annual, "base CPR")?;
    if let Some(speed) = record.psa_speed {
        validate_nonnegative_finite(speed, "PSA speed")?;
    }
    if let Some(speed) = record.sda_speed {
        validate_nonnegative_finite(speed, "SDA speed")?;
    }
    if let Some(speed) = record.abs_speed_monthly {
        validate_unit_interval(speed, "ABS monthly speed")?;
    }
    Ok(())
}

fn validate_rmbs_stochastic_record(record: &RmbsStochasticRecord) -> Result<()> {
    validate_unit_interval(record.base_cdr, "RMBS stochastic base CDR")?;
    validate_unit_interval(
        record.default_correlation,
        "RMBS stochastic default correlation",
    )?;
    validate_unit_interval(record.base_cpr, "RMBS stochastic base CPR")?;
    validate_factor_loading(
        record.prepay_factor_loading,
        "RMBS stochastic prepay factor loading",
    )?;
    validate_unit_interval(record.cpr_volatility, "RMBS stochastic CPR volatility")?;
    validate_nonnegative_finite(
        record.default_factor_sensitivity,
        "RMBS stochastic default factor sensitivity",
    )?;
    validate_nonnegative_finite(
        record.default_mean_reversion,
        "RMBS stochastic default mean reversion",
    )?;
    validate_nonnegative_finite(
        record.default_volatility,
        "RMBS stochastic default volatility",
    )?;
    validate_nonnegative_finite(record.refi_sensitivity, "RMBS stochastic refi sensitivity")?;
    validate_unit_interval(record.burnout_rate, "RMBS stochastic burnout rate")
}

fn validate_clo_stochastic_record(record: &CloStochasticRecord) -> Result<()> {
    validate_unit_interval(record.base_cdr, "CLO stochastic base CDR")?;
    validate_unit_interval(
        record.default_correlation,
        "CLO stochastic default correlation",
    )?;
    validate_unit_interval(record.base_cpr, "CLO stochastic base CPR")?;
    validate_factor_loading(
        record.prepay_factor_loading,
        "CLO stochastic prepay factor loading",
    )?;
    validate_unit_interval(record.cpr_volatility, "CLO stochastic CPR volatility")?;
    validate_nonnegative_finite(
        record.default_factor_sensitivity,
        "CLO stochastic default factor sensitivity",
    )?;
    validate_nonnegative_finite(
        record.default_mean_reversion,
        "CLO stochastic default mean reversion",
    )?;
    validate_nonnegative_finite(
        record.default_volatility,
        "CLO stochastic default volatility",
    )
}

fn validate_cmbs_stochastic_record(record: &CmbsStochasticRecord) -> Result<()> {
    validate_unit_interval(record.base_cdr, "CMBS stochastic base CDR")?;
    validate_unit_interval(
        record.default_correlation,
        "CMBS stochastic default correlation",
    )?;
    validate_unit_interval(record.base_cpr, "CMBS stochastic base CPR")?;
    validate_factor_loading(
        record.prepay_factor_loading,
        "CMBS stochastic prepay factor loading",
    )?;
    validate_unit_interval(record.cpr_volatility, "CMBS stochastic CPR volatility")
}

fn validate_concentration_limits(record: &ConcentrationLimitsRecord) -> Result<()> {
    validate_unit_interval(
        record.max_obligor_concentration,
        "maximum obligor concentration",
    )?;
    validate_unit_interval(record.max_top5_concentration, "maximum top 5 concentration")?;
    validate_unit_interval(
        record.max_top10_concentration,
        "maximum top 10 concentration",
    )?;
    validate_unit_interval(record.max_second_lien, "maximum second lien concentration")?;
    validate_unit_interval(record.max_cov_lite, "maximum covenant-lite concentration")?;
    validate_unit_interval(record.max_dip, "maximum DIP concentration")
}

fn validate_fee_record(record: &FeeRecord) -> Result<()> {
    validate_nonnegative_finite(record.trustee_fee_annual, "trustee annual fee")?;
    validate_nonnegative_finite(record.senior_mgmt_fee_bps, "senior management fee bps")?;
    validate_nonnegative_finite(
        record.subordinated_mgmt_fee_bps,
        "subordinated management fee bps",
    )?;
    validate_nonnegative_finite(record.servicing_fee_bps, "servicing fee bps")?;
    if let Some(fee) = record.master_servicer_fee_bps {
        validate_nonnegative_finite(fee, "master servicer fee bps")?;
    }
    if let Some(fee) = record.special_servicer_fee_bps {
        validate_nonnegative_finite(fee, "special servicer fee bps")?;
    }
    Ok(())
}

fn validate_constructor_record(record: &ConstructorRecord) -> Result<()> {
    if !(1..=12).contains(&record.first_payment_month) {
        return Err(Error::Validation(format!(
            "structured-credit constructor first payment month must be in 1..=12, got {}",
            record.first_payment_month
        )));
    }
    validate_nonnegative_finite(record.prepayment.rate, "constructor prepayment rate")?;
    if matches!(
        record.prepayment.kind,
        ConstructorPrepaymentKind::CmbsLockout
    ) && record.prepayment.lockout_months.is_none()
    {
        return Err(Error::Validation(
            "CMBS lockout constructor prepayment must include lockout_months".to_string(),
        ));
    }
    validate_unit_interval(record.default_cdr_annual, "constructor default CDR")?;
    validate_unit_interval(record.recovery_rate, "constructor recovery rate")?;
    if let Some(ltv) = record.ltv {
        validate_unit_interval(ltv, "constructor LTV")?;
    }
    Ok(())
}

fn validate_seasonality(label: &str, values: &[f64]) -> Result<()> {
    if values.len() != 12 {
        return Err(Error::Validation(format!(
            "structured-credit assumptions registry {label} seasonality must contain 12 months"
        )));
    }
    validate_grid(label, values)
}

fn validate_grid(label: &str, values: &[f64]) -> Result<()> {
    if values.is_empty() {
        return Err(Error::Validation(format!(
            "structured-credit assumptions registry {label} grid is empty"
        )));
    }
    for value in values {
        validate_nonnegative_finite(*value, label)?;
    }
    Ok(())
}

fn validate_nonzero_u32(value: u32, label: &str) -> Result<()> {
    if value == 0 {
        Err(Error::Validation(format!(
            "structured-credit assumptions registry has invalid {label} {value}"
        )))
    } else {
        Ok(())
    }
}

fn validate_unit_interval(value: f64, label: &str) -> Result<()> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(Error::Validation(format!(
            "structured-credit assumptions registry has invalid {label} {value}"
        )))
    }
}

fn validate_nonnegative_finite(value: f64, label: &str) -> Result<()> {
    if value.is_finite() && value >= 0.0 {
        Ok(())
    } else {
        Err(Error::Validation(format!(
            "structured-credit assumptions registry has invalid {label} {value}"
        )))
    }
}

fn validate_factor_loading(value: f64, label: &str) -> Result<()> {
    if value.is_finite() && (-1.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(Error::Validation(format!(
            "structured-credit assumptions registry has invalid {label} {value}"
        )))
    }
}

fn validate_ids<'a>(kind: &str, records: impl Iterator<Item = &'a [String]>) -> Result<()> {
    let mut seen = BTreeSet::new();
    for ids in records {
        if ids.is_empty() {
            return Err(Error::Validation(format!(
                "structured-credit assumptions registry contains {kind} without an id"
            )));
        }
        for id in ids {
            if id.trim().is_empty() {
                return Err(Error::Validation(format!(
                    "structured-credit assumptions registry contains blank {kind} id"
                )));
            }
            if !seen.insert(id.clone()) {
                return Err(Error::Validation(format!(
                    "structured-credit assumptions registry contains duplicate {kind} id '{id}'"
                )));
            }
        }
    }
    Ok(())
}

fn has_id(ids: &[String], id: &str) -> bool {
    ids.iter().any(|candidate| candidate == id)
}

fn not_found(kind: &str, id: &str) -> Error {
    Error::Validation(format!("{kind} '{id}' not found"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// M2.14 golden values: the canonical SDA shape at 100% SDA with the
    /// standard registry curve (peak 0.60% at month 30, plateau through 60,
    /// decline to 0.03% by 120, flat after). Historical re-implementations
    /// omitted the plateau and ended the decline at month 60, giving 0.315%
    /// at month 45 and 0.03% at month 90.
    #[test]
    fn sda_curve_golden_values() {
        let curve = embedded_registry().expect("embedded registry").sda_curve();

        // Month 45: on the plateau — full peak CDR.
        assert!(
            (curve.cdr_at(45) - 0.006).abs() < 1e-12,
            "month 45 must be on the 30-60 plateau at 0.60%, got {}",
            curve.cdr_at(45)
        );
        // Month 90: halfway down the 61-120 decline — 0.006 − 0.5·(0.006−0.0003).
        assert!(
            (curve.cdr_at(90) - 0.00315).abs() < 1e-12,
            "month 90 must be mid-decline at 0.315%, got {}",
            curve.cdr_at(90)
        );
        // Month 130: past the decline — terminal CDR.
        assert!(
            (curve.cdr_at(130) - 0.0003).abs() < 1e-12,
            "month 130 must be at the 0.03% terminal rate, got {}",
            curve.cdr_at(130)
        );

        // Shape anchors: zero at month 0, linear ramp, peak exactly at 30.
        assert_eq!(curve.multiplier_at(0), 0.0);
        assert!((curve.multiplier_at(15) - 0.5).abs() < 1e-12);
        assert!((curve.multiplier_at(30) - 1.0).abs() < 1e-12);
        assert!((curve.multiplier_at(60) - 1.0).abs() < 1e-12);
        assert!((curve.multiplier_at(120) - 0.05).abs() < 1e-12);
    }
}
