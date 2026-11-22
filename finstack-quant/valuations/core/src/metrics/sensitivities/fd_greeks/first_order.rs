//! First-order generic finite-difference Greek calculators (`Delta`, `Vega`).

use super::bump::{
    bumped_vol_surface_ids, clone_with_crn_seed, current_spot, effective_spot_bump, ensure_finite,
    eval_raw_with_scratch_bumps, spot_id,
};
#[cfg(doc)]
use crate::instruments::Instrument;
use crate::metrics::finite_difference::{VOL_POINTS_PER_ABSOLUTE_VOL, active_minimum_vol};
use crate::metrics::sensitivities::config as sens_config;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Generic delta calculator using finite differences.
///
/// Calculates delta (price sensitivity to underlying spot) using the central
/// finite difference method. Works with any instrument that implements the
/// required trait: [`Instrument`]. The instrument must expose its instrument overrides
/// through [`Instrument::get_instrument_pricing_overrides_mut`].
///
/// # Mathematical Foundation
///
/// Delta is computed using the central finite difference formula:
/// ```text
/// Δ = (PV(S + h) - PV(S - h)) / (2h)
/// ```
/// where `S` is the spot price and `h` is the bump size.
#[derive(Default)]
pub struct GenericFdDelta;

impl MetricCalculator for GenericFdDelta {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let as_of = context.as_of;
        let defaults = sens_config::resolve(context)?;

        let dependencies = context.instrument.market_dependencies()?;
        let spot_id = spot_id(&dependencies, "delta")?.clone();
        let current_spot = current_spot(&context.curves, &spot_id, "delta")?;

        // Fixed bump size from `FinstackConfig` (user-facing, reproducible).
        let bump_pct = defaults.spot_bump_decimal;

        // Use the same effective bump in the market and denominator.
        let bump = effective_spot_bump(current_spot, bump_pct);

        // Common Random Numbers: same seed for all scenarios ensures variance reduction.
        let seeded_instrument = clone_with_crn_seed(context.instrument.as_ref())?;

        let (pv_up, pv_down) = context.with_market_scratch(|context, scratch| {
            let pv_up = eval_raw_with_scratch_bumps(
                context,
                scratch,
                seeded_instrument.as_ref(),
                as_of,
                Some((&spot_id, bump.relative)),
                None,
            )?;
            let pv_down = eval_raw_with_scratch_bumps(
                context,
                scratch,
                seeded_instrument.as_ref(),
                as_of,
                Some((&spot_id, -bump.relative)),
                None,
            )?;
            Ok((pv_up, pv_down))
        })?;

        // Central difference: delta = (PV_up - PV_down) / (2 * bump_size).
        // Route through the guarded helper so a degenerate bump width is
        // surfaced as an error rather than producing a silent NaN/∞.
        let delta = crate::metrics::finite_difference::central_diff_by_width(
            pv_up,
            pv_down,
            2.0 * bump.absolute,
        )?;

        ensure_finite(delta, "fd_delta")
    }
}

/// Generic vega calculator using finite differences on a volatility surface.
///
/// Works with any instrument that has an associated volatility surface.
#[derive(Default)]
pub struct GenericFdVega;

impl MetricCalculator for GenericFdVega {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let as_of = context.as_of;
        let defaults = sens_config::resolve(context)?;

        let dependencies = context.instrument.market_dependencies()?;
        let vol_surface_ids = bumped_vol_surface_ids(
            context.instrument.as_ref(),
            &dependencies,
            &context.curves,
            "vega",
        )?;
        // Fixed bump size from `FinstackConfig` (user-facing, reproducible).
        // Interpreted as an **absolute** implied vol bump in decimal units (e.g., 0.01 = +1 vol point).
        let bump_abs = defaults.vol_bump_decimal;

        let seeded_instrument = clone_with_crn_seed(context.instrument.as_ref())?;
        let instrument_id = context.instrument.id().to_string();

        // The additive down-bump `σ - h` is clamped at zero wherever `σ < h`
        // (VolSurface `Bumpable`). When that clamp triggers, the down-bumped
        // surface does NOT represent a uniform `-h` move, so a central
        // difference divided by the full `2h` is asymmetric and biased.
        //
        let min_vol = active_minimum_vol(
            context.instrument.as_ref(),
            &context.curves,
            &vol_surface_ids,
        )?;
        let clamp_active = min_vol.map(|m| m <= bump_abs).unwrap_or(false);

        let vega = if clamp_active {
            tracing::warn!(
                instrument_id = %instrument_id,
                vol_surface_ids = ?vol_surface_ids,
                min_vol = min_vol,
                bump_abs = bump_abs,
                "vega down-bump would reach or breach σ <= 0; using one-sided forward difference"
            );
            context.with_market_scratch(|context, scratch| {
                let pv_up = eval_raw_with_scratch_bumps(
                    context,
                    scratch,
                    seeded_instrument.as_ref(),
                    as_of,
                    None,
                    Some((&vol_surface_ids, bump_abs)),
                )?;
                let pv_base =
                    context.reprice_instrument_raw(seeded_instrument.as_ref(), scratch, as_of)?;
                // Forward difference: (PV(σ+h) - PV(σ)) / h.
                crate::metrics::finite_difference::central_diff_by_width(
                    pv_up,
                    pv_base,
                    bump_abs * VOL_POINTS_PER_ABSOLUTE_VOL,
                )
            })?
        } else {
            context.with_market_scratch(|context, scratch| {
                let pv_up = eval_raw_with_scratch_bumps(
                    context,
                    scratch,
                    seeded_instrument.as_ref(),
                    as_of,
                    None,
                    Some((&vol_surface_ids, bump_abs)),
                )?;
                let pv_down = eval_raw_with_scratch_bumps(
                    context,
                    scratch,
                    seeded_instrument.as_ref(),
                    as_of,
                    None,
                    Some((&vol_surface_ids, -bump_abs)),
                )?;
                // Central difference: (PV(σ+h) - PV(σ-h)) / 2h.
                crate::metrics::finite_difference::central_diff_by_width(
                    pv_up,
                    pv_down,
                    2.0 * bump_abs * VOL_POINTS_PER_ABSOLUTE_VOL,
                )
            })?
        };

        // MetricId::Vega is reported per 1 vol point (0.01 absolute vol), not per 1.00 vol.
        ensure_finite(vega, "fd_vega")
    }
}
