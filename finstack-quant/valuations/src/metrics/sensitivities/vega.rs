//! Vega calculators for volatility sensitivity.
//!
//! Provides parallel and key-rate vega calculators for instruments with volatility surfaces.

use crate::instruments::common_impl::traits::Instrument;
use crate::metrics::core::finite_difference::{
    apply_parallel_surface_bumps_in_place, min_grid_vol, revert_scratch_bumps,
    scalar_numeric_value, VOL_POINTS_PER_ABSOLUTE_VOL,
};
use crate::metrics::sensitivities::config as sens_config;
use crate::metrics::MetricCalculator;
use crate::metrics::{MetricContext, MetricId};
use finstack_quant_core::math::NeumaierAccumulator;
use std::marker::PhantomData;

/// Standard expiry buckets in years for equity options (1m, 3m, 6m, 1y, 2y, 3y, 5y).
const STANDARD_EQUITY_EXPIRY_BUCKETS: [f64; 7] =
    [1.0 / 12.0, 3.0 / 12.0, 6.0 / 12.0, 1.0, 2.0, 3.0, 5.0];

/// Standard strike buckets (relative to spot) for equity options.
const STANDARD_STRIKE_RATIOS: [f64; 7] = [0.5, 0.75, 0.9, 1.0, 1.1, 1.25, 1.5];

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
    pub(crate) fn new(expiries: Vec<f64>, strikes: Vec<f64>) -> Self {
        Self {
            expiries,
            strikes,
            _phantom: PhantomData,
        }
    }
}

impl<I> Default for KeyRateVega<I> {
    /// Standard equity buckets: expiries 1m–5y and strike ratios 0.5–1.5.
    fn default() -> Self {
        Self::new(
            STANDARD_EQUITY_EXPIRY_BUCKETS.to_vec(),
            STANDARD_STRIKE_RATIOS.to_vec(),
        )
    }
}

impl<I> MetricCalculator for KeyRateVega<I>
where
    I: Instrument + 'static,
{
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let instrument: &I = context.instrument_as()?;
        let defaults = sens_config::from_context_or_default(
            context.get_config(),
            context.get_metric_overrides(),
        )?;

        let dependencies = instrument.market_dependencies()?;
        let vol_surface_ids = dependencies.present_vol_surface_ids(&context.curves);
        if vol_surface_ids.is_empty() {
            return Err(finstack_quant_core::InputError::Invalid.into());
        }

        let curves = std::sync::Arc::clone(&context.curves);
        let base_ctx = curves.as_ref();
        let vol_surfaces = vol_surface_ids
            .iter()
            .map(|vol_surface_id| {
                Ok((
                    vol_surface_id.clone(),
                    base_ctx.get_surface(vol_surface_id.as_str())?,
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
                    apply_parallel_surface_bumps_in_place(scratch, &vol_surface_ids, bump_pct)?;
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
                        vol_surface_ids = ?vol_surface_ids,
                        min_vol = min_vol,
                        bump = bump_pct,
                        "key-rate vega parallel down-bump would clamp σ at 0; \
                         using one-sided forward difference"
                    );
                    let pv_base = ctx.reprice_money(base_ctx, as_of)?;
                    Ok((pv_up.amount() - pv_base.amount())
                        / (bump_pct * VOL_POINTS_PER_ABSOLUTE_VOL))
                } else {
                    let tokens_down = apply_parallel_surface_bumps_in_place(
                        scratch,
                        &vol_surface_ids,
                        -bump_pct,
                    )?;
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
            .map(|(vol_surface_id, _)| {
                if !use_ratio_strikes {
                    return Ok(self.strikes.clone());
                }
                let spot_id = dependencies
                    .volatility_dependencies
                    .iter()
                    .find(|dependency| {
                        dependency.vol_surface_id == *vol_surface_id
                            && dependency.underlying_id.is_some()
                    })
                    .and_then(|dependency| dependency.underlying_id.as_ref())
                    .map(|id| id.as_str())
                    .or_else(|| dependencies.market_scalar_ids.first().map(String::as_str))
                    .ok_or_else(|| {
                        finstack_quant_core::Error::from(finstack_quant_core::InputError::Invalid)
                    })?;
                let spot = scalar_numeric_value(base_ctx.get_price(spot_id)?);
                Ok(self.strikes.iter().map(|k| k * spot).collect())
            })
            .collect::<finstack_quant_core::Result<Vec<Vec<f64>>>>()?;

        let multiple_surfaces = vol_surfaces.len() > 1;
        let (raw_matrix, raw_total, row_labels) = context.with_market_scratch(|ctx, scratch| {
            let mut raw_matrix = Vec::new();
            let mut raw_total = NeumaierAccumulator::new();
            let mut row_labels = Vec::new();

            for ((vol_surface_id, vol_surface), strike_grid) in
                vol_surfaces.iter().zip(&surface_strike_grids)
            {
                let use_one_sided =
                    min_grid_vol(vol_surface).is_some_and(|minimum| minimum < bump_pct);
                for &expiry in &self.expiries {
                    let mut row = Vec::new();
                    for &strike in strike_grid {
                        let token_up = scratch.apply_surface_point_absolute_bump_in_place(
                            vol_surface_id.as_str(),
                            expiry,
                            strike,
                            bump_pct,
                        )?;
                        let pv_up = ctx.reprice_money(scratch, as_of);
                        scratch.revert_scratch_bump(token_up)?;
                        let pv_up = pv_up?;

                        let vega = if use_one_sided {
                            let pv_base = ctx.reprice_money(base_ctx, as_of)?;
                            (pv_up.amount() - pv_base.amount())
                                / (bump_pct * VOL_POINTS_PER_ABSOLUTE_VOL)
                        } else {
                            let token_down = scratch.apply_surface_point_absolute_bump_in_place(
                                vol_surface_id.as_str(),
                                expiry,
                                strike,
                                -bump_pct,
                            )?;
                            let pv_down = ctx.reprice_money(scratch, as_of);
                            scratch.revert_scratch_bump(token_down)?;
                            let pv_down = pv_down?;
                            (pv_up.amount() - pv_down.amount())
                                / (2.0 * bump_pct * VOL_POINTS_PER_ABSOLUTE_VOL)
                        };
                        row.push(vega);
                        raw_total.add(vega);
                    }
                    raw_matrix.push(row);
                    let expiry = sens_config::format_bucket_label_cow(expiry).into_owned();
                    row_labels.push(if multiple_surfaces {
                        format!("{}::{expiry}", vol_surface_id.as_str())
                    } else {
                        expiry
                    });
                }
            }

            Ok((raw_matrix, raw_total.total(), row_labels))
        })?;

        let residual = target_total - raw_total;
        context
            .computed
            .insert(MetricId::custom("bucketed_vega_residual"), residual);
        if residual.abs() > 0.10 * target_total.abs().max(f64::EPSILON) {
            tracing::warn!(
                raw_bucket_total = raw_total,
                parallel_vega = target_total,
                uncovered_residual = residual,
                "point-bump bucketed vega does not span the parallel surface bump"
            );
        }

        let col_labels: Vec<String> = self.strikes.iter().map(|&k| format!("{k:.2}")).collect();
        let _ = context.store_matrix2d(MetricId::BucketedVega, row_labels, col_labels, raw_matrix);

        Ok(raw_total)
    }

    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::Vega]
    }
}

#[cfg(test)]
mod tests {
    use super::KeyRateVega;
    use crate::instruments::common_impl::dependencies::{MarketDependencies, VolatilityDependency};
    use crate::instruments::common_impl::traits::{Attributes, Instrument};
    use crate::metrics::{MetricCalculator, MetricContext, MetricId};
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
                .try_fold(0.0, |total, (vol_surface_id, coefficient)| {
                    let surface = market.get_surface(vol_surface_id.as_str())?;
                    let vol =
                        finstack_quant_models::volatility::get_surface_vol(&surface, 1.0, 100.0)?;
                    Ok(total + coefficient * vol)
                })
        }
    }

    impl Instrument for MultiSurfaceVegaInstrument {
        fn market_dependencies(&self) -> finstack_quant_core::Result<MarketDependencies> {
            let mut dependencies = MarketDependencies::new();
            for (vol_surface_id, _) in &self.surface_terms {
                dependencies.add_volatility_dependency(VolatilityDependency::new(
                    vol_surface_id.clone(),
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
            Ok(Money::new(self.raw_value(market)?, Currency::USD).expect("valid money fixture"))
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

        assert!(
            (vega - (first_coefficient + second_coefficient) * 0.01).abs() < 1e-9,
            "raw total={vega}, expected={}",
            (first_coefficient + second_coefficient) * 0.01
        );
        let matrix = context
            .computed_matrix
            .get(&MetricId::BucketedVega)
            .expect("bucket matrix");
        assert!((matrix.values[0][0] - first_coefficient * 0.01).abs() < 1e-9);
        assert!((matrix.values[1][0] - second_coefficient * 0.01).abs() < 1e-9);
        assert!(context.computed[&MetricId::custom("bucketed_vega_residual")].abs() < 1e-9);
    }
}
