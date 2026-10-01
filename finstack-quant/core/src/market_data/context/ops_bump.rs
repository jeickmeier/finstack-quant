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
    ///
    /// Money arithmetic preserves the stored Decimal amount. Invalid inputs,
    /// non-finite scalar results, and Decimal overflow leave the context unchanged.
    ///
    /// # Arguments
    ///
    /// * `price_id` - Identifier of the scalar price to shock.
    /// * `bump_pct` - Finite relative price change as a decimal; `0.01` adds 1%.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing price, a non-finite input or result, or
    /// a Money amount that exceeds Decimal's representable range after the shock.
    pub fn apply_price_bump_pct_in_place(
        &mut self,
        price_id: &str,
        bump_pct: f64,
    ) -> Result<ContextScratchBump> {
        if !bump_pct.is_finite() {
            return Err(crate::Error::Validation(format!(
                "price bump percentage must be finite, got {bump_pct}"
            )));
        }
        let key = CurveId::from(price_id);
        let current = self.prices.get(price_id).cloned().ok_or_else(|| {
            crate::error::InputError::NotFound {
                id: price_id.to_string(),
            }
        })?;
        let bumped = match current {
            crate::market_data::scalars::MarketScalar::Unitless(v) => {
                let bumped = v * (1.0 + bump_pct);
                if !bumped.is_finite() {
                    return Err(crate::Error::Validation(
                        "price bump must produce a finite scalar".into(),
                    ));
                }
                crate::market_data::scalars::MarketScalar::Unitless(bumped)
            }
            crate::market_data::scalars::MarketScalar::Price(m) => {
                crate::market_data::scalars::MarketScalar::Price(m.checked_mul_f64(1.0 + bump_pct)?)
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
        vol_surface_id: &str,
        spec: BumpSpec,
    ) -> Result<ContextScratchBump> {
        spec.validate_finite()?;
        let key = CurveId::from(vol_surface_id);
        let previous = self.surfaces.get(vol_surface_id).cloned().ok_or_else(|| {
            crate::error::InputError::NotFound {
                id: vol_surface_id.to_string(),
            }
        })?;
        let bumped = previous.apply_bump(spec)?;
        Arc::make_mut(&mut self.surfaces).insert(key.clone(), Arc::new(bumped));
        Ok(ContextScratchBump::Surface { id: key, previous })
    }
    /// Apply an absolute volatility bump to one surface point in place.
    ///
    /// # Arguments
    ///
    /// * `vol_surface_id` - Identifier of the volatility surface to mutate.
    /// * `expiry` - Expiry in years used to select the nearest grid row.
    /// * `strike` - Strike coordinate used to select the nearest grid column.
    /// * `bump_abs` - Absolute volatility change in decimal units.
    pub fn apply_surface_point_absolute_bump_in_place(
        &mut self,
        vol_surface_id: &str,
        expiry: f64,
        strike: f64,
        bump_abs: f64,
    ) -> Result<ContextScratchBump> {
        if !(expiry.is_finite() && strike.is_finite() && bump_abs.is_finite()) {
            return Err(crate::Error::Validation(format!(
                "absolute surface point bump inputs must be finite, got expiry={expiry}, \
                 strike={strike}, bump_abs={bump_abs}"
            )));
        }
        let key = CurveId::from(vol_surface_id);
        let surface = Arc::make_mut(&mut self.surfaces)
            .get_mut(vol_surface_id)
            .ok_or_else(|| crate::error::InputError::NotFound {
                id: vol_surface_id.to_string(),
            })?;
        let original_vol =
            Arc::make_mut(surface).bump_point_absolute_in_place(expiry, strike, bump_abs)?;
        Ok(ContextScratchBump::SurfacePoint {
            id: key,
            expiry,
            strike,
            original_vol,
        })
    }

    /// Apply a curve bump in place and return a token that restores the
    /// original curve and any credit indices that were rebound.
    pub fn apply_curve_bump_in_place(
        &mut self,
        curve_id: &CurveId,
        spec: BumpSpec,
    ) -> Result<ContextScratchBump> {
        spec.validate_finite()?;
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
            ContextScratchBump::SurfacePoint {
                id,
                expiry,
                strike,
                original_vol,
            } => {
                let surface = Arc::make_mut(&mut self.surfaces)
                    .get_mut(id.as_str())
                    .ok_or_else(|| crate::error::InputError::NotFound { id: id.to_string() })?;
                Arc::make_mut(surface).unbump_point_in_place(expiry, strike, original_vol)?;
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
    ///
    /// # Arguments
    ///
    /// * `bumps` - Curve, FX, vol-surface, and base-correlation shocks applied
    ///   in iterator order on a cloned context. Repeated targets compose in
    ///   that order; for example, factors `1.1` and `1.2` multiply to `1.32`.
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
    ///
    /// # Arguments
    ///
    /// * `bumps` - Shocks applied in iterator order to a cloned context.
    ///   Repeated or overlapping targets compose sequentially. Failure returns
    ///   no partially bumped context and leaves the source unchanged.
    pub fn bump_observed<I>(&self, bumps: I) -> Result<(Self, ContextMutationInfo)>
    where
        I: IntoIterator<Item = MarketBump>,
    {
        use crate::error::InputError;

        let mut ctx = self.clone();
        let mut needs_credit_rebind = false;
        let mut processed_bumps = 0usize;

        for bump in bumps {
            processed_bumps += 1;
            match bump {
                MarketBump::Curve { id, spec } => {
                    spec.validate_finite()?;
                    needs_credit_rebind |= ctx.apply_curve_bump(id, spec)?;
                }
                MarketBump::FxPct {
                    base,
                    quote,
                    pct,
                    as_of,
                } => {
                    let fx = ctx.fx.as_ref().ok_or_else(|| InputError::NotFound {
                        id: "FX matrix".to_string(),
                    })?;
                    let bumped = fx.with_bumped_rate(base, quote, pct / 100.0, as_of)?;
                    ctx.fx = Some(Arc::new(bumped));
                }
                MarketBump::VolBucketPct {
                    vol_surface_id,
                    expiries,
                    strikes,
                    pct,
                } => {
                    // `None` filters mean "all buckets"; route through the same
                    // multiplicative `apply_bucket_bump` path as filtered bumps so
                    // semantics (vol × (1 + pct/100)) are identical with or without
                    // filters.
                    let surface = ctx.get_surface(vol_surface_id.as_str())?;
                    let bumped = surface
                        .apply_bucket_bump(expiries.as_deref(), strikes.as_deref(), pct)
                        .ok_or(InputError::DimensionMismatch)?;
                    ctx.insert_surface_mut(bumped);
                }
                MarketBump::BaseCorrBucketPts {
                    surface_id,
                    detachments,
                    points,
                } => {
                    let curve = ctx.get_base_correlation(surface_id.as_str())?;
                    let bumped = curve
                        .apply_bucket_bump(detachments.as_deref(), points)
                        .ok_or(InputError::DimensionMismatch)?;
                    needs_credit_rebind |= ctx.curve_affects_credit_indices(&surface_id);
                    Arc::make_mut(&mut ctx.curves)
                        .insert(surface_id, CurveStorage::BaseCorrelation(Arc::new(bumped)));
                }
            }
        }

        let invalidated_credit_indices = if needs_credit_rebind {
            ctx.rebind_all_credit_indices()
        } else {
            Vec::new()
        };
        let mutation_info = ContextMutationInfo {
            invalidated_credit_indices,
        };

        tracing::debug!(
            processed_bumps,
            needs_credit_rebind,
            invalidated_count = mutation_info.invalidated_credit_indices.len(),
            "applied MarketContext bumps"
        );

        Ok((ctx, mutation_info))
    }

    /// Apply one curve, surface, price, or series bump and report whether credit
    /// indices must be rebound after the ordered batch finishes.
    ///
    /// `CurveStorage::apply_bump_preserving_id` handles curve-specific bumping
    /// and ID preservation.
    fn apply_curve_bump(&mut self, curve_id: CurveId, bump_spec: BumpSpec) -> Result<bool> {
        let cid = curve_id.as_str();

        if let Some(storage) = Arc::make_mut(&mut self.curves).get_mut(cid) {
            storage.apply_bump_preserving_id(&curve_id, bump_spec)?;
            return Ok(self.curve_affects_credit_indices(&curve_id));
        }

        if let Some(original) = self.surfaces.get(cid).cloned() {
            let bumped = original.apply_bump(bump_spec)?;
            Arc::make_mut(&mut self.surfaces).insert(curve_id, Arc::new(bumped));
            return Ok(false);
        }

        if let Some(original) = self.prices.get(cid).cloned() {
            let bumped = original.apply_bump(bump_spec)?;
            Arc::make_mut(&mut self.prices).insert(curve_id, bumped);
            return Ok(false);
        }

        if let Some(original) = self.series.get(cid).cloned() {
            let bumped = original.apply_bump(bump_spec)?;
            Arc::make_mut(&mut self.series).insert(curve_id, bumped);
            return Ok(false);
        }

        Err(crate::error::InputError::NotFound {
            id: cid.to_string(),
        }
        .into())
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
    use crate::math::interp::{ExtrapolationPolicy, InterpStyle};
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
        let context = MarketContext::new().insert_price("SPOT", MarketScalar::Unitless(100.0));
        let bumped = context
            .bump([1.1, 1.2].map(|factor| MarketBump::Curve {
                id: CurveId::new("SPOT"),
                spec: BumpSpec::multiplier(factor),
            }))
            .expect("ordered scalar bumps");
        let MarketScalar::Unitless(value) = bumped.get_price("SPOT").expect("spot") else {
            panic!("unitless spot");
        };
        assert!((*value - 132.0).abs() < 1e-12);
        assert!(matches!(
            context.get_price("SPOT"),
            Ok(MarketScalar::Unitless(100.0))
        ));

        let failed = context.bump([
            MarketBump::Curve {
                id: CurveId::new("SPOT"),
                spec: BumpSpec::multiplier(1.1),
            },
            MarketBump::Curve {
                id: CurveId::new("SPOT"),
                spec: BumpSpec::multiplier(f64::NAN),
            },
        ]);
        assert!(failed.is_err());
        assert!(matches!(
            context.get_price("SPOT"),
            Ok(MarketScalar::Unitless(100.0))
        ));
    }

    #[test]
    fn ordered_bumps_compose_mixed_bucket_and_parallel_shocks_to_one_curve() {
        let context = MarketContext::new()
            .insert(DiscountCurve::flat("USD", as_of(), 0.05).expect("flat discount curve"));
        let bumps = [
            BumpSpec::parallel_bp(10.0),
            BumpSpec::triangular_key_rate_bp(3.0, 5.0, 7.0, 20.0),
            BumpSpec::parallel_bp(-4.0),
        ]
        .map(|spec| MarketBump::Curve {
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
            let expected = original.df(time) * (-(0.0006_f64 + 0.002 * weight) * time).exp();
            assert!((batched.df(time) - expected).abs() < 1e-14);
            assert!((batched.df(time) - sequential.df(time)).abs() < 1e-14);
            assert!((original.df(time) - (-0.05 * time).exp()).abs() < 1e-14);
        }
    }

    #[test]
    fn ordered_bumps_preserve_mixed_surface_operations() {
        let surface = VolSurface::builder("VOL")
            .expiries(&[1.0, 2.0])
            .strikes(&[90.0, 100.0])
            .row(&[0.2, 0.2])
            .row(&[0.2, 0.2])
            .build()
            .expect("surface");
        let context = MarketContext::new().insert_surface(surface);
        let additive = MarketBump::Curve {
            id: CurveId::new("VOL"),
            spec: surface_spec(0.1),
        };
        let relative = MarketBump::VolBucketPct {
            vol_surface_id: CurveId::new("VOL"),
            expiries: None,
            strikes: None,
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
    fn projection_bump_preserves_published_inflation_index_history() {
        let index = InflationIndex::new(
            "US-CPI",
            vec![
                (
                    Date::from_calendar_date(2024, Month::January, 1).expect("date"),
                    300.0,
                ),
                (
                    Date::from_calendar_date(2024, Month::May, 1).expect("date"),
                    302.0,
                ),
                (
                    Date::from_calendar_date(2025, Month::January, 1).expect("date"),
                    306.0,
                ),
            ],
            Currency::USD,
        )
        .expect("inflation index");
        let cutoff = Date::from_calendar_date(2024, Month::June, 1).expect("date");
        let bumped_index = index
            .apply_projection_bump(cutoff, BumpSpec::inflation_shift_pct(0.01))
            .expect("index projection bump");
        assert_eq!(
            bumped_index
                .value_on(Date::from_calendar_date(2024, Month::January, 1).expect("date"))
                .expect("published CPI"),
            300.0
        );
        assert_eq!(
            bumped_index
                .value_on(Date::from_calendar_date(2024, Month::May, 1).expect("date"))
                .expect("published CPI"),
            302.0
        );
        let end = Date::from_calendar_date(2025, Month::January, 1).expect("date");
        assert!(bumped_index.value_on(end).expect("CPI") > 306.0);
    }

    #[test]
    fn inflation_curve_bump_preserves_realized_same_id_index() {
        let index = InflationIndex::new(
            "US-CPI",
            vec![
                (as_of(), 300.0),
                (as_of() + time::Duration::days(365), 306.0),
            ],
            Currency::USD,
        )
        .expect("inflation index");
        let curve = InflationCurve::builder("US-CPI")
            .base_date(as_of())
            .base_cpi(300.0)
            .knots([(0.0, 300.0), (1.0, 306.0)])
            .build()
            .expect("inflation curve");
        let context = MarketContext::new()
            .insert(curve)
            .insert_inflation_index("US-CPI", index);

        let bumped = context
            .bump([MarketBump::Curve {
                id: CurveId::new("US-CPI"),
                spec: BumpSpec::inflation_shift_pct(0.01),
            }])
            .expect("inflation market bump");
        assert!(
            bumped
                .get_inflation_curve("US-CPI")
                .expect("curve")
                .cpi(1.0)
                > 306.0
        );
        assert_eq!(
            bumped
                .get_inflation_index("US-CPI")
                .expect("index")
                .value_on(as_of() + time::Duration::days(365))
                .expect("CPI"),
            306.0
        );
    }

    #[test]
    fn inflation_curve_key_rate_bump_preserves_interp_and_extrapolation() {
        let curve = InflationCurve::builder("US-CPI")
            .base_date(as_of())
            .base_cpi(300.0)
            .knots([(0.0, 300.0), (1.0, 306.0), (2.0, 312.0)])
            .interp(InterpStyle::Linear)
            .extrapolation(ExtrapolationPolicy::FlatZero)
            .build()
            .expect("inflation curve");
        let bumped = MarketContext::new()
            .insert(curve)
            .bump([MarketBump::Curve {
                id: CurveId::new("US-CPI"),
                spec: BumpSpec::triangular_key_rate_bp(0.0, 1.0, 2.0, 10.0),
            }])
            .expect("inflation curve key-rate bump");

        let rebuilt = bumped.get_inflation_curve("US-CPI").expect("bumped curve");
        assert_eq!(rebuilt.interp_style(), InterpStyle::Linear);
        assert_eq!(rebuilt.extrapolation(), ExtrapolationPolicy::FlatZero);
        assert_eq!(rebuilt.id().as_str(), "US-CPI");
    }

    #[test]
    fn inflation_curve_bump_preserves_extrapolation_policy() {
        let curve = InflationCurve::builder("US-CPI")
            .base_date(as_of())
            .base_cpi(300.0)
            .knots([(0.0, 300.0), (1.0, 306.0)])
            .extrapolation(ExtrapolationPolicy::FlatZero)
            .build()
            .expect("inflation curve");
        let bumped = MarketContext::new()
            .insert(curve)
            .bump([MarketBump::Curve {
                id: CurveId::new("US-CPI"),
                spec: BumpSpec::inflation_shift_pct(0.01),
            }])
            .expect("inflation curve bump");

        assert_eq!(
            bumped
                .get_inflation_curve("US-CPI")
                .expect("bumped curve")
                .extrapolation(),
            ExtrapolationPolicy::FlatZero
        );
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
    fn failed_price_bump_is_atomic_for_non_finite_percentages() {
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut ctx = MarketContext::new().insert_price("SPOT", MarketScalar::Unitless(100.0));

            let Err(error) = ctx.apply_price_bump_pct_in_place("SPOT", invalid) else {
                panic!("non-finite price bump must fail");
            };

            assert!(error.to_string().contains("finite"));
            match ctx.get_price("SPOT").expect("unchanged spot") {
                MarketScalar::Unitless(value) => assert_eq!(value.to_bits(), 100.0f64.to_bits()),
                MarketScalar::Price(_) => panic!("expected unitless price"),
            }
        }
    }

    #[test]
    fn scratch_money_bump_preserves_decimal_precision_and_zero_identity() {
        let original = crate::money::Money::from_decimal_str("1.0000000000000001", Currency::USD)
            .expect("exact price");
        let mut ctx = MarketContext::new().insert_price("SPOT", MarketScalar::Price(original));
        for (percentage, expected) in [(0.0, "1.0000000000000001"), (0.01, "1.010000000000000101")]
        {
            let token = ctx
                .apply_price_bump_pct_in_place("SPOT", percentage)
                .expect("checked bump");
            let MarketScalar::Price(actual) = ctx.get_price("SPOT").expect("price") else {
                panic!("expected Money price");
            };
            assert_eq!(
                actual.amount_decimal(),
                crate::money::Money::from_decimal_str(expected, Currency::USD)
                    .expect("expected exact price")
                    .amount_decimal()
            );
            ctx.revert_scratch_bump(token).expect("restore");
            let MarketScalar::Price(restored) = ctx.get_price("SPOT").expect("price") else {
                panic!("expected Money price");
            };
            assert_eq!(restored.amount_decimal(), original.amount_decimal());
        }
    }

    #[test]
    fn overflowing_scratch_price_bumps_leave_the_original_price_unchanged() {
        for original in [
            MarketScalar::Unitless(f64::MAX),
            MarketScalar::Price(
                crate::money::Money::from_decimal_str(
                    "10000000000000000000000000000",
                    Currency::USD,
                )
                .expect("representable price"),
            ),
        ] {
            let mut ctx = MarketContext::new().insert_price("SPOT", original.clone());
            assert!(ctx.apply_price_bump_pct_in_place("SPOT", 9.0).is_err());
            match (ctx.get_price("SPOT").expect("price"), &original) {
                (MarketScalar::Unitless(actual), MarketScalar::Unitless(expected)) => {
                    assert_eq!(actual.to_bits(), expected.to_bits());
                }
                (MarketScalar::Price(actual), MarketScalar::Price(expected)) => {
                    assert_eq!(actual.amount_decimal(), expected.amount_decimal());
                }
                _ => panic!("price variant changed after a failed bump"),
            }
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
        assert!((bumped.vols()[0] - 0.21).abs() < 1e-12);

        ctx.revert_scratch_bump(token).expect("revert");
        let restored = ctx.get_surface("VOL").expect("restored surface");
        assert!((restored.vols()[0] - 0.2).abs() < 1e-12);
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

    #[test]
    fn failed_scratch_and_copy_bumps_leave_live_curves_unchanged() {
        let discount = DiscountCurve::builder("USD-OIS")
            .base_date(as_of())
            .knots([(0.0, 1.0), (5.0, 0.85)])
            .build()
            .expect("discount curve");
        let forward = ForwardCurve::builder("USD-SOFR-3M", 0.25)
            .base_date(as_of())
            .knots([(0.0, 0.03), (5.0, 0.04)])
            .build()
            .expect("forward curve");
        let mut scratch = MarketContext::new().insert(discount).insert(forward);
        let discount_before = scratch.get_discount("USD-OIS").expect("discount").zero(5.0);
        let forward_before = scratch
            .get_forward("USD-SOFR-3M")
            .expect("forward")
            .rate(5.0);

        assert!(scratch
            .apply_curve_bump_in_place(
                &CurveId::from("USD-SOFR-3M"),
                BumpSpec::parallel_bp(f64::NAN),
            )
            .is_err());
        assert_eq!(
            scratch
                .get_forward("USD-SOFR-3M")
                .expect("forward")
                .rate(5.0)
                .to_bits(),
            forward_before.to_bits()
        );

        assert!(scratch
            .bump([
                MarketBump::Curve {
                    id: CurveId::from("USD-OIS"),
                    spec: BumpSpec::parallel_bp(1.0),
                },
                MarketBump::Curve {
                    id: CurveId::from("USD-SOFR-3M"),
                    spec: BumpSpec::parallel_bp(f64::INFINITY),
                },
            ])
            .is_err());
        assert_eq!(
            scratch
                .get_discount("USD-OIS")
                .expect("discount")
                .zero(5.0)
                .to_bits(),
            discount_before.to_bits()
        );
        assert_eq!(
            scratch
                .get_forward("USD-SOFR-3M")
                .expect("forward")
                .rate(5.0)
                .to_bits(),
            forward_before.to_bits()
        );
    }
}
