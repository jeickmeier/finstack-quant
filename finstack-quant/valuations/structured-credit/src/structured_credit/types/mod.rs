//! Type definitions for structured credit instruments.
//!
//! This module contains all data structures for structured credit instruments:
//! - `StructuredCredit` - The main instrument type
//! - AssetPool and asset types
//! - Tranche structure and coupon types
//! - Waterfall distribution types
//! - Behavioral model specifications
//! - Result types for valuation

pub(crate) mod borrowing_base;
pub(crate) mod call;
pub(crate) mod card;
pub(crate) mod cmbs;
pub(crate) mod collateral;
pub(crate) mod constants;
pub(crate) mod delinquency;
pub(crate) mod draws;
pub(crate) mod enums;
pub(crate) mod hedges;
pub(crate) mod npl;
pub(crate) mod pool;
pub(crate) mod pool_report;
/// SoA layout for pool assets.
pub(crate) mod pool_state;
pub(crate) mod results;
pub(crate) mod setup;
pub(crate) mod tranches;
pub(crate) mod waterfall;

pub(crate) mod constructors;
pub(crate) mod deal;
mod instrument;
mod pricing_methods;
mod resolved;
mod stochastic;
mod structured_credit_impl;
pub(crate) mod tranche_view;

pub(crate) use borrowing_base::LiveCollateral;
pub(crate) use call::{CallAssumption, CallScope};
pub(crate) use card::CardPortfolioSpec;
pub(crate) use collateral::{
    CallExercisePolicy, CollateralInstrument, InstrumentCollateral, PutExercisePolicy,
    ReserveInterestDestination,
};
pub(crate) use deal::{CreditModelConfig, MarketConditions, Metadata, StructuredCredit};
pub(crate) use delinquency::{AdvancingPolicy, DelinquencyModel};
pub(crate) use draws::{TrancheDraw, TrancheReadvance};
pub(crate) use enums::{
    AssetType, DealType, LossAllocationPolicy, LossRecognition, TriggerConsequence,
};
pub(crate) use finstack_quant_valuations_core::results::structured_credit::TrancheSeniority;
pub(crate) use hedges::{HedgeSwap, SwapNotional, SwapPriority};
pub(crate) use pool::{AssetPool, PoolAsset, ReinvestmentAssumptions};
#[cfg(test)]
pub(crate) use pool::{ReinvestmentCriteria, ReinvestmentPeriod};
pub(crate) use pool_state::PoolState;
pub(crate) use results::{DealAccruals, TrancheAccrualPeriod, TrancheCashflows};
pub(crate) use setup::{DealFees, IncentiveFeeSpec};
pub(crate) use tranches::{CoverageTrigger, Tranche, TrancheStructure};
#[cfg(test)]
pub(crate) use waterfall::ExcessSpreadSpec;
pub(crate) use waterfall::{
    AfcSpec, AllocationMode, ControlledAccumulationSpec, CoverageRules, CoverageTestAction,
    CoverageTestSpec, CoverageTestType, DefaultedValuation, DiversionRecord, EquityHistory,
    FundingSource, ManagementFeeType, PaymentCalculation, PaymentRecord, PaymentType, Recipient,
    RecipientType, RoundingConvention, ShiftMode, ShiftingInterestSpec, ShiftingInterestStep,
    StepDownSpec, StepDownTrigger, TargetOcSpec, TemplateFees, Waterfall, WaterfallDistribution,
    WaterfallRules, WaterfallTier,
};
