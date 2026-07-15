//! Type definitions for structured credit instruments.
//!
//! This module contains all data structures for structured credit instruments:
//! - `StructuredCredit` - The main instrument type
//! - AssetPool and asset types
//! - Tranche structure and coupon types
//! - Waterfall distribution types
//! - Behavioral model specifications
//! - Result types for valuation

// ============================================================================
// TYPE DEFINITION MODULES
// ============================================================================

pub(crate) mod constants;
pub(crate) mod enums;
pub(crate) mod pool;
/// SoA layout for pool assets.
pub(crate) mod pool_state;
pub(crate) mod results;
pub(crate) mod setup;
pub(crate) mod tranches;
pub(crate) mod waterfall;

// ============================================================================
// INTERNAL MODULES
// ============================================================================

mod constructors;
mod reinvestment;
mod stochastic;

// ============================================================================
// RE-EXPORTS FROM TYPE MODULES
// ============================================================================

// Enums
pub use enums::TrancheSeniority;
pub use enums::{AssetType, DealType, PaymentMode, TriggerConsequence};

// AssetPool types
pub use pool::AssetPool;
pub use pool::{
    calculate_pool_stats, ConcentrationCheckResult, ConcentrationViolation, PoolAsset, PoolStats,
    ReinvestmentCriteria, ReinvestmentPeriod, RepLine,
};
pub(crate) use pool_state::PoolState;

// Tranche types
pub use tranches::{
    CoverageTrigger, CreditEnhancement, Tranche, TrancheBehaviorType, TrancheBuilder,
    TrancheCoupon, TrancheStructure,
};

// Setup/configuration types
pub use setup::{CoverageTestConfig, DealConfig, DealDates, DealFees, DefaultAssumptions};

// Reinvestment
pub use reinvestment::ReinvestmentManager;

// Waterfall types
pub(crate) use waterfall::DiversionRecord;
pub use waterfall::{
    AfcSpec, AllocationMode, ControlledAccumulationSpec, CoverageTestType, EarlyAmortizationSpec,
    ExcessSpreadSpec, ManagementFeeType, PaymentCalculation, PaymentRecord, PaymentType, Recipient,
    RecipientType, RoundingConvention, ShiftingInterestSpec, ShiftingInterestStep, StepDownSpec,
    StepDownTrigger, Waterfall, WaterfallBuilder, WaterfallDistribution, WaterfallRules,
    WaterfallTier, WaterfallWorkspace,
};

// Result types
pub use results::{TrancheCashflows, TrancheValuation};

// Stochastic specs - re-export from pricing
pub use crate::instruments::fixed_income::structured_credit::pricing::{
    CorrelationStructure, StochasticDefaultSpec, StochasticPrepaySpec,
};

// ============================================================================
// BEHAVIORAL MODEL SPECS
// ============================================================================

// Re-export deterministic model specs from cashflow builder
pub use crate::cashflow::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};

// ============================================================================
// IMPORTS FOR STRUCTUREDCREDIT
// ============================================================================

use crate::cashflow::traits::CashflowProvider;
use crate::cashflow::traits::{schedule_from_classified_flows, ScheduleBuildOpts};
use crate::constants::DECIMAL_TO_PERCENT;
use crate::correlation::RecoverySpec as StochasticRecoverySpec;
use crate::instruments::common_impl::traits::{Attributes, Instrument};
use crate::instruments::fixed_income::structured_credit::assumptions::embedded_registry_or_panic;
use crate::instruments::fixed_income::structured_credit::pricing::stochastic::pricer::{
    PricingMode, StochasticPricer, StochasticPricerConfig, StochasticPricingResult,
};
use crate::instruments::fixed_income::structured_credit::pricing::stochastic::tree::{
    BranchingSpec, ScenarioTreeConfig,
};
use crate::instruments::fixed_income::structured_credit::utils::rates::{
    clamped_cdr_to_mdr, clamped_cpr_to_smm,
};
use crate::instruments::model_params::ModelParamsSnapshot;
use crate::instruments::rates::irs::InterestRateSwap;
use crate::metrics::{MetricContext, MetricId};
use finstack_quant_core::dates::{
    BusinessDayConvention, Date, DateExt, DayCount, DayCountContext, Tenor,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_core::Error;

use finstack_quant_core::HashMap;

use crate::impl_instrument_base;
use serde::{Deserialize, Serialize};

// ============================================================================
// MARKET CONDITIONS AND CREDIT FACTORS
// ============================================================================

/// Market conditions that affect prepayment behavior.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MarketConditions {
    /// Current refinancing rate.
    pub refi_rate: f64,
    /// Rate at origination for refinancing incentive calculation.
    pub original_rate: Option<f64>,
    /// Home price appreciation (for mortgages).
    pub hpa: Option<f64>,
    /// Unemployment rate.
    pub unemployment: Option<f64>,
    /// Seasonal adjustment factor.
    pub seasonal_factor: Option<f64>,
    /// Custom market factors.
    pub custom_factors: HashMap<String, f64>,
}

impl Default for MarketConditions {
    fn default() -> Self {
        let (refi_rate, seasonal_factor) = embedded_registry_or_panic().market_conditions();
        Self {
            refi_rate,
            original_rate: None,
            hpa: None,
            unemployment: None,
            seasonal_factor,
            custom_factors: HashMap::default(),
        }
    }
}

/// Credit factors affecting default probability.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CreditFactors {
    /// Current FICO/credit score.
    pub credit_score: Option<u32>,
    /// Debt-to-income ratio.
    pub dti: Option<f64>,
    /// Loan-to-value ratio.
    pub ltv: Option<f64>,
    /// Payment delinquency status (days).
    pub delinquency_days: u32,
    /// Unemployment rate.
    pub unemployment_rate: Option<f64>,
    /// Annual net operating income for CMBS collateral, when provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annual_noi: Option<Money>,
    /// Annual debt service for CMBS collateral, when provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annual_debt_service: Option<Money>,
    /// Additional custom factors.
    pub custom_factors: HashMap<String, f64>,
}

// ============================================================================
// DEAL METADATA AND OVERRIDES
// ============================================================================

/// Deal metadata (counterparties and identifiers).
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Metadata {
    /// Manager identifier (for CLO).
    pub manager_id: Option<String>,
    /// Servicer identifier (for ABS/RMBS/CMBS).
    pub servicer_id: Option<String>,
    /// Master servicer identifier (for CMBS/RMBS).
    pub master_servicer_id: Option<String>,
    /// Special servicer identifier (for CMBS).
    pub special_servicer_id: Option<String>,
    /// Trustee identifier (for ABS).
    pub trustee_id: Option<String>,
}

/// Behavioral overrides for prepayment, default, and recovery assumptions.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Overrides {
    /// Override prepayment with constant annual CPR.
    pub cpr_annual: Option<f64>,
    /// Override prepayment with monthly ABS speed.
    pub abs_speed: Option<f64>,
    /// Override prepayment with PSA multiplier.
    pub psa_speed_multiplier: Option<f64>,
    /// Override default with constant annual CDR.
    pub cdr_annual: Option<f64>,
    /// Override default with SDA multiplier.
    pub sda_speed_multiplier: Option<f64>,
    /// Override recovery with constant rate.
    pub recovery_rate: Option<f64>,
    /// Override recovery lag (months).
    pub recovery_lag_months: Option<u32>,
    /// Reinvestment price constraint (% of par).
    pub reinvestment_price: Option<f64>,
}

// ============================================================================
// CREDIT MODEL CONFIGURATION
// ============================================================================

/// Configuration for deterministic + optional stochastic credit behavior models.
///
/// This groups the "credit model" knobs that were previously exposed as many
/// top-level fields on [`StructuredCredit`]. The struct is intended to be
/// embedded via `#[serde(flatten)]` to preserve the existing JSON shape.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CreditModelConfig {
    /// Prepayment model specification.
    #[serde(default = "CreditModelConfig::default_prepayment_spec")]
    pub prepayment_spec: PrepaymentModelSpec,

    /// Default model specification.
    #[serde(default = "CreditModelConfig::default_default_spec")]
    pub default_spec: DefaultModelSpec,

    /// Recovery model specification.
    #[serde(default = "CreditModelConfig::default_recovery_spec")]
    pub recovery_spec: RecoveryModelSpec,

    /// Optional stochastic prepayment model specification.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stochastic_prepay_spec: Option<StochasticPrepaySpec>,

    /// Optional stochastic default model specification.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stochastic_default_spec: Option<StochasticDefaultSpec>,

    /// Optional correlation structure for stochastic modeling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_structure: Option<CorrelationStructure>,
}

impl Default for CreditModelConfig {
    fn default() -> Self {
        Self {
            prepayment_spec: Self::default_prepayment_spec(),
            default_spec: Self::default_default_spec(),
            recovery_spec: Self::default_recovery_spec(),
            stochastic_prepay_spec: None,
            stochastic_default_spec: None,
            correlation_structure: None,
        }
    }
}

impl CreditModelConfig {
    fn default_prepayment_spec() -> PrepaymentModelSpec {
        embedded_registry_or_panic().default_prepayment_spec()
    }

    fn default_default_spec() -> DefaultModelSpec {
        embedded_registry_or_panic().default_default_spec()
    }

    fn default_recovery_spec() -> RecoveryModelSpec {
        embedded_registry_or_panic().default_recovery_spec()
    }
}

// ============================================================================
// STRUCTURED CREDIT INSTRUMENT
// ============================================================================

/// Unified structured credit instrument representation.
///
/// This single type handles CLO, ABS, CMBS, and RMBS instruments using
/// composition for deal-specific differences.
#[derive(
    Clone,
    finstack_quant_valuations_macros::FinancialBuilder,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct StructuredCredit {
    /// Unique instrument identifier.
    pub id: InstrumentId,

    /// Deal classification (ABS/CLO/CMBS/RMBS).
    pub deal_type: DealType,

    /// Asset pool definition.
    pub pool: AssetPool,

    /// Tranche structure.
    pub tranches: TrancheStructure,

    /// Key dates.
    /// Deal closing date (issuance).
    #[schemars(with = "String")]
    pub closing_date: Date,
    /// First payment date to tranches.
    #[schemars(with = "String")]
    pub first_payment_date: Date,
    /// End of reinvestment period (if applicable).
    #[schemars(with = "Option<String>")]
    pub reinvestment_end_date: Option<Date>,
    /// Legal final maturity date.
    #[schemars(with = "String")]
    pub maturity: Date,

    /// Payment frequency for the structure.
    pub frequency: Tenor,

    /// Optional payment calendar identifier for schedule adjustments.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payment_calendar_id: Option<String>,

    /// Business day convention for tranche payments (defaults to Following).
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payment_bdc: Option<BusinessDayConvention>,

    /// Discount curve for valuation.
    pub discount_curve_id: CurveId,

    /// Attributes for scenario selection.
    #[serde(default)]
    #[builder(default)]
    pub pricing_overrides: crate::instruments::PricingOverrides,

    /// Attributes for scenario selection.
    pub attributes: Attributes,

    /// Credit model configuration (prepayment/default/recovery + optional stochastic specs).
    ///
    /// Serialized keys are flattened for flat JSON layout.
    #[builder(default)]
    #[serde(default, flatten)]
    pub credit_model: CreditModelConfig,

    /// Market conditions impacting behavior.
    pub market_conditions: MarketConditions,

    /// Credit factors impacting default behavior.
    pub credit_factors: CreditFactors,

    /// Deal metadata (counterparties, identifiers).
    #[serde(default)]
    pub deal_metadata: Metadata,

    /// Behavioral assumption overrides.
    #[serde(default)]
    pub behavior_overrides: Overrides,

    /// Default behavioral assumptions for the deal.
    #[serde(default)]
    pub default_assumptions: DefaultAssumptions,

    /// Interest rate swaps used to hedge basis or interest rate risk.
    #[serde(default)]
    pub hedge_swaps: Vec<InterestRateSwap>,

    /// Clean-up call pool factor threshold (percentage of original balance).
    ///
    /// When the pool factor (current balance / original balance) drops below
    /// this threshold, the deal is optionally redeemed and all outstanding
    /// tranche balances are returned. Industry standard: typically 10%.
    ///
    /// Set to `None` to disable clean-up call (default).
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cleanup_call_pct: Option<f64>,

    /// Declarative waterfall rules (available-funds caps, etc.) layered onto the
    /// base waterfall by `resolve_waterfall`. `None` reproduces the base
    /// waterfall exactly.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waterfall_rules: Option<WaterfallRules>,
}

impl StructuredCredit {
    /// Set the payment calendar ID for business day adjustments.
    ///
    /// This is required for accurate schedule generation. Structured credit deals
    /// are calendar-specific (e.g., NY, TARGET2), and using the wrong calendar
    /// shifts payment dates around holidays, breaking WAC/WAL and OC tests.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use finstack_quant_core::dates::BusinessDayConvention;
    /// use finstack_quant_valuations::instruments::fixed_income::structured_credit::StructuredCredit;
    ///
    /// let clo = StructuredCredit::example()
    ///     .with_payment_calendar("nyse")
    ///     .with_payment_bdc(BusinessDayConvention::ModifiedFollowing);
    /// # let _ = clo;
    /// ```
    #[must_use]
    pub fn with_payment_calendar(mut self, calendar_id: impl Into<String>) -> Self {
        self.payment_calendar_id = Some(calendar_id.into());
        self
    }

    /// Set the business day convention for payment date adjustments.
    ///
    /// If not specified, defaults to `BusinessDayConvention::Following`.
    #[must_use]
    pub fn with_payment_bdc(mut self, convention: BusinessDayConvention) -> Self {
        self.payment_bdc = Some(convention);
        self
    }

    /// Set the clean-up call pool factor threshold.
    ///
    /// When the pool factor (current balance / original balance) drops below
    /// this threshold, the deal may be optionally redeemed. Tranches are paid
    /// in seniority order (senior first), bounded by remaining pool value.
    ///
    /// Industry standard: typically 0.10 (10%).
    ///
    /// # Errors
    /// Returns a validation error if `threshold` is not finite or is outside
    /// `(0.0, 1.0)`.
    pub fn with_cleanup_call(mut self, threshold: f64) -> finstack_quant_core::Result<Self> {
        if !threshold.is_finite() || threshold <= 0.0 || threshold >= 1.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "cleanup_call_pct must be finite and in (0, 1), got {threshold}"
            )));
        }
        self.cleanup_call_pct = Some(threshold);
        Ok(self)
    }

    /// Calculate current loss percentage of the pool.
    ///
    /// Reconstructs the original pool balance as the denominator:
    /// `current_balance + cumulative_defaults + cumulative_prepayments + cumulative_scheduled_amortization`
    ///
    /// This avoids inflating the loss rate as the pool amortizes and aligns with
    /// Moody's/S&P convention of reporting cumulative losses against original pool balance.
    pub fn current_loss_percentage(&self) -> finstack_quant_core::Result<f64> {
        let current_balance = self.pool.total_balance()?.amount();
        let scheduled_amort = self
            .pool
            .cumulative_scheduled_amortization
            .map(|m| m.amount())
            .unwrap_or(0.0);
        // Reconstruct original balance from all tracked reductions
        let original_balance = current_balance
            + self.pool.cumulative_defaults.amount()
            + self.pool.cumulative_prepayments.amount()
            + scheduled_amort;

        if original_balance <= 0.0 {
            return Ok(0.0);
        }

        Ok(
            (self.pool.cumulative_defaults.amount() - self.pool.cumulative_recoveries.amount())
                / original_balance
                * DECIMAL_TO_PERCENT,
        )
    }

    /// Calculate expected life of the structure.
    pub fn expected_life(&self, as_of: Date) -> finstack_quant_core::Result<f64> {
        Ok(self.pool.weighted_avg_maturity(as_of))
    }

    /// Create waterfall from instrument configuration.
    pub fn create_waterfall(&self) -> Waterfall {
        self.create_waterfall_internal()
    }

    /// Internal waterfall creation (called by constructors).
    fn create_waterfall_internal(&self) -> Waterfall {
        // Use the standard sequential waterfall as default
        Waterfall::standard_sequential(self.pool.base_currency(), &self.tranches, vec![])
    }

    /// Calculate prepayment rate (SMM) for a given period.
    pub fn calculate_prepayment_rate(
        &self,
        pay_date: Date,
        seasoning_months: u32,
    ) -> finstack_quant_core::Result<f64> {
        if let Some(override_rate) = self.prepayment_rate_override(pay_date, seasoning_months) {
            return Ok(override_rate);
        }
        Ok(self
            .credit_model
            .prepayment_spec
            .smm(seasoning_months)?
            .max(0.0))
    }

    /// Calculate default rate (MDR) for a given period.
    pub fn calculate_default_rate(
        &self,
        pay_date: Date,
        seasoning_months: u32,
    ) -> finstack_quant_core::Result<f64> {
        if let Some(override_rate) = self.default_rate_override(pay_date, seasoning_months) {
            return Ok(override_rate);
        }
        Ok(self
            .credit_model
            .default_spec
            .mdr(seasoning_months)?
            .max(0.0))
    }

    /// Stochastic pricing convenience that defaults to Monte Carlo.
    pub fn price_stochastic(
        &self,
        context: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<StochasticPricingResult> {
        let num_paths = self
            .pricing_overrides
            .model_config
            .mc_paths
            .unwrap_or(10_000);
        self.price_stochastic_with_mode(
            context,
            as_of,
            PricingMode::MonteCarlo {
                num_paths,
                antithetic: num_paths > 1,
            },
        )
    }

    /// Stochastic pricing with an explicit mode (tree, Monte Carlo, or hybrid).
    pub fn price_stochastic_with_mode(
        &self,
        context: &MarketContext,
        as_of: Date,
        pricing_mode: PricingMode,
    ) -> finstack_quant_core::Result<StochasticPricingResult> {
        let mut tree_config = self.build_scenario_tree_config(as_of)?;
        if let Some(tree_steps) = self.pricing_overrides.model_config.tree_steps {
            tree_config.num_periods = tree_steps.max(1);
        }
        let discount_curve = context.get_discount(self.discount_curve_id.as_str())?;
        let mut config = StochasticPricerConfig::new(as_of, discount_curve, tree_config)
            .with_pricing_mode(pricing_mode);
        if let Some(granularity) = self
            .pricing_overrides
            .model_config
            .structured_credit_pool_granularity
        {
            config = config.with_pool_granularity(granularity);
        }
        self.run_stochastic_pricer(config, context)
    }

    fn run_stochastic_pricer(
        &self,
        config: StochasticPricerConfig,
        context: &MarketContext,
    ) -> finstack_quant_core::Result<StochasticPricingResult> {
        let notional = self.pool.total_balance()?.amount();

        if notional.abs() <= f64::EPSILON {
            return Err(finstack_quant_core::Error::Validation(
                "structured-credit stochastic pricing requires positive pool notional".to_string(),
            ));
        }

        self.validate_stochastic_tranches()?;

        let pricer = StochasticPricer::new(config);
        let result = pricer.price(self, context)?;

        if result.tranche_results.len() != self.tranches.tranches.len() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "stochastic pricing produced {} tranche results for {} input tranches",
                result.tranche_results.len(),
                self.tranches.tranches.len()
            )));
        }

        Ok(result)
    }

    pub(crate) fn build_scenario_tree_config(
        &self,
        as_of: Date,
    ) -> finstack_quant_core::Result<ScenarioTreeConfig> {
        let months_to_maturity = as_of.months_until(self.maturity).max(1) as usize;
        let horizon_years = DayCount::Act365F
            .year_fraction(as_of, self.maturity, DayCountContext::default())?
            .abs()
            .max(0.25);

        let mut tree_config =
            ScenarioTreeConfig::new(months_to_maturity, horizon_years, BranchingSpec::fixed(3));

        let (prepay, default, correlation) = self.effective_stochastic_specs();
        correlation
            .validate()
            .map_err(finstack_quant_core::Error::Validation)?;
        tree_config.prepay_spec = prepay;
        tree_config.default_spec = default;
        tree_config.recovery_spec =
            StochasticRecoverySpec::constant(self.credit_model.recovery_spec.rate)
                .map_err(|err| finstack_quant_core::Error::Validation(err.to_string()))?;
        tree_config.correlation = correlation;
        tree_config.pool_coupon = self.pool.weighted_avg_coupon();
        tree_config.initial_balance = self.pool.total_balance()?.amount().max(1.0);
        let seasoning = if as_of > self.closing_date {
            self.closing_date.months_until(as_of)
        } else {
            0
        };
        tree_config.initial_seasoning = seasoning;
        tree_config = tree_config.with_seed(self.derive_seed(as_of));
        Ok(tree_config)
    }

    fn validate_stochastic_tranches(&self) -> finstack_quant_core::Result<()> {
        let mut previous_detachment = 0.0;
        const EPS: f64 = 1e-9;

        for (idx, tranche) in self.tranches.tranches.iter().enumerate() {
            let attachment = tranche.attachment_point;
            let detachment = tranche.detachment_point;
            if !attachment.is_finite() || !detachment.is_finite() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "structured-credit tranche '{}' has non-finite attachment/detachment",
                    tranche.id
                )));
            }
            if !(0.0..=100.0).contains(&attachment)
                || !(0.0..=100.0).contains(&detachment)
                || attachment >= detachment
            {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "structured-credit tranche '{}' has invalid attachment/detachment [{attachment}, {detachment}]",
                    tranche.id
                )));
            }
            if idx == 0 && attachment.abs() > EPS {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "structured-credit tranche '{}' starts at {attachment}; first attachment must be 0",
                    tranche.id
                )));
            }
            if idx > 0 && (attachment - previous_detachment).abs() > EPS {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "structured-credit tranche '{}' creates a gap/overlap: attachment {attachment} after previous detachment {previous_detachment}",
                    tranche.id
                )));
            }
            previous_detachment = detachment;
        }

        if !self.tranches.tranches.is_empty() && (previous_detachment - 100.0).abs() > EPS {
            return Err(finstack_quant_core::Error::Validation(format!(
                "structured-credit final tranche detachment must be exactly 100, got {previous_detachment}"
            )));
        }

        Ok(())
    }

    fn derive_seed(&self, as_of: Date) -> u64 {
        // Use a simple deterministic mixing of the ID and date bytes to ensure reproducibility
        // across different Rust versions/platforms (unlike DefaultHasher).
        let mut seed: u64 = 0xcbf29ce484222325; // FNV offset basis

        for byte in self.id.as_bytes() {
            seed ^= *byte as u64;
            seed = seed.wrapping_mul(0x100000001b3); // FNV prime
        }

        // Mix in date
        let date_val = as_of.to_julian_day() as u64;
        seed ^= date_val;
        seed = seed.wrapping_mul(0x100000001b3);

        seed
    }

    fn effective_stochastic_specs(
        &self,
    ) -> (
        StochasticPrepaySpec,
        StochasticDefaultSpec,
        CorrelationStructure,
    ) {
        let prepay = self
            .credit_model
            .stochastic_prepay_spec
            .clone()
            .unwrap_or_else(|| {
                StochasticPrepaySpec::deterministic(self.credit_model.prepayment_spec.clone())
            });

        let default = self
            .credit_model
            .stochastic_default_spec
            .clone()
            .unwrap_or_else(|| {
                StochasticDefaultSpec::deterministic(self.credit_model.default_spec.clone())
            });

        let correlation = self
            .credit_model
            .correlation_structure
            .clone()
            .unwrap_or_else(|| match self.deal_type {
                DealType::RMBS => CorrelationStructure::rmbs_standard(),
                DealType::CLO | DealType::CBO => CorrelationStructure::clo_standard(),
                DealType::CMBS => CorrelationStructure::cmbs_standard(),
                _ => CorrelationStructure::abs_auto_standard(),
            });

        (prepay, default, correlation)
    }

    /// Calculate Z-Spread given a market price.
    ///
    /// Solves for the constant spread over the discount curve that equates the
    /// present value of deterministic cashflows to the market price.
    ///
    /// This uses deterministic cashflows (no prepayment optionality).
    /// For true OAS with stochastic prepayment, use `StochasticPricer`.
    pub fn calculate_z_spread(
        &self,
        context: &MarketContext,
        as_of: Date,
        market_price: f64,
    ) -> finstack_quant_core::Result<f64> {
        use finstack_quant_core::math::solver::{BrentSolver, Solver};

        let flows = self.dated_cashflows(context, as_of)?;
        let discount_curve = context.get_discount(&self.discount_curve_id)?;

        let price_fn = |spread: f64| -> f64 {
            let mut pv = finstack_quant_core::math::summation::NeumaierAccumulator::new();
            for (date, amount) in &flows {
                // Calculate discount factor with spread
                // DF = exp(-(r + s) * t)
                // We assume continuous compounding for the spread application

                let Ok(t) =
                    DayCount::Act365F.year_fraction(as_of, *date, DayCountContext::default())
                else {
                    return f64::NAN; // Solver handles NAN/Inf usually by erroring, but Brent might need finite
                };

                if t <= 0.0 {
                    // Flow is today or past, assume full value or ignore?
                    // Usually ignore past flows, but dated_cashflows may already exclude settled flows.
                    // If today, DF=1.
                    pv.add(amount.amount());
                    continue;
                }

                let Ok(df_base) = discount_curve.df_between_dates(as_of, *date) else {
                    return f64::NAN;
                };

                // Adjustment: df_spread = exp(-spread * t)
                let df_spread = (-spread * t).exp();
                let df = df_base * df_spread;

                pv.add(amount.amount() * df);
            }
            pv.total() - market_price
        };

        // Solve for spread
        // Initial guess: 100 bps (0.01)
        // Bracket: -10% to +50%?
        // BrentSolver finds bracket automatically if not provided.
        let solver = BrentSolver::new().tolerance(1e-6);
        solver.solve(price_fn, 0.01)
    }

    fn prepayment_rate_override(&self, _pay_date: Date, seasoning: u32) -> Option<f64> {
        if let Some(abs_speed) = self.behavior_overrides.abs_speed {
            return Some(abs_speed);
        }

        if let Some(cpr) = self.behavior_overrides.cpr_annual {
            return Some(clamped_cpr_to_smm(cpr));
        }

        if let Some(psa_mult) = self.behavior_overrides.psa_speed_multiplier {
            let psa_curve = embedded_registry_or_panic().psa_curve();
            let base_cpr = if seasoning <= psa_curve.ramp_months {
                (seasoning as f64 / psa_curve.ramp_months as f64) * psa_curve.terminal_cpr
            } else {
                psa_curve.terminal_cpr
            };
            let cpr = base_cpr * psa_mult;
            return Some(clamped_cpr_to_smm(cpr));
        }

        None
    }

    fn default_rate_override(&self, _pay_date: Date, seasoning: u32) -> Option<f64> {
        if let Some(cdr) = self.behavior_overrides.cdr_annual {
            return Some(clamped_cdr_to_mdr(cdr));
        }

        if let Some(sda_mult) = self.behavior_overrides.sda_speed_multiplier {
            // Canonical PSA SDA shape (ramp / plateau / decline / terminal)
            // lives on `SdaCurveDefaults::cdr_at`.
            let cdr = embedded_registry_or_panic().sda_curve().cdr_at(seasoning) * sda_mult;

            return Some(clamped_cdr_to_mdr(cdr));
        }

        None
    }
}

// ============================================================================
// TRAIT IMPLEMENTATIONS
// ============================================================================

impl finstack_quant_cashflows::CashflowScheduleSource for StructuredCredit {
    fn notional(&self) -> Option<Money> {
        // Return total pool balance as the notional
        self.pool.total_balance().ok()
    }

    fn raw_cashflow_schedule(
        &self,
        context: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<crate::cashflow::builder::CashFlowSchedule> {
        let detailed_flows =
            crate::instruments::fixed_income::structured_credit::pricing::run_simulation(
                self, context, as_of,
            )?
            .into_values()
            .flat_map(|result| result.detailed_flows.into_iter())
            .collect();
        // Use deal-type-appropriate day count convention:
        // CLO/CBO: ACT/360 (standard for leveraged loan market)
        // RMBS/CMBS: 30/360 (standard for mortgage market)
        // ABS/Auto/Card: ACT/360
        let dc = match self.deal_type {
            DealType::RMBS | DealType::CMBS => finstack_quant_core::dates::DayCount::Thirty360,
            _ => finstack_quant_core::dates::DayCount::Act360,
        };
        Ok(schedule_from_classified_flows(
            detailed_flows,
            dc,
            ScheduleBuildOpts {
                notional_hint: self.notional(),
                meta: crate::cashflow::builder::CashFlowMeta {
                    representation: crate::cashflow::builder::CashflowRepresentation::Projected,
                    ..Default::default()
                },
            },
        ))
    }
}

impl Instrument for StructuredCredit {
    impl_instrument_base!(crate::pricer::InstrumentType::StructuredCredit);

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        let mut deps = crate::instruments::common_impl::dependencies::MarketDependencies::new();
        deps.add_discount_curve(self.discount_curve_id.clone());

        for index_id in self
            .pool
            .assets
            .iter()
            .filter_map(|asset| asset.index_id.as_deref())
            .chain(
                self.pool
                    .rep_lines
                    .iter()
                    .flatten()
                    .filter_map(|line| line.index_id.as_deref()),
            )
        {
            deps.add_forward_curve(index_id);
            deps.add_series_id(index_id);
        }
        for tranche in &self.tranches.tranches {
            if let TrancheCoupon::Floating(spec) = &tranche.coupon {
                deps.add_forward_curve(spec.index_id.clone());
                deps.add_series_id(spec.index_id.as_str());
            }
        }
        Ok(deps)
    }

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        if let Some(threshold) = self.cleanup_call_pct {
            if !threshold.is_finite() || threshold <= 0.0 || threshold >= 1.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "cleanup_call_pct must be finite and in (0, 1), got {threshold}"
                )));
            }
        }
        Ok(())
    }

    fn base_value(
        &self,
        context: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        let disc = context.get_discount(self.discount_curve_id.as_str())?;
        let flows = self.dated_cashflows(context, as_of)?;

        use finstack_quant_core::cashflow::Discountable;
        flows.npv(disc.as_ref(), as_of)
    }

    fn seed_metric_context(
        &self,
        context: &mut crate::metrics::MetricContext,
        market: &MarketContext,
        as_of: Date,
    ) {
        context.discount_curve_id = Some(self.discount_curve_id.to_owned());
        context.notional = self.pool.total_balance().ok();
        // Preserve classified rows for metrics such as WAL while deriving the
        // cash-only compatibility view from the same canonical schedule.
        if let Ok(schedule) = self.cashflow_schedule(market, as_of) {
            context.cashflows = Some(
                schedule
                    .get_flows()
                    .iter()
                    .filter(|flow| crate::cashflow::primitives::is_cash_settlement_kind(flow.kind))
                    .map(|flow| (flow.date, flow.amount))
                    .collect(),
            );
            context.tagged_cashflows = Some(schedule.into_flows());
        }
    }

    fn effective_start_date(&self) -> Option<Date> {
        None
    }

    fn model_params_snapshot(&self) -> ModelParamsSnapshot {
        ModelParamsSnapshot::StructuredCredit {
            prepayment_spec: self.credit_model.prepayment_spec.clone(),
            default_spec: self.credit_model.default_spec.clone(),
            recovery_spec: self.credit_model.recovery_spec.clone(),
        }
    }

    fn with_model_params(
        &self,
        params: &ModelParamsSnapshot,
    ) -> finstack_quant_core::Result<Box<dyn Instrument>> {
        match params {
            ModelParamsSnapshot::StructuredCredit {
                prepayment_spec,
                default_spec,
                recovery_spec,
            } => {
                let mut modified = self.clone();
                modified.credit_model.prepayment_spec = prepayment_spec.clone();
                modified.credit_model.default_spec = default_spec.clone();
                modified.credit_model.recovery_spec = recovery_spec.clone();
                Ok(Box::new(modified))
            }
            ModelParamsSnapshot::None => Ok(self.clone_box()),
            ModelParamsSnapshot::Convertible { .. } => Err(Error::Validation(
                "Instrument type mismatch: expected StructuredCredit model parameters".to_string(),
            )),
        }
    }

    fn pricing_overrides_mut(
        &mut self,
    ) -> Option<&mut crate::instruments::pricing_overrides::PricingOverrides> {
        Some(&mut self.pricing_overrides)
    }

    fn pricing_overrides(
        &self,
    ) -> Option<&crate::instruments::pricing_overrides::PricingOverrides> {
        Some(&self.pricing_overrides)
    }
}

impl StructuredCredit {
    /// Generate cashflows for a specific tranche after waterfall allocation.
    pub fn get_tranche_cashflows(
        &self,
        tranche_id: &str,
        context: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<TrancheCashflows> {
        crate::instruments::fixed_income::structured_credit::pricing::generate_tranche_cashflows(
            self, tranche_id, context, as_of,
        )
    }

    /// Calculate present value for a specific tranche.
    pub fn value_tranche(
        &self,
        tranche_id: &str,
        context: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        let cashflows = self.get_tranche_cashflows(tranche_id, context, as_of)?;
        let disc = context.get_discount(&self.discount_curve_id)?;

        let mut pv = Money::new(0.0, self.pool.base_currency());
        for (date, amount) in &cashflows.cashflows {
            if *date > as_of {
                let df = disc.df_between_dates(as_of, *date)?;
                let flow_pv = Money::new(amount.amount() * df, amount.currency());
                pv = pv.checked_add(flow_pv)?;
            }
        }

        Ok(pv)
    }

    /// Get full valuation with metrics for a specific tranche.
    pub fn value_tranche_with_metrics(
        &self,
        tranche_id: &str,
        context: &MarketContext,
        as_of: Date,
        metrics: &[MetricId],
    ) -> finstack_quant_core::Result<TrancheValuation> {
        use crate::instruments::fixed_income::structured_credit::metrics::{
            calculate_tranche_cs01, calculate_tranche_duration, calculate_tranche_wal,
            calculate_tranche_z_spread,
        };

        let cashflow_result = self.get_tranche_cashflows(tranche_id, context, as_of)?;
        let pv = self.value_tranche(tranche_id, context, as_of)?;

        let mut metric_context = crate::metrics::MetricContext::new(
            std::sync::Arc::new(self.clone())
                as std::sync::Arc<dyn crate::instruments::common_impl::traits::Instrument>,
            std::sync::Arc::new(context.clone()),
            as_of,
            pv,
            MetricContext::default_config(),
        );
        metric_context.cashflows = Some(cashflow_result.cashflows.clone());
        metric_context.tagged_cashflows = Some(cashflow_result.detailed_flows.clone());
        metric_context.detailed_tranche_cashflows = Some(cashflow_result.clone());
        metric_context.discount_curve_id = Some(self.discount_curve_id.to_owned());

        let registry = crate::metrics::standard_registry();
        let computed_metrics = registry.compute(metrics, &mut metric_context)?;

        let tranche = self
            .tranches
            .tranches
            .iter()
            .find(|t| t.id.as_str() == tranche_id)
            .ok_or_else(|| {
                finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                    id: format!("tranche:{}", tranche_id),
                })
            })?;

        let notional = tranche.original_balance.amount();

        let dirty_price = if notional > 0.0 {
            (pv.amount() / notional) * 100.0
        } else {
            0.0
        };

        let accrued_value = computed_metrics
            .get(&MetricId::Accrued)
            .copied()
            .unwrap_or(0.0);
        let accrued = Money::new(accrued_value, pv.currency());

        let clean_price = if notional > 0.0 {
            dirty_price - (accrued.amount() / notional) * 100.0
        } else {
            dirty_price
        };

        let wal = match computed_metrics.get(&MetricId::WAL) {
            Some(v) => *v,
            None => calculate_tranche_wal(&cashflow_result, as_of)?,
        };

        let disc = context.get_discount(&self.discount_curve_id)?;
        let modified_duration = computed_metrics
            .get(&MetricId::DurationMod)
            .copied()
            .unwrap_or_else(|| {
                calculate_tranche_duration(&cashflow_result.cashflows, &disc, as_of, pv)
                    .unwrap_or(0.0)
            });

        let z_spread = computed_metrics
            .get(&MetricId::ZSpread)
            .copied()
            .unwrap_or_else(|| {
                calculate_tranche_z_spread(&cashflow_result.cashflows, &disc, pv, as_of)
                    .unwrap_or(0.0)
            });

        let z_spread_decimal = z_spread / 10_000.0;
        let cs01 = computed_metrics
            .get(&MetricId::Cs01)
            .copied()
            .unwrap_or_else(|| {
                calculate_tranche_cs01(&cashflow_result.cashflows, &disc, z_spread_decimal, as_of)
                    .unwrap_or(0.0)
            });

        let ytm = computed_metrics
            .get(&MetricId::Ytm)
            .copied()
            .unwrap_or(0.05);

        let final_metrics: finstack_quant_core::HashMap<MetricId, f64> =
            computed_metrics.into_iter().collect();

        Ok(TrancheValuation {
            tranche_id: tranche_id.to_string(),
            pv,
            clean_price,
            dirty_price,
            accrued,
            wal,
            modified_duration,
            z_spread_bps: z_spread,
            cs01,
            ytm,
            metrics: final_metrics,
        })
    }
}

impl core::fmt::Debug for StructuredCredit {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("StructuredCredit")
            .field("id", &self.id)
            .field("deal_type", &self.deal_type)
            .field("closing_date", &self.closing_date)
            .field("first_payment_date", &self.first_payment_date)
            .field("maturity", &self.maturity)
            .field("frequency", &self.frequency)
            .field("discount_curve_id", &self.discount_curve_id)
            .finish()
    }
}

impl core::fmt::Display for StructuredCredit {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let pool_balance = self
            .pool
            .total_balance()
            .unwrap_or(Money::new(0.0, self.pool.base_currency()));
        let tranche_count = self.tranches.tranches.len();

        write!(
            f,
            "{} {:?} | AssetPool: {} {} | {} tranches | {} -> {}",
            self.id.as_str(),
            self.deal_type,
            pool_balance.amount(),
            pool_balance.currency(),
            tranche_count,
            self.closing_date,
            self.maturity,
        )
    }
}
