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
pub(crate) mod pricing;
pub(crate) mod types;
pub(crate) mod utils;

pub use utils::{validate_tiers, ValidationError};

pub use types::MarketConditions;
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
    LossRecognition, ManagementFeeType, Metadata, ModificationSpec, PaymentCalculation,
    PaymentRecord, PaymentType, PenaltyStep, PoolAsset, PoolStats, PrepaymentPenalty,
    PutExercisePolicy, Recipient, RecipientType, ReinvestmentAssumptions, ReinvestmentCriteria,
    ReinvestmentPeriod, RepLine, ReserveAccountSpec, ReserveInterestDestination, ReserveTarget,
    RoundingConvention, ShiftMode, ShiftingInterestSpec, ShiftingInterestStep,
    SpecialServicingSpec, StandardDealParams, StepDownSpec, StepDownTrigger, StructuredCredit,
    StructuredCreditBuilder, SwapNotional, SwapPriority, TargetOcSpec, TemplateFees, Tranche,
    TrancheAccrualPeriod, TrancheBuilder, TrancheCashflows, TrancheDraw, TrancheReadvance,
    TrancheSeniority, TrancheStructure, TriggerConsequence, Waterfall, WaterfallBuilder,
    WaterfallDistribution, WaterfallRules, WaterfallTier,
};

pub use pricing::{execute_waterfall, run_simulation};

pub use finstack_quant_valuations_core::results::structured_credit::{
    StochasticPricingResult, StructuredCreditPricingMode, TranchePricingResult,
};
pub use pricing::coverage_tests::{TestContext, TestResult};
pub use pricing::waterfall::WaterfallContext;
pub use pricing::{
    run_simulation_with_diagnostics, CoverageTestDiagnostic, PeriodDiagnostics,
    SimulationDiagnostics, SimulationRun,
};

pub use metrics::{
    calculate_equity_metrics,
    calculate_tranche_convexity,
    calculate_tranche_cs01,
    calculate_tranche_duration,
    calculate_tranche_wal,
    calculate_tranche_z_spread,
    tranche_breakeven_cdr,
    tranche_discount_margin,
    tranche_metrics,
    tranche_oas,
    tranche_scenario_table,
    // Deal and pool metric calculators
    AbsChargeOffCalculator,
    AbsCreditEnhancementCalculator,
    AbsExcessSpreadCalculator,
    AbsPaymentRateCalculator,
    CdrCalculator,
    CloWarfCalculator,
    CmbsDscrCalculator,
    CprCalculator,
    EquityMetrics,
    OasConfig,
    OasResult,
    ScenarioCell,
    ScenarioGrid,
    ScenarioTable,
    TrancheMetrics,
};
