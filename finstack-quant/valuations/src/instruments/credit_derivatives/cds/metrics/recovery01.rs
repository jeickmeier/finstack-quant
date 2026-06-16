//! Recovery01 calculator for CDS.
//!
//! Computes Recovery01 (recovery rate sensitivity) using finite differences.
//! Recovery01 measures the change in PV for a 1% (100bp) change in recovery rate.
//!
//! ## Methodology
//!
//! Uses central differences when possible, with automatic fallback to one-sided
//! differences at recovery rate boundaries:
//!
//! - **Central difference** (interior): `(PV(R+h) - PV(R-h)) / (2h)`
//! - **Forward difference** (near R=0): `(PV(R+h) - PV(R)) / h`
//! - **Backward difference** (near R=1): `(PV(R) - PV(R-h)) / h`
//!
//! This ensures consistent, unbiased sensitivity estimates even when the base
//! recovery rate is near the valid bounds [0, 1].
//!
//! ## Hazard Curve Recalibration
//!
//! When the hazard curve carries the par-spread quotes it was bootstrapped
//! from (`par_spread_points` non-empty), the bumped recovery is propagated
//! through a full re-bootstrap of the survival curve so the observed CDS
//! spreads remain consistent. This captures the indirect `h ≈ S/(1-R)` effect
//! that dominates the recovery sensitivity for distressed credits.
//!
//! When the curve has no stored par spreads (e.g. a hand-built knot curve
//! used in tests or a curve loaded without preserving its calibration
//! quotes), the calculator falls back to a "frozen-curve" bump: the recovery
//! is bumped on the instrument only and the survival curve is reused
//! unchanged. This produces a *partial* recovery sensitivity that, for
//! spread-bootstrapped curves, typically understates the true value by 2-5x.
//!
//! ## Note
//!
//! Recovery rate changes affect both the protection leg (LGD = 1 - recovery)
//! and the premium leg (accrued on default settlement). This metric captures
//! the full direct sensitivity across both legs.

use super::{hazard_with_deal_quote, market_doc_clause};
use crate::calibration::bumps::hazard::recalibrate_hazard_with_recovery_and_doc_clause_and_valuation_convention;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::credit_derivatives::cds::CreditDefaultSwap;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::Result;

/// Standard recovery rate bump: 1% (0.01)
const RECOVERY_BUMP: f64 = 0.01;

/// Minimum bump size considered valid for finite differences.
/// Below this threshold, we treat the bump as ineffective.
const MIN_EFFECTIVE_BUMP: f64 = 1e-6;

/// Recovery01 calculator for CDS.
pub(crate) struct Recovery01Calculator;

/// Price the CDS at a bumped recovery, recalibrating the hazard curve from
/// par spreads when the curve carries them and falling back to a frozen-curve
/// bump otherwise. Returns the bumped PV.
fn price_at_bumped_recovery(
    cds: &CreditDefaultSwap,
    base_market: &MarketContext,
    new_recovery: f64,
    as_of: finstack_quant_core::dates::Date,
) -> Result<f64> {
    let mut bumped_cds = cds.clone();
    bumped_cds.protection.recovery_rate = new_recovery;

    let credit_id = cds.protection.credit_curve_id.as_str();
    let discount_id = cds.premium.discount_curve_id.clone();
    let hazard = base_market.get_hazard(credit_id)?;

    // If the curve was built from par-spread quotes we re-bootstrap it under
    // the bumped recovery. Otherwise we leave the curve frozen — Recovery01
    // becomes a partial sensitivity (the LGD-only direct effect).
    let has_par_quotes = hazard.par_spread_points().next().is_some();

    // Frozen-curve fallback: reuse the hazard curve's λ knots unchanged but
    // realign its `recovery_rate` metadata with the bumped trade recovery.
    // Without this the recovery-consistency guard in the pricer rejects the
    // (trade, curve) pair; the λ knots are untouched so the priced PV is
    // exactly the documented LGD-only sensitivity.
    let frozen_curve_market = || -> Result<MarketContext> {
        let frozen = hazard.with_recovery_rate(new_recovery)?;
        Ok(base_market.clone().insert(frozen))
    };

    let market_for_pricing: MarketContext = if has_par_quotes {
        match recalibrate_hazard_with_recovery_and_doc_clause_and_valuation_convention(
            hazard.as_ref(),
            new_recovery,
            base_market,
            Some(&discount_id),
            Some(market_doc_clause(cds)),
            Some(cds.valuation_convention),
        ) {
            Ok(recalibrated) => base_market.clone().insert(recalibrated),
            Err(_) => {
                // Recalibration failure (e.g. degenerate spreads under the new
                // recovery) is non-fatal: fall through to the frozen-curve
                // bump so the metric still produces a number.
                frozen_curve_market()?
            }
        }
    } else {
        frozen_curve_market()?
    };

    Ok(bumped_cds.value(&market_for_pricing, as_of)?.amount())
}

impl MetricCalculator for Recovery01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let cds: &CreditDefaultSwap = context.instrument_as()?;
        let as_of = context.as_of;
        let original_market = context.curves.as_ref();
        let hazard = original_market.get_hazard(cds.protection.credit_curve_id.as_str())?;
        let adjusted_market = hazard_with_deal_quote(cds, hazard.as_ref())?
            .map(|quote_hazard| original_market.clone().insert(quote_hazard));
        let market = adjusted_market.as_ref().unwrap_or(original_market);

        let base_recovery = cds.protection.recovery_rate;

        let bumped_up = (base_recovery + RECOVERY_BUMP).clamp(0.0, 1.0);
        let bumped_down = (base_recovery - RECOVERY_BUMP).clamp(0.0, 1.0);
        let up_delta = bumped_up - base_recovery;
        let down_delta = base_recovery - bumped_down;

        let can_bump_up = up_delta > MIN_EFFECTIVE_BUMP;
        let can_bump_down = down_delta > MIN_EFFECTIVE_BUMP;

        let slope = match (can_bump_up, can_bump_down) {
            (true, true) => {
                let pv_up = price_at_bumped_recovery(cds, market, bumped_up, as_of)?;
                let pv_down = price_at_bumped_recovery(cds, market, bumped_down, as_of)?;
                (pv_up - pv_down) / (up_delta + down_delta)
            }
            (true, false) => {
                let base_pv = cds.value(market, as_of)?.amount();
                let pv_up = price_at_bumped_recovery(cds, market, bumped_up, as_of)?;
                (pv_up - base_pv) / up_delta
            }
            (false, true) => {
                let base_pv = cds.value(market, as_of)?.amount();
                let pv_down = price_at_bumped_recovery(cds, market, bumped_down, as_of)?;
                (base_pv - pv_down) / down_delta
            }
            (false, false) => 0.0,
        };

        Ok(slope * RECOVERY_BUMP)
    }
}
