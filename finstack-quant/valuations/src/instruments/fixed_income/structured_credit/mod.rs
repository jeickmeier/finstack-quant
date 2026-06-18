//! Structured credit instruments: ABS, RMBS, CMBS, and CLO with waterfall modeling.
//!
//! modeling for asset-backed securities with:
//! - Collateral pool management (prepayment, default, recovery)
//! - Multi-tranche capital structure with seniority
//! - Sequential-pay and pro-rata waterfall logic
//! - Overcollateralization and coverage tests
//! - Deal-specific metrics (WAL, WARF, WAS, DSCR, LTV)
//!
//! # Module Organization
//!
//! - `types`: All data structures (instrument, pool, tranches, waterfall, results)
//! - `pricing`: Pure functions for cashflow simulation and waterfall execution
//! - `metrics`: Risk metrics organized by category
//! - `utils`: Helper functions (rate conversions, validation)
//!
//! # See Also
//!
//! - `StructuredCredit` for main instrument struct
//! - `DealType` for ABS/RMBS/CMBS/CLO specification
//! - `AssetPool` for collateral pool modeling
//! - `Tranche` for tranche structure
//! - waterfall engine for cashflow distribution

// New module structure
pub(crate) mod assumptions;
pub(crate) mod metrics;
pub(crate) mod pricer;
pub(crate) mod pricing;
pub(crate) mod types;
pub(crate) mod utils;

/// Waterfall-specific public types.
pub mod waterfall {
    pub use super::types::waterfall::CoverageTrigger;
}

// ============================================================================
// MAIN TYPES
// ============================================================================

pub use types::{
    // AssetPool types
    calculate_pool_stats,
    AfcSpec,
    // Waterfall types
    AllocationMode,
    AssetPool,
    // Enums
    AssetType,
    // Metadata
    ConcentrationCheckResult,
    ConcentrationViolation,
    // Stochastic specs
    CorrelationStructure,
    // Configuration
    CoverageTestConfig,
    CoverageTestType,
    // Tranche types
    CoverageTrigger,
    CreditEnhancement,
    CreditModelConfig,
    DealConfig,
    DealDates,
    DealFees,
    DealType,
    DefaultAssumptions,
    ExcessSpreadSpec,
    ManagementFeeType,
    Metadata,
    Overrides,
    PaymentCalculation,
    PaymentMode,
    PaymentRecord,
    PaymentType,
    PoolAsset,
    PoolStats,
    Recipient,
    RecipientType,
    ReinvestmentCriteria,
    // Reinvestment
    ReinvestmentManager,
    ReinvestmentPeriod,
    RepLine,
    RoundingConvention,
    StochasticDefaultSpec,
    StochasticPrepaySpec,
    // Main instrument
    StructuredCredit,
    Tranche,
    TrancheBehaviorType,
    TrancheBuilder,
    // Result types
    TrancheCashflows,
    TrancheCoupon,
    TrancheSeniority,
    TrancheStructure,
    TrancheValuation,
    TriggerConsequence,
    Waterfall,
    WaterfallBuilder,
    WaterfallDistribution,
    WaterfallRules,
    WaterfallTier,
    WaterfallWorkspace,
};

// Behavioral models
pub use crate::cashflow::builder::{DefaultCurve, PrepaymentCurve};
pub use types::{
    CreditFactors, DefaultModelSpec, MarketConditions, PrepaymentModelSpec, RecoveryModelSpec,
};

// ============================================================================
// UTILITIES
// ============================================================================

pub use utils::{
    cdr_to_mdr, cpr_to_smm, get_validation_errors, is_valid_waterfall_spec, mdr_to_cdr, psa_to_cpr,
    smm_to_cpr, ValidationError,
};

// ============================================================================
// PRICING FUNCTIONS
// ============================================================================

pub use pricing::{
    execute_waterfall, execute_waterfall_with_workspace, generate_cashflows,
    generate_tranche_cashflows, resolve_waterfall, run_simulation,
};

pub use pricing::coverage_tests::{CoverageTest, TestContext, TestResult};
pub use pricing::diversion::{DiversionCondition, DiversionEngine, DiversionRule};
pub use pricing::stochastic::{PoolGranularity, PricingMode};
pub use pricing::stochastic::{StochasticPricingResult, TranchePricingResult};
pub use pricing::waterfall::execute_waterfall_with_explanation;
pub use pricing::waterfall::WaterfallContext;

// ============================================================================
// METRICS
// ============================================================================

pub use metrics::{
    calculate_tranche_breakeven_cdr,
    calculate_tranche_convexity,
    calculate_tranche_cs01,
    calculate_tranche_discount_margin,
    calculate_tranche_duration,
    calculate_tranche_wal,
    calculate_tranche_z_spread,
    scenario_table,
    // Deal-specific metrics
    AbsChargeOffCalculator,
    AbsCreditEnhancementCalculator,
    AbsDelinquencyCalculator,
    AbsExcessSpreadCalculator,
    AbsSpeedCalculator,
    // Pricing metrics
    AccruedCalculator,
    CdrCalculator,
    CleanPriceCalculator,
    CloWalCalculator,
    CloWarfCalculator,
    CloWasCalculator,
    CmbsDscrCalculator,
    CmbsLtvCalculator,
    ConvexityCalculator,
    CprCalculator,
    Cs01Calculator,
    DirtyPriceCalculator,
    // Risk metrics
    MacaulayDurationCalculator,
    ModifiedDurationCalculator,
    RmbsFicoCalculator,
    RmbsLtvCalculator,
    RmbsWalCalculator,
    ScenarioCell,
    ScenarioGrid,
    ScenarioTable,
    SpreadDurationCalculator,
    WalCalculator,
    // AssetPool metrics
    WamCalculator,
    YtmCalculator,
    ZSpreadCalculator,
};

// ============================================================================
// CONSTANTS
// ============================================================================

pub use types::constants::{
    abs_auto_standard_cdr, abs_auto_standard_recovery, abs_auto_standard_speed,
    abs_servicing_fee_bps, abs_trustee_fee_annual, baseline_unemployment_rate,
    clo_senior_mgmt_fee_bps, clo_standard_cdr, clo_standard_cpr, clo_standard_recovery,
    clo_subordinated_mgmt_fee_bps, clo_trustee_fee_annual, cmbs_master_servicer_fee_bps,
    cmbs_special_servicer_fee_bps, cmbs_standard_cdr, cmbs_standard_cpr, cmbs_standard_recovery,
    cmbs_trustee_fee_annual, credit_card_seasonality, default_auto_abs_speed,
    default_auto_ramp_months, default_burnout_threshold_months, default_max_cov_lite,
    default_max_dip, default_max_obligor_concentration, default_max_second_lien,
    default_max_top10_concentration, default_max_top5_concentration, default_resolution_lag_months,
    mortgage_seasonality, pool_balance_cleanup_threshold, psa_ramp_months, psa_terminal_cpr,
    rmbs_servicing_fee_bps, rmbs_standard_cdr, rmbs_standard_cpr, rmbs_standard_psa,
    rmbs_standard_recovery, rmbs_standard_sda, rmbs_trustee_fee_annual, sda_peak_cdr,
    sda_peak_month, sda_terminal_cdr, standard_cdr_rates, standard_psa_speeds,
    standard_severity_rates, AVERAGE_DAYS_PER_YEAR, BASIS_POINTS_DIVISOR, MIN_PREPAYMENT_RATE,
    MONTHS_PER_YEAR, PERCENTAGE_MULTIPLIER, QUARTERLY_PERIODS_PER_YEAR,
};
