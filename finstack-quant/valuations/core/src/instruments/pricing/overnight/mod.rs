//! Projection engine for compounded overnight-RFR coupons of swap-style instruments.
//!
//! IRS, basis, cross-currency, futures, cap/floor, TRS, revolver and risk
//! pathways use this module so lookback, observation-shift, cutoff, fixing,
//! and sensitivity semantics stay identical among them. The cashflow builder
//! (bonds, term loans, structured coupons, bond LSMC) compounds through its
//! own engine, `finstack_quant_cashflows::builder` (`overnight` and
//! `floating_replay`), which carries a non-business accrual start as a
//! leading slice where the IRS path snaps endpoints to business days. Mirror
//! convention changes in both.

mod projection;
#[cfg(test)]
mod tests;

#[cfg(test)]
use projection::last_overnight_observation;
pub use projection::{
    OvernightCouponProjection, OvernightCouponProjectionInput, OvernightObservationExposure,
    OvernightProjectionCurve, adjust_overnight_accrual_boundaries, interval_context,
    project_overnight_coupon, resolve_overnight_fixing_calendar,
};
