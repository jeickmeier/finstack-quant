//! Shared rates curve bumping logic (plan-driven calibration).

use super::currency::infer_currency_from_id;
use super::BumpRequest;
use crate::calibration::api::schema::{DiscountCurveParams, StepParams};
use crate::calibration::config::CalibrationMethod;
use crate::calibration::config::RatesStepConventions;
use crate::calibration::step_runtime;
use crate::calibration::CalibrationConfig;
use crate::market::quotes::ids::{Pillar, QuoteId};
use crate::market::quotes::market_quote::MarketQuote;
use crate::market::quotes::rates::RateQuote;
use finstack_core::currency::Currency;
use finstack_core::dates::{Date, DayCount, DayCountContext};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::scalars::ScalarTimeSeries;
use finstack_core::market_data::term_structures::{
    DiscountCurve, DiscountCurveRateCalibration, DiscountCurveRateQuoteType,
};
use finstack_core::math::interp::ExtrapolationPolicy;
use finstack_core::types::IndexId;
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
) -> finstack_core::Result<DiscountCurve> {
    let as_of = params.base_date;

    // Clone quotes to apply bumps.
    let mut bumped_quotes: Vec<RateQuote> = quotes.to_vec();

    match bump {
        BumpRequest::Parallel(bp) => {
            bumped_quotes = bumped_quotes
                .into_iter()
                .map(|q| q.bump_rate_bp(*bp))
                .collect();
        }
        BumpRequest::Tenors(targets) => {
            for (target_t, bp) in targets {
                if let Some(idx) = find_closest_quote(&bumped_quotes, *target_t, as_of) {
                    bumped_quotes[idx] = bumped_quotes[idx].bump_rate_bp(*bp);
                }
            }
        }
    }

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
pub(crate) fn bump_discount_curve_from_rate_calibration(
    curve: &DiscountCurve,
    calibration: &DiscountCurveRateCalibration,
    context: &MarketContext,
    bump: &BumpRequest,
) -> finstack_core::Result<DiscountCurve> {
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

    bump_discount_curve(&quotes, &params, &base_context, bump)
}

/// Helper to resolve maturity date of a quote.
fn resolve_maturity(q: &RateQuote, base_date: Date) -> Option<Date> {
    // Basic resolution using base_date + pillar
    // This ignores spot lag or BDC, but is sufficient for "closest quote" heuristics.
    match q {
        RateQuote::Deposit { pillar, .. } => resolve_pillar(pillar, base_date),
        RateQuote::Fra { end, .. } => resolve_pillar(end, base_date),
        RateQuote::Futures { expiry, .. } => Some(*expiry),
        RateQuote::Swap { pillar, .. } => resolve_pillar(pillar, base_date),
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
                finstack_core::dates::BusinessDayConvention::Following,
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
    curve: &finstack_core::market_data::term_structures::DiscountCurve,
    context: &MarketContext,
    bump: &BumpRequest,
    as_of: Date,
    currency: Currency,
) -> finstack_core::Result<DiscountCurve> {
    let curve_id = curve.id();
    let base_date = as_of;
    let knots = curve.knots();

    // Choose synthetic indices. Deposits use a short-dated money-market index,
    // while swaps must use the corresponding OIS index conventions.
    let deposit_index_id = match currency {
        Currency::USD => "USD-SOFR-1M",
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
            index: crate::market::conventions::ids::IndexId::new(deposit_index_id),
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
