//! Shared utilities for commodity option metrics.
//!
//! This module contains common types and functions used by multiple metric
//! calculators (gamma, vanna) to determine the forward price driver and
//! apply appropriate bumps.

use crate::instruments::commodity::commodity_option::CommodityOption;
use crate::metrics::MetricContext;
use finstack_quant_core::Result;

/// Determines the forward price driver for bumping in Greek calculations.
///
/// The forward price can come from multiple sources, and the bump strategy
/// must match the pricing retrieval order to ensure consistent sensitivities.
///
/// # Priority Order
///
/// 1. **`quoted_forward`**: If the instrument has a direct forward price override
/// 2. **`PriceCurve`**: If a PriceCurve exists for `forward_curve_id` in market data
/// 3. **`spot_id`**: Fallback to spot scalar (propagates via cost-of-carry)
///
/// # Usage
///
/// This enum is used by gamma and vanna calculators to determine what to bump
/// when computing forward-based sensitivities for Black-76 priced options.
#[derive(Debug, Clone)]
pub enum ForwardDriver {
    /// Bump the instrument's `quoted_forward` field directly.
    ///
    /// The contained value is the current quoted forward price.
    QuotedForward(f64),

    /// Bump the PriceCurve in market data (parallel percent bump).
    ///
    /// This applies when the forward price is sourced from a curve rather
    /// than a direct quote.
    PriceCurve,

    /// Bump the spot scalar (cost-of-carry fallback).
    ///
    /// The contained value is the spot price ID. The forward price will
    /// update through the cost-of-carry relationship: F = S × exp(r × T).
    SpotScalar(String),
}

impl ForwardDriver {
    /// Determine the forward price driver based on pricing retrieval priority.
    ///
    /// This matches the priority order used in `CommodityOption::forward_price()`:
    /// 1. `quoted_forward` (instrument override)
    /// 2. `PriceCurve` (market data curve)
    /// 3. `spot_id` (cost-of-carry fallback)
    ///
    /// # Errors
    ///
    /// Returns an error if no valid driver is found (no `quoted_forward`,
    /// no `PriceCurve`, and no `spot_id`).
    pub fn determine(option: &CommodityOption, context: &MetricContext) -> Result<Self> {
        // 1. If quoted_forward is set, bump that
        if let Some(fwd) = option.quoted_forward {
            return Ok(ForwardDriver::QuotedForward(fwd));
        }

        // 2. Try to find a PriceCurve
        if context
            .curves
            .get_price_curve(option.forward_curve_id.as_str())
            .is_ok()
        {
            return Ok(ForwardDriver::PriceCurve);
        }

        // 3. Fall back to spot scalar (cost-of-carry)
        if let Some(ref spot_id) = option.spot_id {
            return Ok(ForwardDriver::SpotScalar(spot_id.clone()));
        }

        // No valid driver found
        Err(finstack_quant_core::Error::Validation(
            "Cannot compute forward-based Greek: no quoted_forward, PriceCurve, or spot_id available"
                .to_string(),
        ))
    }
}
