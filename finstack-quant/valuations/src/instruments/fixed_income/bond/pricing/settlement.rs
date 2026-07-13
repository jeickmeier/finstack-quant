//! Settlement and quote-date utilities for bond pricing and metrics.
//!
//! This module provides helpers for computing settlement dates and
//! quote-date-anchored values used by the quote engine and yield/spread metrics.
//!
//! # Conventions
//!
//! - **PV (present value)** is always anchored at `as_of` (valuation date).
//! - **Quote-derived metrics** (YTM, Z-spread, DM, OAS, duration) are computed
//!   relative to the **quote date** (= settlement date when `settlement_days`
//!   is set, otherwise `as_of`).
//! - Accrued interest for market quotes is computed at the quote date.

use finstack_quant_core::dates::{adjust, BusinessDayConvention, Date, DateExt};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::Result;

use super::super::types::Bond;
use super::super::CashflowSpec;

/// Compute the settlement date from a trade/valuation date.
///
/// If the bond has `settlement_days` set, computes the date by adding that
/// many business days (using the bond's calendar if available). Otherwise,
/// returns `as_of` unchanged.
pub(crate) fn settlement_date(bond: &Bond, as_of: Date) -> Result<Date> {
    let Some(sd_u32) = bond.settlement_days() else {
        return Ok(as_of);
    };

    let sd: i32 = sd_u32 as i32;
    let (calendar_id, bdc) = match &bond.cashflow_spec {
        CashflowSpec::Fixed(spec) => (Some(spec.schedule.calendar_id.as_str()), spec.schedule.bdc),
        CashflowSpec::Floating(spec) => {
            (Some(spec.schedule.calendar_id.as_str()), spec.schedule.bdc)
        }
        CashflowSpec::StepUp(spec) => (Some(spec.schedule.calendar_id.as_str()), spec.schedule.bdc),
        CashflowSpec::Amortizing { base, .. } => match &**base {
            CashflowSpec::Fixed(spec) => {
                (Some(spec.schedule.calendar_id.as_str()), spec.schedule.bdc)
            }
            CashflowSpec::Floating(spec) => {
                (Some(spec.schedule.calendar_id.as_str()), spec.schedule.bdc)
            }
            CashflowSpec::StepUp(spec) => {
                (Some(spec.schedule.calendar_id.as_str()), spec.schedule.bdc)
            }
            CashflowSpec::Amortizing { .. } => (None, BusinessDayConvention::Following),
        },
    };

    if let Some(id) = calendar_id {
        if let Some(cal) = finstack_quant_core::dates::calendar::calendar_by_id(id) {
            let d = as_of.add_business_days(sd, cal)?;
            return adjust(d, bdc, cal);
        }
    }

    Ok(as_of.add_weekdays(sd))
}

/// Quote-date context for yield/spread metric calculations.
///
/// Contains pre-computed values needed by metrics that interpret market quotes:
/// - `quote_date`: The date at which the quote is interpreted (settlement date)
/// - `accrued_at_quote_date`: Accrued interest in currency at the quote date
///
/// # Usage
///
/// Use this struct when computing YTM, Z-spread, DM, OAS, and other quote-derived
/// metrics to ensure consistent handling of settlement conventions.
#[derive(Debug, Clone, Copy)]
pub(crate) struct QuoteDateContext {
    /// The date at which the market quote is interpreted.
    /// Equals `settlement_date(bond, as_of)` when `settlement_days` is set,
    /// otherwise equals `as_of`.
    pub(crate) quote_date: Date,
    /// Accrued interest (in currency) computed at `quote_date`.
    pub(crate) accrued_at_quote_date: f64,
}

impl QuoteDateContext {
    /// Create a quote-date context for a bond at a given valuation date.
    ///
    /// # Arguments
    ///
    /// * `bond` - The bond to compute context for
    /// * `curves` - Market context containing curves for floating coupon fixings
    /// * `as_of` - Valuation date (trade date)
    ///
    /// # Returns
    ///
    /// A `QuoteDateContext` with the quote date and accrued interest.
    pub(crate) fn new(bond: &Bond, curves: &MarketContext, as_of: Date) -> Result<Self> {
        let quote_date = settlement_date(bond, as_of)?;

        // Compute accrued interest at the quote date
        let schedule = bond.full_cashflow_schedule(curves)?;
        let accrued_at_quote_date = crate::cashflow::accrual::accrued_interest_amount(
            &schedule,
            quote_date,
            &bond.accrual_config(),
        )?;

        Ok(Self {
            quote_date,
            accrued_at_quote_date,
        })
    }

    /// Compute dirty price in currency from a clean price quote (% of par).
    ///
    /// # Arguments
    ///
    /// * `clean_price_pct` - Clean price as percentage of par (e.g., 99.5)
    /// * `notional` - Bond notional in currency
    ///
    /// # Returns
    ///
    /// Dirty price in currency = (clean_pct × notional / 100) + accrued
    #[inline]
    pub(crate) fn dirty_from_clean_pct(&self, clean_price_pct: f64, notional: f64) -> f64 {
        clean_price_pct * notional / 100.0 + self.accrued_at_quote_date
    }
}
