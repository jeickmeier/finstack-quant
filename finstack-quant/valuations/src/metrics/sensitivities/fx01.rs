//! Generic FX01 calculator.
//!
//! Computes the sensitivity of any instrument's PV to a **1% relative** move
//! in its FX exposure via central finite difference.
//!
//! # Convention
//!
//! FX01 = `(PV(S + 1%) − PV(S − 1%)) / 2`, where `+ 1%` means the FX matrix
//! is bumped by `+1%` of spot via `MarketBump::FxPct`. The result is
//! "$ per 1% relative spot move", matching:
//!
//! - `core::market_data::diff::measure_fx_shift`'s output (percentage points)
//! - `finstack_quant_attribution::metrics_based::attribute_pnl_metrics_based`'s
//!   consumer (`fx01 × fx_shift_pct` gives the right P&L magnitude).
//!
//! When an instrument has no FX exposure (`fx_pairs` empty), FX01 is `0.0`.
//!
//! # Multi-pair instruments
//!
//! When an instrument depends on more than one FX pair, every pair is bumped
//! together by `+1%` (then `−1%`) in the same reprice. The result is the
//! joint sensitivity to a simultaneous 1% move across all pairs.

use std::sync::Arc;

use finstack_quant_core::market_data::bumps::MarketBump;
use finstack_quant_core::Result;

use crate::metrics::{MetricCalculator, MetricContext};

/// Generic, instrument-agnostic FX01 calculator.
///
pub struct GenericFx01Calculator;

impl GenericFx01Calculator {
    /// Bump size (percentage points of spot). 1.0 means +1% of spot.
    const BUMP_PCT: f64 = 1.0;
}

impl MetricCalculator for GenericFx01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let deps = context.instrument.market_dependencies()?;
        let pairs = deps.fx_pairs;
        if pairs.is_empty() {
            return Ok(0.0);
        }

        let as_of = context.as_of;
        let market = std::sync::Arc::clone(&context.curves);

        let make_bumps = |direction: f64| -> Vec<MarketBump> {
            pairs
                .iter()
                .map(|pair| MarketBump::FxPct {
                    base: pair.base,
                    quote: pair.quote,
                    pct: direction * Self::BUMP_PCT,
                    as_of,
                })
                .collect()
        };

        let market_up = market.bump(make_bumps(1.0))?;
        let market_dn = market.bump(make_bumps(-1.0))?;

        let pv_up = context.reprice_raw(&market_up, as_of)?;
        let pv_dn = context.reprice_raw(&market_dn, as_of)?;

        // Central finite difference. Bump size is 1% on each side, so the
        // denominator is 2 (PV per 1% relative spot move).
        Ok((pv_up - pv_dn) / 2.0)
    }
}

/// Wrap the generic FX01 calculator in an `Arc` for registry insertion.
pub fn arc_generic_fx01() -> Arc<dyn MetricCalculator> {
    Arc::new(GenericFx01Calculator)
}
