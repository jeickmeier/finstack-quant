//! Waterfall type definitions for structured credit instruments.
//!
//! This module contains all data structures for waterfall distribution:
//! - Payment recipients and calculation methods
//! - Tier structures and allocation modes
//! - Coverage-test positions (`PaymentType::CoverageTest` tiers carrying
//!   `CoverageTestSpec`s) for OC/IC diversion
//! - Result types for waterfall execution
//!
//! Execution logic is in `crate::structured_credit::pricing::waterfall`.
//! The declarative period rules live in [`rules`], the coverage-test
//! declarations in [`coverage`] and construction in [`builder`].

pub(crate) mod builder;
pub(crate) mod coverage;
pub(crate) mod rules;
pub(crate) mod spec;

pub(crate) use coverage::{
    CoverageRules, CoverageTestAction, CoverageTestSpec, CoverageTestType, DefaultedValuation,
};
#[cfg(test)]
pub(crate) use rules::ExcessSpreadSpec;
pub(crate) use rules::{
    AfcSpec, ControlledAccumulationSpec, ShiftMode, ShiftingInterestSpec, ShiftingInterestStep,
    StepDownSpec, StepDownTrigger, TargetOcSpec, WaterfallRules,
};
pub(crate) use spec::{
    AllocationMode, DiversionRecord, EquityHistory, FundingSource, ManagementFeeType,
    PaymentCalculation, PaymentRecord, PaymentType, Recipient, RecipientType, RoundingConvention,
    TemplateFees, Waterfall, WaterfallDistribution, WaterfallTier,
};
