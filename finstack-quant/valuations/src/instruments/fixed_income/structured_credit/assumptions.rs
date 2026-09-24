//! Embedded structured-credit assumptions registry.

use crate::cashflow::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};
use crate::instruments::fixed_income::structured_credit::pricing::stochastic::calibrations::{
    CloCalibration, CmbsCalibration, RmbsCalibration,
};
use crate::instruments::fixed_income::structured_credit::types::{
    CreditFactors, DealFees, DealType, IncentiveFeeSpec,
};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Tenor;
use finstack_quant_core::embedded_registry::EmbeddedJsonRegistry;
use finstack_quant_core::money::Money;
use finstack_quant_core::{Error, Result};
use serde::{Deserialize, Serialize};

static EMBEDDED_REGISTRY: EmbeddedJsonRegistry<StructuredCreditAssumptionRegistry> =
    EmbeddedJsonRegistry::new(
        include_str!("../../../../data/assumptions/structured_credit_assumptions.v1.json"),
        None,
        "structured-credit assumptions",
    );

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StructuredCreditAssumptionRegistry {
    schema: String,
    market_conditions: MarketConditionsRecord,
    credit_model_defaults: CreditModelDefaultsRecord,
    cmo_collateral_defaults: CmoCollateralDefaultsRecord,
    scenario_grids: ScenarioGridsRecord,
    simulation: SimulationRecord,
    concentration_limits: ConcentrationLimitsRecord,
    prepayment_models: PrepaymentModelsRecord,
    default_models: DefaultModelsRecord,
    stochastic_calibrations: StochasticCalibrationsRecord,
    deal_profiles: Vec<DealProfileRecord>,
}

impl StructuredCreditAssumptionRegistry {
    pub(crate) fn market_conditions(&self) -> f64 {
        self.market_conditions.refi_rate
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

    pub(crate) fn deal_fees(&self, id: &str, base_currency: Currency) -> Result<DealFees> {
        let fees = &self.deal_profile(id)?.fees;
        Ok(DealFees {
            trustee_fee_annual: Money::new(fees.trustee_fee_annual, base_currency)?,
            senior_mgmt_fee_bp: fees.senior_mgmt_fee_bp,
            subordinated_mgmt_fee_bp: fees.subordinated_mgmt_fee_bp,
            servicing_fee_bp: fees.servicing_fee_bp,
            master_servicer_fee_bp: fees.master_servicer_fee_bp,
            special_servicer_fee_bp: fees.special_servicer_fee_bp,
            workout_fee_pct: None,
            liquidation_fee_pct: None,
            incentive_fee: fees.incentive_fee,
        })
    }

    pub(crate) fn constructor_defaults(&self, id: &str) -> Result<ConstructorDefaults> {
        let profile = self.deal_profile(id)?;
        Ok(ConstructorDefaults {
            frequency: profile.constructor.frequency.tenor(),
            prepayment_spec: profile.constructor.prepayment.spec(),
            default_spec: DefaultModelSpec::constant_cdr(profile.constructor.default_cdr_annual),
            recovery_spec: RecoveryModelSpec::with_lag(
                profile.constructor.recovery_rate,
                profile.constructor.recovery_lag_months,
            ),
            credit_factors: CreditFactors::default(),
        })
    }

    pub(crate) fn standard_rates(&self, id: &str) -> Result<StandardRates> {
        let profile = self.deal_profile(id)?;
        Ok(StandardRates {
            prepayment_rate: profile.constructor.prepayment.rate,
            cdr_annual: profile.constructor.default_cdr_annual,
            recovery_rate: profile.constructor.recovery_rate,
        })
    }

    fn deal_profile(&self, id: &str) -> Result<&DealProfileRecord> {
        self.deal_profiles
            .iter()
            .find(|profile| profile.ids.iter().any(|candidate| candidate == id))
            .ok_or_else(|| not_found("structured-credit deal profile", id))
    }

    fn validate(&self) -> Result<()> {
        if self.schema != "finstack_quant.structured_credit_assumptions/1" {
            return Err(Error::Validation(format!(
                "unsupported structured-credit assumptions schema version '{}'",
                self.schema
            )));
        }
        finstack_quant_core::validation::validate_f64_unit_interval(
            self.market_conditions.refi_rate,
            "refi rate",
        )?;
        finstack_quant_core::validation::validate_f64_unit_interval(
            self.credit_model_defaults.prepayment_cpr_annual,
            "default prepayment CPR",
        )?;
        finstack_quant_core::validation::validate_f64_unit_interval(
            self.credit_model_defaults.default_cdr_annual,
            "default CDR",
        )?;
        finstack_quant_core::validation::validate_f64_unit_interval(
            self.credit_model_defaults.recovery_rate,
            "default recovery rate",
        )?;
        finstack_quant_core::validation::validate_f64_unit_interval(
            self.cmo_collateral_defaults.wac,
            "CMO collateral WAC",
        )?;
        validate_nonzero_u32(
            self.cmo_collateral_defaults.wam_months,
            "CMO collateral WAM",
        )?;
        finstack_quant_core::validation::validate_f64_unit_interval(
            self.cmo_collateral_defaults.servicing_fee_rate,
            "CMO collateral servicing fee rate",
        )?;
        finstack_quant_core::validation::validate_f64_unit_interval(
            self.cmo_collateral_defaults.guarantee_fee_rate,
            "CMO collateral guarantee fee rate",
        )?;
        validate_nonnegative_finite(
            self.cmo_collateral_defaults.psa_multiplier,
            "CMO collateral PSA multiplier",
        )?;
        validate_nonnegative_finite(
            self.simulation.pool_balance_cleanup_threshold,
            "pool balance cleanup threshold",
        )?;
        finstack_quant_core::validation::validate_f64_unit_interval(
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
        finstack_quant_core::validation::validate_f64_unit_interval(
            self.prepayment_models.psa.terminal_cpr,
            "PSA terminal CPR",
        )?;
        validate_nonzero_u32(self.default_models.sda.peak_month, "SDA peak month")?;
        finstack_quant_core::validation::validate_f64_unit_interval(
            self.default_models.sda.peak_cdr,
            "SDA peak CDR",
        )?;
        finstack_quant_core::validation::validate_f64_unit_interval(
            self.default_models.sda.terminal_cdr,
            "SDA terminal CDR",
        )?;
        validate_grid("standard PSA speed", &self.scenario_grids.psa_speeds)?;
        validate_grid("standard CDR rate", &self.scenario_grids.cdr_rates)?;
        validate_grid(
            "standard severity rate",
            &self.scenario_grids.severity_rates,
        )?;
        finstack_quant_core::validation::validate_unique_ids(
            "structured-credit assumptions registry",
            "structured-credit deal profile",
            self.deal_profiles
                .iter()
                .map(|profile| profile.ids.as_slice()),
        )?;
        finstack_quant_core::validation::validate_unique_ids(
            "structured-credit assumptions registry",
            "RMBS stochastic calibration",
            self.stochastic_calibrations
                .rmbs_profiles
                .iter()
                .map(|record| record.ids.as_slice()),
        )?;
        finstack_quant_core::validation::validate_unique_ids(
            "structured-credit assumptions registry",
            "CLO stochastic calibration",
            self.stochastic_calibrations
                .clo_profiles
                .iter()
                .map(|record| record.ids.as_slice()),
        )?;
        finstack_quant_core::validation::validate_unique_ids(
            "structured-credit assumptions registry",
            "CMBS stochastic calibration",
            self.stochastic_calibrations
                .cmbs_profiles
                .iter()
                .map(|record| record.ids.as_slice()),
        )?;

        for record in &self.stochastic_calibrations.rmbs_profiles {
            validate_rmbs_stochastic_record(record)?;
        }
        for record in &self.stochastic_calibrations.clo_profiles {
            validate_clo_stochastic_record(record)?;
        }
        for record in &self.stochastic_calibrations.cmbs_profiles {
            validate_cmbs_stochastic_record(record)?;
        }
        for profile in &self.deal_profiles {
            validate_fee_record(&profile.fees)?;
            validate_constructor_record(&profile.constructor)?;
        }
        Ok(())
    }
}

pub(crate) struct ConstructorDefaults {
    pub(crate) frequency: Tenor,
    pub(crate) prepayment_spec: PrepaymentModelSpec,
    pub(crate) default_spec: DefaultModelSpec,
    pub(crate) recovery_spec: RecoveryModelSpec,
    pub(crate) credit_factors: CreditFactors,
}

/// Headline rates of a deal profile's constructor record, as exposed by the
/// deal-type constants (`clo_standard_cdr`, `rmbs_standard_psa`, ...).
#[derive(Debug, Clone, Copy)]
pub(crate) struct StandardRates {
    /// Prepayment speed in the profile's own unit: annual CPR for CLO and
    /// CMBS, PSA multiplier for RMBS, monthly ABS speed for auto ABS.
    pub(crate) prepayment_rate: f64,
    /// Annual constant default rate.
    pub(crate) cdr_annual: f64,
    /// Recovery rate as a decimal fraction of defaulted par.
    pub(crate) recovery_rate: f64,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PsaRecord {
    ramp_months: u32,
    terminal_cpr: f64,
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
struct DealProfileRecord {
    ids: Vec<String>,
    deal_type: DealType,
    fees: FeeRecord,
    constructor: ConstructorRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FeeRecord {
    trustee_fee_annual: f64,
    senior_mgmt_fee_bp: f64,
    subordinated_mgmt_fee_bp: f64,
    servicing_fee_bp: f64,
    master_servicer_fee_bp: Option<f64>,
    special_servicer_fee_bp: Option<f64>,
    #[serde(default)]
    incentive_fee: Option<IncentiveFeeSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConstructorRecord {
    frequency: ConstructorFrequency,
    prepayment: ConstructorPrepaymentRecord,
    default_cdr_annual: f64,
    recovery_rate: f64,
    recovery_lag_months: u32,
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
            ConstructorPrepaymentKind::MonthlyAbsSpeed => PrepaymentModelSpec::abs(self.rate),
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
    EMBEDDED_REGISTRY.load(validate_registry)
}

#[allow(clippy::expect_used)]
pub(crate) fn embedded_registry_or_panic() -> &'static StructuredCreditAssumptionRegistry {
    embedded_registry().expect("embedded structured-credit assumptions are compile-time assets")
}

/// Unwrap a lookup into the embedded, compile-time-validated assumptions
/// registry. The registry ships with the crate, so a missing entry is a
/// packaging defect rather than a caller error.
#[allow(clippy::expect_used)]
pub(crate) fn required_assumption<T>(result: Result<T>) -> T {
    result.expect("embedded structured-credit assumptions registry value should exist")
}

fn validate_registry(
    registry: StructuredCreditAssumptionRegistry,
) -> Result<StructuredCreditAssumptionRegistry> {
    registry.validate()?;
    Ok(registry)
}

fn validate_rmbs_stochastic_record(record: &RmbsStochasticRecord) -> Result<()> {
    finstack_quant_core::validation::validate_f64_unit_interval(
        record.base_cdr,
        "RMBS stochastic base CDR",
    )?;
    finstack_quant_core::validation::validate_f64_unit_interval(
        record.default_correlation,
        "RMBS stochastic default correlation",
    )?;
    finstack_quant_core::validation::validate_f64_unit_interval(
        record.base_cpr,
        "RMBS stochastic base CPR",
    )?;
    validate_factor_loading(
        record.prepay_factor_loading,
        "RMBS stochastic prepay factor loading",
    )?;
    finstack_quant_core::validation::validate_f64_unit_interval(
        record.cpr_volatility,
        "RMBS stochastic CPR volatility",
    )?;
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
    finstack_quant_core::validation::validate_f64_unit_interval(
        record.burnout_rate,
        "RMBS stochastic burnout rate",
    )
}

fn validate_clo_stochastic_record(record: &CloStochasticRecord) -> Result<()> {
    finstack_quant_core::validation::validate_f64_unit_interval(
        record.base_cdr,
        "CLO stochastic base CDR",
    )?;
    finstack_quant_core::validation::validate_f64_unit_interval(
        record.default_correlation,
        "CLO stochastic default correlation",
    )?;
    finstack_quant_core::validation::validate_f64_unit_interval(
        record.base_cpr,
        "CLO stochastic base CPR",
    )?;
    validate_factor_loading(
        record.prepay_factor_loading,
        "CLO stochastic prepay factor loading",
    )?;
    finstack_quant_core::validation::validate_f64_unit_interval(
        record.cpr_volatility,
        "CLO stochastic CPR volatility",
    )?;
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
    finstack_quant_core::validation::validate_f64_unit_interval(
        record.base_cdr,
        "CMBS stochastic base CDR",
    )?;
    finstack_quant_core::validation::validate_f64_unit_interval(
        record.default_correlation,
        "CMBS stochastic default correlation",
    )?;
    finstack_quant_core::validation::validate_f64_unit_interval(
        record.base_cpr,
        "CMBS stochastic base CPR",
    )?;
    validate_factor_loading(
        record.prepay_factor_loading,
        "CMBS stochastic prepay factor loading",
    )?;
    finstack_quant_core::validation::validate_f64_unit_interval(
        record.cpr_volatility,
        "CMBS stochastic CPR volatility",
    )
}

fn validate_concentration_limits(record: &ConcentrationLimitsRecord) -> Result<()> {
    finstack_quant_core::validation::validate_f64_unit_interval(
        record.max_obligor_concentration,
        "maximum obligor concentration",
    )?;
    finstack_quant_core::validation::validate_f64_unit_interval(
        record.max_top5_concentration,
        "maximum top 5 concentration",
    )?;
    finstack_quant_core::validation::validate_f64_unit_interval(
        record.max_top10_concentration,
        "maximum top 10 concentration",
    )?;
    finstack_quant_core::validation::validate_f64_unit_interval(
        record.max_second_lien,
        "maximum second lien concentration",
    )?;
    finstack_quant_core::validation::validate_f64_unit_interval(
        record.max_cov_lite,
        "maximum covenant-lite concentration",
    )?;
    finstack_quant_core::validation::validate_f64_unit_interval(
        record.max_dip,
        "maximum DIP concentration",
    )
}

fn validate_fee_record(record: &FeeRecord) -> Result<()> {
    validate_nonnegative_finite(record.trustee_fee_annual, "trustee annual fee")?;
    validate_nonnegative_finite(record.senior_mgmt_fee_bp, "senior management fee bp")?;
    validate_nonnegative_finite(
        record.subordinated_mgmt_fee_bp,
        "subordinated management fee bp",
    )?;
    validate_nonnegative_finite(record.servicing_fee_bp, "servicing fee bp")?;
    if let Some(fee) = record.master_servicer_fee_bp {
        validate_nonnegative_finite(fee, "master servicer fee bp")?;
    }
    if let Some(incentive) = record.incentive_fee {
        if !incentive.hurdle_irr.is_finite()
            || !incentive.share_pct.is_finite()
            || !(0.0..=1.0).contains(&incentive.share_pct)
        {
            return Err(finstack_quant_core::Error::Validation(
                "incentive fee needs a finite hurdle IRR and a share in [0, 1]".into(),
            ));
        }
    }
    if let Some(fee) = record.special_servicer_fee_bp {
        validate_nonnegative_finite(fee, "special servicer fee bp")?;
    }
    Ok(())
}

fn validate_constructor_record(record: &ConstructorRecord) -> Result<()> {
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
    finstack_quant_core::validation::validate_f64_unit_interval(
        record.default_cdr_annual,
        "constructor default CDR",
    )?;
    finstack_quant_core::validation::validate_f64_unit_interval(
        record.recovery_rate,
        "constructor recovery rate",
    )?;
    Ok(())
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

fn has_id(ids: &[String], id: &str) -> bool {
    ids.iter().any(|candidate| candidate == id)
}

fn not_found(kind: &str, id: &str) -> Error {
    Error::Validation(format!("{kind} '{id}' not found"))
}

#[cfg(test)]
mod tests {

    /// M2.14 golden values: the canonical SDA shape at 100% SDA with the
    /// standard registry curve (peak 0.60% at month 30, plateau through 60,
    /// decline to 0.03% by 120, flat after). Historical re-implementations
    /// omitted the plateau and ended the decline at month 60, giving 0.315%
    /// at month 45 and 0.03% at month 90.
    #[test]
    fn sda_curve_golden_values() {
        let spec = crate::cashflow::builder::DefaultModelSpec::sda(1.0);
        for (month, expected) in [
            (0, 0.0),
            (15, 0.003),
            (30, 0.006),
            (45, 0.006),
            (60, 0.006),
            (90, 0.00315),
            (120, 0.0003),
            (130, 0.0003),
        ] {
            let annual = 1.0 - (1.0 - spec.mdr(month).expect("SDA")).powi(12);
            assert!(
                (annual - expected).abs() < 1e-12,
                "month {month}: {annual} vs {expected}"
            );
        }
    }
}
