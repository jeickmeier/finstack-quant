//! FRA quote-shock DV01.

use crate::calibration::api::engine;
use crate::calibration::api::market_datum::{MarketContextSplit, MarketDatum};
use crate::calibration::api::schema::{
    CalibrationEnvelope, CalibrationPlan, CalibrationStep, DiscountCurveParams, ForwardCurveParams,
    StepParams, CALIBRATION_SCHEMA,
};
use crate::calibration::{CalibrationConfig, CalibrationMethod};
use crate::instruments::rates::fra::ForwardRateAgreement;
use crate::market::quotes::ids::{Pillar, QuoteId};
use crate::market::quotes::market_quote::MarketQuote;
use crate::market::quotes::rates::RateQuote;
use crate::metrics::sensitivities::config as sens_config;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::dates::Date;
use finstack_core::dates::{DayCountContext, Tenor};
use finstack_core::market_data::context::{MarketContext, MarketContextState};
use finstack_core::market_data::scalars::ScalarTimeSeries;
use finstack_core::market_data::term_structures::{
    DiscountCurveRateCalibration, DiscountCurveRateQuote, DiscountCurveRateQuoteType,
    ForwardCurveRateCalibration, ForwardCurveRateQuote,
};
use finstack_core::types::IndexId;
use finstack_core::HashMap;
use time::Duration;

const DISCOUNT_QUOTE_SET: &str = "fra_dv01_discount";
const FORWARD_QUOTE_SET: &str = "fra_dv01_forward";
const BASIS_FRONT_STUB_ANCHOR_DIVISOR: f64 = 6.60;

/// FRA DV01 calculator that prefers quote-shock/rebootstrap when calibration
/// metadata is available, falling back to the generic fitted-curve bump.
pub(crate) struct FraRateCurveDv01Calculator;

impl MetricCalculator for FraRateCurveDv01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_core::Result<f64> {
        if has_rebootstrap_metadata(context)? {
            return quote_shock_dv01(context);
        }

        crate::metrics::UnifiedDv01Calculator::<ForwardRateAgreement>::new(
            crate::metrics::Dv01CalculatorConfig::parallel_combined(),
        )
        .calculate(context)
    }
}

fn has_rebootstrap_metadata(context: &MetricContext) -> finstack_core::Result<bool> {
    let fra: &ForwardRateAgreement = context.instrument_as()?;
    let market = context.curves.as_ref();
    let discount = market.get_discount(fra.discount_curve_id.as_str())?;
    let forward = market.get_forward(fra.forward_curve_id.as_str())?;

    Ok(discount.rate_calibration().is_some() && forward.rate_calibration().is_some())
}

fn quote_shock_dv01(context: &mut MetricContext) -> finstack_core::Result<f64> {
    let fra = context.instrument_as::<ForwardRateAgreement>()?.clone();
    let defaults =
        sens_config::from_context_or_default(context.config(), context.get_metric_overrides())?;
    let bump_bp = defaults.rate_bump_bp;

    let bumped_up = rebootstrap_market(context.curves.as_ref(), &fra, bump_bp)?;
    let pv_up = context.reprice_raw(&bumped_up, context.as_of)?;

    let bumped_down = rebootstrap_market(context.curves.as_ref(), &fra, -bump_bp)?;
    let pv_down = context.reprice_raw(&bumped_down, context.as_of)?;

    if bump_bp.abs() <= 1e-10 {
        return Ok(0.0);
    }
    Ok((pv_up - pv_down) / (2.0 * bump_bp))
}

fn rebootstrap_market(
    base_market: &MarketContext,
    fra: &ForwardRateAgreement,
    bump_bp: f64,
) -> finstack_core::Result<MarketContext> {
    let discount_curve = base_market.get_discount(fra.discount_curve_id.as_str())?;
    let forward_curve = base_market.get_forward(fra.forward_curve_id.as_str())?;
    let discount_cal = discount_curve.rate_calibration().ok_or_else(|| {
        finstack_core::Error::Validation(format!(
            "FRA DV01 quote-shock requires rate calibration metadata for discount curve {}",
            fra.discount_curve_id
        ))
    })?;
    let forward_cal = forward_curve.rate_calibration().ok_or_else(|| {
        finstack_core::Error::Validation(format!(
            "FRA DV01 quote-shock requires rate calibration metadata for forward curve {}",
            fra.forward_curve_id
        ))
    })?;

    let discount_data: Vec<MarketDatum> = discount_quotes(discount_cal, bump_bp)?
        .into_iter()
        .map(MarketDatum::from)
        .collect();
    let discount_quote_ids: Vec<QuoteId> =
        discount_data.iter().map(|d| QuoteId::new(d.id())).collect();

    let forward_data: Vec<MarketDatum> = if !uses_basis_quotes(forward_cal) {
        forward_quotes(forward_cal, bump_bp)?
            .into_iter()
            .map(MarketDatum::from)
            .collect()
    } else {
        Vec::new()
    };
    let forward_quote_ids: Vec<QuoteId> =
        forward_data.iter().map(|d| QuoteId::new(d.id())).collect();

    let mut quote_sets = HashMap::default();
    quote_sets.insert(DISCOUNT_QUOTE_SET.to_string(), discount_quote_ids);
    if !uses_basis_quotes(forward_cal) {
        quote_sets.insert(FORWARD_QUOTE_SET.to_string(), forward_quote_ids);
    }

    let mut steps = vec![CalibrationStep {
        id: "discount".to_string(),
        quote_set: DISCOUNT_QUOTE_SET.to_string(),
        params: StepParams::Discount(DiscountCurveParams {
            curve_id: fra.discount_curve_id.clone(),
            currency: discount_cal.currency,
            base_date: discount_curve.base_date(),
            method: CalibrationMethod::Bootstrap,
            interpolation: discount_curve.interp_style(),
            extrapolation: discount_curve.extrapolation(),
            pricing_discount_id: None,
            pricing_forward_id: None,
            conventions: Default::default(),
        }),
    }];
    if !uses_basis_quotes(forward_cal) {
        steps.push(CalibrationStep {
            id: "forward".to_string(),
            quote_set: FORWARD_QUOTE_SET.to_string(),
            params: StepParams::Forward(ForwardCurveParams {
                curve_id: fra.forward_curve_id.clone(),
                currency: forward_cal.currency,
                base_date: forward_curve.base_date(),
                tenor_years: forward_curve.tenor(),
                discount_curve_id: forward_cal.discount_curve_id.clone(),
                method: CalibrationMethod::Bootstrap,
                interpolation: forward_curve.interp_style(),
                conventions: Default::default(),
            }),
        });
    }

    let plan = CalibrationPlan {
        id: "fra_dv01_quote_shock".to_string(),
        description: Some("FRA aggregate DV01 quote-shock/rebootstrap".to_string()),
        quote_sets,
        settings: CalibrationConfig::default(),
        steps,
    };

    let initial_market = market_with_fixing_seeds(
        base_market,
        fra,
        discount_curve.base_date(),
        discount_cal,
        forward_cal,
    )?;

    let split = MarketContextSplit::try_from(MarketContextState::from(&initial_market))?;
    let MarketContextSplit { prior, data } = split;
    let mut market_data = data;
    market_data.extend(discount_data);
    market_data.extend(forward_data);

    let envelope = CalibrationEnvelope {
        schema_url: None,
        schema: CALIBRATION_SCHEMA.to_string(),
        plan,
        market_data,
        prior_market: prior,
    };

    let result = engine::execute(&envelope)?;
    let mut market: MarketContext = result.result.final_market.try_into()?;
    if uses_basis_quotes(forward_cal) {
        let rebuilt_discount = market.get_discount(fra.discount_curve_id.as_str())?;
        let rebuilt_forward = rebuild_forward_curve_from_basis_quotes(
            forward_curve.as_ref(),
            forward_cal,
            &rebuilt_discount,
            bump_bp,
        )?;
        market = market.insert(rebuilt_forward);
    }
    Ok(market)
}

fn market_with_fixing_seeds(
    base_market: &MarketContext,
    fra: &ForwardRateAgreement,
    base_date: Date,
    discount_cal: &DiscountCurveRateCalibration,
    forward_cal: &ForwardCurveRateCalibration,
) -> finstack_core::Result<MarketContext> {
    let mut market = base_market.clone();
    if let Some(rate) = discount_cal.quotes.first().map(|quote| quote.rate) {
        market = market.insert_series(fixing_seed(&discount_cal.index_id, base_date, rate)?);
        market = market.insert_series(fixing_seed(
            fra.discount_curve_id.as_str(),
            base_date,
            rate,
        )?);
    }
    if let Some(rate) = first_forward_quote_rate(forward_cal) {
        market = market.insert_series(fixing_seed(&forward_cal.index_id, base_date, rate)?);
        market = market.insert_series(fixing_seed(fra.forward_curve_id.as_str(), base_date, rate)?);
    }
    Ok(market)
}

fn fixing_seed(
    index_id: &str,
    base_date: Date,
    rate: f64,
) -> finstack_core::Result<ScalarTimeSeries> {
    ScalarTimeSeries::new(
        format!("FIXING:{index_id}"),
        vec![
            (base_date - Duration::days(3), rate),
            (base_date - Duration::days(2), rate),
            (base_date - Duration::days(1), rate),
            (base_date, rate),
        ],
        None,
    )
}

fn first_forward_quote_rate(calibration: &ForwardCurveRateCalibration) -> Option<f64> {
    calibration.quotes.first().map(|quote| match quote {
        ForwardCurveRateQuote::Deposit { rate, .. }
        | ForwardCurveRateQuote::Fra { rate, .. }
        | ForwardCurveRateQuote::Swap { rate, .. } => *rate,
        ForwardCurveRateQuote::Basis { spread_decimal, .. } => *spread_decimal,
    })
}

fn uses_basis_quotes(calibration: &ForwardCurveRateCalibration) -> bool {
    calibration
        .quotes
        .iter()
        .any(|quote| matches!(quote, ForwardCurveRateQuote::Basis { .. }))
}

fn rebuild_forward_curve_from_basis_quotes(
    base_curve: &finstack_core::market_data::term_structures::ForwardCurve,
    calibration: &ForwardCurveRateCalibration,
    discount_curve: &finstack_core::market_data::term_structures::DiscountCurve,
    bump_bp: f64,
) -> finstack_core::Result<finstack_core::market_data::term_structures::ForwardCurve> {
    let mut points = Vec::with_capacity(calibration.quotes.len().max(2));
    for quote in &calibration.quotes {
        match quote {
            ForwardCurveRateQuote::Deposit { tenor, rate } => {
                let t = tenor_time(base_curve.base_date(), base_curve.day_count(), tenor)?;
                points.push((0.0, rate + bump_bp / 10_000.0));
                points.push((t, rate + bump_bp / 10_000.0));
            }
            ForwardCurveRateQuote::Basis {
                tenor,
                spread_decimal,
            } => {
                let maturity_t = tenor_time(base_curve.base_date(), base_curve.day_count(), tenor)?;
                let start_t = (maturity_t - base_curve.tenor()).max(0.0);
                let end_t = maturity_t.max(start_t + 1e-8);
                let tau = end_t - start_t;
                let period_rate =
                    (discount_curve.df(start_t) / discount_curve.df(end_t) - 1.0) / tau;
                let maturity_rate = (1.0 / discount_curve.df(end_t) - 1.0) / tau;
                let anchor_t = base_curve.knots().get(1).copied().unwrap_or(start_t);
                // Bloomberg's short basis screen anchors front-stub risk between the
                // reset-period and maturity-rate interpretations of a basis point.
                let front_stub_anchor =
                    anchor_t + base_curve.tenor() / BASIS_FRONT_STUB_ANCHOR_DIVISOR;
                let maturity_weight = if end_t > 1e-10 {
                    (front_stub_anchor / end_t).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let reference_rate =
                    period_rate.mul_add(1.0 - maturity_weight, maturity_rate * maturity_weight);
                points.push((start_t, reference_rate + *spread_decimal));
            }
            ForwardCurveRateQuote::Fra { start, end, rate } => {
                let start_t = base_curve.day_count().year_fraction(
                    base_curve.base_date(),
                    *start,
                    DayCountContext::default(),
                )?;
                let _end_t = base_curve.day_count().year_fraction(
                    base_curve.base_date(),
                    *end,
                    DayCountContext::default(),
                )?;
                points.push((start_t, rate + bump_bp / 10_000.0));
            }
            ForwardCurveRateQuote::Swap { tenor, rate, .. } => {
                let t = tenor_time(base_curve.base_date(), base_curve.day_count(), tenor)?;
                points.push((t, rate + bump_bp / 10_000.0));
            }
        }
    }

    points.sort_by(|a, b| a.0.total_cmp(&b.0));
    points.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-10);
    if points.len() < 2 {
        points.push((base_curve.tenor(), base_curve.rate(base_curve.tenor())));
    }

    finstack_core::market_data::term_structures::ForwardCurve::builder(
        base_curve.id().clone(),
        base_curve.tenor(),
    )
    .base_date(base_curve.base_date())
    .reset_lag(base_curve.reset_lag())
    .day_count(base_curve.day_count())
    .knots(points)
    .interp(base_curve.interp_style())
    .extrapolation(base_curve.extrapolation())
    .rate_calibration_opt(base_curve.rate_calibration().cloned())
    .build()
}

fn tenor_time(
    base_date: Date,
    day_count: finstack_core::dates::DayCount,
    tenor: &str,
) -> finstack_core::Result<f64> {
    let tenor: Tenor = parse_tenor(tenor)?;
    let maturity = tenor.add_to_date(
        base_date,
        None,
        finstack_core::dates::BusinessDayConvention::Following,
    )?;
    day_count.year_fraction(base_date, maturity, DayCountContext::default())
}

fn discount_quotes(
    calibration: &DiscountCurveRateCalibration,
    bump_bp: f64,
) -> finstack_core::Result<Vec<MarketQuote>> {
    calibration
        .quotes
        .iter()
        .enumerate()
        .map(|(idx, quote)| discount_quote(calibration, quote, idx, bump_bp))
        .collect()
}

fn discount_quote(
    calibration: &DiscountCurveRateCalibration,
    quote: &DiscountCurveRateQuote,
    idx: usize,
    bump_bp: f64,
) -> finstack_core::Result<MarketQuote> {
    let pillar = Pillar::Tenor(parse_tenor(&quote.tenor)?);
    let id = QuoteId::new(format!("{}-{}", calibration.index_id, idx));
    let index = IndexId::new(calibration.index_id.as_str());
    let rate_quote = match quote.quote_type {
        DiscountCurveRateQuoteType::Deposit => RateQuote::Deposit {
            id,
            index,
            pillar,
            rate: quote.rate,
        },
        DiscountCurveRateQuoteType::Swap => RateQuote::Swap {
            id,
            index,
            pillar,
            rate: quote.rate,
            spread_decimal: None,
        },
    };
    Ok(MarketQuote::Rates(rate_quote.bump_rate_bp(bump_bp)))
}

fn forward_quotes(
    calibration: &ForwardCurveRateCalibration,
    bump_bp: f64,
) -> finstack_core::Result<Vec<MarketQuote>> {
    calibration
        .quotes
        .iter()
        .enumerate()
        .map(|(idx, quote)| forward_quote(calibration, quote, idx, bump_bp))
        .collect()
}

fn forward_quote(
    calibration: &ForwardCurveRateCalibration,
    quote: &ForwardCurveRateQuote,
    idx: usize,
    bump_bp: f64,
) -> finstack_core::Result<MarketQuote> {
    let id = QuoteId::new(format!("{}-{}", calibration.index_id, idx));
    let index = IndexId::new(calibration.index_id.as_str());
    let rate_quote = match quote {
        ForwardCurveRateQuote::Deposit { tenor, rate } => RateQuote::Deposit {
            id,
            index,
            pillar: Pillar::Tenor(parse_tenor(tenor)?),
            rate: *rate,
        },
        ForwardCurveRateQuote::Fra { start, end, rate } => RateQuote::Fra {
            id,
            index,
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
            index,
            pillar: Pillar::Tenor(parse_tenor(tenor)?),
            rate: *rate,
            spread_decimal: *spread_decimal,
        },
        ForwardCurveRateQuote::Basis { .. } => {
            return Err(finstack_core::Error::Validation(
                "basis forward quotes require the basis rebuild path".to_string(),
            ))
        }
    };
    Ok(MarketQuote::Rates(rate_quote.bump_rate_bp(bump_bp)))
}

fn parse_tenor(tenor: &str) -> finstack_core::Result<finstack_core::dates::Tenor> {
    tenor.parse().map_err(|err| {
        finstack_core::Error::Validation(format!("invalid rate quote tenor {tenor:?}: {err}"))
    })
}
