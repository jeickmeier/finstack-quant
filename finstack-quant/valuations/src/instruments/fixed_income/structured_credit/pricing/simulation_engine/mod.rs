//! Shared cashflow simulation engine for structured credit instruments.
//!
//! This module provides pure functions for running period-by-period
//! cashflow simulation through the waterfall engine. Deterministic and
//! stochastic pricing differ only in how they source pool SMM/MDR/recovery
//! assumptions for each legal payment period.

use crate::cashflow::traits::DatedFlows;
use crate::instruments::fixed_income::structured_credit::assumptions::embedded_registry;
use crate::instruments::fixed_income::structured_credit::types::{
    AssetPool, PoolState, RecipientType, StructuredCredit, Tranche, TrancheCashflows,
    TrancheSeniority, TrancheStructure, Waterfall, WaterfallDistribution,
};
use crate::instruments::fixed_income::structured_credit::utils::simulation::RecoveryQueue;
use finstack_quant_core::cashflow::{CFKind, CashFlow};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::HolidayCalendar;
use finstack_quant_core::dates::{
    adjust, BusinessDayConvention, Date, DateExt, DayCount, DayCountContext, StubKind,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::fixings;
use finstack_quant_core::market_data::term_structures::ForwardCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::HashMap;
use finstack_quant_core::Result;
use finstack_quant_models::credit::pool::{PerNameCopulaDefault, PoolGranularity};
use finstack_quant_models::monte_carlo::rng::philox::PhiloxRng;
use finstack_quant_models::monte_carlo::traits::RandomStream;
use std::sync::Arc;

mod conservation;
mod orchestration;
mod period_helpers;
mod pool_flow_source;
mod pool_flows;
mod simulate_period;
mod state;
mod triggers;

use conservation::{assert_cash_conserved, recycle_reinvestment_principal};
use period_helpers::{
    collateral_asset_rate_for_period, live_afc_cap_rate, tranche_period_interest_due,
    SimulationPeriod, TrancheAccrualDates,
};
use pool_flows::{
    calculate_pool_flows_with_rates, PeriodDefaultOutcome, PoolFlowRates, PoolFlows,
    RatedPoolFlowRequest,
};
use simulate_period::simulate_period;
use state::{
    cleanup_call_premium, step_down_metrics, SimulationState, StateTemplate, WRITEDOWN_DE_MINIMIS,
};

#[cfg(test)]
use conservation::par_acquired_at_price;
#[cfg(test)]
use period_helpers::{current_collateral_wac, term_rate_for_period};

pub(crate) use orchestration::{
    aggregate_tranche_cashflows, prepare_deal_simulation, run_prepared_simulation_with_source,
    run_simulation_with_source, take_tranche_cashflows, PreparedDealSimulation,
};
pub(crate) use pool_flow_source::{
    DeterministicPoolFlowSource, OasPathFlowSource, PerNameDefaultEngine, PerNamePeriodInput,
    PeriodPoolShock, PoolFlowRequest, PoolFlowSource, StochasticPathFlowSource,
};

#[cfg(test)]
mod tests;
