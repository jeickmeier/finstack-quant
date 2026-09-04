//! Node-coupon descriptor construction for stochastic-rate floating resets.
//!
//! The rates-credit lattice values a future floating coupon's node
//! dependence as an **increment** over its deterministic projection (see
//! [`NodeCoupon`]). This module scans an instrument's canonical
//! [`CashFlowSchedule`] and turns every future-reset floating coupon into a
//! descriptor, using only metadata the emission already stamps on each
//! flow: the reset (observation) date, the accrual period, and the
//! projected pre-composition index rate. No second coupon specification is
//! introduced.
//!
//! Used by the callable-term-loan recombining-tree engine. Credit-risky bonds
//! replay their floating and PIK state pathwise in the LSMC engine.
//!
//! # What stays deterministic
//!
//! - Coupons whose observation date is on or before the valuation origin
//!   (known fixings, including the reset-lag sliver where the fixing has
//!   published but accrual has not started).
//! - Coupons the emission itself projected deterministically via a
//!   fallback policy (`SpreadOnly` / `FixedRate`), identified by a missing
//!   `projected_index_rate`.
//! - Every fixed-rate, fee, and principal flow.

use crate::cashflow::builder::rate_helpers::FloatingRateParams;
use crate::cashflow::builder::schedule::CashFlowSchedule;
use crate::cashflow::builder::specs::{FloatingRateSpec, OvernightIndexConstraintApplication};
use crate::cashflow::primitives::CFKind;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::{Error, Result};
use finstack_quant_models::trees::two_factor_rates_credit::NodeCoupon;
use rust_decimal::prelude::ToPrimitive;

/// Convert a [`FloatingRateSpec`] into the f64 composition parameters the
/// increment shares with the emission (same mapping the term-loan
/// discounting pricer applies for realized fixings).
pub(crate) fn params_from_spec(spec: &FloatingRateSpec) -> FloatingRateParams {
    FloatingRateParams {
        spread_bp: spec.spread_bp.to_f64().unwrap_or_default(),
        gearing: spec.gearing.to_f64().unwrap_or(1.0),
        gearing_includes_spread: spec.gearing_includes_spread,
        index_floor_bp: spec.index_floor_bp.and_then(|value| value.to_f64()),
        index_cap_bp: spec.index_cap_bp.and_then(|value| value.to_f64()),
        all_in_floor_bp: spec.all_in_floor_bp.and_then(|value| value.to_f64()),
        all_in_cap_bp: spec.all_in_cap_bp.and_then(|value| value.to_f64()),
    }
}

/// Whether period-level composition must drop index-level floor/cap
/// because the leg applies them daily inside an overnight-compounded
/// window (mirrors the emission's `period_rate_params_for_overnight`).
pub(crate) fn strips_index_constraints(spec: &FloatingRateSpec) -> bool {
    spec.overnight_compounding.is_some()
        && spec.overnight_index_constraints == OvernightIndexConstraintApplication::Daily
}

fn ceil_step(time_steps: &[f64], t: f64) -> usize {
    let last = time_steps.len() - 1;
    if t <= time_steps[0] {
        0
    } else if t >= time_steps[last] {
        last
    } else {
        time_steps.partition_point(|&grid_time| grid_time < t)
    }
}

/// Inputs for [`build_node_coupons`].
pub(crate) struct NodeCouponBuildInputs<'a> {
    /// Canonical emitted schedule (already projected off today's curves).
    pub schedule: &'a CashFlowSchedule,
    /// Instrument-level floating-rate composition parameters.
    pub params: FloatingRateParams,
    /// Valuation origin — the tree's `t = 0`.
    pub grid_origin: Date,
    /// Uniform tree time grid (`time_steps[0] == 0.0`).
    pub time_steps: &'a [f64],
    /// Day count of the tree's time axis (the discount curve's).
    pub day_count: DayCount,
    /// The tree's discount curve (slice forwards and timing corrections).
    pub discount: &'a dyn Discounting,
    /// Whether to drop index-level floor/cap from the composition because
    /// the leg applies them **daily** inside an overnight-compounded
    /// observation window (`OvernightIndexConstraintApplication::Daily`);
    /// the emission strips them from period-level composition in that
    /// case, and the increment must compose the same way.
    pub strip_index_constraints: bool,
}

/// Scan the schedule for future-reset floating coupons and build their
/// node-coupon descriptors.
///
/// `notional_at_step` supplies the outstanding principal the coupon
/// accrues on, indexed by the snapped reset step — the same step-indexed
/// outstanding the engine uses for recovery and redemption.
///
/// # Errors
///
/// Returns [`Error::Validation`] when a future floating coupon's reset and
/// payment dates snap to the same tree slice (grid too coarse) or a
/// year-fraction computation fails.
pub(crate) fn build_node_coupons(
    inputs: &NodeCouponBuildInputs<'_>,
    notional_at_step: impl Fn(usize) -> f64,
) -> Result<Vec<NodeCoupon>> {
    let steps = inputs.time_steps.len() - 1;
    let mut coupons = Vec::new();

    let mut params = inputs.params.clone();
    if inputs.strip_index_constraints {
        params.index_floor_bp = None;
        params.index_cap_bp = None;
    }

    for cf in inputs.schedule.get_flows() {
        if cf.kind != CFKind::FloatReset {
            continue;
        }
        // Future observation only: a published fixing (reset on or before
        // the origin) is deterministic and must not move with rate vol.
        let future_reset = match cf.reset_date {
            Some(reset) => reset > inputs.grid_origin,
            None => false,
        };
        if !future_reset {
            continue;
        }
        let Some(accrual) = cf.accrual else {
            continue;
        };
        // Fallback-projected coupons (SpreadOnly / FixedRate) carry no
        // projected index rate; the emission treated them as deterministic
        // and the lattice must agree.
        let Some(base_index_rate) = accrual.projected_index_rate else {
            continue;
        };
        let tau = cf.accrual_factor;
        if tau <= 0.0 || !tau.is_finite() {
            continue;
        }

        let ctx = DayCountContext::default();
        let t_start = inputs
            .day_count
            .year_fraction(inputs.grid_origin, accrual.start, ctx)?;
        let t_pay = inputs
            .day_count
            .year_fraction(inputs.grid_origin, cf.date, ctx)?;
        let reset_step = ceil_step(inputs.time_steps, t_start.max(0.0));
        let payment_step = ceil_step(inputs.time_steps, t_pay);
        if reset_step >= payment_step {
            return Err(Error::Validation(format!(
                "floating coupon accruing {} to {} (paid {}) snaps to a single \
                 tree slice (step {reset_step}) under stochastic rates; the tree \
                 grid is too coarse to distinguish its reset from its payment. \
                 Increase tree_steps (currently {steps}).",
                accrual.start, accrual.end, cf.date
            )));
        }

        let t_slice_reset = inputs.time_steps[reset_step];
        let t_slice_pay = inputs.time_steps[payment_step];
        let df_reset = inputs.discount.df(t_slice_reset);
        let df_pay = inputs.discount.df(t_slice_pay);
        if df_pay <= f64::EPSILON || df_reset <= f64::EPSILON {
            return Err(Error::Validation(format!(
                "degenerate discount factors over floating period slices \
                 [{t_slice_reset}, {t_slice_pay}]"
            )));
        }
        let base_discount_forward = (df_reset / df_pay - 1.0) / tau;
        // Same DF timing correction the deterministic booking applies via
        // `value_at_step_time`: value at the true payment date, expressed
        // at the snapped payment slice.
        let timing_scale = inputs.discount.df(t_pay) / df_pay;

        coupons.push(NodeCoupon {
            reset_step,
            payment_step,
            accrual: tau,
            notional: notional_at_step(reset_step),
            base_index_rate,
            base_discount_forward,
            timing_scale,
            params: params.clone(),
        });
    }

    Ok(coupons)
}

/// Whether the schedule carries any capitalizing (PIK) flow after the
/// valuation origin.
///
/// A future floating PIK coupon capitalizes a node-dependent amount into
/// principal, making outstanding balance, recovery, and redemption
/// path-dependent. The engines reject that combination in stochastic-rate
/// mode rather than misstate principal.
pub(crate) fn has_future_pik(schedule: &CashFlowSchedule, grid_origin: Date) -> bool {
    schedule
        .get_flows()
        .iter()
        .any(|cf| cf.kind == CFKind::Pik && cf.date > grid_origin)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn term_loan_events_snap_to_the_next_slice() {
        let grid = [0.0, 0.5, 1.0, 1.5, 2.0];
        assert_eq!(ceil_step(&grid, 0.70), 2);
        assert_eq!(ceil_step(&grid, 0.50), 1);
        assert_eq!(ceil_step(&grid, -0.1), 0);
        assert_eq!(ceil_step(&grid, 9.0), 4);
    }
}
