//! Vega calculators for volatility sensitivity.
//!
//! Provides parallel and key-rate vega calculators for instruments with volatility surfaces.

use crate::instruments::common_impl::traits::Instrument;
use crate::metrics::core::finite_difference::{
    apply_parallel_surface_bumps_in_place, revert_scratch_bumps,
};
use crate::metrics::sensitivities::config as sens_config;
use crate::metrics::MetricCalculator;
use crate::metrics::{MetricContext, MetricId};
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::math::{neumaier_sum, NeumaierAccumulator};
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

/// Smallest implied vol on a surface's `(expiry, strike)` grid.
///
/// Used to detect whether an additive parallel down-bump of size `h` would
/// clamp `σ - h` at zero (which happens wherever `σ < h`), in which case a
/// central difference divided by `2h` is biased.
fn min_grid_vol(surface: &finstack_quant_core::market_data::surfaces::VolSurface) -> Option<f64> {
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

fn expiry_label(t: f64) -> String {
    if t < 1.0 {
        format!("{:.0}m", (t * 12.0).round())
    } else {
        format!("{:.0}y", t)
    }
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
/// use finstack_quant_valuations::instruments::EquityOption;
/// use finstack_quant_valuations::metrics::KeyRateVega;
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
    /// use finstack_quant_valuations::metrics::sensitivities::vega::KeyRateVega;
    /// use finstack_quant_valuations::instruments::EquityOption;
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
    /// use finstack_quant_valuations::metrics::sensitivities::vega::KeyRateVega;
    /// use finstack_quant_valuations::instruments::EquityOption;
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
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let instrument: &I = context.instrument_as()?;
        let defaults =
            sens_config::from_context_or_default(context.config(), context.get_metric_overrides())?;

        let dependencies = instrument.market_dependencies()?;
        let surface_ids: Vec<_> = dependencies
            .unique_vol_surface_ids()
            .into_iter()
            .filter(|surface_id| context.curves.get_surface(surface_id.as_str()).is_ok())
            .collect();
        if surface_ids.is_empty() {
            return Err(finstack_quant_core::InputError::Invalid.into());
        }

        let curves = std::sync::Arc::clone(&context.curves);
        let base_ctx = curves.as_ref();
        let vol_surfaces = surface_ids
            .iter()
            .map(|surface_id| {
                Ok((
                    surface_id.clone(),
                    base_ctx.get_surface(surface_id.as_str())?,
                ))
            })
            .collect::<finstack_quant_core::Result<Vec<_>>>()?;

        let as_of = context.as_of;

        let bump_pct = defaults.vol_bump_pct;

        // Use already-computed Vega when available to keep totals consistent
        let target_total = if let Some(existing) = context.computed.get(&MetricId::Vega) {
            *existing
        } else {
            context.with_market_scratch(|ctx, scratch| {
                // Central difference O(h²) — consistent with bucketed approach.
                let tokens_up =
                    apply_parallel_surface_bumps_in_place(scratch, &surface_ids, bump_pct)?;
                let pv_up = ctx.reprice_money(scratch, as_of);
                revert_scratch_bumps(scratch, tokens_up)?;
                let pv_up = pv_up?;

                // The additive down-bump `σ - h` clamps at zero wherever `σ < h`,
                // making a central difference divided by the full `2h` biased.
                // Detect the clamp via the surface's minimum vol and fall back to
                // a one-sided forward difference near zero vol.
                let min_vol = vol_surfaces
                    .iter()
                    .filter_map(|(_, surface)| min_grid_vol(surface))
                    .reduce(f64::min);
                if min_vol.map(|m| m < bump_pct).unwrap_or(false) {
                    tracing::warn!(
                        surface_ids = ?surface_ids,
                        min_vol = min_vol,
                        bump = bump_pct,
                        "key-rate vega parallel down-bump would clamp σ at 0; \
                         using one-sided forward difference"
                    );
                    let pv_base = ctx.reprice_money(base_ctx, as_of)?;
                    Ok((pv_up.amount() - pv_base.amount())
                        / (bump_pct * VOL_POINTS_PER_ABSOLUTE_VOL))
                } else {
                    let tokens_down =
                        apply_parallel_surface_bumps_in_place(scratch, &surface_ids, -bump_pct)?;
                    let pv_down = ctx.reprice_money(scratch, as_of);
                    revert_scratch_bumps(scratch, tokens_down)?;
                    let pv_down = pv_down?;
                    Ok((pv_up.amount() - pv_down.amount())
                        / (2.0 * bump_pct * VOL_POINTS_PER_ABSOLUTE_VOL))
                }
            })?
        };

        let use_ratio_strikes = self.strikes.iter().all(|k| *k <= 10.0);
        let surface_strike_grids = vol_surfaces
            .iter()
            .map(|(surface_id, _)| {
                if !use_ratio_strikes {
                    return Ok(self.strikes.clone());
                }
                let spot_id = dependencies
                    .volatility_dependencies
                    .iter()
                    .find(|dependency| {
                        dependency.surface_id == *surface_id && dependency.underlying_id.is_some()
                    })
                    .and_then(|dependency| dependency.underlying_id.as_ref())
                    .map(|id| id.as_str())
                    .or_else(|| dependencies.spot_ids.first().map(String::as_str))
                    .ok_or_else(|| {
                        finstack_quant_core::Error::from(finstack_quant_core::InputError::Invalid)
                    })?;
                let spot = match base_ctx.get_price(spot_id)? {
                    MarketScalar::Price(m) => m.amount(),
                    MarketScalar::Unitless(v) => *v,
                };
                Ok(self.strikes.iter().map(|k| k * spot).collect())
            })
            .collect::<finstack_quant_core::Result<Vec<Vec<f64>>>>()?;

        let multiple_surfaces = vol_surfaces.len() > 1;
        let (raw_matrix, raw_total, row_labels) = context.with_market_scratch(|ctx, scratch| {
            let mut raw_matrix = Vec::new();
            let mut raw_total = NeumaierAccumulator::new();
            let mut row_labels = Vec::new();

            for ((surface_id, vol_surface), strike_grid) in
                vol_surfaces.iter().zip(&surface_strike_grids)
            {
                for &expiry in &self.expiries {
                    let mut row = Vec::new();
                    for &strike in strike_grid {
                        // Central differences: O(h²) accuracy, consistent with other Greeks.
                        let bumped_up = vol_surface.bump_point(expiry, strike, bump_pct)?;
                        let bumped_down = vol_surface.bump_point(expiry, strike, -bump_pct)?;
                        scratch.insert_surface_mut(bumped_up);
                        let pv_up = ctx.reprice_money(scratch, as_of);
                        scratch.insert_surface_mut(std::sync::Arc::clone(vol_surface));
                        let pv_up = pv_up?;

                        scratch.insert_surface_mut(bumped_down);
                        let pv_down = ctx.reprice_money(scratch, as_of);
                        scratch.insert_surface_mut(std::sync::Arc::clone(vol_surface));
                        let pv_down = pv_down?;

                        let vega = (pv_up.amount() - pv_down.amount())
                            / (2.0 * bump_pct * VOL_POINTS_PER_ABSOLUTE_VOL);
                        row.push(vega);
                        raw_total.add(vega);
                    }
                    raw_matrix.push(row);
                    let expiry = expiry_label(expiry);
                    row_labels.push(if multiple_surfaces {
                        format!("{}::{expiry}", surface_id.as_str())
                    } else {
                        expiry
                    });
                }
            }

            Ok((raw_matrix, raw_total.total(), row_labels))
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
    use super::{scaled_bucketed_vega_matrix, KeyRateVega};
    use crate::instruments::common_impl::dependencies::{MarketDependencies, VolatilityDependency};
    use crate::instruments::common_impl::traits::{Attributes, Instrument};
    use crate::metrics::{MetricCalculator, MetricContext};
    use crate::pricer::InstrumentType;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::CurveId;
    use std::sync::Arc;
    use time::macros::date;

    #[derive(Clone)]
    struct MultiSurfaceVegaInstrument {
        attributes: Attributes,
        surface_terms: Vec<(CurveId, f64)>,
    }

    crate::impl_empty_cashflow_provider!(
        MultiSurfaceVegaInstrument,
        crate::cashflow::builder::CashflowRepresentation::NoResidual
    );

    impl MultiSurfaceVegaInstrument {
        fn raw_value(&self, market: &MarketContext) -> finstack_quant_core::Result<f64> {
            self.surface_terms
                .iter()
                .try_fold(0.0, |total, (surface_id, coefficient)| {
                    let vol = market
                        .get_surface(surface_id.as_str())?
                        .value_checked(1.0, 100.0)?;
                    Ok(total + coefficient * vol)
                })
        }
    }

    impl Instrument for MultiSurfaceVegaInstrument {
        fn market_dependencies(&self) -> finstack_quant_core::Result<MarketDependencies> {
            let mut dependencies = MarketDependencies::new();
            for (surface_id, _) in &self.surface_terms {
                dependencies.add_volatility_dependency(VolatilityDependency::new(
                    surface_id.clone(),
                    None,
                    None,
                ));
            }
            Ok(dependencies)
        }

        fn id(&self) -> &str {
            "MULTI-SURFACE-KEY-RATE-VEGA"
        }

        fn key(&self) -> InstrumentType {
            InstrumentType::Equity
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }

        fn clone_box(&self) -> Box<dyn Instrument> {
            Box::new(self.clone())
        }

        fn attributes(&self) -> &Attributes {
            &self.attributes
        }

        fn attributes_mut(&mut self) -> &mut Attributes {
            &mut self.attributes
        }

        fn base_value(
            &self,
            market: &MarketContext,
            _as_of: Date,
        ) -> finstack_quant_core::Result<Money> {
            Ok(Money::new(self.raw_value(market)?, Currency::USD))
        }

        fn base_value_raw(
            &self,
            market: &MarketContext,
            _as_of: Date,
        ) -> finstack_quant_core::Result<f64> {
            self.raw_value(market)
        }
    }

    fn flat_surface(id: &str, vol: f64) -> VolSurface {
        VolSurface::builder(id)
            .expiries(&[1.0])
            .strikes(&[100.0])
            .row(&[vol])
            .build()
            .expect("surface")
    }

    #[test]
    fn zero_target_vega_forces_bucket_matrix_to_zero() {
        let (matrix, scale) = scaled_bucketed_vega_matrix(vec![vec![10.0, -10.0]], 0.0, 0.0);

        assert_eq!(scale, 0.0);
        assert_eq!(matrix, vec![vec![0.0, 0.0]]);
    }

    #[test]
    fn key_rate_vega_includes_every_present_unique_surface() {
        let first_coefficient = 1_000.0;
        let second_coefficient = 2_500.0;
        let instrument = MultiSurfaceVegaInstrument {
            attributes: Attributes::new(),
            surface_terms: vec![
                (CurveId::new("VOL-A"), first_coefficient),
                (CurveId::new("VOL-B"), second_coefficient),
            ],
        };
        let market = MarketContext::new()
            .insert_surface(flat_surface("VOL-A", 0.20))
            .insert_surface(flat_surface("VOL-B", 0.30));
        let as_of = date!(2025 - 01 - 01);
        let base_value = instrument.value(&market, as_of).expect("base value");
        let mut context = MetricContext::new(
            Arc::new(instrument),
            Arc::new(market),
            as_of,
            base_value,
            MetricContext::default_config(),
        );

        let vega = KeyRateVega::<MultiSurfaceVegaInstrument>::new(vec![1.0], vec![100.0])
            .calculate(&mut context)
            .expect("key-rate vega");

        assert!((vega - (first_coefficient + second_coefficient) * 0.01).abs() < 1e-9);
    }
}
