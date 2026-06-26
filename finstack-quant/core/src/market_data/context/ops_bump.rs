//! Scenario bump operations for [`MarketContext`](super::MarketContext).
//!
//! This submodule implements the canonical heterogeneous bump entry points used
//! for risk and scenario analysis across curves, surfaces, market scalars, and FX.

use crate::market_data::bumps::{BumpSpec, Bumpable, MarketBump};
use crate::types::CurveId;
use crate::Result;
use std::sync::Arc;

use super::{ContextMutationInfo, ContextScratchBump, CurveStorage, MarketContext};

impl MarketContext {
    /// Apply a scalar price bump in place and return a token that restores the
    /// original value.
    pub fn apply_price_bump_pct_in_place(
        &mut self,
        price_id: &str,
        bump_pct: f64,
    ) -> Result<ContextScratchBump> {
        let key = CurveId::from(price_id);
        let current = self.prices.get(price_id).cloned().ok_or_else(|| {
            crate::error::InputError::NotFound {
                id: price_id.to_string(),
            }
        })?;
        let bumped = match current {
            crate::market_data::scalars::MarketScalar::Unitless(v) => {
                crate::market_data::scalars::MarketScalar::Unitless(v * (1.0 + bump_pct))
            }
            crate::market_data::scalars::MarketScalar::Price(m) => {
                crate::market_data::scalars::MarketScalar::Price(crate::money::Money::new(
                    m.amount() * (1.0 + bump_pct),
                    m.currency(),
                ))
            }
        };
        Arc::make_mut(&mut self.prices).insert(key.clone(), bumped);
        Ok(ContextScratchBump::Price {
            id: key,
            previous: current,
        })
    }

    /// Apply an absolute parallel volatility bump in place and return a token
    /// that restores the original surface.
    pub fn apply_surface_bump_in_place(
        &mut self,
        surface_id: &str,
        spec: BumpSpec,
    ) -> Result<ContextScratchBump> {
        let key = CurveId::from(surface_id);
        let previous = self.surfaces.get(surface_id).cloned().ok_or_else(|| {
            crate::error::InputError::NotFound {
                id: surface_id.to_string(),
            }
        })?;
        let bumped = previous.apply_bump(spec)?;
        Arc::make_mut(&mut self.surfaces).insert(key.clone(), Arc::new(bumped));
        Ok(ContextScratchBump::Surface { id: key, previous })
    }

    /// Apply a curve bump in place and return a token that restores the
    /// original curve and any credit indices that were rebound.
    pub fn apply_curve_bump_in_place(
        &mut self,
        curve_id: &CurveId,
        spec: BumpSpec,
    ) -> Result<ContextScratchBump> {
        let previous = self.curves.get(curve_id.as_str()).cloned().ok_or_else(|| {
            crate::error::InputError::NotFound {
                id: curve_id.to_string(),
            }
        })?;
        // Snapshot the credit-index map only when this curve actually feeds one;
        // the common rates DV01/CS01 bucket-bump case touches no credit indices
        // and should not clone the (possibly large) map per bucket.
        let affects_credit = self.curve_affects_credit_indices(curve_id);
        let previous_credit_indices = if affects_credit {
            Some(Arc::clone(&self.credit_indices))
        } else {
            None
        };
        let storage = Arc::make_mut(&mut self.curves)
            .get_mut(curve_id.as_str())
            .ok_or_else(|| crate::error::InputError::NotFound {
                id: curve_id.to_string(),
            })?;
        storage.apply_bump_preserving_id(curve_id, spec)?;
        if affects_credit {
            let _invalidated = self.rebind_all_credit_indices();
        }
        Ok(ContextScratchBump::Curve {
            id: curve_id.clone(),
            previous,
            previous_credit_indices,
        })
    }

    /// Revert a scratch bump token produced by one of the in-place bump helpers.
    pub fn revert_scratch_bump(&mut self, bump: ContextScratchBump) -> Result<()> {
        match bump {
            ContextScratchBump::Price { id, previous } => {
                Arc::make_mut(&mut self.prices).insert(id, previous);
            }
            ContextScratchBump::Surface { id, previous } => {
                Arc::make_mut(&mut self.surfaces).insert(id, previous);
            }
            ContextScratchBump::Curve {
                id,
                previous,
                previous_credit_indices,
            } => {
                Arc::make_mut(&mut self.curves).insert(id, previous);
                // Only restore the credit-index map if it was actually rebound
                // during the bump (see `apply_curve_bump_in_place`).
                if let Some(previous_credit_indices) = previous_credit_indices {
                    self.credit_indices = previous_credit_indices;
                }
            }
        }
        Ok(())
    }

    /// Apply a heterogeneous list of market bumps (curves, surfaces, prices, FX).
    ///
    /// This is the **single canonical** entry point for market bumping. It supports:
    /// - Curve/surface/scalar/series bumps addressed by [`CurveId`] (via [`MarketBump::Curve`])
    /// - FX percentage shocks (via [`MarketBump::FxPct`])
    /// - Volatility surface bucket bumps (via [`MarketBump::VolBucketPct`])
    /// - Base correlation bucket bumps (via [`MarketBump::BaseCorrBucketPts`])
    ///
    /// # Errors
    ///
    /// Returns an error if any bumped entry is missing, the bump type is unsupported,
    /// or reconstruction fails.
    pub fn bump<I>(&self, bumps: I) -> Result<Self>
    where
        I: IntoIterator<Item = MarketBump>,
    {
        let (ctx, _info) = self.bump_observed(bumps)?;
        Ok(ctx)
    }

    /// Like [`bump`](Self::bump), but also returns a [`ContextMutationInfo`]
    /// describing any credit indices that were invalidated.
    ///
    /// Use this in production workflows where silent credit-index invalidation
    /// is a risk.
    pub fn bump_observed<I>(&self, bumps: I) -> Result<(Self, ContextMutationInfo)>
    where
        I: IntoIterator<Item = MarketBump>,
    {
        use crate::collections::HashMap;
        use crate::error::InputError;

        // First pass: classify bumps to determine which maps need cloning.
        let mut curve_bumps: HashMap<CurveId, BumpSpec> = HashMap::default();
        let mut fx_bumps = Vec::new();
        let mut vol_bumps = Vec::new();
        let mut base_corr_bumps = Vec::new();
        let mut needs_credit_rebind = false;
        let mut processed_bumps = 0usize;

        for bump in bumps {
            {
                processed_bumps += 1;
            }
            match bump {
                MarketBump::Curve { id, spec } => {
                    curve_bumps.insert(id, spec);
                }
                MarketBump::FxPct {
                    base,
                    quote,
                    pct,
                    as_of,
                } => {
                    fx_bumps.push((base, quote, pct, as_of));
                }
                MarketBump::VolBucketPct {
                    surface_id,
                    expiries,
                    strikes,
                    pct,
                } => {
                    // `None` filters mean "all buckets"; route through the same
                    // multiplicative `apply_bucket_bump` path as filtered bumps so
                    // semantics (vol × (1 + pct/100)) are identical with or without
                    // filters .
                    vol_bumps.push((surface_id, expiries, strikes, pct));
                }
                MarketBump::BaseCorrBucketPts {
                    surface_id,
                    detachments,
                    points,
                } => {
                    base_corr_bumps.push((surface_id, detachments, points));
                }
            }
        }

        // This helper returns a bumped copy of the whole context. The map clone is
        // shallow (Arc bumps, not deep data copies), but callers doing many bump /
        // revert cycles in tight loops should prefer the in-place scratch workflow
        // exposed by `bump_observed_in_place` to avoid repeated context cloning.

        let mut ctx = self.clone();

        // Apply FX bumps
        for (base, quote, pct, as_of) in fx_bumps {
            let fx = ctx.fx.as_ref().ok_or_else(|| InputError::NotFound {
                id: "FX matrix".to_string(),
            })?;
            let bumped = fx.with_bumped_rate(base, quote, pct / 100.0, as_of)?;
            ctx.fx = Some(Arc::new(bumped));
        }

        // Apply vol bucket bumps
        for (surface_id, expiries, strikes, pct) in vol_bumps {
            let surface =
                ctx.get_surface(surface_id.as_str())
                    .map_err(|_| InputError::NotFound {
                        id: surface_id.to_string(),
                    })?;
            let bumped = surface
                .apply_bucket_bump(expiries.as_deref(), strikes.as_deref(), pct)
                .ok_or(InputError::DimensionMismatch)?;
            ctx = ctx.insert_surface(bumped);
        }

        // Apply base correlation bumps
        for (surface_id, detachments, points) in base_corr_bumps {
            let curve = ctx.get_base_correlation(surface_id.as_str()).map_err(|_| {
                InputError::NotFound {
                    id: surface_id.to_string(),
                }
            })?;
            let bumped = curve
                .apply_bucket_bump(detachments.as_deref(), points)
                .ok_or(InputError::DimensionMismatch)?;
            Arc::make_mut(&mut ctx.curves)
                .insert(surface_id, CurveStorage::BaseCorrelation(Arc::new(bumped)));
            needs_credit_rebind = true;
        }

        // Apply curve bumps
        let curve_invalidated = if !curve_bumps.is_empty() {
            ctx.apply_curve_bumps(curve_bumps)?
        } else {
            Vec::new()
        };
        let mut mutation_info = ContextMutationInfo::default();
        if needs_credit_rebind {
            let base_corr_invalidated = ctx.rebind_all_credit_indices();
            mutation_info.invalidated_credit_indices = base_corr_invalidated;
        }
        for id in curve_invalidated {
            if !mutation_info.invalidated_credit_indices.contains(&id) {
                mutation_info.invalidated_credit_indices.push(id);
            }
        }

        tracing::debug!(
            processed_bumps,
            needs_credit_rebind,
            invalidated_count = mutation_info.invalidated_credit_indices.len(),
            "applied MarketContext bumps"
        );

        Ok((ctx, mutation_info))
    }

    /// Apply curve bumps using the centralized bump-and-rebuild logic in `CurveStorage`.
    ///
    /// This method iterates over the bump specifications and applies them to curves,
    /// surfaces, prices, or series. The `CurveStorage::apply_bump_preserving_id` method
    /// handles the curve-specific bumping and ID preservation logic.
    fn apply_curve_bumps(
        &mut self,
        bumps: crate::collections::HashMap<CurveId, BumpSpec>,
    ) -> Result<Vec<CurveId>> {
        let mut needs_credit_rebind = false;
        for (curve_id, bump_spec) in bumps {
            let cid = curve_id.as_str();

            if let Some(storage) = Arc::make_mut(&mut self.curves).get_mut(cid) {
                storage.apply_bump_preserving_id(&curve_id, bump_spec)?;
                if !needs_credit_rebind {
                    needs_credit_rebind = self.curve_affects_credit_indices(&curve_id);
                }
                continue;
            }

            if let Some(original) = self.surfaces.get(cid).cloned() {
                let bumped = original.apply_bump(bump_spec)?;
                Arc::make_mut(&mut self.surfaces).insert(curve_id.clone(), Arc::new(bumped));
                continue;
            }

            if let Some(original) = self.prices.get(cid).cloned() {
                let bumped = original.apply_bump(bump_spec)?;
                Arc::make_mut(&mut self.prices).insert(curve_id.clone(), bumped);
                continue;
            }

            if let Some(original) = self.series.get(cid).cloned() {
                let bumped = original.apply_bump(bump_spec)?;
                Arc::make_mut(&mut self.series).insert(curve_id.clone(), bumped);
                continue;
            }

            return Err(crate::error::InputError::NotFound {
                id: cid.to_string(),
            }
            .into());
        }

        let invalidated = if needs_credit_rebind {
            self.rebind_all_credit_indices()
        } else {
            Vec::new()
        };

        Ok(invalidated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dates::Date;
    use crate::market_data::bumps::{BumpMode, BumpType, BumpUnits};
    use crate::market_data::scalars::MarketScalar;
    use crate::market_data::surfaces::VolSurface;
    use crate::market_data::term_structures::DiscountCurve;
    use crate::math::interp::InterpStyle;
    use time::Month;

    fn as_of() -> Date {
        Date::from_calendar_date(2025, Month::January, 1).expect("valid date")
    }

    fn surface_spec(value: f64) -> BumpSpec {
        BumpSpec {
            mode: BumpMode::Additive,
            units: BumpUnits::Fraction,
            value,
            bump_type: BumpType::Parallel,
        }
    }

    #[test]
    fn scratch_price_bump_restores_original_value() {
        let mut ctx = MarketContext::new().insert_price("SPOT", MarketScalar::Unitless(100.0));

        let token = ctx
            .apply_price_bump_pct_in_place("SPOT", 0.01)
            .expect("price bump");
        let bumped = ctx.get_price("SPOT").expect("bumped spot");
        match bumped {
            MarketScalar::Unitless(v) => assert!((*v - 101.0).abs() < 1e-12),
            MarketScalar::Price(_) => panic!("expected unitless price"),
        }

        ctx.revert_scratch_bump(token).expect("revert");
        let restored = ctx.get_price("SPOT").expect("restored spot");
        match restored {
            MarketScalar::Unitless(v) => assert!((*v - 100.0).abs() < 1e-12),
            MarketScalar::Price(_) => panic!("expected unitless price"),
        }
    }

    #[test]
    fn scratch_surface_bump_restores_original_surface() {
        let surface =
            VolSurface::from_grid("VOL", &[0.5, 1.0], &[90.0, 100.0], &[0.2; 4]).expect("surface");
        let mut ctx = MarketContext::new().insert_surface(surface);

        let token = ctx
            .apply_surface_bump_in_place("VOL", surface_spec(0.01))
            .expect("surface bump");
        let bumped = ctx.get_surface("VOL").expect("bumped surface");
        assert!((bumped.value_checked(0.5, 90.0).expect("bumped value") - 0.21).abs() < 1e-12);

        ctx.revert_scratch_bump(token).expect("revert");
        let restored = ctx.get_surface("VOL").expect("restored surface");
        assert!((restored.value_checked(0.5, 90.0).expect("restored value") - 0.2).abs() < 1e-12);
    }

    #[test]
    fn scratch_curve_bump_restores_original_curve() {
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of())
            .interp(InterpStyle::LogLinear)
            .knots([(0.0, 1.0), (5.0, 0.85)])
            .build()
            .expect("curve");
        let mut ctx = MarketContext::new().insert(curve);

        let token = ctx
            .apply_curve_bump_in_place(&CurveId::from("USD-OIS"), BumpSpec::parallel_bp(1.0))
            .expect("curve bump");
        let bumped_zero = ctx.get_discount("USD-OIS").expect("bumped curve").zero(5.0);

        ctx.revert_scratch_bump(token).expect("revert");
        let restored_zero = ctx
            .get_discount("USD-OIS")
            .expect("restored curve")
            .zero(5.0);

        assert!(
            (bumped_zero - restored_zero).abs() > 1e-9,
            "bump should change the curve before restore"
        );
        assert!(
            (restored_zero - (-0.85f64.ln() / 5.0)).abs() < 1e-12,
            "restored curve should match original"
        );
    }
}
