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

pub(crate) mod assumptions;
pub(crate) mod metrics;
pub(crate) mod pricer;
pub(crate) mod pricing;
pub(crate) mod types;
pub(crate) mod utils;

pub use types::{
    calculate_pool_stats, AdvanceRate, AdvancingPolicy, AfcSpec, AllocationMode, AssetPool,
    AssetType, BalloonSpec, BorrowingBaseReport, BorrowingBaseRules, CallAssumption,
    CallExercisePolicy, CallScope, CardPortfolioSpec, CccBucketRule, CollateralInstrument,
    ConcentrationLimit, ConcentrationScope, ControlledAccumulationSpec, CoveragePlacement,
    CoverageRules, CoverageTestAction, CoverageTestSpec, CoverageTestType, CoverageTrigger,
    CreditModelConfig, DealFees, DealType, DefaultedValuation, DelinquencyModel,
    DiscountObligationRule, EarlyAmortizationSpec, EligibilityRule, EquityHistory,
    ExcessSpreadSpec, FundingSource, HedgeSwap, IncentiveFeeSpec, InstrumentCollateral,
    InstrumentExerciseOverride, LiquidationSpec, LiveCollateral, LossAllocationPolicy,
    LossRecognition, ManagementFeeType, Metadata, ModificationSpec, Overrides, PaymentCalculation,
    PaymentMode, PaymentRecord, PaymentType, PenaltyStep, PoolAsset, PoolStats, PrepaymentPenalty,
    PutExercisePolicy, Recipient, RecipientType, ReinvestmentAssumptions, ReinvestmentCriteria,
    ReinvestmentPeriod, RepLine, ReserveAccountSpec, ReserveInterestDestination, ReserveTarget,
    RoundingConvention, ShiftMode, ShiftingInterestSpec, ShiftingInterestStep,
    SpecialServicingSpec, StepDownSpec, StepDownTrigger, StructuredCredit, StructuredCreditBuilder,
    SwapNotional, SwapPriority, TargetOcSpec, TemplateFees, Tranche, TrancheAccrualPeriod,
    TrancheBuilder, TrancheCashflows, TrancheCoupon, TrancheDraw, TrancheReadvance,
    TrancheSeniority, TrancheStructure, TrancheValuation, TriggerConsequence, Waterfall,
    WaterfallBuilder, WaterfallDistribution, WaterfallRules, WaterfallTier,
};

pub use crate::cashflow::builder::{DefaultCurve, PrepaymentCurve};
pub use types::{
    CreditFactors, DefaultModelSpec, MarketConditions, PrepaymentModelSpec, RecoveryModelSpec,
    StructuredCreditTranche,
};

pub use utils::{
    clamped_cdr_to_mdr, clamped_cpr_to_smm, clamped_mdr_to_cdr, clamped_smm_to_cpr,
    get_validation_errors, is_valid_waterfall_spec, psa_to_cpr, ValidationError,
};

pub use pricing::{
    execute_waterfall, generate_cashflows, generate_tranche_cashflows, run_simulation,
};

pub use pricing::coverage_tests::{CoverageTest, TestContext, TestResult};
pub use pricing::stochastic::PricingMode;
pub use pricing::stochastic::{StochasticPricingResult, TranchePricingResult};
pub use pricing::waterfall::execute_waterfall_with_explanation;
pub use pricing::waterfall::WaterfallContext;
pub use pricing::{
    run_simulation_with_diagnostics, CoverageTestDiagnostic, PeriodDiagnostics,
    SimulationDiagnostics, SimulationRun,
};

pub use metrics::{
    calculate_equity_metrics,
    calculate_tranche_breakeven_cdr,
    calculate_tranche_convexity,
    calculate_tranche_cs01,
    calculate_tranche_discount_margin,
    calculate_tranche_duration,
    calculate_tranche_metrics,
    calculate_tranche_oas,
    calculate_tranche_wal,
    calculate_tranche_z_spread,
    scenario_table,
    // Deal-specific metrics
    AbsChargeOffCalculator,
    AbsCreditEnhancementCalculator,
    AbsDelinquencyCalculator,
    AbsExcessSpreadCalculator,
    AbsPaymentRateCalculator,
    // Pricing metrics
    AccruedCalculator,
    CdrCalculator,
    CleanPriceCalculator,
    CloWarfCalculator,
    CloWasCalculator,
    CmbsDscrCalculator,
    ConvexityCalculator,
    CprCalculator,
    Cs01Calculator,
    DirtyPriceCalculator,
    EquityMetrics,
    // Risk metrics
    MacaulayDurationCalculator,
    ModifiedDurationCalculator,
    OasConfig,
    OasResult,
    ScenarioCell,
    ScenarioGrid,
    ScenarioTable,
    SpreadDurationCalculator,
    TrancheMetrics,
    WalCalculator,
    // AssetPool metrics
    WamCalculator,
    YtmCalculator,
    ZSpreadCalculator,
};

pub use types::constants::{
    abs_auto_standard_cdr, abs_auto_standard_recovery, abs_auto_standard_speed,
    abs_servicing_fee_bp, abs_trustee_fee_annual, baseline_unemployment_rate,
    clo_senior_mgmt_fee_bp, clo_standard_cdr, clo_standard_cpr, clo_standard_recovery,
    clo_subordinated_mgmt_fee_bp, clo_trustee_fee_annual, cmbs_master_servicer_fee_bp,
    cmbs_special_servicer_fee_bp, cmbs_standard_cdr, cmbs_standard_cpr, cmbs_standard_recovery,
    cmbs_trustee_fee_annual, default_burnout_threshold_months, default_max_cov_lite,
    default_max_dip, default_max_obligor_concentration, default_max_second_lien,
    default_max_top10_concentration, default_max_top5_concentration, default_resolution_lag_months,
    pool_balance_cleanup_threshold, psa_ramp_months, psa_terminal_cpr, rmbs_servicing_fee_bp,
    rmbs_standard_cdr, rmbs_standard_psa, rmbs_standard_recovery, rmbs_trustee_fee_annual,
    sda_peak_cdr, sda_peak_month, sda_terminal_cdr, standard_cdr_rates, standard_psa_speeds,
    standard_severity_rates, AVERAGE_DAYS_PER_YEAR, BASIS_POINTS_DIVISOR, MIN_PREPAYMENT_RATE,
    MONTHS_PER_YEAR, PERCENTAGE_MULTIPLIER, QUARTERLY_PERIODS_PER_YEAR,
};
