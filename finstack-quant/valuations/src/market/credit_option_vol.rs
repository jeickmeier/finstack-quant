//! Select a credit volatility from a CDX/iTraxx option surface and convert it
//! into the additive hazard volatility the callable lattice consumes.
//!
//! Two conversions have to happen between an index option-vol surface and a
//! callable credit-risky bond, and both are easy to get silently wrong:
//!
//! 1. **Coordinate.** A surface is queried at the strike's *native displayed*
//!    coordinate — a decimal spread such as `0.0325` for CDX IG and iTraxx, a
//!    clean price in percentage points such as `107.0` for CDX HY. Querying a
//!    HY surface at `1.07`, or at a spread-equivalent, silently returns the
//!    wrong grid point rather than failing.
//! 2. **Units.** Surface values are lognormal **forward-spread model**
//!    volatilities in every case, including on a price-quoted strike axis. A
//!    "price-quoted" surface does not hold price-point volatilities, and a
//!    fractional spread vol is not an additive hazard vol — see
//!    [`models::credit::market_anchored`](finstack_quant_models::credit::market_anchored).
//!
//! This module makes both explicit: it resolves a typed strike (directly, by
//! moneyness, or by delta), queries the surface strictly at the native
//! coordinate, and returns the hazard volatility together with every
//! intermediate quantity used to get there.
//!
//! # What this is not
//!
//! The conversion is a first-order local mapping, not a calibration: feeding
//! the resulting hazard volatility into the callable lattice will not exactly
//! reprice the index option it came from. The index-derived **fractional**
//! volatility is applied to the **target** curve's own reference hazard, so a
//! single-name bond is never anchored to the index's hazard level; any
//! issuer/index beta is a caller decision.

use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::surfaces::{VolQuoteType, VolSurfaceAxis};
use finstack_quant_core::types::CurveId;
use finstack_quant_core::{Error, Result};
use rust_decimal::Decimal;

use crate::instruments::credit_derivatives::cds_option::{
    CDSOption, CDSOptionStrike, CDSOptionStrikeKind,
};
use crate::instruments::OptionType;
use finstack_quant_models::credit::market_anchored::CreditVolatilityConversion;

/// How to pick the point on the option-volatility surface.
#[derive(Debug, Clone, Copy)]
pub enum CreditVolSelector {
    /// Use the option's own typed strike, queried at its native coordinate.
    Strike(CDSOptionStrike),
    /// Native strike divided by the canonical native ATM-forward coordinate.
    ///
    /// `1.0` is at-the-money. The ratio is taken in the strike's own native
    /// units, so `1.05` means 5% above the ATM forward *spread* on a
    /// spread-struck option and 5% above the ATM forward *clean price* on a
    /// price-struck one.
    Moneyness(f64),
    /// Solve for the strike carrying a given absolute delta.
    Delta {
        /// Target |Δ| in `(0, 1)`.
        absolute_delta: f64,
        /// Payer (`Call`) or receiver (`Put`) — the two carry opposite delta
        /// signs, so the sign alone does not identify the strike.
        option_type: OptionType,
    },
}

/// A request to resolve a hazard volatility for a target credit curve.
#[derive(Debug)]
pub struct CreditVolRequest<'a> {
    /// The index option supplying the expiry, underlying convention, index
    /// state, strike kind, and `vol_surface_id`.
    pub option: &'a CDSOption,
    /// How to pick the surface point.
    pub selector: CreditVolSelector,
    /// Hazard curve the resulting volatility will be applied to. This is the
    /// **target** instrument's curve, not the index's: the index supplies a
    /// fractional volatility, which is anchored on this curve's own hazard.
    pub target_hazard_curve_id: &'a CurveId,
    /// Date bounding the horizon over which the target curve's reference
    /// hazard is averaged.
    pub hazard_horizon: Date,
}

/// The resolved volatility with every quantity used to derive it.
#[derive(Debug, Clone, PartialEq)]
pub struct HazardVolatilityQuote {
    /// Strike the selector resolved to.
    pub resolved_strike: CDSOptionStrike,
    /// Native coordinate the surface was queried at — a decimal spread or a
    /// percentage clean price, matching the strike kind.
    pub surface_coordinate: f64,
    /// Lognormal forward-spread model volatility read from the surface.
    pub model_spread_volatility: f64,
    /// Conditional average hazard of the **target** curve over the horizon.
    pub reference_hazard: f64,
    /// Reference par spread implied by the credit triangle.
    pub reference_spread: f64,
    /// Additive hazard volatility for the callable lattice's
    /// `hazard_volatility` input.
    pub hazard_volatility: f64,
    /// Full conversion diagnostics.
    pub conversion: CreditVolatilityConversion,
}

/// Resolve a hazard volatility for the request's target curve.
///
/// # Arguments
///
/// * `request` - index option description, selector, horizon, and target
///   credit curve
/// * `market` - market context holding the vol surface and credit curves
/// * `as_of` - valuation date anchoring expiry and horizon year fractions
///
/// # Errors
///
/// Returns [`Error::Validation`] when the surface is missing, uses a
/// non-strike secondary axis or a non-lognormal quote type, the resolved
/// coordinate falls outside the surface grid, the selector is out of range,
/// a delta solve cannot bracket a root inside the surface's strike bounds, or
/// the target hazard curve does not support the requested horizon.
pub fn resolve_hazard_volatility(
    request: &CreditVolRequest<'_>,
    market: &MarketContext,
    as_of: Date,
) -> Result<HazardVolatilityQuote> {
    let option = request.option;
    let surface = market.get_surface(option.vol_surface_id.as_str())?;
    if surface.secondary_axis() != VolSurfaceAxis::Strike {
        return Err(Error::Validation(format!(
            "credit option-vol surface '{}' must use a strike secondary axis, \
             got {:?}",
            option.vol_surface_id,
            surface.secondary_axis()
        )));
    }
    // Stored values are lognormal forward-spread model vols on every strike
    // axis, price-quoted included. A provider "price vol" must be converted to
    // a premium and inverted through the full CDS-option pricer before it can
    // populate a surface; it is never inferable from the quote type alone.
    surface.require_quote_type(VolQuoteType::BlackLognormal)?;

    let expiry_time = option.time_to_expiry(as_of)?;
    let resolved_strike = resolve_strike(request, market, as_of, expiry_time)?;
    let surface_coordinate = resolved_strike.native_surface_coordinate()?;
    // Checked lookup: extrapolating a credit-vol surface silently is how a
    // deep-OTM strike ends up priced off the nearest grid edge.
    let model_spread_volatility = finstack_quant_models::volatility::get_surface_vol(
        &surface,
        expiry_time,
        surface_coordinate,
    )?;

    let conversion = target_curve_conversion(request, market, as_of, model_spread_volatility)?;

    Ok(HazardVolatilityQuote {
        resolved_strike,
        surface_coordinate,
        model_spread_volatility,
        reference_hazard: conversion.reference_hazard,
        reference_spread: conversion.reference_spread,
        hazard_volatility: conversion.hazard_volatility,
        conversion,
    })
}

/// Anchor the index-derived fractional volatility on the **target** curve.
fn target_curve_conversion(
    request: &CreditVolRequest<'_>,
    market: &MarketContext,
    as_of: Date,
    fractional_vol: f64,
) -> Result<CreditVolatilityConversion> {
    let hazard = market.get_hazard(request.target_hazard_curve_id.as_str())?;
    let day_count = hazard.day_count();
    let base = hazard.base_date();
    let t_start = day_count.signed_year_fraction(
        base,
        as_of,
        finstack_quant_core::dates::DayCountContext::default(),
    )?;
    let horizon = day_count.year_fraction(
        as_of,
        request.hazard_horizon,
        finstack_quant_core::dates::DayCountContext::default(),
    )?;
    if horizon <= 0.0 {
        return Err(Error::Validation(format!(
            "hazard_horizon {} must be after the valuation date {as_of}",
            request.hazard_horizon
        )));
    }
    CreditVolatilityConversion::from_survival_window(
        fractional_vol,
        hazard.sp(t_start),
        hazard.sp(t_start + horizon),
        horizon,
        hazard.recovery_rate(),
    )
}

/// Resolve the selector to a typed strike in native units.
fn resolve_strike(
    request: &CreditVolRequest<'_>,
    market: &MarketContext,
    as_of: Date,
    expiry_time: f64,
) -> Result<CDSOptionStrike> {
    match &request.selector {
        CreditVolSelector::Strike(strike) => Ok(*strike),
        CreditVolSelector::Moneyness(moneyness) => {
            if !moneyness.is_finite() || *moneyness <= 0.0 {
                return Err(Error::Validation(format!(
                    "moneyness must be finite and positive, got {moneyness}"
                )));
            }
            let atm = native_atm_coordinate(request.option, market, as_of)?;
            native_to_strike(request.option.strike.kind(), atm * moneyness)
        }
        CreditVolSelector::Delta {
            absolute_delta,
            option_type,
        } => solve_delta_strike(
            request.option,
            market,
            as_of,
            expiry_time,
            *absolute_delta,
            *option_type,
        ),
    }
}

/// Canonical native ATM-forward coordinate for the option's strike kind.
///
/// Spread strikes use the Bloomberg CDSO displayed ATM forward spread in
/// decimal; clean-price strikes use the payer/receiver parity strike in
/// percentage price points, which comes from the same payoff the quadrature
/// integrates rather than a separate price approximation.
fn native_atm_coordinate(option: &CDSOption, market: &MarketContext, as_of: Date) -> Result<f64> {
    use crate::instruments::credit_derivatives::cds_option::bloomberg_quadrature::ForwardCdsContext;
    use crate::instruments::credit_derivatives::cds_option::pricer::synthetic_underlying_cds;

    let cds = synthetic_underlying_cds(option, as_of)?;
    let disc = market.get_discount(&option.discount_curve_id)?;
    let surv = market.get_hazard(&option.credit_curve_id)?;
    let ctx = ForwardCdsContext::build(option, disc.as_ref(), surv.as_ref(), &cds, as_of, 0.0)?;
    match option.strike.kind() {
        CDSOptionStrikeKind::Spread => Ok(ctx.forward_par_spread),
        CDSOptionStrikeKind::CleanPricePct => ctx.native_atm_forward_clean_price_pct(),
    }
}

/// Wrap a native coordinate back into a typed strike of the given kind.
fn native_to_strike(kind: CDSOptionStrikeKind, native: f64) -> Result<CDSOptionStrike> {
    let decimal = Decimal::try_from(native).map_err(|e| {
        Error::Validation(format!(
            "cannot represent resolved strike coordinate {native} as a decimal: {e}"
        ))
    })?;
    Ok(match kind {
        CDSOptionStrikeKind::Spread => CDSOptionStrike::Spread(decimal),
        CDSOptionStrikeKind::CleanPricePct => CDSOptionStrike::CleanPricePct(decimal),
    })
}

/// Root-solve the native strike carrying the requested absolute delta.
///
/// Uses sticky-strike local volatility: each candidate strike is priced with
/// the surface volatility **at that strike**, which is what a desk means by a
/// delta-referenced quote. Spread strikes use the closed-form Black spread
/// delta; clean-price strikes use the curve-reprice price-strike delta, since
/// a clean price is not a valid argument to the Black `d₁`.
fn solve_delta_strike(
    option: &CDSOption,
    market: &MarketContext,
    as_of: Date,
    expiry_time: f64,
    absolute_delta: f64,
    option_type: OptionType,
) -> Result<CDSOptionStrike> {
    if !absolute_delta.is_finite() || !(0.0..1.0).contains(&absolute_delta) || absolute_delta == 0.0
    {
        return Err(Error::Validation(format!(
            "absolute_delta must be finite and in (0, 1), got {absolute_delta}"
        )));
    }
    let surface = market.get_surface(option.vol_surface_id.as_str())?;
    let strikes = surface.strikes();
    let (Some(&lo), Some(&hi)) = (strikes.first(), strikes.last()) else {
        return Err(Error::Validation(format!(
            "credit option-vol surface '{}' has no strike grid to solve within",
            option.vol_surface_id
        )));
    };

    let kind = option.strike.kind();
    // |Δ| at a candidate native strike, priced with that strike's own vol.
    let abs_delta_at = |native: f64| -> Result<f64> {
        let strike = native_to_strike(kind, native)?;
        let mut probe = option.clone();
        probe.strike = strike;
        probe.option_type = option_type;
        let sigma =
            finstack_quant_models::volatility::get_surface_vol(&surface, expiry_time, native)?;
        probe
            .instrument_pricing_overrides
            .market_quotes
            .implied_volatility = Some(sigma);
        Ok(probe.delta(market, as_of)?.abs())
    };

    let (delta_lo, delta_hi) = (abs_delta_at(lo)?, abs_delta_at(hi)?);
    // |Δ| is monotone in the strike within a kind, but the direction differs
    // by convention, so bracket on the observed endpoints rather than assuming.
    let brackets = (delta_lo - absolute_delta) * (delta_hi - absolute_delta) <= 0.0;
    if !brackets {
        return Err(Error::Validation(format!(
            "|Δ| = {absolute_delta} is outside the range this surface can \
             express: the strike grid spans [{lo}, {hi}] where |Δ| runs from \
             {delta_lo:.4} to {delta_hi:.4}. Widen the surface's strike grid or \
             request a delta inside that range."
        )));
    }

    let (mut a, mut b) = (lo, hi);
    let (mut fa, _fb) = (delta_lo - absolute_delta, delta_hi - absolute_delta);
    for _ in 0..100 {
        let mid = 0.5 * (a + b);
        let fm = abs_delta_at(mid)? - absolute_delta;
        if fm == 0.0 || (b - a).abs() <= 1e-12 * (1.0 + b.abs()) {
            return native_to_strike(kind, mid);
        }
        if (fa < 0.0) == (fm < 0.0) {
            a = mid;
            fa = fm;
        } else {
            b = mid;
        }
    }
    native_to_strike(kind, 0.5 * (a + b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::credit_derivatives::cds_option::CDSOptionParams;
    use crate::instruments::CreditParams;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::DateExt;
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_core::market_data::term_structures::{
        DiscountCurve, HazardCurve, ParInterp,
    };
    use finstack_quant_core::money::Money;
    use time::macros::date;

    fn as_of() -> Date {
        date!(2025 - 01 - 01)
    }

    fn discount() -> DiscountCurve {
        DiscountCurve::builder("USD-OIS")
            .base_date(as_of())
            .knots([
                (0.0, 1.0),
                (1.0, (-0.03_f64).exp()),
                (5.0, (-0.15_f64).exp()),
                (10.0, (-0.30_f64).exp()),
            ])
            .build()
            .expect("discount curve")
    }

    fn hazard(id: &str, rate: f64) -> HazardCurve {
        HazardCurve::builder(id)
            .base_date(as_of())
            .recovery_rate(0.4)
            .knots([(0.0, rate), (5.0, rate), (10.0, rate)])
            .par_interp(ParInterp::Linear)
            .build()
            .expect("hazard curve")
    }

    /// Spread-coordinate surface: strikes are decimal spreads.
    fn spread_surface() -> VolSurface {
        VolSurface::builder("CDX-IG-CDSO-VOL")
            .expiries(&[0.25, 3.0])
            .strikes(&[0.002, 0.02])
            .row(&[0.30, 0.50])
            .row(&[0.30, 0.50])
            .build()
            .expect("spread surface")
    }

    fn market() -> MarketContext {
        MarketContext::new()
            .insert(discount())
            .insert(hazard("CDX-IG", 0.01))
            .insert(hazard("TARGET-HAZ", 0.03))
            .insert_surface(spread_surface())
    }

    fn index_option(strike_spread: f64) -> CDSOption {
        let params = CDSOptionParams::call(
            CDSOptionStrike::Spread(Decimal::try_from(strike_spread).expect("valid strike")),
            as_of().add_months(6),
            as_of().add_months(66),
            Money::from((10_000_000_i64, Currency::USD)),
        )
        .expect("params")
        .as_index(1.0)
        .expect("index factor")
        .with_underlying_cds_coupon(Decimal::new(1, 2));
        let credit = CreditParams::corporate_standard("CDX", "CDX-IG");
        CDSOption::new(
            "CDX-IG-CDSO",
            &params,
            &credit,
            "USD-OIS",
            "CDX-IG-CDSO-VOL",
        )
        .expect("option")
    }

    fn request<'a>(
        option: &'a CDSOption,
        selector: CreditVolSelector,
        target: &'a CurveId,
    ) -> CreditVolRequest<'a> {
        CreditVolRequest {
            option,
            selector,
            target_hazard_curve_id: target,
            hazard_horizon: as_of().add_months(60),
        }
    }

    /// A CDX IG surface is queried at the decimal spread coordinate, and the
    /// index-derived fractional vol is anchored on the *target* curve's hazard
    /// (3%), not the index's (1%).
    #[test]
    fn spread_surface_queries_decimal_coordinate_and_anchors_on_target_curve() {
        let market = market();
        let option = index_option(0.011);
        let target = CurveId::from("TARGET-HAZ");
        let quote = resolve_hazard_volatility(
            &request(&option, CreditVolSelector::Strike(option.strike), &target),
            &market,
            as_of(),
        )
        .expect("resolve");

        assert!(
            (quote.surface_coordinate - 0.011).abs() < 1e-12,
            "must query the decimal spread coordinate, got {}",
            quote.surface_coordinate
        );
        // 0.011 sits halfway across [0.002, 0.02]: vol interpolates to 0.40.
        assert!((quote.model_spread_volatility - 0.40).abs() < 1e-9);
        // Anchored on the target curve's 3% hazard, not the index's 1%.
        assert!(
            (quote.reference_hazard - 0.03).abs() < 1e-9,
            "reference hazard must come from the target curve: {}",
            quote.reference_hazard
        );
        assert!((quote.hazard_volatility - 0.40 * 0.03).abs() < 1e-12);
        // The diagnostics expose the scale difference that makes this safe.
        assert!(quote.model_spread_volatility / quote.hazard_volatility > 30.0);
        assert!((quote.reference_spread - 0.6 * 0.03).abs() < 1e-12);
    }

    /// Strike, moneyness, and delta selectors all reach the same surface point
    /// when they describe the same strike.
    #[test]
    fn strike_moneyness_and_delta_selectors_agree() {
        let market = market();
        let option = index_option(0.011);
        let target = CurveId::from("TARGET-HAZ");

        let by_strike = resolve_hazard_volatility(
            &request(&option, CreditVolSelector::Strike(option.strike), &target),
            &market,
            as_of(),
        )
        .expect("by strike");

        // Moneyness that reproduces the same native coordinate.
        let atm = native_atm_coordinate(&option, &market, as_of()).expect("atm");
        let by_moneyness = resolve_hazard_volatility(
            &request(
                &option,
                CreditVolSelector::Moneyness(by_strike.surface_coordinate / atm),
                &target,
            ),
            &market,
            as_of(),
        )
        .expect("by moneyness");
        assert!(
            (by_moneyness.surface_coordinate - by_strike.surface_coordinate).abs() < 1e-9,
            "moneyness must reproduce the strike coordinate: {} vs {}",
            by_moneyness.surface_coordinate,
            by_strike.surface_coordinate
        );
        assert!((by_moneyness.hazard_volatility - by_strike.hazard_volatility).abs() < 1e-9);

        // Delta selector: solve for the |Δ| the strike itself carries.
        let mut at_strike = option.clone();
        at_strike
            .instrument_pricing_overrides
            .market_quotes
            .implied_volatility = Some(by_strike.model_spread_volatility);
        let target_delta = at_strike.delta(&market, as_of()).expect("delta").abs();

        let by_delta = resolve_hazard_volatility(
            &request(
                &option,
                CreditVolSelector::Delta {
                    absolute_delta: target_delta,
                    option_type: OptionType::Call,
                },
                &target,
            ),
            &market,
            as_of(),
        )
        .expect("by delta");
        assert!(
            (by_delta.surface_coordinate - by_strike.surface_coordinate).abs() < 1e-4,
            "delta inversion must recover the strike: {} vs {}",
            by_delta.surface_coordinate,
            by_strike.surface_coordinate
        );
    }

    /// Delta inversion works for both payer and receiver, and errors when the
    /// target is outside what the grid can express.
    #[test]
    fn delta_inversion_handles_both_types_and_reports_unreachable_targets() {
        let market = market();
        let option = index_option(0.011);
        let target = CurveId::from("TARGET-HAZ");

        for option_type in [OptionType::Call, OptionType::Put] {
            let mut probe = option.clone();
            probe.option_type = option_type;
            probe
                .instrument_pricing_overrides
                .market_quotes
                .implied_volatility = Some(0.40);
            let reachable = probe.delta(&market, as_of()).expect("delta").abs();

            resolve_hazard_volatility(
                &request(
                    &option,
                    CreditVolSelector::Delta {
                        absolute_delta: reachable,
                        option_type,
                    },
                    &target,
                ),
                &market,
                as_of(),
            )
            .unwrap_or_else(|e| panic!("{option_type:?} delta must invert: {e}"));
        }

        // A delta no strike on the grid carries. A narrow, deep-ITM strike
        // grid leaves a payer's |Δ| high at both endpoints, so a small target
        // cannot be bracketed.
        let narrow = VolSurface::builder("CDX-IG-CDSO-VOL")
            .expiries(&[0.25, 3.0])
            .strikes(&[0.0005, 0.001])
            .row(&[0.30, 0.31])
            .row(&[0.30, 0.31])
            .build()
            .expect("narrow surface");
        let narrow_market = MarketContext::new()
            .insert(discount())
            .insert(hazard("CDX-IG", 0.01))
            .insert(hazard("TARGET-HAZ", 0.03))
            .insert_surface(narrow);
        let err = resolve_hazard_volatility(
            &request(
                &option,
                CreditVolSelector::Delta {
                    absolute_delta: 0.02,
                    option_type: OptionType::Call,
                },
                &target,
            ),
            &narrow_market,
            as_of(),
        )
        .expect_err("unreachable delta must fail");
        assert!(
            err.to_string().contains("outside the range"),
            "error should say the grid cannot express it: {err}"
        );

        // Out-of-range delta arguments.
        for bad in [0.0, 1.0, -0.5, f64::NAN] {
            assert!(
                resolve_hazard_volatility(
                    &request(
                        &option,
                        CreditVolSelector::Delta {
                            absolute_delta: bad,
                            option_type: OptionType::Call,
                        },
                        &target,
                    ),
                    &market,
                    as_of(),
                )
                .is_err(),
                "absolute_delta = {bad} must be rejected"
            );
        }
    }

    /// The lookup is strict: wrong axis, wrong quote type, and out-of-grid
    /// coordinates all fail instead of clamping.
    #[test]
    fn strict_surface_contract_is_enforced() {
        let option = index_option(0.011);
        let target = CurveId::from("TARGET-HAZ");
        let base = MarketContext::new()
            .insert(discount())
            .insert(hazard("CDX-IG", 0.01))
            .insert(hazard("TARGET-HAZ", 0.03));

        // Missing surface.
        assert!(resolve_hazard_volatility(
            &request(&option, CreditVolSelector::Strike(option.strike), &target),
            &base,
            as_of(),
        )
        .is_err());

        // Wrong secondary axis.
        let tenor = VolSurface::builder("CDX-IG-CDSO-VOL")
            .expiries(&[0.25, 3.0])
            .strikes(&[0.002, 0.02])
            .row(&[0.30, 0.50])
            .row(&[0.30, 0.50])
            .secondary_axis(VolSurfaceAxis::Tenor)
            .build()
            .expect("tenor surface");
        let err = resolve_hazard_volatility(
            &request(&option, CreditVolSelector::Strike(option.strike), &target),
            &base.clone().insert_surface(tenor),
            as_of(),
        )
        .expect_err("tenor axis must fail");
        assert!(err.to_string().contains("strike secondary axis"), "{err}");

        // Wrong quote type.
        let normal = VolSurface::builder("CDX-IG-CDSO-VOL")
            .expiries(&[0.25, 3.0])
            .strikes(&[0.002, 0.02])
            .row(&[0.30, 0.50])
            .row(&[0.30, 0.50])
            .quote_type(VolQuoteType::Normal)
            .build()
            .expect("normal surface");
        assert!(resolve_hazard_volatility(
            &request(&option, CreditVolSelector::Strike(option.strike), &target),
            &base.clone().insert_surface(normal),
            as_of(),
        )
        .is_err());

        // Strike outside the grid: checked lookup, never clamped.
        let narrow = VolSurface::builder("CDX-IG-CDSO-VOL")
            .expiries(&[0.25, 3.0])
            .strikes(&[0.002, 0.004])
            .row(&[0.30, 0.32])
            .row(&[0.30, 0.32])
            .build()
            .expect("narrow surface");
        assert!(
            resolve_hazard_volatility(
                &request(&option, CreditVolSelector::Strike(option.strike), &target),
                &base.clone().insert_surface(narrow),
                as_of(),
            )
            .is_err(),
            "a strike beyond the grid must fail rather than clamp to the edge"
        );

        // Expiry outside the grid.
        let short = VolSurface::builder("CDX-IG-CDSO-VOL")
            .expiries(&[0.01, 0.05])
            .strikes(&[0.002, 0.02])
            .row(&[0.30, 0.50])
            .row(&[0.30, 0.50])
            .build()
            .expect("short surface");
        assert!(resolve_hazard_volatility(
            &request(&option, CreditVolSelector::Strike(option.strike), &target),
            &base.insert_surface(short),
            as_of(),
        )
        .is_err());
    }

    /// A CDX HY surface is queried at the clean-price-percent coordinate —
    /// `107.0`, not `1.07` and not a spread equivalent. This is the selector
    /// path the companion price-strike work unblocked.
    #[test]
    fn clean_price_surface_queries_percent_coordinate() {
        let hy_surface = VolSurface::builder("CDX-HY-CDSO-VOL")
            .expiries(&[0.25, 3.0])
            .strikes(&[100.0, 110.0])
            .row(&[0.30, 0.50])
            .row(&[0.30, 0.50])
            .build()
            .expect("clean-price surface");
        let market = MarketContext::new()
            .insert(discount())
            .insert(hazard("CDX-HY", 0.05))
            .insert(hazard("TARGET-HAZ", 0.03))
            .insert_surface(hy_surface);

        let params = CDSOptionParams::call(
            CDSOptionStrike::CleanPricePct(Decimal::new(1070, 1)),
            as_of().add_months(6),
            as_of().add_months(66),
            Money::from((10_000_000_i64, Currency::USD)),
        )
        .expect("params")
        .as_index(0.99)
        .expect("index factor")
        .with_strike_index_factor(1.0)
        .expect("strike factor")
        .with_underlying_cds_coupon(Decimal::new(5, 2));
        let credit = CreditParams::corporate_standard("CDXHY", "CDX-HY");
        let option = CDSOption::new(
            "CDX-HY-CDSO",
            &params,
            &credit,
            "USD-OIS",
            "CDX-HY-CDSO-VOL",
        )
        .expect("option");
        let target = CurveId::from("TARGET-HAZ");

        let quote = resolve_hazard_volatility(
            &request(&option, CreditVolSelector::Strike(option.strike), &target),
            &market,
            as_of(),
        )
        .expect("HY resolve");

        assert!(
            (quote.surface_coordinate - 107.0).abs() < 1e-12,
            "must query 107.0, not 1.07 or a spread: {}",
            quote.surface_coordinate
        );
        // 107 is 70% across [100, 110]: vol interpolates to 0.44.
        assert!((quote.model_spread_volatility - 0.44).abs() < 1e-9);
        // Still anchored on the target curve's hazard, and the stored value is
        // a spread vol despite the price-quoted axis.
        assert!((quote.reference_hazard - 0.03).abs() < 1e-9);
        assert!((quote.hazard_volatility - 0.44 * 0.03).abs() < 1e-12);
    }

    /// Moneyness is measured in the strike's native units against the native
    /// ATM-forward coordinate.
    #[test]
    fn moneyness_is_relative_to_the_native_atm_forward() {
        let market = market();
        let option = index_option(0.011);
        let target = CurveId::from("TARGET-HAZ");
        let atm = native_atm_coordinate(&option, &market, as_of()).expect("atm");

        let quote = resolve_hazard_volatility(
            &request(&option, CreditVolSelector::Moneyness(1.0), &target),
            &market,
            as_of(),
        )
        .expect("atm moneyness");
        assert!(
            (quote.surface_coordinate - atm).abs() < 1e-9,
            "moneyness 1.0 must land on the native ATM forward: {} vs {atm}",
            quote.surface_coordinate
        );

        for bad in [0.0, -1.0, f64::NAN] {
            assert!(
                resolve_hazard_volatility(
                    &request(&option, CreditVolSelector::Moneyness(bad), &target),
                    &market,
                    as_of(),
                )
                .is_err(),
                "moneyness {bad} must be rejected"
            );
        }
    }
}
