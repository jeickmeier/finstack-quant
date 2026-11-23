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

pub use utils::validation::{ValidationError, validate_tiers};

pub use types::borrowing_base::{
    AdvanceRate, BorrowingBaseReport, BorrowingBaseRules, ConcentrationLimit, ConcentrationScope,
    EligibilityRule, LiveCollateral,
};
pub use types::call::{CallAssumption, CallScope};
pub use types::card::CardPortfolioSpec;
pub use types::cmbs::{BalloonSpec, PenaltyStep, PrepaymentPenalty, SpecialServicingSpec};
pub use types::collateral::{
    CallExercisePolicy, CollateralInstrument, InstrumentCollateral, InstrumentExerciseOverride,
    PutExercisePolicy, ReserveInterestDestination,
};
pub use types::constructors::{StandardDealFamily, StandardDealParams, standard_deal_families};
pub use types::deal::MarketConditions;
pub use types::deal::{CreditModelConfig, Metadata, StructuredCredit, StructuredCreditBuilder};
pub use types::delinquency::{AdvancingPolicy, DelinquencyModel, ModificationSpec};
pub use types::draws::{TrancheDraw, TrancheReadvance};
pub use types::enums::{
    AssetType, DealType, LossAllocationPolicy, LossRecognition, TriggerConsequence,
};
pub use types::hedges::{HedgeSwap, SwapNotional, SwapPriority};
pub use types::npl::LiquidationSpec;
pub use types::pool::analytics::calculate_pool_stats;
pub use types::pool::asset::PoolAsset;
pub use types::pool::asset_pool::AssetPool;
pub use types::pool::config::{
    PoolAccounts, PoolStats, ReinvestmentAssumptions, ReinvestmentCriteria, ReinvestmentPeriod,
    RepLine,
};
pub use types::pool_report::{PoolConcentrationEntry, PoolReport, pool_report};
pub use types::results::{TrancheAccrualPeriod, TrancheCashflows};
pub use types::setup::{DealFees, IncentiveFeeSpec};
pub use types::tranche_view::StructuredCreditTranche;
pub use types::tranches::{CoverageTrigger, Tranche, TrancheBuilder, TrancheStructure};
pub use types::waterfall::builder::WaterfallBuilder;
pub use types::waterfall::coverage::{
    CccBucketRule, CoveragePlacement, CoverageRules, CoverageTestAction, CoverageTestSpec,
    CoverageTestType, DefaultedValuation, DiscountObligationRule,
};
pub use types::waterfall::rules::{
    AfcSpec, ControlledAccumulationSpec, EarlyAmortizationSpec, ExcessSpreadSpec,
    ReserveAccountSpec, ReserveTarget, ShiftMode, ShiftingInterestSpec, ShiftingInterestStep,
    StepDownSpec, StepDownTrigger, TargetOcSpec, WaterfallRules,
};
pub use types::waterfall::spec::{
    AllocationMode, DiversionRecord, EquityHistory, FundingSource, ManagementFeeType,
    PaymentCalculation, PaymentRecord, PaymentType, Recipient, RecipientType, RoundingConvention,
    TemplateFees, Waterfall, WaterfallDistribution, WaterfallTier,
};

pub use pricing::waterfall::execute::execute_waterfall;

pub use pricing::coverage::{TestContext, TestResult};
pub use pricing::simulation_engine::orchestration::SimulationRun;
pub use pricing::simulation_engine::state::{
    CoverageTestBreach, CoverageTestDiagnostic, NonWaterfallPayment, NonWaterfallSource,
    PeriodDiagnostics, SimulationDiagnostics, TrancheCreditEnhancement,
};
pub use pricing::waterfall::execute::WaterfallContext;

pub use metrics::deal_specific::abs::{
    AbsChargeOffCalculator, AbsCreditEnhancementCalculator, AbsExcessSpreadCalculator,
    AbsPaymentRateCalculator,
};
pub use metrics::deal_specific::cmbs::CmbsDscrCalculator;
pub use metrics::pool::{CdrCalculator, CloWarfCalculator, CprCalculator};
pub use metrics::pricing::wal::calculate_tranche_wal;
pub use metrics::risk::breakeven_cdr::tranche_breakeven_cdr;
pub use metrics::risk::convexity::calculate_tranche_convexity;
pub use metrics::risk::duration::calculate_tranche_duration;
pub use metrics::risk::oas::{OasConfig, OasResult, tranche_oas};
pub use metrics::risk::spreads::{
    calculate_tranche_cs01, calculate_tranche_z_spread, tranche_discount_margin,
};
pub use metrics::scenario::{ScenarioCell, ScenarioGrid, ScenarioTable, tranche_scenario_table};
pub use metrics::summary::{
    EquityMetrics, TrancheMetrics, calculate_equity_metrics, tranche_metrics,
};
