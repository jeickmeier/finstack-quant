//! Core [`BondValuator`] state and constructor.

use super::super::TreePricer;
use crate::bond::Bond;
use finstack_quant_core::Result;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;

/// Bond valuator for tree-based pricing of callable/putable bonds.
///
/// Implements [`TreeValuator`](finstack_quant_models::trees::tree_framework::TreeValuator) trait for backward induction pricing with embedded options.
/// Maps bond cashflows and call/put schedules to tree time steps and handles
/// exercise decisions during backward induction.
///
/// # Call/Put Redemption Convention
///
/// Call/put redemption prices are computed as `outstanding_principal × (price_pct_of_par / 100)`,
/// where `outstanding_principal` is the remaining principal at the exercise date after
/// any amortization. This correctly handles amortizing callable bonds.
///
/// # Performance
///
/// Uses `Vec` instead of `HashMap` for step-indexed lookups to eliminate hashing
/// overhead in the backward induction hot path. For a 200-step tree, this provides
/// significant speedup over hash-based lookups.
///
/// # Thread Safety
///
/// `BondValuator` is `Send + Sync` (all fields are owned data or primitives),
/// making it safe to share across threads for parallel portfolio pricing.
pub struct BondValuator {
    pub(super) bond: Bond,
    /// Holder-view cashflow amounts indexed by time step (dense vector for O(1) access).
    /// Includes coupons, amortization, and final redemption — all positive receipts
    /// from the holder's perspective. Index `i` corresponds to time step `i`.
    /// Default value is 0.0.
    pub(in super::super) cashflow_vec: Vec<f64>,
    /// Base-curve-timing-adjusted cashflow pieces and their event-minus-step
    /// time offsets. The offsets let OAS discounting preserve each original
    /// cashflow date even when a uniform tree grid is used.
    pub(super) cashflow_components: Vec<Vec<(f64, f64)>>,
    /// Positive notional repayments mapped with the same timing convention as
    /// `cashflow_vec`. At the terminal node these are the contractual
    /// redemption alternative to a maturity call or put, rather than an
    /// unconditional payment in addition to exercise proceeds.
    pub(super) redemption_vec: Vec<f64>,
    /// OAS timing components for `redemption_vec`.
    pub(super) redemption_components: Vec<Vec<(f64, f64)>>,
    /// Call prices indexed by time step (sparse via Option for memory efficiency).
    /// `Some(price)` indicates a call option is exercisable at that step.
    /// Price is computed as `outstanding_principal × (price_pct / 100)`.
    pub(in super::super) call_vec: Vec<Option<f64>>,
    /// Put prices indexed by time step (sparse via Option for memory efficiency).
    /// `Some(price)` indicates a put option is exercisable at that step.
    /// Price is computed as `outstanding_principal × (price_pct / 100)`.
    pub(in super::super) put_vec: Vec<Option<f64>>,
    /// Outstanding principal indexed by time step for amortizing bonds.
    /// Used for call/put redemption and recovery calculations.
    pub(in super::super) outstanding_principal_vec: Vec<f64>,
    /// Time steps for tree pricing
    pub(super) time_steps: Vec<f64>,
    /// Base discount factors at each time step, conditional on `as_of`.
    pub(super) step_discount_factors: Vec<f64>,
    /// Optional recovery rate sourced from a hazard curve in MarketContext
    pub(super) recovery_rate: Option<f64>,
    /// Issuer call exercise friction in **cents per 100** of outstanding principal.
    ///
    /// This raises the exercise threshold (issuer calls only when continuation exceeds
    /// `call_price + friction_amount`), but redemption still occurs at `call_price`.
    pub(super) call_friction_cents: f64,
}

impl BondValuator {
    /// Create a new bond valuator for tree pricing.
    ///
    /// Builds maps of coupons, call prices, and put prices indexed by tree step.
    /// Cashflows and option exercise dates are mapped to the nearest tree step
    /// using the discount curve's day-count convention.
    ///
    /// # Arguments
    ///
    /// * `bond` - The bond to value
    /// * `market` - Market data including curves
    /// * `as_of` - Valuation date (time origin for the tree)
    /// * `time_to_maturity` - Time from `as_of` to maturity in years
    /// * `tree_steps` - Number of tree steps
    ///
    /// # Returns
    ///
    /// A `BondValuator` instance ready for tree-based pricing.
    ///
    /// # Errors
    ///
    /// Returns `Err` when:
    /// - Discount curve is not found
    /// - Cashflow schedule building fails
    /// - Time fraction calculations fail
    ///
    /// # Time Axis Consistency
    ///
    /// The `as_of` date defines the time origin (t=0) for the tree. All cashflow
    /// times and option exercise times are measured from `as_of` using the discount
    /// curve's day-count convention to ensure consistency with tree calibration.
    pub fn new(
        bond: Bond,
        market: &MarketContext,
        as_of: Date,
        time_to_maturity: f64,
        tree_steps: usize,
    ) -> Result<Self> {
        let dt = time_to_maturity / tree_steps as f64;
        let time_steps: Vec<f64> = (0..=tree_steps).map(|i| i as f64 * dt).collect();
        Self::new_with_time_steps(bond, market, as_of, time_steps)
    }

    /// Create a bond valuator on an explicit (possibly non-uniform) time
    /// grid, e.g. the grid of a [`HullWhiteTree`](finstack_quant_models::trees::hull_white_tree::HullWhiteTree) calibrated through
    /// mandatory call/coupon dates.
    ///
    /// `time_steps` must be a strictly increasing grid starting at `0.0`
    /// whose last entry is the time to maturity. Cashflows and exercise
    /// dates are mapped onto this grid by nearest-point lookup, with
    /// discount-factor timing corrections preserving each flow's PV.
    ///
    /// # Errors
    ///
    /// Returns `Err` when the grid is invalid, curves are missing, or
    /// schedule/day-count computation fails.
    pub fn new_with_time_steps(
        bond: Bond,
        market: &MarketContext,
        as_of: Date,
        time_steps: Vec<f64>,
    ) -> Result<Self> {
        let grid_day_count = market.get_discount(&bond.discount_curve_id)?.day_count();
        Self::new_with_time_steps_and_day_count(bond, market, as_of, time_steps, grid_day_count)
    }

    /// Get the total holder-view cashflow amount at this time step.
    ///
    /// This includes coupons, amortization, and final redemption — all positive
    /// receipts from the holder's perspective.
    #[inline]
    pub(super) fn cashflow_at(&self, step: usize) -> f64 {
        self.cashflow_vec.get(step).copied().unwrap_or(0.0)
    }

    /// Cashflow at a tree step with continuous OAS timing preserved at each
    /// original event date.
    #[inline]
    pub(super) fn cashflow_at_oas(&self, step: usize, oas_rate: f64) -> f64 {
        if oas_rate.abs() <= f64::EPSILON {
            return self.cashflow_at(step);
        }
        self.cashflow_components
            .get(step)
            .map(|components| {
                components
                    .iter()
                    .map(|(amount, event_minus_step)| amount * (-oas_rate * event_minus_step).exp())
                    .sum()
            })
            .unwrap_or(0.0)
    }

    /// Positive contractual notional repayment at a tree step, preserving the
    /// original event date under OAS discounting.
    #[inline]
    pub(super) fn redemption_at_oas(&self, step: usize, oas_rate: f64) -> f64 {
        if oas_rate.abs() <= f64::EPSILON {
            return self.redemption_vec.get(step).copied().unwrap_or(0.0);
        }
        self.redemption_components
            .get(step)
            .map(|components| {
                components
                    .iter()
                    .map(|(amount, event_minus_step)| amount * (-oas_rate * event_minus_step).exp())
                    .sum()
            })
            .unwrap_or(0.0)
    }

    /// Check if there's a call option at this time step.
    #[inline]
    pub(super) fn call_at(&self, step: usize) -> Option<f64> {
        self.call_vec.get(step).copied().flatten()
    }

    /// Check if there's a put option at this time step.
    #[inline]
    pub(super) fn put_at(&self, step: usize) -> Option<f64> {
        self.put_vec.get(step).copied().flatten()
    }

    /// Get outstanding principal at this time step.
    ///
    /// For bullet bonds, this returns the original notional.
    /// For amortizing bonds, this returns the remaining principal after amortization.
    #[inline]
    pub(super) fn outstanding_principal_at(&self, step: usize) -> f64 {
        self.outstanding_principal_vec
            .get(step)
            .copied()
            .unwrap_or(self.bond.notional.amount())
    }

    /// Resolve the recovery rate from the bond's explicit `credit_curve_id`
    /// opt-in, consistent with the hazard-rate and tree pricing kernels.
    ///
    /// Returns `None` when the bond has no `credit_curve_id` or the named
    /// hazard curve is absent from the market context.
    pub(super) fn resolve_recovery_rate(bond: &Bond, market: &MarketContext) -> Option<f64> {
        // Recovery comes only from the bond's explicit `credit_curve_id`
        // opt-in; implicit discovery by naming convention is not supported.
        let credit_id = bond.credit_curve_id.as_ref()?;
        market
            .get_hazard(credit_id.as_str())
            .ok()
            .map(|hc| hc.recovery_rate())
    }
}

const _: () = {
    pub(super) fn _assert_send<T: Send>() {}
    pub(super) fn _assert_sync<T: Sync>() {}
    pub(super) fn _assertions() {
        _assert_send::<BondValuator>();
        _assert_sync::<BondValuator>();
        _assert_send::<TreePricer>();
        _assert_sync::<TreePricer>();
    }
};
