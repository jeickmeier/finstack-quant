//! FX option market-input collection and spot-rate resolution.

use super::forward::*;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::money::fx::FxQuery;
use finstack_quant_core::types::{CurveId, PriceId};
use finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides;

/// Shared request for FX option-style market inputs.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FxOptionInputRequest<'a> {
    /// Market data context.
    pub(crate) market: &'a MarketContext,
    /// Valuation date.
    pub(crate) as_of: Date,
    /// Base currency of the FX pair.
    pub(crate) base_currency: Currency,
    /// Quote currency of the FX pair.
    pub(crate) quote_currency: Currency,
    /// Option expiry.
    pub(crate) expiry: Date,
    /// Delivery date of the underlying exposure.
    pub(crate) delivery: Date,
    /// Value-date basis of the active spot quote.
    pub(crate) spot_basis: FxSpotBasis,
    /// Volatility time basis.
    pub(crate) day_count: DayCount,
    /// Domestic discount curve.
    pub(crate) domestic_discount_curve_id: &'a CurveId,
    /// Foreign discount curve.
    pub(crate) foreign_discount_curve_id: &'a CurveId,
    /// Volatility surface id.
    pub(crate) vol_surface_id: &'a str,
    /// Strike used for volatility lookup.
    pub(crate) strike: f64,
    /// Instrument-owned pricing inputs.
    pub(crate) instrument_pricing_overrides: &'a InstrumentPricingOverrides,
    /// Explicit spot scalar; `None` reads the FX matrix.
    pub(crate) spot_id: Option<&'a PriceId>,
    /// Context label for rate-conversion errors.
    pub(crate) rate_context: &'a str,
}

/// Shared no-volatility FX option inputs.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FxOptionInputsNoVol {
    /// FX spot.
    pub(crate) spot: f64,
    /// Domestic continuously compounded rate on the vol time basis.
    pub(crate) r_domestic: f64,
    /// Foreign continuously compounded rate on the vol time basis.
    pub(crate) r_foreign: f64,
    /// Time to expiry on the vol basis.
    pub(crate) t: f64,
    /// Date-based domestic discount factor from `as_of` to expiry (`1.0` once expired).
    pub(crate) df_domestic: f64,
    /// Date-based foreign discount factor from `as_of` to expiry (`1.0` once expired).
    pub(crate) df_foreign: f64,
}

/// Shared FX option inputs including volatility.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FxOptionInputs {
    /// FX spot.
    pub(crate) spot: f64,
    /// Domestic continuously compounded rate on the vol time basis.
    pub(crate) r_domestic: f64,
    /// Foreign continuously compounded rate on the vol time basis.
    pub(crate) r_foreign: f64,
    /// Volatility.
    pub(crate) sigma: f64,
    /// Time to expiry on the vol basis.
    pub(crate) t: f64,
    /// Date-based domestic discount factor from `as_of` to expiry (`1.0` once expired).
    pub(crate) df_domestic: f64,
    /// Date-based base-currency factor to delivery.
    pub(crate) df_foreign: f64,
    /// Sensitivity of valuation-date cash FX to the active spot number.
    pub(crate) spot_conversion: f64,
}

/// Quote-per-base FX spot from the `spot_id` scalar when given, else the FX matrix.
///
/// A missing `spot_id` scalar is an error; it never falls back to the matrix.
pub(crate) fn resolve_fx_spot(
    market: &MarketContext,
    as_of: Date,
    base_currency: Currency,
    quote_currency: Currency,
    spot_id: Option<&PriceId>,
    context: &str,
) -> finstack_quant_core::Result<f64> {
    let spot = match spot_id {
        Some(spot_id) => finstack_quant_valuations_core::metrics::scalar_numeric_value(
            market.get_price(spot_id)?,
        ),
        None => {
            market
                .fx_required()?
                .rate(FxQuery::new(
                    base_currency,
                    quote_currency,
                    as_of,
                    finstack_quant_core::money::fx::FxConversionPolicy::CashflowDate,
                ))?
                .rate
        }
    };
    if !spot.is_finite() || spot <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{context} spot must be finite and > 0, got {spot}"
        )));
    }
    Ok(spot)
}

impl FxOptionInputRequest<'_> {
    /// Spot read from the request's source.
    pub(crate) fn spot(&self) -> finstack_quant_core::Result<f64> {
        resolve_fx_spot(
            self.market,
            self.as_of,
            self.base_currency,
            self.quote_currency,
            self.spot_id,
            self.rate_context,
        )
    }
}

pub(crate) fn collect_fx_option_inputs_no_vol(
    request: FxOptionInputRequest<'_>,
) -> finstack_quant_core::Result<FxOptionInputsNoVol> {
    let spot = request.spot()?;
    collect_fx_option_inputs_no_vol_at_spot(request, spot)
}

fn collect_fx_option_inputs_no_vol_at_spot(
    request: FxOptionInputRequest<'_>,
    spot: f64,
) -> finstack_quant_core::Result<FxOptionInputsNoVol> {
    if request.as_of >= request.expiry {
        return Ok(FxOptionInputsNoVol {
            spot,
            r_domestic: 0.0,
            r_foreign: 0.0,
            t: 0.0,
            df_domestic: 1.0,
            df_foreign: 1.0,
        });
    }

    let domestic_disc = request
        .market
        .get_discount(request.domestic_discount_curve_id.as_str())?;
    let foreign_disc = request
        .market
        .get_discount(request.foreign_discount_curve_id.as_str())?;
    let spot = request.spot_basis.cash_rate(
        spot,
        domestic_disc.as_ref(),
        foreign_disc.as_ref(),
        request.as_of,
    )?;
    let df_d = domestic_disc.df_between_dates(request.as_of, request.delivery)?;
    let df_f = foreign_disc.df_between_dates(request.as_of, request.delivery)?;
    let t = request.day_count.year_fraction(
        request.as_of,
        request.expiry,
        DayCountContext::default(),
    )?;
    let r_domestic =
        finstack_quant_valuations_core::instruments::pricing::discounting::zero_rate_from_df(
            df_d,
            t,
            &format!("{} domestic discount", request.rate_context),
        )?;
    let r_foreign =
        finstack_quant_valuations_core::instruments::pricing::discounting::zero_rate_from_df(
            df_f,
            t,
            &format!("{} foreign discount", request.rate_context),
        )?;

    Ok(FxOptionInputsNoVol {
        spot,
        r_domestic,
        r_foreign,
        t,
        df_domestic: df_d,
        df_foreign: df_f,
    })
}

pub(crate) fn collect_fx_option_inputs(
    request: FxOptionInputRequest<'_>,
) -> finstack_quant_core::Result<FxOptionInputs> {
    let spot = request.spot()?;
    collect_fx_option_inputs_at_spot(request, spot)
}

/// [`collect_fx_option_inputs`] with the spot already resolved by the caller.
pub(crate) fn collect_fx_option_inputs_at_spot(
    request: FxOptionInputRequest<'_>,
    spot: f64,
) -> finstack_quant_core::Result<FxOptionInputs> {
    let no_vol = collect_fx_option_inputs_no_vol_at_spot(request, spot)?;
    if request.as_of >= request.expiry {
        return Ok(FxOptionInputs {
            spot: no_vol.spot,
            r_domestic: 0.0,
            r_foreign: 0.0,
            sigma: 0.0,
            t: 0.0,
            df_domestic: 1.0,
            df_foreign: 1.0,
            spot_conversion: 1.0,
        });
    }

    let forward = no_vol.spot * no_vol.df_foreign / no_vol.df_domestic;
    let pair = finstack_quant_core::money::fx::CurrencyPair::try_new(
        request.base_currency,
        request.quote_currency,
    )?;
    if request
        .instrument_pricing_overrides
        .market_quotes
        .implied_volatility
        .is_none()
    {
        finstack_quant_core::validation::validate_f64_positive(
            forward,
            "FX option delivery forward",
        )?;
    }
    let sigma = finstack_quant_valuations_core::instruments::pricing::volatility::resolve_fx_sigma(
        finstack_quant_valuations_core::instruments::pricing::volatility::FxVolatilityRequest {
            overrides: &request.instrument_pricing_overrides.market_quotes,
            curves: request.market,
            vol_surface_id: request.vol_surface_id,
            pair,
            as_of: request.as_of,
            expiry: request.expiry,
            model_time: no_vol.t,
            strike: request.strike,
            forward,
        },
    )?;

    Ok(FxOptionInputs {
        spot: no_vol.spot,
        r_domestic: no_vol.r_domestic,
        r_foreign: no_vol.r_foreign,
        sigma,
        t: no_vol.t,
        df_domestic: no_vol.df_domestic,
        df_foreign: no_vol.df_foreign,
        spot_conversion: no_vol.spot / spot,
    })
}
