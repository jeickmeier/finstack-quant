//! Shared rates curve bumping logic (plan-driven calibration).

use super::currency::infer_currency_from_id;
use super::BumpRequest;
use crate::calibration::api::schema::{DiscountCurveParams, ForwardCurveParams, StepParams};
use crate::calibration::config::CalibrationMethod;
use crate::calibration::config::RatesStepConventions;
use crate::calibration::step_runtime;
use crate::calibration::CalibrationConfig;
use crate::market::quotes::ids::{Pillar, QuoteId};
use crate::market::quotes::market_quote::MarketQuote;
use crate::market::quotes::rates::RateQuote;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
use finstack_quant_core::market_data::term_structures::{
    DiscountCurve, DiscountCurveRateCalibration, DiscountCurveRateQuoteType, ForwardCurve,
    ForwardCurveRateCalibration, ForwardCurveRateQuote,
};
use finstack_quant_core::math::interp::ExtrapolationPolicy;
use finstack_quant_core::types::{CurveId, IndexId};
use time::Duration;

/// Infer currency from a discount curve ID using token-by-token heuristics.
///
/// Best-effort fallback for callers that don't have explicit currency metadata.
/// Returns USD if no known currency or benchmark-rate token appears in the ID.
pub fn infer_currency_from_discount_curve_id(curve: &DiscountCurve) -> Currency {
    infer_currency_from_id(curve.id().as_str())
}

/// Bump a discount curve by shocking rate quotes and re-calibrating.
///
/// This applies a [`BumpRequest`] to a collection of [`RateQuote`]s and
/// re-executes the calibration step to produce a new [`DiscountCurve`].
pub fn bump_discount_curve(
    quotes: &[RateQuote],
    params: &DiscountCurveParams,
    base_context: &MarketContext,
    bump: &BumpRequest,
) -> finstack_quant_core::Result<DiscountCurve> {
    let bumped_quotes = apply_bump_to_rate_quotes(quotes.to_vec(), bump, params.base_date);
    let market_quotes: Vec<MarketQuote> =
        bumped_quotes.into_iter().map(MarketQuote::Rates).collect();
    let step = StepParams::Discount(params.clone());
    // Re-calibration uses the default CalibrationConfig — see the "Calibration
    // config — known limitation" note in this module's docs (`bumps/mod.rs`).
    let cfg = CalibrationConfig::default();
    let (ctx, _report) =
        step_runtime::execute_params_and_apply(&step, &market_quotes, base_context, &cfg)?;

    Ok(ctx.get_discount(params.curve_id.as_str())?.as_ref().clone())
}

/// Bump a discount curve by shocking its stored market-rate calibration quotes.
///
/// The re-bootstrapped curves are applied as a *delta overlay* on the stored
/// curve: both the bumped and the unbumped quote sets are bootstrapped, and
/// only their discount-factor ratio is applied to the stored knots. Stored
/// curves transcribed from an external source (e.g. Bloomberg screen
/// fixtures) are not necessarily the exact bootstrap solution of their stored
/// quotes; repricing risk directly off a re-bootstrapped curve would shift
/// the base level and contaminate the sensitivity with a base-shape change.
/// For self-consistent curves the unbumped re-bootstrap reproduces the stored
/// curve and the overlay is exact.
pub(crate) fn bump_discount_curve_from_rate_calibration(
    curve: &DiscountCurve,
    calibration: &DiscountCurveRateCalibration,
    context: &MarketContext,
    bump: &BumpRequest,
) -> finstack_quant_core::Result<DiscountCurve> {
    let index = IndexId::new(calibration.index_id.as_str());
    let mut quotes = Vec::with_capacity(calibration.quotes.len());
    for quote in &calibration.quotes {
        let pillar = Pillar::Tenor(quote.tenor.parse()?);
        let id = QuoteId::new(format!("{}-{}", curve.id(), quote.tenor));
        let rate_quote = match quote.quote_type {
            DiscountCurveRateQuoteType::Deposit => RateQuote::Deposit {
                id,
                index: index.clone(),
                pillar,
                rate: quote.rate,
            },
            DiscountCurveRateQuoteType::Swap => RateQuote::Swap {
                id,
                index: index.clone(),
                pillar,
                rate: quote.rate,
                spread_decimal: None,
            },
        };
        quotes.push(rate_quote);
    }

    let first_rate = calibration
        .quotes
        .first()
        .map(|quote| quote.rate)
        .unwrap_or(0.0);
    let fixings = ScalarTimeSeries::new(
        format!("FIXING:{}", curve.id()),
        vec![
            (curve.base_date() - Duration::days(3), first_rate),
            (curve.base_date() - Duration::days(2), first_rate),
            (curve.base_date() - Duration::days(1), first_rate),
            (curve.base_date(), first_rate),
        ],
        None,
    )?;
    let base_context = context.clone().insert_series(fixings);

    let params = DiscountCurveParams {
        curve_id: curve.id().clone(),
        currency: calibration.currency,
        base_date: curve.base_date(),
        method: CalibrationMethod::Bootstrap,
        interpolation: curve.interp_style(),
        extrapolation: curve.extrapolation(),
        pricing_discount_id: None,
        pricing_forward_id: None,
        conventions: RatesStepConventions {
            ois_compounding: None,
            curve_day_count: Some(curve.day_count()),
        },
    };

    let bumped = bump_discount_curve(&quotes, &params, &base_context, bump)?;
    let unbumped =
        bump_discount_curve(&quotes, &params, &base_context, &BumpRequest::Parallel(0.0))?;

    let overlaid: Vec<(f64, f64)> = curve
        .knots()
        .iter()
        .zip(curve.dfs())
        .map(|(&t, &df)| {
            let base_df = unbumped.df(t);
            let ratio = if base_df > 0.0 {
                bumped.df(t) / base_df
            } else {
                1.0
            };
            (t, df * ratio)
        })
        .collect();

    DiscountCurve::builder(curve.id().clone())
        .base_date(curve.base_date())
        .day_count(curve.day_count())
        .knots(overlaid)
        .interp(curve.interp_style())
        .extrapolation(curve.extrapolation())
        .min_forward_tenor(curve.min_forward_tenor())
        .rate_calibration(calibration.clone())
        .build()
}

/// Bump a forward curve by shocking its stored market-rate calibration quotes
/// and globally recalibrating against the supplied market context.
///
/// The provided `context` must already contain the discount curve referenced by
/// `calibration.discount_curve_id` (in its bumped form, when bumping both curves
/// together). The helper does not support [`ForwardCurveRateQuote::Basis`]
/// quotes; callers handling basis-tenor calibrations must rebuild the forward
/// curve explicitly.
///
/// Like [`bump_discount_curve_from_rate_calibration`], the recalibration is
/// applied as a delta overlay on the stored curve: the bumped and unbumped
/// global solves are both run and only their forward-rate difference is added
/// to the stored knots, so transcribed curves keep their base shape.
pub(crate) fn bump_forward_curve_from_rate_calibration(
    curve: &ForwardCurve,
    calibration: &ForwardCurveRateCalibration,
    context: &MarketContext,
    bump: &BumpRequest,
) -> finstack_quant_core::Result<ForwardCurve> {
    let index = IndexId::new(calibration.index_id.as_str());
    let mut quotes = Vec::with_capacity(calibration.quotes.len());
    for (idx, quote) in calibration.quotes.iter().enumerate() {
        let id = QuoteId::new(format!("{}-{}", curve.id(), idx));
        let rate_quote = match quote {
            ForwardCurveRateQuote::Deposit { tenor, rate } => RateQuote::Deposit {
                id,
                index: index.clone(),
                pillar: Pillar::Tenor(tenor.parse()?),
                rate: *rate,
            },
            ForwardCurveRateQuote::Fra { start, end, rate } => RateQuote::Fra {
                id,
                index: index.clone(),
                start: Pillar::Date(*start),
                end: Pillar::Date(*end),
                rate: *rate,
            },
            ForwardCurveRateQuote::Swap {
                tenor,
                rate,
                spread_decimal,
            } => RateQuote::Swap {
                id,
                index: index.clone(),
                pillar: Pillar::Tenor(tenor.parse()?),
                rate: *rate,
                spread_decimal: *spread_decimal,
            },
            ForwardCurveRateQuote::Basis { .. } => {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "forward curve {} calibration uses basis quotes; \
                     bump_forward_curve_from_rate_calibration cannot recalibrate them — \
                     callers must rebuild the basis curve explicitly",
                    curve.id()
                )));
            }
        };
        quotes.push(rate_quote);
    }

    let params = ForwardCurveParams {
        curve_id: curve.id().clone(),
        currency: calibration.currency,
        base_date: curve.base_date(),
        tenor_years: curve.tenor(),
        discount_curve_id: calibration.discount_curve_id.clone(),
        method: CalibrationMethod::GlobalSolve {
            use_analytical_jacobian: false,
        },
        interpolation: curve.interp_style(),
        conventions: RatesStepConventions {
            ois_compounding: None,
            curve_day_count: Some(curve.day_count()),
        },
    };

    let bumped = rebootstrap_forward_curve(curve, quotes.clone(), &params, context, Some(bump))?;
    let unbumped = rebootstrap_forward_curve(curve, quotes, &params, context, None)?;

    let overlaid: Vec<(f64, f64)> = curve
        .knots()
        .iter()
        .zip(curve.forwards())
        .map(|(&t, &fwd)| (t, fwd + bumped.rate(t) - unbumped.rate(t)))
        .collect();

    ForwardCurve::builder(curve.id().clone(), curve.tenor())
        .base_date(curve.base_date())
        .reset_lag(curve.reset_lag())
        .day_count(curve.day_count())
        .knots(overlaid)
        .projection_grid_opt(
            unbumped
                .projection_grid()
                .map(<[f64]>::to_vec)
                .or_else(|| curve.projection_grid().map(<[f64]>::to_vec)),
        )
        .interp(curve.interp_style())
        .extrapolation(curve.extrapolation())
        .rate_calibration(calibration.clone())
        .build()
}

/// Globally recalibrate a forward curve from (optionally bumped) rate quotes
/// using the stored curve's conventions.
fn rebootstrap_forward_curve(
    curve: &ForwardCurve,
    quotes: Vec<RateQuote>,
    params: &ForwardCurveParams,
    context: &MarketContext,
    bump: Option<&BumpRequest>,
) -> finstack_quant_core::Result<ForwardCurve> {
    let quotes = match bump {
        Some(bump) => apply_bump_to_rate_quotes(quotes, bump, curve.base_date()),
        None => quotes,
    };
    let market_quotes: Vec<MarketQuote> = quotes.into_iter().map(MarketQuote::Rates).collect();
    let step = StepParams::Forward(params.clone());
    let cfg = CalibrationConfig::default();
    let (ctx, _report) =
        step_runtime::execute_params_and_apply(&step, &market_quotes, context, &cfg)?;
    Ok(ctx.get_forward(params.curve_id.as_str())?.as_ref().clone())
}

/// Re-bootstrap both a discount curve and its dependent forward curve from
/// stored rate-calibration metadata under a parallel quote shock.
///
/// Both curves must carry [`DiscountCurve::rate_calibration`] / [`ForwardCurve::rate_calibration`]
/// metadata. Index fixings are seeded from the first quote of each calibration
/// (keyed by both index_id and curve_id) so the calibration engine has the
/// reference fixings it needs when re-bootstrapping. Returns an error if the
/// forward curve uses basis quotes; callers needing basis support must combine
/// [`bump_discount_curve_from_rate_calibration`] with a bespoke forward rebuild.
pub(crate) fn bump_market_via_rate_quote_shock(
    market: &MarketContext,
    discount_curve_id: &CurveId,
    forward_curve_id: &CurveId,
    bump_bp: f64,
) -> finstack_quant_core::Result<MarketContext> {
    let discount = market.get_discount(discount_curve_id.as_str())?;
    let forward = market.get_forward(forward_curve_id.as_str())?;
    let discount_cal = discount.rate_calibration().ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "discount curve {} has no rate_calibration metadata; cannot quote-shock DV01",
            discount_curve_id
        ))
    })?;
    let forward_cal = forward.rate_calibration().ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "forward curve {} has no rate_calibration metadata; cannot quote-shock DV01",
            forward_curve_id
        ))
    })?;

    let seeded = seed_calibration_fixings(
        market,
        discount.base_date(),
        discount_curve_id,
        discount_cal,
        forward_curve_id,
        forward_cal,
    )?;

    let bump = BumpRequest::Parallel(bump_bp);

    let bumped_discount =
        bump_discount_curve_from_rate_calibration(discount.as_ref(), discount_cal, &seeded, &bump)?;
    let seeded_with_discount = seeded.insert(bumped_discount);

    let bumped_forward = bump_forward_curve_from_rate_calibration(
        forward.as_ref(),
        forward_cal,
        &seeded_with_discount,
        &bump,
    )?;
    Ok(seeded_with_discount.insert(bumped_forward))
}

/// Seed bootstrap-time fixings for both curve and index identifiers so the
/// calibration engine has the reference rates it needs when re-bootstrapping
/// after a quote shock. Uses the first quote of each calibration set as the
/// historical fixing — sufficient for risk re-bootstrapping where only the
/// shape of the curve matters, not the historical realized path.
fn seed_calibration_fixings(
    market: &MarketContext,
    base_date: Date,
    discount_curve_id: &CurveId,
    discount_cal: &DiscountCurveRateCalibration,
    forward_curve_id: &CurveId,
    forward_cal: &ForwardCurveRateCalibration,
) -> finstack_quant_core::Result<MarketContext> {
    let mut seeded = market.clone();
    if let Some(rate) = discount_cal.quotes.first().map(|q| q.rate) {
        seeded = seeded.insert_series(fixing_seed(&discount_cal.index_id, base_date, rate)?);
        seeded = seeded.insert_series(fixing_seed(discount_curve_id.as_str(), base_date, rate)?);
    }
    if let Some(rate) = first_forward_calibration_rate(forward_cal) {
        seeded = seeded.insert_series(fixing_seed(&forward_cal.index_id, base_date, rate)?);
        seeded = seeded.insert_series(fixing_seed(forward_curve_id.as_str(), base_date, rate)?);
    }
    Ok(seeded)
}

fn first_forward_calibration_rate(calibration: &ForwardCurveRateCalibration) -> Option<f64> {
    calibration.quotes.first().map(|q| match q {
        ForwardCurveRateQuote::Deposit { rate, .. }
        | ForwardCurveRateQuote::Fra { rate, .. }
        | ForwardCurveRateQuote::Swap { rate, .. } => *rate,
        ForwardCurveRateQuote::Basis { spread_decimal, .. } => *spread_decimal,
    })
}

fn fixing_seed(
    id: &str,
    base_date: Date,
    rate: f64,
) -> finstack_quant_core::Result<ScalarTimeSeries> {
    ScalarTimeSeries::new(
        format!("FIXING:{id}"),
        vec![
            (base_date - Duration::days(3), rate),
            (base_date - Duration::days(2), rate),
            (base_date - Duration::days(1), rate),
            (base_date, rate),
        ],
        None,
    )
}

/// Apply a [`BumpRequest`] to a vector of [`RateQuote`]s.
///
/// Parallel bumps shift every quote; tenor bumps locate the closest quote to
/// each target year fraction and shift only that quote. Pure data transform —
/// no calibration engine involvement.
fn apply_bump_to_rate_quotes(
    quotes: Vec<RateQuote>,
    bump: &BumpRequest,
    as_of: Date,
) -> Vec<RateQuote> {
    match bump {
        BumpRequest::Parallel(bp) => quotes.into_iter().map(|q| q.bump_rate_bp(*bp)).collect(),
        BumpRequest::Tenors(targets) => {
            let mut q = quotes;
            for (target_t, bp) in targets {
                if let Some(idx) = find_closest_quote(&q, *target_t, as_of) {
                    q[idx] = q[idx].bump_rate_bp(*bp);
                }
            }
            q
        }
    }
}

/// Helper to resolve maturity date of a quote.
fn resolve_maturity(q: &RateQuote, base_date: Date) -> Option<Date> {
    // Basic resolution using base_date + pillar
    // This ignores spot lag or BDC, but is sufficient for "closest quote" heuristics.
    match q {
        RateQuote::Deposit { pillar, .. } | RateQuote::Swap { pillar, .. } => {
            resolve_pillar(pillar, base_date)
        }
        RateQuote::Fra { end, .. } => resolve_pillar(end, base_date),
        RateQuote::Futures { expiry, .. } => Some(*expiry),
    }
}

fn resolve_pillar(pillar: &Pillar, base_date: Date) -> Option<Date> {
    match pillar {
        Pillar::Date(d) => Some(*d),
        Pillar::Tenor(t) => {
            // Approx add tenor
            // For bumping grouping, exact BDC usually doesn't change the "closest" logic significantly.
            t.add_to_date(
                base_date,
                None,
                finstack_quant_core::dates::BusinessDayConvention::Following,
            )
            .ok()
        }
    }
}

/// Find the quote closest to the target maturity.
pub(crate) fn find_closest_quote(
    quotes: &[RateQuote],
    target_years: f64,
    as_of: Date,
) -> Option<usize> {
    let dc = DayCount::Act365F; // Simple day count for proximity check
    quotes
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            let a_date = resolve_maturity(a, as_of).unwrap_or(as_of);
            let b_date = resolve_maturity(b, as_of).unwrap_or(as_of);

            let a_yf = dc
                .year_fraction(as_of, a_date, DayCountContext::default())
                .unwrap_or(0.0);
            let b_yf = dc
                .year_fraction(as_of, b_date, DayCountContext::default())
                .unwrap_or(0.0);
            let a_dist = (a_yf - target_years).abs();
            let b_dist = (b_yf - target_years).abs();
            a_dist
                .partial_cmp(&b_dist)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(i, _)| i)
}

/// Bump discount curve by synthesizing par instruments from the curve, shocking them, and re-calibrating.
///
/// Used when original quotes are unavailable. It implies par rates from
/// the current curve discount factors, applies shocks, and re-bootstraps.
///
/// # Arguments
/// * `currency` - Currency of the curve (required; DiscountCurve does not carry currency metadata).
pub fn bump_discount_curve_synthetic(
    curve: &finstack_quant_core::market_data::term_structures::DiscountCurve,
    context: &MarketContext,
    bump: &BumpRequest,
    as_of: Date,
    currency: Currency,
) -> finstack_quant_core::Result<DiscountCurve> {
    let curve_id = curve.id();
    let base_date = as_of;
    let knots = curve.knots();

    // Choose synthetic indices. Deposits use a short-dated money-market index,
    // while swaps must use the corresponding OIS index conventions.
    let deposit_index_id = match currency {
        // Align with `rate_index_conventions.json` (there is no `EUR-ESTR-1M` alias today).
        Currency::EUR => "EUR-ESTR-OIS",
        Currency::GBP => "GBP-SONIA-1M",
        Currency::JPY => "JPY-TONA-1M",
        _ => "USD-SOFR-1M",
    };

    // Synthesize quotes for each knot (excluding t≈0) and re-calibrate.
    // Use deposit-style quotes for all maturities here. The synthetic bump path
    // is a deterministic approximation used when original quotes are unavailable,
    // and staying in deposit space avoids OIS swap seasoning/fixings requirements
    // during scenario shock application.

    let mut quotes = Vec::new();
    let dc = DayCount::Act365F;
    let dc_ctx = DayCountContext::default();

    for &t in knots {
        if t <= 0.0001 {
            continue;
        }

        let df = curve.df(t);
        let maturity_days = (t * 365.25).round() as i64;
        let maturity = base_date + time::Duration::days(maturity_days);

        let yf = dc.year_fraction(base_date, maturity, dc_ctx).unwrap_or(t);

        let rate = if yf > 1e-4 {
            (1.0 / df - 1.0) / yf
        } else {
            0.0
        };
        quotes.push(RateQuote::Deposit {
            id: QuoteId::new(format!("SYNTH-DEP-{}", t)),
            index: finstack_quant_core::types::IndexId::new(deposit_index_id),
            pillar: Pillar::Date(maturity),
            rate,
        });
    }

    let params = DiscountCurveParams {
        curve_id: curve_id.clone(),
        currency,
        base_date,
        method: CalibrationMethod::Bootstrap,
        interpolation: Default::default(),
        extrapolation: ExtrapolationPolicy::FlatForward,
        pricing_discount_id: None,
        pricing_forward_id: None,
        conventions: RatesStepConventions {
            ois_compounding: None,
            curve_day_count: Some(DayCount::Act365F),
        },
    };

    bump_discount_curve(&quotes, &params, context, bump)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::market::conventions::ids::IrFutureContractId;

    /// Parallel "rate bp" bumps must shock every quote's *rate* by +1bp,
    /// including futures, where price = 100·(1 − rate) means the price must
    /// fall by 0.01. Regression for the bug where the decimal bump was added
    /// to the futures price verbatim (wrong sign, 1/100 magnitude), silently
    /// mis-shocking futures pillars in plan-driven parallel/key-rate bumps.
    #[test]
    fn parallel_bump_shifts_futures_implied_rate_up() {
        let as_of = Date::from_calendar_date(2026, time::Month::June, 9).expect("valid date");
        let quotes = vec![
            RateQuote::Deposit {
                id: QuoteId::new("USD-DEP-3M"),
                index: IndexId::new("USD-SOFR-3M"),
                pillar: Pillar::Tenor("3M".parse().expect("valid tenor")),
                rate: 0.05,
            },
            RateQuote::Futures {
                id: QuoteId::new("USD-FUT-SEP26"),
                contract: IrFutureContractId::new("CME:SR3"),
                expiry: Date::from_calendar_date(2026, time::Month::September, 16)
                    .expect("valid date"),
                price: 96.00, // implied rate 4%
                convexity_adjustment: Some(0.0),
                vol_surface_id: None,
            },
            RateQuote::Swap {
                id: QuoteId::new("USD-SWAP-2Y"),
                index: IndexId::new("USD-SOFR-OIS"),
                pillar: Pillar::Tenor("2Y".parse().expect("valid tenor")),
                rate: 0.045,
                spread_decimal: None,
            },
        ];

        let implied_rate = |q: &RateQuote| -> f64 {
            match q {
                RateQuote::Deposit { rate, .. }
                | RateQuote::Fra { rate, .. }
                | RateQuote::Swap { rate, .. } => *rate,
                RateQuote::Futures { price, .. } => (100.0 - price) / 100.0,
            }
        };
        let base_rates: Vec<f64> = quotes.iter().map(implied_rate).collect();

        let bumped = apply_bump_to_rate_quotes(quotes, &BumpRequest::Parallel(1.0), as_of);

        for (q, base) in bumped.iter().zip(base_rates.iter()) {
            let moved = implied_rate(q) - base;
            assert!(
                (moved - 1e-4).abs() < 1e-12,
                "{}: implied rate must move +1bp, moved {moved:.8}",
                q.id().as_str()
            );
        }
    }
}
