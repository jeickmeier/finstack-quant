//! Least-squares Monte Carlo for option-bearing credit-risky bonds.
//!
//! Training and pricing use disjoint Philox seed domains. The regression
//! policy is fitted on standardized rate, hazard, balance, locked-coupon,
//! accrued-interest, and cumulative-distribution state, then frozen before
//! the pricing paths are sampled.
//!
//! Floating PIK changes principal at the economic accrual boundary while cash
//! settles independently on the adjusted payment date. Stochastic floating PIK
//! combined with amortization is rejected: the replay template does not carry
//! canonical runtime amortization targets, so projected repayment amounts would
//! violate remaining-balance targets on sampled paths.

use super::bond_valuator::BondValuator;
use crate::bond::pricing::return_floor::realized_distributions;
use crate::bond::{Bond, CallPut, CashflowSpec, ProtectionWindow, ReturnFloorKind};
use crate::rates_credit::continuous_frp_weight;
use finstack_quant_cashflows::builder::{
    CompiledFloatingCoupon, FloatingCouponEconomics, FloatingCouponPeriod,
    FloatingCouponReplayState, FloatingRateObservation, OvernightObservationSchedule,
    OvernightRateConstraints, observed_index_fixing,
};
use finstack_quant_cashflows::builder::{CouponType, FloatingCouponSpec};
use finstack_quant_core::cashflow::{CFKind, CashFlow};
use finstack_quant_core::dates::calendar_by_id_strict;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::fixings::fixing_series_id;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::{Error, Result};
use finstack_quant_models::monte_carlo::pricer::polynomial::{
    PolynomialDegree, StandardizedPolynomialPolicy,
};
use finstack_quant_models::trees::two_factor_rates_credit::{
    BandTable, RatesCreditPathCheckpoint, RatesCreditPathState, RatesCreditTree,
};
use rust_decimal::prelude::ToPrimitive;
use smallvec::SmallVec;
use std::collections::{BTreeMap, BTreeSet};
use time::Duration;

mod build;
mod engine;
mod policy;
mod replay;
mod state;
#[cfg(test)]
mod tests;

use build::*;
use engine::*;
pub(crate) use engine::{BondLsmcConfig, BondLsmcResult, price_bond_lsmc};
use policy::*;
use state::*;
