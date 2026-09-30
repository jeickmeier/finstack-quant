use finstack_quant_calibration::api::engine;
use finstack_quant_calibration::api::market_datum::{
    CollateralEntry, DividendScheduleDatum, FxSpotDatum, MarketDatum, PriceDatum,
};
use finstack_quant_calibration::api::prior_market::PriorMarketObject;
use finstack_quant_calibration::api::schema::{
    CalibrationEnvelope, CalibrationPlan, CalibrationStep, StepParams,
};
use finstack_quant_calibration::quotes::ids::QuoteId;
use finstack_quant_calibration::quotes::market_quote::MarketQuote;
use finstack_quant_calibration::{CalibrationConfig, CalibrationReport};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::context::{CurveState, MarketContext, MarketContextState};
use finstack_quant_core::Result;
use finstack_quant_valuations::instruments::PricingOptions;
use std::sync::Arc;

/// Build request-scoped pricing options with a fresh cached recalibration provider.
pub fn pricing_options() -> PricingOptions {
    PricingOptions::default().with_recalibration_provider(Arc::new(
        finstack_quant_calibration::recalibration::CachedRecalibrationProvider::new(),
    ))
}

/// Extract the `QuoteId` of each [`MarketQuote`] in the slice.
///
/// Used to build v3 `quote_sets: HashMap<String, Vec<QuoteId>>` from the
/// legacy `Vec<MarketQuote>` form. Pair with [`extend_market_data`] to
/// also flatten the quotes onto `market_data`.
pub fn quote_set_ids(quotes: &[MarketQuote]) -> Vec<QuoteId> {
    quotes
        .iter()
        .map(|q| match q {
            MarketQuote::Rates(q) => q.id().clone(),
            MarketQuote::Cds(q) => q.id().clone(),
            MarketQuote::CdsTranche(q) => q.id().clone(),
            MarketQuote::Inflation(q) => q.id().clone(),
            MarketQuote::Vol(q) => q.id().clone(),
            MarketQuote::Xccy(q) => q.id().clone(),
        })
        .collect()
}

/// Append the `MarketDatum::from(quote)` projection of every quote in
/// `quotes` to `market_data`. Companion to [`quote_set_ids`].
pub fn extend_market_data(market_data: &mut Vec<MarketDatum>, quotes: &[MarketQuote]) {
    market_data.extend(quotes.iter().cloned().map(MarketDatum::from));
}

pub(crate) fn split_market_context_state(
    state: MarketContextState,
) -> Result<(Vec<PriorMarketObject>, Vec<MarketDatum>)> {
    let mut prior = state
        .curves
        .into_iter()
        .map(|curve| match curve {
            CurveState::Discount(c) => PriorMarketObject::DiscountCurve(c),
            CurveState::Forward(c) => PriorMarketObject::ForwardCurve(c),
            CurveState::Hazard(c) => PriorMarketObject::HazardCurve(c),
            CurveState::Inflation(c) => PriorMarketObject::InflationCurve(c),
            CurveState::BaseCorrelation(c) => PriorMarketObject::BaseCorrelationCurve(c),
            CurveState::BasisSpread(c) => PriorMarketObject::BasisSpreadCurve(c),
            CurveState::Parametric(c) => PriorMarketObject::ParametricCurve(c),
            CurveState::Price(c) => PriorMarketObject::PriceCurve(c),
            CurveState::VolIndex(c) => PriorMarketObject::VolatilityIndexCurve(c),
        })
        .collect::<Vec<_>>();
    prior.extend(
        state
            .surfaces
            .into_iter()
            .map(PriorMarketObject::VolSurface),
    );

    let mut data = Vec::new();
    if let Some(fx) = state.fx {
        data.extend(fx.quotes.into_iter().map(|(from, to, rate)| {
            MarketDatum::FxSpot(FxSpotDatum {
                id: format!("{from}/{to}"),
                from,
                to,
                rate,
            })
        }));
    }
    data.extend(
        state
            .prices
            .into_iter()
            .map(|(id, scalar)| MarketDatum::Price(PriceDatum { id, scalar })),
    );
    data.extend(state.series.into_iter().map(MarketDatum::FixingSeries));
    data.extend(
        state
            .inflation_indices
            .into_iter()
            .map(MarketDatum::InflationFixings),
    );
    data.extend(
        state
            .dividends
            .into_iter()
            .map(|schedule| MarketDatum::DividendSchedule(DividendScheduleDatum { schedule })),
    );
    data.extend(
        state
            .credit_indices
            .into_iter()
            .map(MarketDatum::CreditIndex),
    );
    data.extend(
        state
            .fx_delta_vol_surfaces
            .into_iter()
            .map(MarketDatum::FxVolSurface),
    );
    data.extend(state.vol_cubes.into_iter().map(MarketDatum::VolCube));
    for (currency, csa_currency) in state.collateral {
        data.push(MarketDatum::Collateral(CollateralEntry {
            id: parse_snapshot_currency(&currency, "collateral currency")?,
            csa_currency: parse_snapshot_currency(&csa_currency, "CSA currency")?,
        }));
    }
    Ok((prior, data))
}

fn parse_snapshot_currency(value: &str, field: &str) -> Result<Currency> {
    value
        .parse()
        .map_err(|err| finstack_quant_core::Error::Calibration {
            message: format!("Invalid {field} in market context snapshot: '{value}' ({err})"),
            category: "market_context_split".to_string(),
        })
}

/// Split a [`MarketContext`] into the canonical `(prior_market,
/// market_data)` inputs used by calibration tests.
pub fn split_market_context(ctx: &MarketContext) -> (Vec<PriorMarketObject>, Vec<MarketDatum>) {
    split_market_context_state(MarketContextState::from(ctx))
        .expect("valid market context snapshot")
}

/// Execute a single calibration step for tests/benchmarks without engaging the full plan engine.
pub fn execute_step(
    params: &StepParams,
    quotes: &[MarketQuote],
    context: &MarketContext,
    global_config: &CalibrationConfig,
) -> Result<(MarketContext, CalibrationReport)> {
    let (prior, mut data) = split_market_context_state(MarketContextState::from(context))?;

    let ids = quote_set_ids(quotes);
    extend_market_data(&mut data, quotes);

    let mut quote_sets = finstack_quant_core::HashMap::default();
    quote_sets.insert("default".to_string(), ids);

    let plan = CalibrationPlan {
        id: "test-plan".to_string(),
        description: None,
        quote_sets: quote_sets.into_iter().collect(),
        steps: vec![CalibrationStep {
            id: "step-0".to_string(),
            quote_set: "default".to_string(),
            params: params.clone(),
        }],
        settings: global_config.clone(),
    };

    let envelope = CalibrationEnvelope {
        schema_url: None,
        schema: finstack_quant_calibration::api::schema::CalibrationSchema::CURRENT,
        plan,
        market_data: data,
        prior_market: prior,
    };

    let result = engine::calibrate(&envelope)?;
    let market = MarketContext::try_from(result.result.final_market)?;
    Ok((market, result.result.report))
}
