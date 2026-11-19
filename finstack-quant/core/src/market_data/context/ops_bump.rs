//! Scenario bump operations for [`MarketContext`](super::MarketContext).
//!
//! This submodule implements the canonical heterogeneous bump entry points used
//! for risk and scenario analysis across curves, surfaces, market scalars, and FX.

use crate::Result;
use crate::market_data::bumps::{BumpSpec, Bumpable, MarketBump};
use crate::market_data::fixings::FIXING_PREFIX;
use crate::market_data::term_structures::PriceCurveKind;
use crate::types::CurveId;
use std::sync::Arc;

use super::{ContextScratchBump, CurveStorage, MarketContext, ScratchBumpState};

impl MarketContext {
    /// Apply a scalar price bump in place and return a token that restores the
    /// original value.
    ///
    /// Money arithmetic preserves the stored Decimal amount. Invalid inputs,
    /// `FIXING:*` identifiers, non-positive multipliers (`1.0 + bump_fraction <= 0.0`),
    /// non-finite scalar results, and Decimal overflow leave the context unchanged.
    ///
    /// # Arguments
    ///
    /// * `price_id` - Identifier of the scalar in `self.prices` to shock.
    /// * `bump_fraction` - Finite relative price change as a decimal fraction;
    ///   `0.01` adds 1% (`multiplier = 1.01`) and `1.0 + bump_fraction` must be strictly positive.
    ///
    /// # Errors
    ///
    /// Returns an error for a `FIXING:*` identifier, a missing scalar, a non-finite
    /// input or result, a shock with `1.0 + bump_fraction <= 0.0`, or a Money amount
    /// that exceeds Decimal's representable range after the shock.
    pub fn apply_price_bump_fraction_in_place(
        &mut self,
        price_id: &str,
        bump_fraction: f64,
    ) -> Result<ContextScratchBump> {
        if price_id.starts_with(FIXING_PREFIX) {
            return Err(crate::Error::Validation(format!(
                "Cannot bump historical fixing identifier '{price_id}' via scalar scratch bump"
            )));
        }
        if !bump_fraction.is_finite() {
            return Err(crate::Error::Validation(format!(
                "price bump fraction must be finite, got {bump_fraction}"
            )));
        }
        let multiplier = 1.0 + bump_fraction;
        if !multiplier.is_finite() || multiplier <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "price bump fraction must satisfy 1.0 + bump_fraction > 0.0, got {bump_fraction}"
            )));
        }
        let key = CurveId::from(price_id);
        let current =
            self.prices
                .get(price_id)
                .cloned()
                .ok_or_else(|| crate::InputError::NotFound {
                    id: price_id.to_string(),
                })?;
        let bumped = current.apply_bump(BumpSpec::multiplier(multiplier))?;
        Arc::make_mut(&mut self.prices).insert(key.clone(), bumped);
        let (context_id, token_id) = self.allocate_scratch_token();
        Ok(ContextScratchBump {
            context_id,
            token_id,
            state: ScratchBumpState::Price {
                id: key,
                previous: current,
            },
        })
    }

    /// Apply an absolute parallel volatility bump in place and return a token
    /// that restores the original surface.
    ///
    /// # Arguments
    ///
    /// * `vol_surface_id` - Identifier of the [`VolSurface`](crate::market_data::surfaces::VolSurface) to shock.
    /// * `spec` - Additive or multiplicative bump specification applied across the surface grid.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Validation`] if `vol_surface_id` resolves only to a
    /// `VolCube` or `FxDeltaVolSurface` or if the bump drives any volatility negative,
    /// and [`crate::InputError::NotFound`] if no volatility surface is registered under `vol_surface_id`.
    pub fn apply_surface_bump_in_place(
        &mut self,
        vol_surface_id: &str,
        spec: BumpSpec,
    ) -> Result<ContextScratchBump> {
        if let Some(err) = self.unsupported_vol_surface_bump_error(vol_surface_id, "surface bump") {
            return Err(err);
        }
        let key = CurveId::from(vol_surface_id);
        let previous = self.surfaces.get(vol_surface_id).cloned().ok_or_else(|| {
            crate::InputError::NotFound {
                id: vol_surface_id.to_string(),
            }
        })?;
        let bumped = previous.apply_bump(spec)?;
        Arc::make_mut(&mut self.surfaces).insert(key.clone(), Arc::new(bumped));
        let (context_id, token_id) = self.allocate_scratch_token();
        Ok(ContextScratchBump {
            context_id,
            token_id,
            state: ScratchBumpState::Surface { id: key, previous },
        })
    }

    /// Apply an additive volatility bump to one exact surface grid node in place.
    ///
    /// # Arguments
    ///
    /// * `vol_surface_id` - Identifier of the volatility surface to mutate.
    /// * `expiry` - Exact stored expiry coordinate in years.
    /// * `secondary_value` - Exact stored secondary-axis coordinate (strike or tenor).
    /// * `bump_abs` - Volatility delta in the surface's native annual quote units.
    pub fn apply_surface_point_absolute_bump_in_place(
        &mut self,
        vol_surface_id: &str,
        expiry: f64,
        secondary_value: f64,
        bump_abs: f64,
    ) -> Result<ContextScratchBump> {
        if !(expiry.is_finite() && secondary_value.is_finite() && bump_abs.is_finite()) {
            return Err(crate::Error::Validation(format!(
                "volatility point bump inputs must be finite, got expiry={expiry}, \
                 secondary={secondary_value}, bump_abs={bump_abs}"
            )));
        }
        if let Some(err) =
            self.unsupported_vol_surface_bump_error(vol_surface_id, "absolute surface point bump")
        {
            return Err(err);
        }
        let key = CurveId::from(vol_surface_id);
        let previous = self.surfaces.get(vol_surface_id).cloned().ok_or_else(|| {
            crate::InputError::NotFound {
                id: vol_surface_id.to_string(),
            }
        })?;
        let mut bumped = (*previous).clone();
        bumped.bump_point_absolute_in_place(expiry, secondary_value, bump_abs)?;
        Arc::make_mut(&mut self.surfaces).insert(key.clone(), Arc::new(bumped));
        let (context_id, token_id) = self.allocate_scratch_token();
        Ok(ContextScratchBump {
            context_id,
            token_id,
            state: ScratchBumpState::Surface { id: key, previous },
        })
    }

    /// Apply a curve bump in place and return a token that restores the
    /// original curve and any credit indices that were rebound.
    ///
    /// # Arguments
    ///
    /// * `curve_id` - Identifier of the curve in `self.curves` to shock.
    /// * `spec` - Bump specification applied via `CurveStorage::apply_bump_preserving_id`.
    pub fn apply_curve_bump_in_place(
        &mut self,
        curve_id: &CurveId,
        spec: BumpSpec,
    ) -> Result<ContextScratchBump> {
        let previous = self.curves.get(curve_id.as_str()).cloned().ok_or_else(|| {
            crate::InputError::NotFound {
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
            .ok_or_else(|| crate::InputError::NotFound {
                id: curve_id.to_string(),
            })?;
        if let Err(err) = storage.apply_bump_preserving_id(curve_id, spec) {
            Arc::make_mut(&mut self.curves).insert(curve_id.clone(), previous);
            return Err(err);
        }
        if affects_credit && let Err(err) = self.rebind_all_credit_indices() {
            Arc::make_mut(&mut self.curves).insert(curve_id.clone(), previous);
            return Err(err);
        }
        let (context_id, token_id) = self.allocate_scratch_token();
        Ok(ContextScratchBump {
            context_id,
            token_id,
            state: ScratchBumpState::Curve {
                id: curve_id.clone(),
                previous,
                previous_credit_indices,
            },
        })
    }

    /// Revert a scratch bump token produced by one of the in-place bump helpers.
    ///
    /// # Arguments
    ///
    /// * `bump` - Exclusive token returned by an `apply_*_in_place` call on this
    ///   context instance, reverted in strict LIFO order.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Validation`] if the token belongs to a different or
    /// cloned context, was invalidated by an intervening mutation, or is reverted
    /// out of LIFO order.
    pub fn revert_scratch_bump(&mut self, bump: ContextScratchBump) -> Result<()> {
        self.pop_scratch_token(bump.context_id, bump.token_id)?;
        match bump.state {
            ScratchBumpState::Price { id, previous } => {
                Arc::make_mut(&mut self.prices).insert(id, previous);
            }
            ScratchBumpState::Surface { id, previous } => {
                Arc::make_mut(&mut self.surfaces).insert(id, previous);
            }
            ScratchBumpState::Curve {
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

    /// Apply a heterogeneous list of market bumps (curves, surfaces, scalars, series, FX).
    ///
    /// This is the **single canonical** entry point for market bumping. It supports:
    /// - Family-typed curve, surface, scalar, and series bumps (`DiscountCurve`, `ForwardCurve`,
    ///   `ParametricCurve`, `HazardCurve`, `InflationCurve`, `PriceCurve`,
    ///   `VolIndexCurve`, `BaseCorrelation`, `VolSurface`, `PriceScalar`, `UnitlessScalar`, `Series`)
    /// - FX percentage shocks (via [`MarketBump::FxPct`])
    /// - Volatility surface bucket bumps (via [`MarketBump::VolBucketPct`])
    /// - Base correlation bucket bumps (via [`MarketBump::BaseCorrBucketPts`])
    ///
    /// # Errors
    ///
    /// Returns an error if any bumped entry is missing, resolves to a different kind
    /// than declared, targets a `FIXING:*` series, or reconstruction fails.
    ///
    /// # Arguments
    ///
    /// * `bumps` - Typed market shocks applied in iterator order on a cloned context.
    ///   Repeated targets compose in that order; for example, factors `1.1` and `1.2`
    ///   multiply to `1.32`.
    pub fn bump<I>(&self, bumps: I) -> Result<Self>
    where
        I: IntoIterator<Item = MarketBump>,
    {
        let mut ctx = self.clone();
        let mut needs_credit_rebind = false;
        let mut processed_bumps = 0usize;

        for bump in bumps {
            processed_bumps += 1;
            match bump {
                MarketBump::DiscountCurve { id, spec } => {
                    needs_credit_rebind |=
                        ctx.apply_typed_curve_bump(id, spec, "DiscountCurve", |storage| {
                            matches!(storage, CurveStorage::Discount(_))
                        })?;
                }
                MarketBump::ForwardCurve { id, spec } => {
                    needs_credit_rebind |=
                        ctx.apply_typed_curve_bump(id, spec, "ForwardCurve", |storage| {
                            matches!(storage, CurveStorage::Forward(_))
                        })?;
                }
                MarketBump::ParametricCurve { id, spec } => {
                    needs_credit_rebind |=
                        ctx.apply_typed_curve_bump(id, spec, "ParametricCurve", |storage| {
                            matches!(storage, CurveStorage::Parametric(_))
                        })?;
                }
                MarketBump::HazardCurve { id, spec } => {
                    needs_credit_rebind |=
                        ctx.apply_typed_curve_bump(id, spec, "HazardCurve", |storage| {
                            matches!(storage, CurveStorage::Hazard(_))
                        })?;
                }
                MarketBump::InflationCurve { id, spec } => {
                    needs_credit_rebind |=
                        ctx.apply_typed_curve_bump(id, spec, "InflationCurve", |storage| {
                            matches!(storage, CurveStorage::Inflation(_))
                        })?;
                }
                MarketBump::PriceCurve { id, spec } => {
                    needs_credit_rebind |= ctx.apply_typed_curve_bump(
                        id,
                        spec,
                        "PriceCurve",
                        |storage| matches!(storage, CurveStorage::Price(c) if c.kind() == PriceCurveKind::Price),
                    )?;
                }
                MarketBump::VolIndexCurve { id, spec } => {
                    needs_credit_rebind |= ctx.apply_typed_curve_bump(
                        id,
                        spec,
                        "VolIndexCurve",
                        |storage| matches!(storage, CurveStorage::Price(c) if c.kind() == PriceCurveKind::VolIndex),
                    )?;
                }
                MarketBump::BaseCorrelation { id, spec } => {
                    needs_credit_rebind |=
                        ctx.apply_typed_curve_bump(id, spec, "BaseCorrelation", |storage| {
                            matches!(storage, CurveStorage::BaseCorrelation(_))
                        })?;
                }
                MarketBump::VolSurface { id, spec } => {
                    if let Some(err) = ctx
                        .unsupported_vol_surface_bump_error(id.as_str(), "volatility surface bump")
                    {
                        return Err(err);
                    }
                    let original = ctx.get_surface(id.as_str())?;
                    let bumped = original.apply_bump(spec)?;
                    Arc::make_mut(&mut ctx.surfaces).insert(id, Arc::new(bumped));
                }
                MarketBump::PriceScalar { id, spec } => {
                    ctx.apply_typed_scalar_bump(id, spec, true)?;
                }
                MarketBump::UnitlessScalar { id, spec } => {
                    ctx.apply_typed_scalar_bump(id, spec, false)?;
                }
                MarketBump::Series { id, spec } => {
                    if id.as_str().starts_with(FIXING_PREFIX) {
                        return Err(crate::Error::Validation(format!(
                            "Cannot bump historical fixing series '{id}'"
                        )));
                    }
                    let original = ctx.get_series(id.as_str())?;
                    let bumped = original.apply_bump(spec)?;
                    Arc::make_mut(&mut ctx.series).insert(id, bumped);
                }
                MarketBump::FxPct {
                    base,
                    quote,
                    pct,
                    as_of,
                    policy,
                } => {
                    let fx = ctx.fx_required()?;
                    let reference = crate::money::fx::FxQuery::new(base, quote, as_of, policy);
                    let bumped = fx.with_bumped_rate(reference, pct / 100.0)?;
                    ctx.fx = Some(Arc::new(bumped));
                }
                MarketBump::VolBucketPct {
                    vol_surface_id,
                    expiries,
                    secondary_values,
                    pct,
                } => {
                    if let Some(err) = ctx.unsupported_vol_surface_bump_error(
                        vol_surface_id.as_str(),
                        "volatility bucket bump",
                    ) {
                        return Err(err);
                    }
                    if !pct.is_finite() {
                        return Err(crate::Error::Validation(format!(
                            "volatility bucket bump percentage must be finite, got {pct}"
                        )));
                    }
                    let factor = 1.0 + pct / 100.0;
                    if !factor.is_finite() || factor < 0.0 {
                        return Err(crate::Error::Validation(format!(
                            "volatility bucket bump percentage must be at least -100%, got {pct}"
                        )));
                    }
                    // `None` filters mean "all buckets"; route through the same
                    // multiplicative `apply_bucket_bump` kernel as parallel scaling.
                    let surface = ctx.get_surface(vol_surface_id.as_str())?;
                    let bumped = surface.apply_bucket_bump(
                        expiries.as_deref(),
                        secondary_values.as_deref(),
                        BumpSpec::multiplier(factor),
                    )?;
                    Arc::make_mut(&mut ctx.surfaces).insert(vol_surface_id, Arc::new(bumped));
                }
                MarketBump::BaseCorrBucketPts {
                    surface_id,
                    detachments,
                    points,
                } => {
                    let curve = ctx.get_base_correlation(surface_id.as_str())?;
                    let bumped = curve.apply_bucket_bump(detachments.as_deref(), points)?;
                    needs_credit_rebind |= ctx.curve_affects_credit_indices(&surface_id);
                    Arc::make_mut(&mut ctx.curves)
                        .insert(surface_id, CurveStorage::BaseCorrelation(Arc::new(bumped)));
                }
            }
        }

        if needs_credit_rebind {
            ctx.rebind_all_credit_indices()?;
        }

        tracing::debug!(
            processed_bumps,
            needs_credit_rebind,
            "applied MarketContext bumps"
        );

        Ok(ctx)
    }

    fn apply_typed_curve_bump(
        &mut self,
        curve_id: CurveId,
        bump_spec: BumpSpec,
        expected: &'static str,
        matches_storage: fn(&CurveStorage) -> bool,
    ) -> Result<bool> {
        let cid = curve_id.as_str();
        let existing = self
            .curves
            .get(cid)
            .ok_or_else(|| crate::InputError::NotFound {
                id: cid.to_string(),
            })?;
        if !matches_storage(existing) {
            let actual = match existing {
                CurveStorage::Price(curve) if curve.kind() == PriceCurveKind::VolIndex => {
                    "VolIndexCurve"
                }
                CurveStorage::Price(_) => "PriceCurve",
                other => other.curve_type(),
            };
            return Err(crate::Error::Validation(format!(
                "MarketBump target '{cid}' expected {expected}, found {actual}"
            )));
        }

        #[cfg(test)]
        super::context_bump_work::record_generic_curves_make_mut(self, cid);

        let storage = Arc::make_mut(&mut self.curves)
            .get_mut(cid)
            .ok_or_else(|| crate::InputError::NotFound {
                id: cid.to_string(),
            })?;
        storage.apply_bump_preserving_id(&curve_id, bump_spec)?;
        Ok(self.curve_affects_credit_indices(&curve_id))
    }

    fn apply_typed_scalar_bump(
        &mut self,
        id: CurveId,
        spec: BumpSpec,
        expect_price: bool,
    ) -> Result<()> {
        if id.as_str().starts_with(FIXING_PREFIX) {
            return Err(crate::Error::Validation(format!(
                "Cannot bump historical fixing identifier '{id}'"
            )));
        }
        let original = self.get_price(id.as_str())?;
        if expect_price && original.is_unitless() {
            return Err(crate::Error::Validation(format!(
                "MarketBump::PriceScalar target '{id}' expected Price scalar, found Unitless"
            )));
        }
        if !expect_price && original.is_price() {
            return Err(crate::Error::Validation(format!(
                "MarketBump::UnitlessScalar target '{id}' expected Unitless scalar, found Price"
            )));
        }
        let bumped = original.apply_bump(spec)?;
        Arc::make_mut(&mut self.prices).insert(id, bumped);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::currency::Currency;
    use crate::dates::Date;
    use crate::market_data::bumps::{BumpMode, BumpType, BumpUnits};
    use crate::market_data::scalars::{InflationIndex, MarketScalar};
    use crate::market_data::surfaces::VolSurface;
    use crate::market_data::term_structures::{DiscountCurve, ForwardCurve, InflationCurve};
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
    fn ordered_bumps_compose_repeated_scalar_targets() {
        let context =
            MarketContext::new().insert_price("SPOT", MarketScalar::unitless(100.0).expect("100"));
        let bumped = context
            .bump([1.1, 1.2].map(|factor| MarketBump::UnitlessScalar {
                id: CurveId::new("SPOT"),
                spec: BumpSpec::multiplier(factor),
            }))
            .expect("ordered scalar bumps");
        let Some(value) = bumped.get_price("SPOT").expect("spot").get_unitless() else {
            panic!("unitless spot");
        };
        assert!((value - 132.0).abs() < 1e-12);
        assert_eq!(
            context.get_price("SPOT").ok(),
            Some(&MarketScalar::unitless(100.0).expect("100"))
        );

        let failed = context.bump([
            MarketBump::UnitlessScalar {
                id: CurveId::new("SPOT"),
                spec: BumpSpec::multiplier(1.1),
            },
            MarketBump::UnitlessScalar {
                id: CurveId::new("SPOT"),
                spec: BumpSpec::multiplier(f64::NAN),
            },
        ]);
        assert!(failed.is_err());
        assert_eq!(
            context.get_price("SPOT").ok(),
            Some(&MarketScalar::unitless(100.0).expect("100"))
        );
    }

    #[test]
    fn ordered_bumps_compose_mixed_bucket_and_parallel_shocks_to_one_curve() {
        let context = MarketContext::new()
            .try_insert(
                DiscountCurve::flat("USD", as_of(), 0.05, crate::dates::DayCount::Act365F)
                    .expect("flat discount curve"),
            )
            .expect("insert market curve");
        let bumps = [
            BumpSpec::parallel_bp(10.0),
            BumpSpec::triangular_key_rate_bp(3.0, 5.0, 7.0, 20.0),
            BumpSpec::parallel_bp(-4.0),
        ]
        .map(|spec| MarketBump::DiscountCurve {
            id: CurveId::new("USD"),
            spec,
        });
        let batched = context.bump(bumps.clone()).expect("composed curve shocks");
        let sequential = bumps.into_iter().fold(context.clone(), |market, bump| {
            market.bump([bump]).expect("single curve shock")
        });
        let original = context.get_discount("USD").expect("original curve");
        let batched = batched.get_discount("USD").expect("batched curve");
        let sequential = sequential.get_discount("USD").expect("sequential curve");
        for (time, weight) in [
            (1.0, 0.0),
            (3.0, 0.0),
            (4.0, 0.5),
            (5.0, 1.0),
            (6.0, 0.5),
            (7.0, 0.0),
            (10.0, 0.0),
        ] {
            let orig_df = original.df(time).expect("orig df");
            let batch_df = batched.df(time).expect("batch df");
            let seq_df = sequential.df(time).expect("seq df");
            let expected = orig_df * (-(0.0006_f64 + 0.002 * weight) * time).exp();
            assert!((batch_df - expected).abs() < 1e-14);
            assert!((batch_df - seq_df).abs() < 1e-14);
            assert!((orig_df - (-0.05 * time).exp()).abs() < 1e-14);
        }
    }

    #[test]
    fn ordered_bumps_preserve_mixed_surface_operations() {
        use crate::market_data::surfaces::{VolInterpolationMode, VolQuoteType, VolSurfaceAxis};
        let surface = VolSurface::builder("VOL")
            .expiries(&[1.0, 2.0])
            .secondary_values(&[90.0, 100.0])
            .secondary_axis(VolSurfaceAxis::Strike)
            .quote_type(VolQuoteType::BlackLognormal)
            .interpolation_mode(VolInterpolationMode::Vol)
            .row(&[0.2, 0.2])
            .row(&[0.2, 0.2])
            .build()
            .expect("surface");
        let context = MarketContext::new().insert_surface(surface);
        let additive = MarketBump::VolSurface {
            id: CurveId::new("VOL"),
            spec: surface_spec(0.1),
        };
        let relative = MarketBump::VolBucketPct {
            vol_surface_id: CurveId::new("VOL"),
            expiries: None,
            secondary_values: None,
            pct: 50.0,
        };
        let add_then_scale = context
            .bump([additive.clone(), relative.clone()])
            .expect("bumps");
        let scale_then_add = context.bump([relative, additive]).expect("bumps");
        for value in add_then_scale.get_surface("VOL").expect("surface").vols() {
            assert!((*value - 0.45).abs() < 1e-12);
        }
        for value in scale_then_add.get_surface("VOL").expect("surface").vols() {
            assert!((*value - 0.4).abs() < 1e-12);
        }
    }

    #[test]
    fn inflation_curve_bump_preserves_realized_same_id_index() {
        use crate::market_data::scalars::InflationObservation;

        let jan = time::macros::date!(2024 - 01 - 01);
        let feb = time::macros::date!(2024 - 02 - 01);
        let cutoff = time::macros::date!(2025 - 01 - 01);
        let index = InflationIndex::new(
            "US-CPI",
            vec![
                InflationObservation {
                    reference_month: jan,
                    value: 300.0,
                    available_on: feb,
                },
                InflationObservation {
                    reference_month: feb,
                    value: 306.0,
                    available_on: time::macros::date!(2024 - 03 - 12),
                },
            ],
            Currency::USD,
            cutoff,
        )
        .expect("inflation index");
        let curve = InflationCurve::builder("US-CPI")
            .base_date(as_of())
            .day_count(crate::dates::DayCount::Act365F)
            .knots([(0.0, 300.0), (1.0, 306.0)])
            .interp(InterpStyle::LogLinear)
            .extrapolation(crate::math::interp::ExtrapolationPolicy::Flat)
            .build()
            .expect("inflation curve");
        let context = MarketContext::new()
            .try_insert(curve)
            .expect("insert market curve")
            .insert_inflation_index(index);

        let bumped = context
            .bump([MarketBump::InflationCurve {
                id: CurveId::new("US-CPI"),
                spec: BumpSpec::inflation_shift_pct(0.01),
            }])
            .expect("inflation market bump");
        assert!(
            bumped
                .get_inflation_curve("US-CPI")
                .expect("curve")
                .cpi(1.0)
                .expect("cpi")
                > 306.0
        );
        assert_eq!(
            bumped
                .get_inflation_index("US-CPI")
                .expect("index")
                .get_observation(feb)
                .expect("CPI")
                .value,
            306.0
        );
    }

    #[test]
    fn inflation_curve_key_rate_bump_preserves_interp_and_extrapolation() {
        let curve = InflationCurve::builder("US-CPI")
            .base_date(as_of())
            .day_count(crate::dates::DayCount::Act365F)
            .knots([(0.0, 300.0), (1.0, 306.0), (2.0, 312.0)])
            .interp(InterpStyle::Linear)
            .extrapolation(crate::math::interp::ExtrapolationPolicy::Flat)
            .build()
            .expect("inflation curve");
        let bumped = MarketContext::new()
            .try_insert(curve)
            .expect("insert market curve")
            .bump([MarketBump::InflationCurve {
                id: CurveId::new("US-CPI"),
                spec: BumpSpec::triangular_key_rate_bp(0.0, 1.0, 2.0, 10.0),
            }])
            .expect("inflation curve key-rate bump");

        let rebuilt = bumped.get_inflation_curve("US-CPI").expect("bumped curve");
        assert_eq!(rebuilt.interp_style(), InterpStyle::Linear);
        assert_eq!(
            rebuilt.extrapolation(),
            crate::math::interp::ExtrapolationPolicy::Flat
        );
        assert_eq!(rebuilt.id().as_str(), "US-CPI");
    }

    #[test]
    fn inflation_curve_bump_preserves_extrapolation_policy() {
        let curve = InflationCurve::builder("US-CPI")
            .base_date(as_of())
            .day_count(crate::dates::DayCount::Act365F)
            .knots([(0.0, 300.0), (1.0, 306.0)])
            .interp(InterpStyle::LogLinear)
            .extrapolation(crate::math::interp::ExtrapolationPolicy::Flat)
            .build()
            .expect("inflation curve");
        let bumped = MarketContext::new()
            .try_insert(curve)
            .expect("insert market curve")
            .bump([MarketBump::InflationCurve {
                id: CurveId::new("US-CPI"),
                spec: BumpSpec::inflation_shift_pct(0.01),
            }])
            .expect("inflation curve bump");

        assert_eq!(
            bumped
                .get_inflation_curve("US-CPI")
                .expect("bumped curve")
                .extrapolation(),
            crate::math::interp::ExtrapolationPolicy::Flat
        );
    }

    #[test]
    fn scratch_price_bump_restores_original_value() {
        let mut ctx =
            MarketContext::new().insert_price("SPOT", MarketScalar::unitless(100.0).expect("100"));

        let token = ctx
            .apply_price_bump_fraction_in_place("SPOT", 0.01)
            .expect("price bump");
        let bumped = ctx.get_price("SPOT").expect("bumped spot");
        match bumped.get_unitless() {
            Some(v) => assert!((v - 101.0).abs() < 1e-12),
            None => panic!("expected unitless price"),
        }

        ctx.revert_scratch_bump(token).expect("revert");
        let restored = ctx.get_price("SPOT").expect("restored spot");
        match restored.get_unitless() {
            Some(v) => assert!((v - 100.0).abs() < 1e-12),
            None => panic!("expected unitless price"),
        }
    }

    #[test]
    fn failed_price_bump_is_atomic_for_non_finite_percentages() {
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut ctx = MarketContext::new()
                .insert_price("SPOT", MarketScalar::unitless(100.0).expect("100"));

            let Err(error) = ctx.apply_price_bump_fraction_in_place("SPOT", invalid) else {
                panic!("non-finite price bump must fail");
            };

            assert!(error.to_string().contains("finite"));
            match ctx
                .get_price("SPOT")
                .expect("unchanged spot")
                .get_unitless()
            {
                Some(value) => assert_eq!(value.to_bits(), 100.0f64.to_bits()),
                None => panic!("expected unitless price"),
            }
        }
    }

    #[test]
    fn scratch_money_bump_preserves_decimal_precision_and_zero_identity() {
        let original = crate::money::Money::from_decimal_str("1.0000000000000001", Currency::USD)
            .expect("exact price");
        let mut ctx = MarketContext::new().insert_price("SPOT", MarketScalar::price(original));
        for (percentage, expected) in [(0.0, "1.0000000000000001"), (0.01, "1.010000000000000101")]
        {
            let token = ctx
                .apply_price_bump_fraction_in_place("SPOT", percentage)
                .expect("checked bump");
            let Some(actual) = ctx.get_price("SPOT").expect("price").get_price() else {
                panic!("expected Money price");
            };
            assert_eq!(
                actual.amount_decimal(),
                crate::money::Money::from_decimal_str(expected, Currency::USD)
                    .expect("expected exact price")
                    .amount_decimal()
            );
            ctx.revert_scratch_bump(token).expect("restore");
            let Some(restored) = ctx.get_price("SPOT").expect("price").get_price() else {
                panic!("expected Money price");
            };
            assert_eq!(restored.amount_decimal(), original.amount_decimal());
        }
    }

    #[test]
    fn overflowing_scratch_price_bumps_leave_the_original_price_unchanged() {
        for original in [
            MarketScalar::unitless(f64::MAX).expect("max"),
            MarketScalar::price(
                crate::money::Money::from_decimal_str(
                    "10000000000000000000000000000",
                    Currency::USD,
                )
                .expect("representable price"),
            ),
        ] {
            let mut ctx = MarketContext::new().insert_price("SPOT", original.clone());
            assert!(ctx.apply_price_bump_fraction_in_place("SPOT", 9.0).is_err());
            let current = ctx.get_price("SPOT").expect("price");
            match (
                current.get_unitless(),
                current.get_price(),
                original.get_unitless(),
                original.get_price(),
            ) {
                (Some(actual), None, Some(expected), None) => {
                    assert_eq!(actual.to_bits(), expected.to_bits());
                }
                (None, Some(actual), None, Some(expected)) => {
                    assert_eq!(actual.amount_decimal(), expected.amount_decimal());
                }
                _ => panic!("price variant changed after a failed bump"),
            }
        }
    }

    #[test]
    fn scratch_surface_bump_restores_original_surface() {
        use crate::market_data::surfaces::{
            VolGridOpts, VolInterpolationMode, VolQuoteType, VolSurfaceAxis,
        };
        let surface = VolSurface::from_grid(
            "VOL",
            &[0.5, 1.0],
            &[90.0, 100.0],
            &[0.2; 4],
            VolGridOpts {
                secondary_axis: VolSurfaceAxis::Strike,
                quote_type: VolQuoteType::BlackLognormal,
                interpolation_mode: VolInterpolationMode::Vol,
            },
            None,
        )
        .expect("surface");
        let mut ctx = MarketContext::new().insert_surface(surface);

        let token = ctx
            .apply_surface_bump_in_place("VOL", surface_spec(0.01))
            .expect("surface bump");
        let bumped = ctx.get_surface("VOL").expect("bumped surface");
        assert!((bumped.vols()[0] - 0.21).abs() < 1e-12);

        ctx.revert_scratch_bump(token).expect("revert");
        let restored = ctx.get_surface("VOL").expect("restored surface");
        assert!((restored.vols()[0] - 0.2).abs() < 1e-12);
    }
    #[test]
    fn scratch_curve_bump_restores_original_curve() {
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of())
            .day_count(crate::dates::DayCount::Act365F)
            .interp(InterpStyle::LogLinear)
            .extrapolation(crate::math::interp::ExtrapolationPolicy::Extend)
            .knots([(0.0, 1.0), (5.0, 0.85)])
            .build()
            .expect("curve");
        let mut ctx = MarketContext::new()
            .try_insert(curve)
            .expect("insert market curve");

        let token = ctx
            .apply_curve_bump_in_place(&CurveId::from("USD-OIS"), BumpSpec::parallel_bp(1.0))
            .expect("curve bump");
        let bumped_zero = ctx
            .get_discount("USD-OIS")
            .expect("bumped curve")
            .zero_rate(5.0, crate::math::Compounding::Continuous)
            .expect("bumped zero");

        ctx.revert_scratch_bump(token).expect("revert");
        let restored_zero = ctx
            .get_discount("USD-OIS")
            .expect("restored curve")
            .zero_rate(5.0, crate::math::Compounding::Continuous)
            .expect("restored zero");

        assert!(
            (bumped_zero - restored_zero).abs() > 1e-9,
            "bump should change the curve before restore"
        );
        assert!(
            (restored_zero - (-0.85f64.ln() / 5.0)).abs() < 1e-12,
            "restored curve should match original"
        );
    }

    #[test]
    fn failed_scratch_and_copy_bumps_leave_live_curves_unchanged() {
        let discount = DiscountCurve::builder("USD-OIS")
            .base_date(as_of())
            .day_count(crate::dates::DayCount::Act365F)
            .interp(InterpStyle::LogLinear)
            .extrapolation(crate::math::interp::ExtrapolationPolicy::Extend)
            .knots([(0.0, 1.0), (5.0, 0.85)])
            .build()
            .expect("discount curve");
        let forward = ForwardCurve::builder("USD-SOFR-3M")
            .base_date(as_of())
            .day_count(crate::dates::DayCount::Act360)
            .interp(InterpStyle::LogLinear)
            .extrapolation(crate::math::interp::ExtrapolationPolicy::Extend)
            .knots([(0.0, 1.0), (5.0, 0.84)])
            .build()
            .expect("forward curve");
        let mut scratch = MarketContext::new()
            .try_insert(discount)
            .expect("insert market curve")
            .try_insert(forward)
            .expect("insert market curve");
        let discount_before = scratch
            .get_discount("USD-OIS")
            .expect("discount")
            .zero_rate(5.0, crate::math::Compounding::Continuous)
            .expect("zero rate");
        let forward_before = scratch
            .get_forward("USD-SOFR-3M")
            .expect("forward")
            .df(5.0)
            .expect("factor");

        assert!(
            scratch
                .apply_curve_bump_in_place(
                    &CurveId::from("USD-SOFR-3M"),
                    BumpSpec::parallel_bp(f64::NAN),
                )
                .is_err()
        );
        assert_eq!(
            scratch
                .get_forward("USD-SOFR-3M")
                .expect("forward")
                .df(5.0)
                .expect("factor")
                .to_bits(),
            forward_before.to_bits()
        );

        assert!(
            scratch
                .bump([
                    MarketBump::DiscountCurve {
                        id: CurveId::from("USD-OIS"),
                        spec: BumpSpec::parallel_bp(1.0),
                    },
                    MarketBump::ForwardCurve {
                        id: CurveId::from("USD-SOFR-3M"),
                        spec: BumpSpec::parallel_bp(f64::INFINITY),
                    },
                ])
                .is_err()
        );
        assert_eq!(
            scratch
                .get_discount("USD-OIS")
                .expect("discount")
                .zero_rate(5.0, crate::math::Compounding::Continuous)
                .expect("zero rate")
                .to_bits(),
            discount_before.to_bits()
        );
        assert_eq!(
            scratch
                .get_forward("USD-SOFR-3M")
                .expect("forward")
                .df(5.0)
                .expect("factor")
                .to_bits(),
            forward_before.to_bits()
        );
    }

    #[test]
    fn missing_curve_bumps_report_the_first_missing_id_in_caller_order() {
        let discount = DiscountCurve::builder("USD-OIS")
            .base_date(as_of())
            .knots([(0.0, 1.0), (5.0, 0.85)])
            .day_count(crate::dates::DayCount::Act365F)
            .interp(crate::math::interp::InterpStyle::LogLinear)
            .extrapolation(crate::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .expect("discount curve");
        let market = MarketContext::new()
            .try_insert(discount)
            .expect("insert market curve");
        // Several missing ids in an order unrelated to their hash order; the
        // error must name the first one the caller passed, on every target.
        let ids = [
            "ZZ-MISSING",
            "AA-MISSING",
            "MM-MISSING",
            "QQ-MISSING",
            "BB-MISSING",
        ];
        let bumps = std::iter::once(MarketBump::DiscountCurve {
            id: CurveId::from("USD-OIS"),
            spec: BumpSpec::parallel_bp(1.0),
        })
        .chain(ids.iter().map(|id| MarketBump::DiscountCurve {
            id: CurveId::from(*id),
            spec: BumpSpec::parallel_bp(1.0),
        }));
        let err = market.bump(bumps).expect_err("missing curves must fail");
        assert!(
            err.to_string().contains("ZZ-MISSING"),
            "expected the first missing id, got {err}"
        );
    }

    #[test]
    fn namespace_local_vol_ids_coexist_and_typed_bumps_isolate_stores() {
        use crate::contract::LoadLimits;
        use crate::market_data::scalars::ScalarTimeSeries;
        use crate::market_data::surfaces::{FxDeltaVolSurface, SabrParameterData, VolCube};

        let surface = {
            use crate::market_data::surfaces::{
                VolGridOpts, VolInterpolationMode, VolQuoteType, VolSurfaceAxis,
            };
            VolSurface::from_grid(
                "SHARED-ID",
                &[1.0],
                &[100.0],
                &[0.2],
                VolGridOpts {
                    secondary_axis: VolSurfaceAxis::Strike,
                    quote_type: VolQuoteType::BlackLognormal,
                    interpolation_mode: VolInterpolationMode::Vol,
                },
                None,
            )
            .expect("surface")
        };
        let sabr = SabrParameterData::new(0.2, 0.5, 0.3, -0.2, None).expect("sabr");
        let cube = VolCube::from_grid("SHARED-ID", &[1.0], &[5.0], &[sabr], &[0.03]).expect("cube");
        let pair = crate::money::fx::CurrencyPair::try_new(
            crate::currency::Currency::EUR,
            crate::currency::Currency::USD,
        )
        .expect("valid pair");
        let fx_delta = FxDeltaVolSurface::builder("SHARED-ID", pair)
            .delta_convention(crate::market_data::surfaces::FxDeltaConvention::ForwardUnadjusted)
            .atm_convention(crate::market_data::surfaces::FxAtmConvention::DeltaNeutralStraddle)
            .butterfly_convention(crate::market_data::surfaces::FxButterflyConvention::Smile)
            .expiries(&[1.0])
            .atm_vols(&[0.09])
            .rr_25d(&[0.01])
            .bf_25d(&[0.005])
            .build()
            .expect("fx delta surface");
        let curve =
            DiscountCurve::flat("SHARED-ID", as_of(), 0.04, crate::dates::DayCount::Act365F)
                .expect("discount");
        let series = ScalarTimeSeries::new(
            "SHARED-ID",
            vec![(as_of(), 1.0)],
            None,
            crate::market_data::scalars::SeriesInterpolation::Step,
            crate::market_data::scalars::SeriesExtrapolation::Error,
        )
        .expect("scalar time series");

        let multi = MarketContext::new()
            .insert_surface(surface)
            .insert_vol_cube(cube)
            .insert_fx_delta_vol_surface(fx_delta)
            .try_insert(curve)
            .expect("curve collection is distinct")
            .insert_price("SHARED-ID", MarketScalar::unitless(1.0).expect("1.0"))
            .insert_series(series);
        assert!(multi.get_surface("SHARED-ID").is_ok());
        assert!(multi.get_vol_cube("SHARED-ID").is_ok());
        assert!(multi.get_fx_delta_vol_surface("SHARED-ID").is_ok());
        assert!(multi.get_discount("SHARED-ID").is_ok());
        assert!(multi.get_price("SHARED-ID").is_ok());
        assert!(multi.get_series("SHARED-ID").is_ok());

        // Typed bumps target only their declared store and leave sibling stores unchanged.
        let bumped = multi
            .bump([
                MarketBump::DiscountCurve {
                    id: CurveId::new("SHARED-ID"),
                    spec: BumpSpec::parallel_bp(10.0),
                },
                MarketBump::UnitlessScalar {
                    id: CurveId::new("SHARED-ID"),
                    spec: BumpSpec::multiplier(2.0),
                },
                MarketBump::VolSurface {
                    id: CurveId::new("SHARED-ID"),
                    spec: surface_spec(0.05),
                },
            ])
            .expect("typed bumps on shared id succeed cleanly");
        assert!(
            (bumped
                .get_discount("SHARED-ID")
                .expect("discount")
                .zero_rate(1.0, crate::math::Compounding::Continuous)
                .expect("zero rate")
                - 0.041)
                .abs()
                < 1e-6
        );
        assert_eq!(
            bumped
                .get_price("SHARED-ID")
                .expect("scalar")
                .get_unitless(),
            Some(2.0)
        );
        assert!((bumped.get_surface("SHARED-ID").expect("surface").vols()[0] - 0.25).abs() < 1e-12);
        assert!(bumped.get_vol_cube("SHARED-ID").is_ok());
        assert!(bumped.get_fx_delta_vol_surface("SHARED-ID").is_ok());
        assert_eq!(
            bumped
                .get_series("SHARED-ID")
                .expect("series")
                .get_value(as_of())
                .expect("value"),
            1.0
        );

        // Persisted state also permits the same ID across vol namespaces.
        let bytes = serde_json::to_vec(&multi).expect("serialize state");
        let (restored, _report) = MarketContext::from_state_slice(&bytes, &LoadLimits::default())
            .expect("namespace-local vol IDs deserialize cleanly");
        assert!(restored.get_surface("SHARED-ID").is_ok());
        assert!(restored.get_vol_cube("SHARED-ID").is_ok());
        assert!(restored.get_fx_delta_vol_surface("SHARED-ID").is_ok());
    }

    #[test]
    fn vol_cube_and_fx_delta_surface_bumps_fail_closed_naming_registry_kind() {
        use crate::market_data::surfaces::{
            FxAtmConvention, FxButterflyConvention, FxDeltaConvention, FxDeltaVolSurface,
            SabrParameterData, VolCube,
        };
        use crate::money::fx::CurrencyPair;

        let sabr = SabrParameterData::new(0.2, 0.5, 0.3, -0.2, None).expect("sabr");
        let cube = VolCube::from_grid("CUBE-VOL", &[1.0], &[5.0], &[sabr], &[0.03]).expect("cube");
        let pair = CurrencyPair::try_new(
            crate::currency::Currency::EUR,
            crate::currency::Currency::USD,
        )
        .expect("valid pair");
        let fx_delta = FxDeltaVolSurface::builder("FXD-VOL", pair)
            .delta_convention(FxDeltaConvention::ForwardUnadjusted)
            .atm_convention(FxAtmConvention::DeltaNeutralStraddle)
            .butterfly_convention(FxButterflyConvention::Smile)
            .expiries(&[1.0])
            .atm_vols(&[0.09])
            .rr_25d(&[0.01])
            .bf_25d(&[0.005])
            .build()
            .expect("fx delta surface");
        let mut ctx = MarketContext::new()
            .insert_vol_cube(cube)
            .insert_fx_delta_vol_surface(fx_delta);

        for (id, expected_kind) in [("CUBE-VOL", "VolCube"), ("FXD-VOL", "FxDeltaVolSurface")] {
            let Err(err_surface) = ctx.apply_surface_bump_in_place(id, surface_spec(0.01)) else {
                panic!("surface bump must fail closed");
            };
            assert!(err_surface.to_string().contains(expected_kind));

            let Err(err_point) =
                ctx.apply_surface_point_absolute_bump_in_place(id, 1.0, 0.03, 0.01)
            else {
                panic!("point bump must fail closed");
            };
            assert!(err_point.to_string().contains(expected_kind));

            let err_bucket = ctx
                .bump([MarketBump::VolBucketPct {
                    vol_surface_id: CurveId::new(id),
                    expiries: None,
                    secondary_values: None,
                    pct: 10.0,
                }])
                .expect_err("bucket bump must fail closed");
            assert!(err_bucket.to_string().contains(expected_kind));

            let err_curve = ctx
                .bump([MarketBump::VolSurface {
                    id: CurveId::new(id),
                    spec: surface_spec(0.01),
                }])
                .expect_err("curve/surface bump must fail closed");
            assert!(err_curve.to_string().contains(expected_kind));
        }
    }

    #[test]
    fn price_bump_fraction_rejects_non_positive_shocked_multipliers_atomically() {
        for invalid in [-1.0, -1.5] {
            let mut ctx = MarketContext::new()
                .insert_price("SPOT", MarketScalar::unitless(100.0).expect("100"));
            let Err(err) = ctx.apply_price_bump_fraction_in_place("SPOT", invalid) else {
                panic!("non-positive multiplier must fail");
            };
            assert!(err.to_string().contains("1.0 + bump_fraction > 0"));
            let Some(value) = ctx.get_price("SPOT").expect("spot").get_unitless() else {
                panic!("expected unitless spot");
            };
            assert_eq!(value.to_bits(), 100.0f64.to_bits());
        }
    }

    #[test]
    fn credit_index_rebind_and_negative_roll_forward_fail_atomically() {
        use crate::market_data::term_structures::{
            BaseCorrelationCurve, CreditIndexData, HazardCurve,
        };
        use std::sync::Arc;

        let hazard = HazardCurve::builder("CDX-HZD")
            .base_date(as_of())
            .recovery_rate(0.4)
            .knots([(1.0, 0.01), (5.0, 0.015)])
            .day_count(crate::dates::DayCount::Act365F)
            .build()
            .expect("hazard");
        let corr = BaseCorrelationCurve::builder("CDX-CORR")
            .knots([(3.0, 0.2), (10.0, 0.45)])
            .build()
            .expect("base correlation");
        let index = CreditIndexData::builder()
            .num_constituents(125)
            .recovery_rate(0.4)
            .index_credit_curve(Arc::new(hazard.clone()))
            .base_correlation_curve(Arc::new(corr.clone()))
            .build()
            .expect("credit index");

        let mut ctx = MarketContext::new()
            .try_insert(hazard)
            .expect("insert market curve")
            .try_insert(corr)
            .expect("insert market curve")
            .insert_credit_index("CDX-IG", index)
            .expect("insert credit index");

        let replacement_discount =
            DiscountCurve::flat("CDX-HZD", as_of(), 0.03, crate::dates::DayCount::Act365F)
                .expect("discount curve");
        let err = ctx
            .try_insert_mut(replacement_discount)
            .expect_err("replacing referenced hazard curve with discount curve must fail");
        assert!(err.to_string().contains("CDX-HZD"));
        assert!(ctx.get_hazard("CDX-HZD").is_ok());
        assert!(ctx.get_credit_index("CDX-IG").is_ok());

        let roll_err = ctx
            .condition(as_of() - time::Duration::days(1))
            .expect_err("conditioning before source_base_date must fail");
        assert!(!roll_err.to_string().is_empty());
    }
}
