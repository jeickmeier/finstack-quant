//! Second-order generic finite-difference Greek calculators (`Gamma`, `Volga`, `Vanna`).

use super::bump::{
    bumped_vol_surface_ids, clone_with_crn_seed, current_spot, effective_spot_bump, ensure_finite,
    eval_raw_with_scratch_bumps, is_expired, spot_id, symmetric_volatility_step,
};
use crate::metrics::finite_difference::VOL_POINTS_PER_ABSOLUTE_VOL;
use crate::metrics::sensitivities::config as sens_config;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Generic gamma calculator using finite differences.
///
/// Computes gamma as the second derivative with respect to spot price:
/// Gamma = [Delta(spot + bump) - Delta(spot - bump)] / (2 * bump_size)
#[derive(Default)]
pub struct GenericFdGamma;

impl MetricCalculator for GenericFdGamma {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let as_of = context.as_of;
        let defaults = sens_config::resolve(context)?;

        let dependencies = context.instrument.market_dependencies()?;
        let spot_id = spot_id(&dependencies, "gamma")?.clone();
        let current_spot = current_spot(&context.curves, &spot_id, "gamma")?;

        // Fixed bump size from `FinstackConfig` (user-facing, reproducible).
        let bump_pct = defaults.spot_bump_decimal;

        // Use the same effective bump in the market and denominator.
        let bump = effective_spot_bump(current_spot, bump_pct);

        // Use the 3-point central difference formula directly:
        //
        //   Γ ≈ (PV(S+h) - 2 PV(S) + PV(S-h)) / h²
        //
        // This avoids "bump-of-bump" scaling artifacts when bumps are applied multiplicatively
        // (percentage bumps) and yields exact results for quadratic payoffs.

        // Common Random Numbers: same seed for all scenarios ensures variance reduction.
        let seeded_instrument = clone_with_crn_seed(context.instrument.as_ref())?;
        let (base_pv, pv_up, pv_down) = context.with_market_scratch(|context, scratch| {
            let base_pv =
                context.reprice_instrument_raw(seeded_instrument.as_ref(), scratch, as_of)?;
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
            Ok((base_pv, pv_up, pv_down))
        })?;

        let gamma = (pv_up - 2.0 * base_pv + pv_down) / (bump.absolute * bump.absolute);

        ensure_finite(gamma, "fd_gamma")
    }
}

/// Generic volga (∂²V/∂σ²) via central differences on a volatility surface.
#[derive(Default)]
pub struct GenericFdVolga;

impl MetricCalculator for GenericFdVolga {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let as_of = context.as_of;
        let defaults = sens_config::resolve(context)?;

        // If expired, volga is zero — mirror vanna's guard. On an expired
        // option, vega ≈ 0, so volga = (vega(σ+h) - vega(σ-h))/h^2 amplifies
        // near-zero noise into NaN/garbage.
        if is_expired(context.instrument.as_ref(), as_of)? {
            return Ok(0.0);
        }

        let dependencies = context.instrument.market_dependencies()?;
        let vol_surface_ids = bumped_vol_surface_ids(
            context.instrument.as_ref(),
            &dependencies,
            &context.curves,
            "volga",
        )?;

        // Common Random Numbers: same seed for all scenarios ensures variance reduction.
        let seeded_instrument = clone_with_crn_seed(context.instrument.as_ref())?;

        // Absolute implied vol bump (vol points).
        let bump_abs = symmetric_volatility_step(
            context.instrument.as_ref(),
            &context.curves,
            &vol_surface_ids,
            defaults.vol_bump_decimal,
        )?;

        let (base_pv, pv_up, pv_down) = context.with_market_scratch(|context, scratch| {
            let base_pv =
                context.reprice_instrument_raw(seeded_instrument.as_ref(), scratch, as_of)?;
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
            Ok((base_pv, pv_up, pv_down))
        })?;

        // Report volga per **vol point squared** (consistent with vega, which
        // is per vol point — see the vega path above and `MetricId::Volga`).
        // The central second difference is taken in absolute-vol units, so the
        // normalizing width is the bump expressed in vol points
        // (`bump_abs * VOL_POINTS_PER_ABSOLUTE_VOL`), squared. Dividing by the
        // raw `bump_abs²` instead would overstate volga by `100² = 10,000×`.
        let width = bump_abs * VOL_POINTS_PER_ABSOLUTE_VOL;
        let volga = (pv_up - 2.0 * base_pv + pv_down) / (width * width);

        ensure_finite(volga, "fd_volga")
    }
}

/// Generic vanna (∂²V/∂S∂σ) via central mixed differences on spot and vol.
#[derive(Default)]
pub struct GenericFdVanna;

impl MetricCalculator for GenericFdVanna {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let as_of = context.as_of;
        let defaults = sens_config::resolve(context)?;

        // If expired, vanna is zero (avoid bumping / repricing beyond expiry).
        if is_expired(context.instrument.as_ref(), as_of)? {
            return Ok(0.0);
        }

        let dependencies = context.instrument.market_dependencies()?;
        let spot_id = spot_id(&dependencies, "vanna")?.clone();
        let vol_surface_ids = bumped_vol_surface_ids(
            context.instrument.as_ref(),
            &dependencies,
            &context.curves,
            "vanna",
        )?;

        // Spot level for bump sizing
        let current_spot = current_spot(&context.curves, &spot_id, "vanna")?;

        // Bump sizes with minimum floor for numerical stability
        let (spot_bump_decimal, vol_bump_abs) =
            (defaults.spot_bump_decimal, defaults.vol_bump_decimal);

        let spot_bump = effective_spot_bump(current_spot, spot_bump_decimal);
        let h_abs = spot_bump.absolute; // absolute spot change
        let k_abs = symmetric_volatility_step(
            context.instrument.as_ref(),
            &context.curves,
            &vol_surface_ids,
            vol_bump_abs,
        )?;

        let seeded_instrument = clone_with_crn_seed(context.instrument.as_ref())?;

        let (v_pp, v_pm, v_mp, v_mm) = context.with_market_scratch(|context, scratch| {
            let v_pp = eval_raw_with_scratch_bumps(
                context,
                scratch,
                seeded_instrument.as_ref(),
                as_of,
                Some((&spot_id, spot_bump.relative)),
                Some((&vol_surface_ids, k_abs)),
            )?;
            let v_pm = eval_raw_with_scratch_bumps(
                context,
                scratch,
                seeded_instrument.as_ref(),
                as_of,
                Some((&spot_id, spot_bump.relative)),
                Some((&vol_surface_ids, -k_abs)),
            )?;
            let v_mp = eval_raw_with_scratch_bumps(
                context,
                scratch,
                seeded_instrument.as_ref(),
                as_of,
                Some((&spot_id, -spot_bump.relative)),
                Some((&vol_surface_ids, k_abs)),
            )?;
            let v_mm = eval_raw_with_scratch_bumps(
                context,
                scratch,
                seeded_instrument.as_ref(),
                as_of,
                Some((&spot_id, -spot_bump.relative)),
                Some((&vol_surface_ids, -k_abs)),
            )?;
            Ok((v_pp, v_pm, v_mp, v_mm))
        })?;

        // Report vanna per **vol point** on the σ axis (consistent with vega
        // and `MetricId::Vanna`). The mixed difference is taken in absolute-vol
        // units, so the vol-axis width is the bump expressed in vol points
        // (`k_abs * VOL_POINTS_PER_ABSOLUTE_VOL`); the spot axis is unchanged.
        let vanna =
            (v_pp - v_pm - v_mp + v_mm) / (4.0 * h_abs * k_abs * VOL_POINTS_PER_ABSOLUTE_VOL);

        ensure_finite(vanna, "fd_vanna")
    }
}
