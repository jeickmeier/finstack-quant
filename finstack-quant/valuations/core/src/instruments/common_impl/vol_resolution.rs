//! Shared helpers for resolving volatility from overrides or a vol surface.
//!
//! The canonical pattern across surface-driven pricers is to first check whether
//! `MarketQuoteOverrides::implied_volatility` is set and, if so, use it as a flat
//! σ across tenor and strike. Otherwise, look up the surface at `(t, strike)`
//! with clamping at the grid edges.
//!
//! This module centralises that pattern so new pricers can opt in with a single
//! call and existing pricers can be migrated away from ad-hoc `if let Some(iv)`
//! blocks.

use crate::instruments::pricing_overrides::MarketQuoteOverrides;
use crate::market::resolve_vol_source;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::surfaces::{VolQuoteType, VolSurfaceAxis};
use finstack_quant_core::money::fx::CurrencyPair;
use finstack_quant_core::{Error, Result};
use finstack_quant_models::volatility::{VolSource, VolatilityConvention};

/// Validate that a resolved volatility quote is finite and non-negative.
///
/// # Arguments
///
/// * `sigma` - Volatility quote to check in the caller's volatility unit.
pub fn validate_sigma(sigma: f64) -> Result<f64> {
    if sigma.is_finite() && sigma >= 0.0 {
        Ok(sigma)
    } else {
        Err(Error::Validation(format!(
            "volatility must be finite and non-negative, got {sigma}"
        )))
    }
}

/// Resolve unshifted Black volatility for equity/FX-style surface consumers.
///
/// Uses `overrides.implied_volatility` when present (still rejecting a displaced
/// surface with `VolQuoteType::ShiftedBlackLognormal`), otherwise queries the
/// volatility surface at `(t, strike)` with edge clamping.
///
/// # Arguments
///
/// * `overrides` - Instrument-level market quote overrides (explicit `implied_volatility` wins).
/// * `curves` - Market context holding volatility surfaces.
/// * `vol_surface_id` - Identifier of the target volatility surface.
/// * `t` - Time to expiry in years on the consumer's model clock.
/// * `strike` - Option strike in the surface's quoted strike units.
#[inline]
pub fn resolve_sigma_at(
    overrides: &MarketQuoteOverrides,
    curves: &MarketContext,
    vol_surface_id: &str,
    t: f64,
    strike: f64,
) -> Result<f64> {
    if let Some(sigma) = overrides.implied_volatility {
        if let Ok(VolSource::Surface(surface)) = resolve_vol_source(curves, vol_surface_id)
            && surface.quote_type() == VolQuoteType::ShiftedBlackLognormal
        {
            return Err(Error::Validation(
                "unshifted Black consumer cannot use a displaced volatility source".to_owned(),
            ));
        }
        return validate_sigma(sigma);
    }

    let source = resolve_vol_source(curves, vol_surface_id)?;
    match &source {
        VolSource::Cube(_) => {
            return Err(Error::Validation(format!(
                "SABR cube '{}' requires a finite positive underlying tenor",
                source.get_id()
            )));
        }
        VolSource::Surface(surface) => {
            surface.require_secondary_axis(VolSurfaceAxis::Strike)?;
        }
        VolSource::FxDelta(_) | VolSource::CubeExpirySlice(_) => {}
    }

    if source.get_convention(t, 0.0)? != VolatilityConvention::Lognormal {
        return Err(Error::Validation(
            "unshifted Black consumer cannot use a displaced volatility source".to_owned(),
        ));
    }

    validate_sigma(source.get_vol_clamped(t, 0.0, strike)?)
}

/// Qualified FX option volatility lookup parameters (`ADR 0045`, `CORE-MARKET-15`).
pub struct FxVolatilityRequest<'a> {
    /// Instrument-level quote overrides (`implied_volatility` short-circuits unused `FxDeltaVolSurface` lookup).
    pub overrides: &'a MarketQuoteOverrides,
    /// Market context holding volatility surfaces and delta-quoted FX surfaces.
    pub curves: &'a MarketContext,
    /// Identifier of the target `VolSurface` or `FxDeltaVolSurface`.
    pub vol_surface_id: &'a str,
    /// Ordered instrument FX currency pair (`base / quote`, strike and forward in `quote` per `base`).
    pub pair: CurrencyPair,
    /// Valuation anchor date (`as_of`).
    pub as_of: Date,
    /// Contractual option expiration calendar date (`expiry > as_of` for active pricing).
    pub expiry: Date,
    /// Strictly positive remaining option time in years on the instrument/model clock.
    pub model_time: f64,
    /// Finite non-negative FX strike in `quote` per `base` (`strike == 0.0` is supported for limiting payoffs on strike-clamped `VolSurface` grids; `FxDeltaVolSurface` requires `strike > 0.0`).
    pub strike: f64,
    /// Strictly positive delivery-aligned FX forward in `quote` per `base`.
    pub forward: f64,
}

fn resolve_clocked_fx_sigma(
    source: &VolSource,
    label: &str,
    clock: &finstack_quant_core::dates::VolatilityClock,
    request: &FxVolatilityRequest<'_>,
    check_lognormal: bool,
) -> Result<f64> {
    if clock.get_reference_date() != request.as_of {
        return Err(Error::Validation(format!(
            "{label} '{}' expiry_clock reference_date {} does not match valuation as_of {}",
            request.vol_surface_id,
            clock.get_reference_date(),
            request.as_of
        )));
    }
    let t_source = clock.expiry_time(request.expiry)?;
    if !t_source.is_finite() || t_source <= 0.0 {
        return Err(Error::Validation(format!(
            "{label} '{}' expiry_clock mapped {} -> {} to non-positive time {t_source}",
            request.vol_surface_id, request.as_of, request.expiry
        )));
    }
    if check_lognormal && source.get_convention(t_source, 0.0)? != VolatilityConvention::Lognormal {
        return Err(Error::Validation(
            "FX Black pricing requires unshifted lognormal volatility".to_owned(),
        ));
    }
    let sigma_source =
        validate_sigma(source.get_fx_vol_clamped(t_source, request.strike, request.forward)?)?;
    if sigma_source == 0.0 || t_source.total_cmp(&request.model_time).is_eq() {
        Ok(sigma_source)
    } else {
        validate_sigma(sigma_source * (t_source / request.model_time).sqrt())
    }
}

/// Resolve unshifted Black volatility for FX consumers using an explicit currency pair,
/// source calibration clock, and delivery-aligned forward rate.
///
/// # Arguments
///
/// * `request` - Qualified FX volatility lookup parameters specifying the market context,
///   surface identifier, currency pair, valuation/expiry dates, model clock time, strike,
///   and delivery-aligned forward rate.
#[inline]
pub fn resolve_fx_sigma(request: FxVolatilityRequest<'_>) -> Result<f64> {
    if let Some(sigma) = request.overrides.implied_volatility {
        if let Ok(VolSource::Surface(surface)) =
            resolve_vol_source(request.curves, request.vol_surface_id)
            && surface.quote_type() == VolQuoteType::ShiftedBlackLognormal
        {
            return Err(Error::Validation(
                "FX Black pricing requires unshifted lognormal volatility, not ShiftedBlackLognormal"
                    .to_owned(),
            ));
        }
        return validate_sigma(sigma);
    }

    if !request.model_time.is_finite() || request.model_time <= 0.0 {
        return Err(Error::Validation(format!(
            "FX model time must be finite and positive, got {}",
            request.model_time
        )));
    }
    if !request.strike.is_finite() || request.strike < 0.0 {
        return Err(Error::Validation(format!(
            "FX strike must be finite and non-negative, got {}",
            request.strike
        )));
    }
    if !request.forward.is_finite() || request.forward <= 0.0 {
        return Err(Error::Validation(format!(
            "FX delivery forward must be finite and positive, got {}",
            request.forward
        )));
    }

    let source = resolve_vol_source(request.curves, request.vol_surface_id)?;
    match &source {
        VolSource::FxDelta(surface) => {
            if request.strike <= 0.0 {
                return Err(Error::Validation(format!(
                    "FX delta surface '{}' requires a strictly positive strike, got {}",
                    request.vol_surface_id, request.strike
                )));
            }
            if surface.pair() != request.pair {
                return Err(Error::Validation(format!(
                    "FX delta surface '{}' has currency pair {}, expected {}",
                    request.vol_surface_id,
                    surface.pair(),
                    request.pair
                )));
            }
            let clock = surface.expiry_clock().ok_or_else(|| {
                Error::Validation(format!(
                    "FX delta surface '{}' requires an explicit expiry_clock for dated valuation",
                    request.vol_surface_id
                ))
            })?;
            resolve_clocked_fx_sigma(&source, "FX delta surface", clock, &request, false)
        }
        VolSource::Surface(surface) => {
            surface.require_secondary_axis(VolSurfaceAxis::Strike)?;
            if let Some(clock) = surface.expiry_clock() {
                resolve_clocked_fx_sigma(&source, "volatility surface", clock, &request, true)
            } else {
                if source.get_convention(request.model_time, 0.0)?
                    != VolatilityConvention::Lognormal
                {
                    return Err(Error::Validation(
                        "FX Black pricing requires unshifted lognormal volatility".to_owned(),
                    ));
                }
                validate_sigma(source.get_fx_vol_clamped(
                    request.model_time,
                    request.strike,
                    request.forward,
                )?)
            }
        }
        VolSource::Cube(_) | VolSource::CubeExpirySlice(_) => validate_sigma(
            source.get_fx_vol_clamped(request.model_time, request.strike, request.forward)?,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{DayCount, DayCountContextState, VolatilityClock};
    use finstack_quant_core::market_data::surfaces::{
        FxAtmConvention, FxButterflyConvention, FxDeltaConvention, FxDeltaVolSurface,
        VolInterpolationMode, VolSurface,
    };
    use time::macros::date;

    #[test]
    fn strike_only_resolver_rejects_black_tenor_surface() {
        let surface = VolSurface::builder("TENOR-VOL")
            .expiries(&[1.0])
            .secondary_values(&[2.0, 10.0])
            .secondary_axis(VolSurfaceAxis::Tenor)
            .quote_type(VolQuoteType::BlackLognormal)
            .interpolation_mode(VolInterpolationMode::Vol)
            .row(&[0.2, 0.4])
            .build()
            .expect("surface");
        let market = MarketContext::new().insert_surface(surface);
        let error = resolve_sigma_at(
            &MarketQuoteOverrides::default(),
            &market,
            "TENOR-VOL",
            1.0,
            100.0,
        )
        .expect_err("a strike-only API cannot select an underlying tenor");
        assert!(error.to_string().contains("secondary axis"));
        let overrides = MarketQuoteOverrides {
            implied_volatility: Some(0.31),
            ..Default::default()
        };
        assert_eq!(
            resolve_sigma_at(&overrides, &market, "TENOR-VOL", 1.0, 100.0).unwrap(),
            0.31
        );
    }

    #[test]
    fn strike_only_resolver_does_not_clamp_a_missing_sabr_tenor() {
        use finstack_quant_core::market_data::surfaces::{SabrParameterData, VolCube};

        let node = SabrParameterData::new(0.2, 1.0, 0.0, -0.25, Some(0.02)).expect("SABR node");
        let cube = VolCube::from_grid("CUBE", &[1.0], &[5.0], &[node], &[0.03]).expect("cube");
        let market = MarketContext::new().insert_vol_cube(cube);
        let error = resolve_sigma_at(&MarketQuoteOverrides::default(), &market, "CUBE", 1.0, 0.03)
            .expect_err("a strike-only option cannot query a swaption tenor");
        assert!(
            error.to_string().contains("positive underlying tenor"),
            "{error}"
        );

        let scalar_override = MarketQuoteOverrides {
            implied_volatility: Some(0.2),
            ..Default::default()
        };
        assert_eq!(
            resolve_sigma_at(&scalar_override, &market, "CUBE", 1.0, 0.03)
                .expect("explicit strike-only quote"),
            0.2
        );
    }

    #[test]
    fn explicit_implied_volatility_wins_without_surface_lookup() {
        let overrides = MarketQuoteOverrides {
            implied_volatility: Some(0.31),
            ..Default::default()
        };

        let sigma = resolve_sigma_at(&overrides, &MarketContext::new(), "MISSING-VOL", 1.0, 100.0)
            .expect("explicit vol should not require a surface");

        assert_eq!(sigma, 0.31);
    }

    #[test]
    fn missing_implied_volatility_interpolates_from_surface() {
        let surface = VolSurface::builder("EQ-VOL")
            .expiries(&[1.0])
            .secondary_values(&[90.0, 110.0])
            .secondary_axis(VolSurfaceAxis::Strike)
            .quote_type(VolQuoteType::BlackLognormal)
            .interpolation_mode(VolInterpolationMode::Vol)
            .row(&[0.20, 0.30])
            .build()
            .expect("surface");
        let market = MarketContext::new().insert_surface(surface);

        let sigma = resolve_sigma_at(
            &MarketQuoteOverrides::default(),
            &market,
            "EQ-VOL",
            1.0,
            100.0,
        )
        .expect("surface vol should resolve");

        assert!(
            (sigma - 0.25).abs() < 1e-12,
            "expected interpolated surface vol, got {sigma}"
        );
    }

    fn fx_pair() -> CurrencyPair {
        CurrencyPair::try_new(Currency::EUR, Currency::USD).expect("valid pair")
    }

    fn flat_fx_delta_surface(clock: Option<VolatilityClock>) -> FxDeltaVolSurface {
        let mut builder = FxDeltaVolSurface::builder("EURUSD-DELTA", fx_pair())
            .delta_convention(FxDeltaConvention::ForwardUnadjusted)
            .atm_convention(FxAtmConvention::DeltaNeutralStraddle)
            .butterfly_convention(FxButterflyConvention::Smile)
            .expiries(&[0.05, 0.25, 1.0])
            .atm_vols(&[0.20, 0.20, 0.20])
            .rr_25d(&[0.0, 0.0, 0.0])
            .bf_25d(&[0.0, 0.0, 0.0]);
        if let Some(c) = clock {
            builder = builder.expiry_clock(c);
        }
        builder.build().expect("valid fx delta surface")
    }

    #[test]
    fn fx_sigma_requires_clock_matching_as_of_and_matching_pair() {
        let as_of = date!(2025 - 01 - 02);
        let expiry = date!(2025 - 02 - 01); // 30 days
        let missing_clock = flat_fx_delta_surface(None);
        let market_missing = MarketContext::new().insert_fx_delta_vol_surface(missing_clock);
        assert!(
            resolve_fx_sigma(FxVolatilityRequest {
                overrides: &MarketQuoteOverrides::default(),
                curves: &market_missing,
                vol_surface_id: "EURUSD-DELTA",
                pair: fx_pair(),
                as_of,
                expiry,
                model_time: 30.0 / 365.0,
                strike: 1.10,
                forward: 1.10,
            })
            .is_err()
        );

        let wrong_ref_clock = VolatilityClock::try_new(
            date!(2025 - 01 - 03),
            DayCount::Act365F,
            DayCountContextState::default(),
        )
        .expect("clock");
        let stale_surface = flat_fx_delta_surface(Some(wrong_ref_clock));
        let market_stale = MarketContext::new().insert_fx_delta_vol_surface(stale_surface);
        assert!(
            resolve_fx_sigma(FxVolatilityRequest {
                overrides: &MarketQuoteOverrides::default(),
                curves: &market_stale,
                vol_surface_id: "EURUSD-DELTA",
                pair: fx_pair(),
                as_of,
                expiry,
                model_time: 30.0 / 365.0,
                strike: 1.10,
                forward: 1.10,
            })
            .is_err()
        );

        let good_clock =
            VolatilityClock::try_new(as_of, DayCount::Act365F, DayCountContextState::default())
                .expect("clock");
        let reversed_surface = FxDeltaVolSurface::builder(
            "EURUSD-DELTA",
            CurrencyPair::try_new(Currency::USD, Currency::EUR).expect("pair"),
        )
        .delta_convention(FxDeltaConvention::ForwardUnadjusted)
        .atm_convention(FxAtmConvention::DeltaNeutralStraddle)
        .butterfly_convention(FxButterflyConvention::Smile)
        .expiries(&[0.05, 0.25, 1.0])
        .atm_vols(&[0.20, 0.20, 0.20])
        .rr_25d(&[0.0, 0.0, 0.0])
        .bf_25d(&[0.0, 0.0, 0.0])
        .expiry_clock(good_clock)
        .build()
        .expect("reversed");
        let market_reversed = MarketContext::new().insert_fx_delta_vol_surface(reversed_surface);
        assert!(
            resolve_fx_sigma(FxVolatilityRequest {
                overrides: &MarketQuoteOverrides::default(),
                curves: &market_reversed,
                vol_surface_id: "EURUSD-DELTA",
                pair: fx_pair(),
                as_of,
                expiry,
                model_time: 30.0 / 365.0,
                strike: 1.10,
                forward: 1.10,
            })
            .is_err()
        );
    }

    #[test]
    fn fx_sigma_applies_variance_bridge_and_override_skips_unused_fx_delta_surface() {
        let as_of = date!(2025 - 01 - 02);
        let expiry = date!(2025 - 02 - 01); // 30 days
        let clock =
            VolatilityClock::try_new(as_of, DayCount::Act365F, DayCountContextState::default())
                .expect("clock");
        let surface = flat_fx_delta_surface(Some(clock));
        let market = MarketContext::new().insert_fx_delta_vol_surface(surface);

        // Act/365F source sigma = 0.20 bridged onto Act/360 model clock.
        let model_time = 30.0 / 360.0;
        let bridged = resolve_fx_sigma(FxVolatilityRequest {
            overrides: &MarketQuoteOverrides::default(),
            curves: &market,
            vol_surface_id: "EURUSD-DELTA",
            pair: fx_pair(),
            as_of,
            expiry,
            model_time,
            strike: 1.10,
            forward: 1.10,
        })
        .expect("bridged sigma");
        let expected = 0.20 * (360.0_f64 / 365.0).sqrt();
        assert!((bridged - expected).abs() < 1e-12);

        // Explicit implied_volatility does not inspect an unused FxDeltaVolSurface (even one missing a clock).
        let clockless = flat_fx_delta_surface(None);
        let market_clockless = MarketContext::new().insert_fx_delta_vol_surface(clockless);
        let overrides = MarketQuoteOverrides {
            implied_volatility: Some(0.18),
            ..Default::default()
        };
        let overridden = resolve_fx_sigma(FxVolatilityRequest {
            overrides: &overrides,
            curves: &market_clockless,
            vol_surface_id: "EURUSD-DELTA",
            pair: CurrencyPair::try_new(Currency::USD, Currency::EUR).expect("pair"),
            as_of,
            expiry,
            model_time,
            strike: 1.10,
            forward: 1.10,
        })
        .expect("scalar override must not inspect unused FxDeltaVolSurface");
        assert_eq!(overridden, 0.18);
    }

    #[test]
    fn clocked_vol_surface_validates_as_of_and_applies_variance_bridge() {
        let as_of = date!(2025 - 01 - 02);
        let expiry = date!(2025 - 02 - 01); // 30 days
        let pair = CurrencyPair::try_new(Currency::USD, Currency::EUR).expect("pair");
        let model_time = 30.0 / 360.0;

        let wrong_clock = VolatilityClock::try_new(
            date!(2025 - 01 - 01),
            DayCount::Act365F,
            DayCountContextState::default(),
        )
        .expect("clock");
        let mismatched = VolSurface::builder("EURUSD-VOL")
            .expiries(&[30.0 / 365.0])
            .secondary_values(&[1.10])
            .secondary_axis(VolSurfaceAxis::Strike)
            .quote_type(VolQuoteType::BlackLognormal)
            .interpolation_mode(VolInterpolationMode::Vol)
            .expiry_clock(wrong_clock)
            .row(&[0.20])
            .build()
            .expect("surface");
        let err = resolve_fx_sigma(FxVolatilityRequest {
            overrides: &MarketQuoteOverrides::default(),
            curves: &MarketContext::new().insert_surface(mismatched),
            vol_surface_id: "EURUSD-VOL",
            pair,
            as_of,
            expiry,
            model_time,
            strike: 1.10,
            forward: 1.10,
        })
        .expect_err("clock reference_date mismatch must fail");
        assert!(err.to_string().contains("reference_date"));

        let matching_clock =
            VolatilityClock::try_new(as_of, DayCount::Act365F, DayCountContextState::default())
                .expect("clock");
        let clocked = VolSurface::builder("EURUSD-VOL")
            .expiries(&[30.0 / 365.0])
            .secondary_values(&[1.10])
            .secondary_axis(VolSurfaceAxis::Strike)
            .quote_type(VolQuoteType::BlackLognormal)
            .interpolation_mode(VolInterpolationMode::Vol)
            .expiry_clock(matching_clock)
            .row(&[0.20])
            .build()
            .expect("surface");
        let bridged = resolve_fx_sigma(FxVolatilityRequest {
            overrides: &MarketQuoteOverrides::default(),
            curves: &MarketContext::new().insert_surface(clocked),
            vol_surface_id: "EURUSD-VOL",
            pair,
            as_of,
            expiry,
            model_time,
            strike: 1.10,
            forward: 1.10,
        })
        .expect("variance-bridged surface vol");
        let expected = 0.20 * (360.0_f64 / 365.0).sqrt();
        assert!((bridged - expected).abs() < 1e-12);
    }
}
