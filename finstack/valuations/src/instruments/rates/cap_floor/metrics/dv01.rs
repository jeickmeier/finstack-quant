//! Market DV01 calculator for caps/floors.

use crate::calibration::api::engine;
use crate::calibration::api::market_datum::{MarketContextSplit, MarketDatum};
use crate::calibration::api::schema::{
    CalibrationEnvelope, CalibrationPlan, CalibrationStep, DiscountCurveParams, ForwardCurveParams,
    StepParams, CALIBRATION_SCHEMA,
};
use crate::calibration::{CalibrationConfig, CalibrationMethod};
use crate::instruments::rates::cap_floor::CapFloor;
use crate::market::quotes::ids::{Pillar, QuoteId};
use crate::market::quotes::market_quote::MarketQuote;
use crate::market::quotes::rates::RateQuote;
use crate::metrics::sensitivities::config as sens_config;
use crate::metrics::sensitivities::cs01::sensitivity_central_diff;
use crate::metrics::{
    Dv01CalculatorConfig, MetricCalculator, MetricContext, UnifiedDv01Calculator,
};
use finstack_core::dates::{Date, Tenor};
use finstack_core::market_data::context::{MarketContext, MarketContextState};
use finstack_core::market_data::scalars::ScalarTimeSeries;
use finstack_core::market_data::term_structures::{
    DiscountCurveRateCalibration, DiscountCurveRateQuote, DiscountCurveRateQuoteType,
    ForwardCurveRateCalibration, ForwardCurveRateQuote,
};
use finstack_core::types::IndexId;
use finstack_core::{HashMap, Result};
use time::Duration;

const DISCOUNT_QUOTE_SET: &str = "cap_floor_dv01_discount";
const FORWARD_QUOTE_SET: &str = "cap_floor_dv01_forward";

/// Cap/floor DV01.
///
/// When the market curves carry quote calibration metadata, this reports
/// quote-shock/rebootstrap DV01. Otherwise it falls back to fitted-curve
/// bump-and-reprice DV01.
pub(crate) struct Dv01Calculator;

impl MetricCalculator for Dv01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        if has_rebootstrap_metadata(context)? {
            return quote_shock_dv01(context);
        }

        let inner =
            UnifiedDv01Calculator::<CapFloor>::new(Dv01CalculatorConfig::parallel_combined());
        inner.calculate(context)
    }
}

fn has_rebootstrap_metadata(context: &MetricContext) -> Result<bool> {
    let cap_floor: &CapFloor = context.instrument_as()?;
    let market = context.curves.as_ref();
    let discount = market.get_discount(cap_floor.discount_curve_id.as_str())?;
    let forward = market.get_forward(cap_floor.forward_curve_id.as_str())?;

    Ok(discount.rate_calibration().is_some() && forward.rate_calibration().is_some())
}

fn quote_shock_dv01(context: &mut MetricContext) -> Result<f64> {
    let cap_floor = context.instrument_as::<CapFloor>()?.clone();
    let defaults =
        sens_config::from_context_or_default(context.config(), context.get_metric_overrides())?;
    let bump_bp = defaults.rate_bump_bp;

    let bumped_up = rebootstrap_market(context.curves.as_ref(), &cap_floor, bump_bp)?;
    let pv_up = context.reprice_raw(&bumped_up, context.as_of)?;

    let bumped_down = rebootstrap_market(context.curves.as_ref(), &cap_floor, -bump_bp)?;
    let pv_down = context.reprice_raw(&bumped_down, context.as_of)?;

    Ok(sensitivity_central_diff(pv_up, pv_down, bump_bp))
}

fn rebootstrap_market(
    base_market: &MarketContext,
    cap_floor: &CapFloor,
    bump_bp: f64,
) -> Result<MarketContext> {
    let discount_curve = base_market.get_discount(cap_floor.discount_curve_id.as_str())?;
    let forward_curve = base_market.get_forward(cap_floor.forward_curve_id.as_str())?;
    let discount_cal = discount_curve.rate_calibration().ok_or_else(|| {
        finstack_core::Error::Validation(format!(
            "Cap/floor DV01 quote-shock requires rate calibration metadata for discount curve {}",
            cap_floor.discount_curve_id
        ))
    })?;
    let forward_cal = forward_curve.rate_calibration().ok_or_else(|| {
        finstack_core::Error::Validation(format!(
            "Cap/floor DV01 quote-shock requires rate calibration metadata for forward curve {}",
            cap_floor.forward_curve_id
        ))
    })?;

    let discount_data: Vec<MarketDatum> = discount_quotes(discount_cal, bump_bp)?
        .into_iter()
        .map(MarketDatum::from)
        .collect();
    let forward_data: Vec<MarketDatum> = forward_quotes(forward_cal, bump_bp)?
        .into_iter()
        .map(MarketDatum::from)
        .collect();

    let discount_quote_ids: Vec<QuoteId> =
        discount_data.iter().map(|d| QuoteId::new(d.id())).collect();
    let forward_quote_ids: Vec<QuoteId> =
        forward_data.iter().map(|d| QuoteId::new(d.id())).collect();

    let mut quote_sets = HashMap::default();
    quote_sets.insert(DISCOUNT_QUOTE_SET.to_string(), discount_quote_ids);
    quote_sets.insert(FORWARD_QUOTE_SET.to_string(), forward_quote_ids);

    let plan = CalibrationPlan {
        id: "cap_floor_dv01_quote_shock".to_string(),
        description: Some("Cap/floor aggregate DV01 quote-shock/rebootstrap".to_string()),
        quote_sets,
        settings: CalibrationConfig::default(),
        steps: vec![
            CalibrationStep {
                id: "discount".to_string(),
                quote_set: DISCOUNT_QUOTE_SET.to_string(),
                params: StepParams::Discount(DiscountCurveParams {
                    curve_id: cap_floor.discount_curve_id.clone(),
                    currency: discount_cal.currency,
                    base_date: discount_curve.base_date(),
                    method: CalibrationMethod::Bootstrap,
                    interpolation: discount_curve.interp_style(),
                    extrapolation: discount_curve.extrapolation(),
                    pricing_discount_id: None,
                    pricing_forward_id: None,
                    conventions: Default::default(),
                }),
            },
            CalibrationStep {
                id: "forward".to_string(),
                quote_set: FORWARD_QUOTE_SET.to_string(),
                params: StepParams::Forward(ForwardCurveParams {
                    curve_id: cap_floor.forward_curve_id.clone(),
                    currency: forward_cal.currency,
                    base_date: forward_curve.base_date(),
                    tenor_years: forward_curve.tenor(),
                    discount_curve_id: forward_cal.discount_curve_id.clone(),
                    method: CalibrationMethod::Bootstrap,
                    interpolation: forward_curve.interp_style(),
                    conventions: Default::default(),
                }),
            },
        ],
    };

    let initial_market = market_with_fixing_seeds(
        base_market,
        cap_floor,
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
    result.result.final_market.try_into()
}

fn market_with_fixing_seeds(
    base_market: &MarketContext,
    cap_floor: &CapFloor,
    base_date: Date,
    discount_cal: &DiscountCurveRateCalibration,
    forward_cal: &ForwardCurveRateCalibration,
) -> Result<MarketContext> {
    let mut market = base_market.clone();
    if let Some(rate) = discount_cal.quotes.first().map(|quote| quote.rate) {
        market = market.insert_series(fixing_seed(&discount_cal.index_id, base_date, rate)?);
        market = market.insert_series(fixing_seed(
            cap_floor.discount_curve_id.as_str(),
            base_date,
            rate,
        )?);
    }
    if let Some(rate) = first_forward_quote_rate(forward_cal) {
        market = market.insert_series(fixing_seed(&forward_cal.index_id, base_date, rate)?);
        market = market.insert_series(fixing_seed(
            cap_floor.forward_curve_id.as_str(),
            base_date,
            rate,
        )?);
    }
    Ok(market)
}

fn fixing_seed(index_id: &str, base_date: Date, rate: f64) -> Result<ScalarTimeSeries> {
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

fn discount_quotes(
    calibration: &DiscountCurveRateCalibration,
    bump_bp: f64,
) -> Result<Vec<MarketQuote>> {
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
) -> Result<MarketQuote> {
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
) -> Result<Vec<MarketQuote>> {
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
) -> Result<MarketQuote> {
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
                "basis forward quotes are not supported for cap/floor DV01 quote-shock".to_string(),
            ))
        }
    };
    Ok(MarketQuote::Rates(rate_quote.bump_rate_bp(bump_bp)))
}

fn parse_tenor(tenor: &str) -> Result<Tenor> {
    tenor.parse().map_err(|err| {
        finstack_core::Error::Validation(format!("invalid rate quote tenor {tenor:?}: {err}"))
    })
}
