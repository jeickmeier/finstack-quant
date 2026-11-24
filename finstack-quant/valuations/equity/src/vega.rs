//! Vega calculators for volatility sensitivity.
//!
//! Provides parallel and key-rate vega calculators for instruments with volatility surfaces.

use finstack_quant_core::math::summation::NeumaierAccumulator;
use finstack_quant_valuations_core::metrics::sensitivities::bump_config;
use finstack_quant_valuations_core::metrics::sensitivities::fd_greeks::GenericFdVega;
use finstack_quant_valuations_core::metrics::{
    MetricCalculator, MetricContext, MetricId, VOL_POINTS_PER_ABSOLUTE_VOL,
    clone_with_crn_seed_if_supported, min_grid_vol, volatility_override,
};

/// Key-rate vega calculator: bumps individual `(expiry, strike)` points.
///
/// Calculates volatility sensitivity at individual points on the volatility surface
/// by bumping each `(expiry, strike)` combination and measuring the present value change.
/// Stores each cell as a 3-part composite key
/// `bucketed_vega::<surface>::<expiry>::<strike>`.
#[derive(Default)]
pub struct KeyRateVega {
    expiries: Vec<f64>,
    strikes: Vec<f64>,
}

impl KeyRateVega {
    /// Create a key-rate vega calculator with custom buckets.
    ///
    /// # Arguments
    ///
    /// * `expiries` - Strictly increasing actual source expiries in years; empty selects every source expiry.
    /// * `strikes` - Strictly increasing actual source strikes; empty selects every source strike.
    #[cfg(test)]
    pub(crate) fn new(expiries: Vec<f64>, strikes: Vec<f64>) -> Self {
        Self { expiries, strikes }
    }
}

impl MetricCalculator for KeyRateVega {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let defaults = bump_config::resolve(context)?;

        let vol_surface_ids = {
            let instrument = context.instrument.as_ref();
            let dependencies = instrument.market_dependencies()?;
            let ids = if volatility_override(instrument).is_some() {
                Vec::new()
            } else {
                dependencies.grid_vol_surface_ids(&context.curves)?
            };
            if ids.is_empty() && volatility_override(instrument).is_none() {
                return Err(finstack_quant_core::Error::Validation(
                    "vega requires a vol surface dependency or a volatility override".to_string(),
                ));
            }
            ids
        };

        let target_total = if let Some(existing) = context.computed.get(&MetricId::Vega) {
            *existing
        } else {
            GenericFdVega.calculate(context)?
        };

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
        let bump_pct = defaults.vol_bump_decimal;
        let seeded_instrument = clone_with_crn_seed_if_supported(context.instrument.as_ref());
        let instrument = seeded_instrument.as_ref();

        let grids = vol_surfaces
            .iter()
            .map(|(id, surface)| {
                Ok((
                    id,
                    surface,
                    source_grid(&self.expiries, surface.expiries(), "expiry")?,
                    source_grid(&self.strikes, surface.secondary_values(), "strike")?,
                ))
            })
            .collect::<finstack_quant_core::Result<Vec<_>>>()?;
        // A union of source strikes lets a single structured matrix retain
        // heterogeneous grids; cells absent from a surface carry zero exposure.
        let mut columns: Vec<f64> = grids
            .iter()
            .flat_map(|(_, _, _, strikes)| strikes.iter().copied())
            .collect();
        columns.sort_by(f64::total_cmp);
        columns.dedup_by(|a, b| a.total_cmp(b).is_eq());
        let mut grid_one_sided = Vec::with_capacity(grids.len());
        for (_, surface, _, _) in &grids {
            grid_one_sided.push(min_grid_vol(surface)?.is_some_and(|vol| vol <= bump_pct));
        }
        let (raw_matrix, raw_total, row_keys) = context.with_market_scratch(|ctx, scratch| {
            let mut matrix = Vec::new();
            let mut total = NeumaierAccumulator::new();
            let mut row_keys = Vec::new();
            let base_raw = ctx.reprice_instrument_raw(instrument, base_ctx, as_of)?;
            for ((id, _, expiries, strikes), one_sided) in grids.iter().zip(grid_one_sided) {
                for &expiry in expiries {
                    let mut row = vec![0.0; columns.len()];
                    for &strike in strikes {
                        let column = columns
                            .binary_search_by(|value| value.total_cmp(&strike))
                            .map_err(|_| {
                                finstack_quant_core::Error::Internal(
                                    "source strike missing from vega matrix".into(),
                                )
                            })?;
                        let token = scratch.apply_surface_point_absolute_bump_in_place(
                            id.as_str(),
                            expiry,
                            strike,
                            bump_pct,
                        )?;
                        let up = ctx.reprice_instrument_raw(instrument, scratch, as_of);
                        scratch.revert_scratch_bump(token)?;
                        let up = up?;
                        let vega = if one_sided {
                            (up - base_raw) / (bump_pct * VOL_POINTS_PER_ABSOLUTE_VOL)
                        } else {
                            let token = scratch.apply_surface_point_absolute_bump_in_place(
                                id.as_str(),
                                expiry,
                                strike,
                                -bump_pct,
                            )?;
                            let down = ctx.reprice_instrument_raw(instrument, scratch, as_of);
                            scratch.revert_scratch_bump(token)?;
                            (up - down?) / (2.0 * bump_pct * VOL_POINTS_PER_ABSOLUTE_VOL)
                        };
                        row[column] = vega;
                        total.add(vega)?;
                    }
                    row_keys.push((id.as_str().to_string(), format!("{expiry}y")));
                    matrix.push(row);
                }
            }
            Ok((matrix, total.get_total()?, row_keys))
        })?;

        let residual = target_total - raw_total;
        context
            .computed
            .insert(MetricId::BucketedVegaResidual, residual);
        if residual.abs() > 0.10 * target_total.abs().max(f64::EPSILON) {
            tracing::warn!(
                raw_bucket_total = raw_total,
                parallel_vega = target_total,
                uncovered_residual = residual,
                "point-bump bucketed vega does not span the parallel surface bump"
            );
        }

        let col_labels: Vec<String> = columns.iter().map(ToString::to_string).collect();
        for ((surface_id, expiry_label), row) in row_keys.iter().zip(raw_matrix.iter()) {
            for (col_label, &value) in col_labels.iter().zip(row.iter()) {
                let key = MetricId::composite(
                    &MetricId::BucketedVega,
                    &[
                        surface_id.as_str(),
                        expiry_label.as_str(),
                        col_label.as_str(),
                    ],
                );
                context.computed.insert(key, value);
            }
        }

        Ok(raw_total)
    }

    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::Vega]
    }
}

/// Custom coordinates must identify actual nodes, never nearest-node aliases.
fn source_grid(
    requested: &[f64],
    source: &[f64],
    axis: &str,
) -> finstack_quant_core::Result<Vec<f64>> {
    if requested.is_empty() {
        return Ok(source.to_vec());
    }
    if requested
        .iter()
        .any(|value| !value.is_finite() || !source.contains(value))
        || requested.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(finstack_quant_core::Error::Validation(format!(
            "vega {axis} coordinates must be unique increasing actual surface nodes"
        )));
    }
    Ok(requested.to_vec())
}

#[cfg(test)]
mod tests {
    use super::KeyRateVega;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::surfaces::{
        VolInterpolationMode, VolQuoteType, VolSurface, VolSurfaceAxis,
    };
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::Attributes;
    use finstack_quant_core::types::CurveId;
    use finstack_quant_valuations_core::instruments::Instrument;
    use finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides;
    use finstack_quant_valuations_core::instruments::{MarketDependencies, VolatilityDependency};
    use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext, MetricId};
    use finstack_quant_valuations_core::pricer::InstrumentType;
    use std::sync::Arc;
    use time::macros::date;

    #[derive(Clone)]
    struct MultiSurfaceVegaInstrument {
        attributes: Attributes,
        pricing_overrides: InstrumentPricingOverrides,
        surface_terms: Vec<(CurveId, f64)>,
    }

    finstack_quant_valuations_core::impl_empty_cashflow_provider!(
        MultiSurfaceVegaInstrument,
        finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
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
        fn includes_valuation_date_cashflows(&self) -> bool {
            true
        }
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

        fn get_instrument_pricing_overrides(&self) -> Option<&InstrumentPricingOverrides> {
            Some(&self.pricing_overrides)
        }

        fn get_instrument_pricing_overrides_mut(
            &mut self,
        ) -> Option<&mut InstrumentPricingOverrides> {
            Some(&mut self.pricing_overrides)
        }

        fn base_value(
            &self,
            market: &MarketContext,
            _as_of: Date,
        ) -> finstack_quant_core::Result<Money> {
            Money::from_model_value(self.raw_value(market)?, Currency::USD)
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
            .secondary_values(&[100.0])
            .secondary_axis(VolSurfaceAxis::Strike)
            .quote_type(VolQuoteType::BlackLognormal)
            .interpolation_mode(VolInterpolationMode::Vol)
            .row(&[vol])
            .build()
            .expect("surface")
    }

    #[test]
    fn production_risk_default_vega_uses_unique_nodes() {
        let instrument = MultiSurfaceVegaInstrument {
            attributes: Attributes::new(),
            pricing_overrides: InstrumentPricingOverrides::default(),
            surface_terms: vec![(CurveId::new("VOL"), 1_000.0)],
        };
        let market = MarketContext::new().insert_surface(flat_surface("VOL", 0.2));
        let as_of = date!(2025 - 01 - 01);
        let base = instrument.value(&market, as_of).expect("base");
        let mut context = MetricContext::new(
            Arc::new(instrument),
            Arc::new(market),
            as_of,
            base,
            MetricContext::default_config(),
        );
        let vega = KeyRateVega::default()
            .calculate(&mut context)
            .expect("actual nodes need no spot mapping");
        assert!((vega - 10.0).abs() < 1e-9, "{vega}");
        assert_eq!(
            context
                .computed
                .keys()
                .filter(|key| key.decode_components(&MetricId::BucketedVega).is_some())
                .count(),
            1
        );
    }

    #[test]
    fn production_risk_custom_vega_rejects_duplicate_source_nodes() {
        let instrument = MultiSurfaceVegaInstrument {
            attributes: Attributes::new(),
            pricing_overrides: InstrumentPricingOverrides::default(),
            surface_terms: vec![(CurveId::new("VOL"), 1_000.0)],
        };
        let market = MarketContext::new().insert_surface(flat_surface("VOL", 0.2));
        let as_of = date!(2025 - 01 - 01);
        let base = instrument.value(&market, as_of).expect("base");
        let mut context = MetricContext::new(
            Arc::new(instrument),
            Arc::new(market),
            as_of,
            base,
            MetricContext::default_config(),
        );
        assert!(
            KeyRateVega::new(vec![1.0, 1.0], vec![100.0])
                .calculate(&mut context)
                .is_err()
        );
    }

    #[test]
    fn key_rate_vega_includes_every_present_unique_surface() {
        let first_coefficient = 1_000.0;
        let second_coefficient = 2_500.0;
        let instrument = MultiSurfaceVegaInstrument {
            attributes: Attributes::new(),
            pricing_overrides: InstrumentPricingOverrides::default(),
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

        let vega = KeyRateVega::new(vec![1.0], vec![100.0])
            .calculate(&mut context)
            .expect("key-rate vega");

        assert!(
            (vega - (first_coefficient + second_coefficient) * 0.01).abs() < 1e-9,
            "raw total={vega}, expected={}",
            (first_coefficient + second_coefficient) * 0.01
        );
        let first = MetricId::composite(&MetricId::BucketedVega, &["VOL-A", "1y", "100"]);
        let second = MetricId::composite(&MetricId::BucketedVega, &["VOL-B", "1y", "100"]);
        assert_eq!(first.as_str(), "bucketed_vega::VOL-A::1y::100");
        assert!((context.computed[&first] - first_coefficient * 0.01).abs() < 1e-9);
        assert!((context.computed[&second] - second_coefficient * 0.01).abs() < 1e-9);
        assert!(context.computed[&MetricId::BucketedVegaResidual].abs() < 1e-9);
    }
}
