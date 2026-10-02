//! Helpers shared by crate-private calibration unit tests.

use crate::api::market_datum::{
    CollateralEntry, DividendScheduleDatum, FxSpotDatum, MarketDatum, PriceDatum,
};
use crate::api::prior_market::PriorMarketObject;
use crate::quotes::ids::QuoteId;
use crate::quotes::market_quote::MarketQuote;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::context::{CurveState, MarketContext, MarketContextState};
use finstack_quant_core::Result;

pub(crate) const STANDARD_NOTIONAL: f64 = 1_000_000.0;
pub(crate) const REPRICE_PV_ABS_TOL_DOLLARS: f64 = 1.0;
pub(crate) const FRA_REPRICE_ABS_TOL_DOLLARS: f64 = 5.0;
pub(crate) const FWD_RATE_ABS_TOL: f64 = 1e-8;

pub(crate) fn quote_set_ids(quotes: &[MarketQuote]) -> Vec<QuoteId> {
    quotes
        .iter()
        .map(|quote| QuoteId::new(quote.id()))
        .collect()
}

pub(crate) fn extend_market_data(market_data: &mut Vec<MarketDatum>, quotes: &[MarketQuote]) {
    market_data.extend(quotes.iter().cloned().map(MarketDatum::from));
}

fn split_market_context_state(
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

pub(crate) fn split_market_context(
    ctx: &MarketContext,
) -> (Vec<PriorMarketObject>, Vec<MarketDatum>) {
    split_market_context_state(MarketContextState::try_from(ctx).expect("coherent market snapshot"))
        .expect("valid market context snapshot")
}
