//! Vega calculators for volatility sensitivity.
//!
//! Provides parallel and key-rate vega calculators for instruments with volatility surfaces.

use crate::instruments::common_impl::traits::Instrument;
use crate::metrics::sensitivities::config as sens_config;
use crate::metrics::MetricCalculator;
use crate::metrics::{MetricContext, MetricId};
use finstack_core::market_data::bumps::{BumpMode, BumpSpec, BumpType, BumpUnits};
use finstack_core::market_data::scalars::MarketScalar;
use finstack_core::math::{neumaier_sum, NeumaierAccumulator};
use std::marker::PhantomData;

const VOL_POINTS_PER_ABSOLUTE_VOL: f64 = 100.0;

/// Standard expiry buckets in years for equity options.
pub(crate) fn standard_equity_expiry_buckets() -> Vec<f64> {
    vec![
        1.0 / 12.0, // 1m
        3.0 / 12.0, // 3m
        6.0 / 12.0, // 6m
        1.0,        // 1y
        2.0,        // 2y
        3.0,        // 3y
        5.0,        // 5y
    ]
}

/// Standard strike buckets (relative to spot) for equity options.
pub(crate) fn standard_strike_ratios() -> Vec<f64> {
    vec![0.5, 0.75, 0.9, 1.0, 1.1, 1.25, 1.5]
}

fn vol_bump(value: f64) -> BumpSpec {
    BumpSpec {
        mode: BumpMode::Additive,
        units: BumpUnits::Fraction,
        value,
        bump_type: BumpType::Parallel,
    }
}

/// Smallest implied vol on a surface's `(expiry, strike)` grid.
///
/// Used to detect whether an additive parallel down-bump of size `h` would
/// clamp `σ - h` at zero (which happens wherever `σ < h`), in which case a
/// central difference divided by `2h` is biased.
fn min_grid_vol(surface: &finstack_core::market_data::surfaces::VolSurface) -> Option<f64> {
    let mut min_vol: Option<f64> = None;
    for &expiry in surface.expiries() {
        for &strike in surface.strikes() {
            if let Ok(vol) = surface.value_checked(expiry, strike) {
                min_vol = Some(min_vol.map_or(vol, |m: f64| m.min(vol)));
            }
        }
    }
    min_vol
}

fn scaled_bucketed_vega_matrix(
    raw_matrix: Vec<Vec<f64>>,
    raw_total: f64,
    target_total: f64,
) -> (Vec<Vec<f64>>, f64) {
    if target_total.abs() <= f64::EPSILON {
        let zero_matrix = raw_matrix
            .into_iter()
            .map(|row| row.into_iter().map(|_| 0.0).collect())
            .collect();
        return (zero_matrix, 0.0);
    }

    let scale = if raw_total.abs() > f64::EPSILON {
        target_total / raw_total
    } else {
        1.0
    };
    let matrix = raw_matrix
        .into_iter()
        .map(|row| row.into_iter().map(|v| v * scale).collect())
        .collect();
    (matrix, scale)
}

/// Key-rate vega calculator: bumps individual (expiry, strike) points.
///
/// Calculates volatility sensitivity at individual points on the volatility surface
/// by bumping each (expiry, strike) combination and measuring the present value change.
/// This provides a detailed view of how the instrument's value depends on different
/// parts of the volatility surface.
///
/// # Type Parameters
///
/// * `I` - Instrument type that implements [`Instrument`] and has a volatility surface
///
/// # Examples
///
/// ```ignore
/// use finstack_valuations::instruments::EquityOption;
/// use finstack_valuations::metrics::KeyRateVega;
///
/// // Standard equity buckets
/// let calculator = KeyRateVega::<EquityOption>::standard();
///
/// // Or custom buckets
/// let expiries = vec![0.25, 0.5, 1.0, 2.0];
/// let strikes = vec![0.9, 1.0, 1.1];
/// let calculator = KeyRateVega::<EquityOption>::new(expiries, strikes);
/// ```
pub(crate) struct KeyRateVega<I> {
    expiries: Vec<f64>,
    strikes: Vec<f64>,
    _phantom: PhantomData<I>,
}

impl<I> KeyRateVega<I> {
    /// Create a key-rate vega calculator with custom buckets.
    ///
    /// # Arguments
    ///
    /// * `expiries` - Expiry times in years for the vega grid
    /// * `strikes` - Strike ratios (relative to spot) for the vega grid
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // KeyRateVega is internal - use MetricId::KeyRateVega via price_with_metrics
    /// use finstack_valuations::metrics::sensitivities::vega::KeyRateVega;
    /// use finstack_valuations::instruments::EquityOption;
    ///
    /// let expiries = vec![0.25, 0.5, 1.0, 2.0];
    /// let strikes = vec![0.9, 1.0, 1.1];
    /// let calculator = KeyRateVega::<EquityOption>::new(expiries, strikes);
    /// ```
    pub(crate) fn new(expiries: Vec<f64>, strikes: Vec<f64>) -> Self {
        Self {
            expiries,
            strikes,
            _phantom: PhantomData,
        }
    }

    /// Create a key-rate vega calculator with standard equity buckets.
    ///
    /// Uses standard expiry buckets (1m, 3m, 6m, 1y, 2y, 3y, 5y) and
    /// standard strike ratios (0.5, 0.75, 0.9, 1.0, 1.1, 1.25, 1.5).
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // KeyRateVega is internal - use MetricId::KeyRateVega via price_with_metrics
    /// use finstack_valuations::metrics::sensitivities::vega::KeyRateVega;
    /// use finstack_valuations::instruments::EquityOption;
    ///
    /// let calculator = KeyRateVega::<EquityOption>::standard();
    /// ```
    pub(crate) fn standard() -> Self {
        Self::new(standard_equity_expiry_buckets(), standard_strike_ratios())
    }
}

impl<I> Default for KeyRateVega<I> {
    fn default() -> Self {
        Self::standard()
    }
}

impl<I> MetricCalculator for KeyRateVega<I>
where
    I: Instrument + 'static,
{
    fn calculate(&self, context: &mut MetricContext) -> finstack_core::Result<f64> {
        let instrument: &I = context.instrument_as()?;
        let defaults =
            sens_config::from_context_or_default(context.config(), context.get_metric_overrides())?;

        let eq_deps = instrument.market_dependencies()?.equity_dependencies();
        let vol_surface_id = eq_deps
            .vol_surface_id
            .ok_or_else(|| finstack_core::Error::from(finstack_core::InputError::Invalid))?;

        let curves = std::sync::Arc::clone(&context.curves);
        let base_ctx = curves.as_ref();
        let vol_surface = base_ctx.get_surface(vol_surface_id.as_str())?;

        let as_of = context.as_of;

        let bump_pct = defaults.vol_bump_pct;

        // Use already-computed Vega when available to keep totals consistent
        let target_total = if let Some(existing) = context.computed.get(&MetricId::Vega) {
            *existing
        } else {
            context.with_market_scratch(|ctx, scratch| {
                // Central difference O(h²) — consistent with bucketed approach.
                let token_up = scratch
                    .apply_surface_bump_in_place(vol_surface_id.as_str(), vol_bump(bump_pct))?;
                let pv_up = ctx.reprice_money(scratch, as_of)?;
                scratch.revert_scratch_bump(token_up)?;

                // The additive down-bump `σ - h` clamps at zero wherever `σ < h`,
                // making a central difference divided by the full `2h` biased.
                // Detect the clamp via the surface's minimum vol and fall back to
                // a one-sided forward difference near zero vol.
                let min_vol = min_grid_vol(&vol_surface);
                if min_vol.map(|m| m < bump_pct).unwrap_or(false) {
                    tracing::warn!(
                        surface_id = %vol_surface_id,
                        min_vol = min_vol,
                        bump = bump_pct,
                        "key-rate vega parallel down-bump would clamp σ at 0; \
                         using one-sided forward difference"
                    );
                    let pv_base = ctx.reprice_money(base_ctx, as_of)?;
                    Ok((pv_up.amount() - pv_base.amount())
                        / (bump_pct * VOL_POINTS_PER_ABSOLUTE_VOL))
                } else {
                    let token_down = scratch.apply_surface_bump_in_place(
                        vol_surface_id.as_str(),
                        vol_bump(-bump_pct),
                    )?;
                    let pv_down = ctx.reprice_money(scratch, as_of)?;
                    scratch.revert_scratch_bump(token_down)?;
                    Ok((pv_up.amount() - pv_down.amount())
                        / (2.0 * bump_pct * VOL_POINTS_PER_ABSOLUTE_VOL))
                }
            })?
        };

        let use_ratio_strikes = self.strikes.iter().all(|k| *k <= 10.0);
        let strike_grid: Vec<f64> = if use_ratio_strikes {
            let spot_id = eq_deps
                .spot_id
                .as_ref()
                .ok_or_else(|| finstack_core::Error::from(finstack_core::InputError::Invalid))?;
            let spot = base_ctx
                .get_price(spot_id)
                .ok()
                .map(|scalar| match scalar {
                    MarketScalar::Price(m) => m.amount(),
                    MarketScalar::Unitless(v) => *v,
                })
                .ok_or_else(|| finstack_core::Error::from(finstack_core::InputError::Invalid))?;

            self.strikes.iter().map(|k| k * spot).collect()
        } else {
            self.strikes.clone()
        };

        let (raw_matrix, raw_total) = context.with_market_scratch(|ctx, scratch| {
            let mut raw_matrix = Vec::new();
            let mut raw_total = NeumaierAccumulator::new();

            for &expiry in &self.expiries {
                let mut row = Vec::new();
                for &strike in &strike_grid {
                    // Central differences: O(h²) accuracy, consistent with other Greeks
                    let bumped_up = vol_surface.bump_point(expiry, strike, bump_pct)?;
                    let bumped_down = vol_surface.bump_point(expiry, strike, -bump_pct)?;
                    scratch.insert_surface_mut(bumped_up);
                    let pv_up = ctx.reprice_money(scratch, as_of)?;
                    scratch.insert_surface_mut(std::sync::Arc::clone(&vol_surface));

                    scratch.insert_surface_mut(bumped_down);
                    let pv_down = ctx.reprice_money(scratch, as_of)?;
                    scratch.insert_surface_mut(std::sync::Arc::clone(&vol_surface));

                    let vega = (pv_up.amount() - pv_down.amount())
                        / (2.0 * bump_pct * VOL_POINTS_PER_ABSOLUTE_VOL);
                    row.push(vega);
                    raw_total.add(vega);
                }
                raw_matrix.push(row);
            }

            Ok((raw_matrix, raw_total.total()))
        })?;

        // Normalize bucketed vegas so they partition the parallel vega.
        let (matrix, scale) = scaled_bucketed_vega_matrix(raw_matrix, raw_total, target_total);

        // Warn if scale factor deviates significantly from 1.0, which may indicate
        // that the bucketed vega grid doesn't capture the true sensitivity distribution
        // (e.g., option expiry/strike far from grid points, or exotic payoff structure)
        const SCALE_DEVIATION_THRESHOLD: f64 = 0.10; // 10% deviation
        if (scale - 1.0).abs() > SCALE_DEVIATION_THRESHOLD {
            tracing::warn!(
                raw_total = raw_total,
                target_total = target_total,
                scale = scale,
                scale_deviation_pct = (scale - 1.0).abs() * 100.0,
                "Bucketed vega scale factor deviates significantly from 1.0. \
                 This may indicate the vega grid doesn't fully capture the instrument's \
                 volatility sensitivity. Consider using a finer grid or reviewing the \
                 instrument's strike/expiry relative to grid points."
            );
        }

        let sum_scaled: f64 = neumaier_sum(matrix.iter().flatten().copied());
        tracing::debug!(
            raw_total = raw_total,
            target_total = target_total,
            scale = scale,
            sum_scaled = sum_scaled,
            "bucketed vega debug"
        );

        // Store as 2D matrix
        let row_labels: Vec<String> = self
            .expiries
            .iter()
            .map(|&t| {
                if t < 1.0 {
                    format!("{:.0}m", (t * 12.0).round())
                } else {
                    format!("{:.0}y", t)
                }
            })
            .collect();
        let col_labels: Vec<String> = self.strikes.iter().map(|&k| format!("{:.2}", k)).collect();

        let _ = context.store_matrix2d(MetricId::BucketedVega, row_labels, col_labels, matrix);

        Ok(target_total)
    }

    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::Vega]
    }
}

#[cfg(test)]
mod tests {
    use super::scaled_bucketed_vega_matrix;

    #[test]
    fn zero_target_vega_forces_bucket_matrix_to_zero() {
        let (matrix, scale) = scaled_bucketed_vega_matrix(vec![vec![10.0, -10.0]], 0.0, 0.0);

        assert_eq!(scale, 0.0);
        assert_eq!(matrix, vec![vec![0.0, 0.0]]);
    }
}
