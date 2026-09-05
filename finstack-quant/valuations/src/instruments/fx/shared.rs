//! Shared pricing and metric helpers for FX instruments.
//!
use crate::instruments::InstrumentPricingOverrides;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::fx::FxQuery;
use finstack_quant_core::types::{CurveId, PriceId};

/// Return whether an FX contractual event is complete under the end-of-day policy.
///
/// FX fixing, expiry, delivery, and settlement cashflows remain live on their
/// event date and are extinguished on the following valuation date.
#[inline]
pub(crate) fn event_has_occurred(event_date: Date, as_of: Date) -> bool {
    event_date < as_of
}

/// Inputs for a covered-interest-parity FX forward rate.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FxForwardRateRequest<'a> {
    /// Market data context.
    pub(crate) market: &'a MarketContext,
    /// Valuation date.
    pub(crate) as_of: Date,
    /// Forward maturity date.
    pub(crate) maturity: Date,
    /// Base currency of the quoted pair.
    pub(crate) base_currency: Currency,
    /// Quote currency of the quoted pair.
    pub(crate) quote_currency: Currency,
    /// Quote-currency discount curve.
    pub(crate) domestic_discount_curve_id: &'a CurveId,
    /// Base-currency discount curve.
    pub(crate) foreign_discount_curve_id: &'a CurveId,
    /// Optional quote-per-base spot override.
    pub(crate) spot_rate_override: Option<f64>,
    /// Label included in validation errors.
    pub(crate) context: &'a str,
}

/// Resolved covered-interest-parity inputs shared by forward rate and PV.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FxForwardInputs {
    /// Quote-per-base spot at the valuation date.
    pub(crate) spot: f64,
    /// Quote-currency discount factor to maturity.
    pub(crate) df_domestic: f64,
    /// Base-currency discount factor to maturity.
    pub(crate) df_foreign: f64,
}

impl FxForwardInputs {
    /// Compute the covered-interest-parity forward rate.
    pub(crate) fn forward_rate(self) -> f64 {
        self.spot * self.df_foreign / self.df_domestic
    }
}

/// Resolve spot and both discount factors once for CIP calculations.
pub(crate) fn collect_fx_forward_inputs(
    request: FxForwardRateRequest<'_>,
) -> finstack_quant_core::Result<FxForwardInputs> {
    if request.maturity < request.as_of {
        return Err(finstack_quant_core::InputError::Invalid.into());
    }
    let domestic = request
        .market
        .get_discount(request.domestic_discount_curve_id.as_str())?;
    let foreign = request
        .market
        .get_discount(request.foreign_discount_curve_id.as_str())?;
    let df_domestic = domestic.df_between_dates(request.as_of, request.maturity)?;
    let df_foreign = foreign.df_between_dates(request.as_of, request.maturity)?;
    if !df_domestic.is_finite() || df_domestic <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{} domestic discount factor ({df_domestic}) is invalid for maturity {}",
            request.context, request.maturity
        )));
    }
    if !df_foreign.is_finite() || df_foreign <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{} foreign discount factor ({df_foreign}) is invalid for maturity {}",
            request.context, request.maturity
        )));
    }
    let spot = if let Some(spot) = request.spot_rate_override {
        spot
    } else {
        let matrix = request.market.fx().ok_or_else(|| {
            finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                id: "fx_matrix".to_string(),
            })
        })?;
        matrix
            .rate(FxQuery::new(
                request.base_currency,
                request.quote_currency,
                request.as_of,
            ))?
            .rate
    };
    if !spot.is_finite() || spot <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{} spot must be finite and positive, got {spot}",
            request.context
        )));
    }
    Ok(FxForwardInputs {
        spot,
        df_domestic,
        df_foreign,
    })
}

/// Compute `S × DF_base / DF_quote` with date-based discount factors.
pub(crate) fn covered_interest_parity_forward(
    request: FxForwardRateRequest<'_>,
) -> finstack_quant_core::Result<f64> {
    Ok(collect_fx_forward_inputs(request)?.forward_rate())
}

/// Source for the FX spot used by an option-style FX pricer.
#[derive(Debug, Clone, Copy)]
pub(crate) enum FxSpotSource<'a> {
    /// Resolve spot from the market FX matrix.
    Matrix,
    /// Resolve spot from an explicit market scalar when present, otherwise use the FX matrix.
    ScalarId(Option<&'a PriceId>),
}

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
    /// Instrument-owned inputs used for implied-vol overrides.
    pub(crate) instrument_pricing_overrides: &'a InstrumentPricingOverrides,
    /// Spot source.
    pub(crate) spot_source: FxSpotSource<'a>,
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
}

pub(crate) fn resolve_fx_spot(
    request: FxOptionInputRequest<'_>,
) -> finstack_quant_core::Result<f64> {
    if let FxSpotSource::ScalarId(Some(spot_id)) = request.spot_source {
        let spot_scalar = request.market.get_price(spot_id)?;
        let spot = crate::metrics::scalar_numeric_value(spot_scalar);
        if !spot.is_finite() || spot <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{} spot must be finite and > 0, got {}",
                request.rate_context, spot
            )));
        }
        return Ok(spot);
    }

    let fx_matrix = request.market.fx().ok_or(finstack_quant_core::Error::from(
        finstack_quant_core::InputError::NotFound {
            id: "fx_matrix".to_string(),
        },
    ))?;
    let spot = fx_matrix
        .rate(FxQuery::new(
            request.base_currency,
            request.quote_currency,
            request.as_of,
        ))?
        .rate;
    if !spot.is_finite() || spot <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{} spot must be finite and > 0, got {}",
            request.rate_context, spot
        )));
    }
    Ok(spot)
}

pub(crate) fn collect_fx_option_inputs_no_vol(
    request: FxOptionInputRequest<'_>,
) -> finstack_quant_core::Result<FxOptionInputsNoVol> {
    let spot = resolve_fx_spot(request)?;
    if request.as_of >= request.expiry {
        return Ok(FxOptionInputsNoVol {
            spot,
            r_domestic: 0.0,
            r_foreign: 0.0,
            t: 0.0,
        });
    }

    let domestic_disc = request
        .market
        .get_discount(request.domestic_discount_curve_id.as_str())?;
    let foreign_disc = request
        .market
        .get_discount(request.foreign_discount_curve_id.as_str())?;
    let df_d = domestic_disc.df_between_dates(request.as_of, request.expiry)?;
    let df_f = foreign_disc.df_between_dates(request.as_of, request.expiry)?;
    let t = request.day_count.year_fraction(
        request.as_of,
        request.expiry,
        DayCountContext::default(),
    )?;
    let r_domestic = crate::instruments::common_impl::helpers::zero_rate_from_df(
        df_d,
        t,
        &format!("{} domestic discount", request.rate_context),
    )?;
    let r_foreign = crate::instruments::common_impl::helpers::zero_rate_from_df(
        df_f,
        t,
        &format!("{} foreign discount", request.rate_context),
    )?;

    Ok(FxOptionInputsNoVol {
        spot,
        r_domestic,
        r_foreign,
        t,
    })
}

pub(crate) fn collect_fx_option_inputs(
    request: FxOptionInputRequest<'_>,
) -> finstack_quant_core::Result<FxOptionInputs> {
    let no_vol = collect_fx_option_inputs_no_vol(request)?;
    if request.as_of >= request.expiry {
        return Ok(FxOptionInputs {
            spot: no_vol.spot,
            r_domestic: 0.0,
            r_foreign: 0.0,
            sigma: 0.0,
            t: 0.0,
        });
    }

    let sigma = crate::instruments::common_impl::vol_resolution::resolve_sigma_at(
        &request.instrument_pricing_overrides.market_quotes,
        request.market,
        request.vol_surface_id,
        no_vol.t,
        request.strike,
    )?;

    Ok(FxOptionInputs {
        spot: no_vol.spot,
        r_domestic: no_vol.r_domestic,
        r_foreign: no_vol.r_foreign,
        sigma,
        t: no_vol.t,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
    use finstack_quant_core::money::Money;
    use std::sync::Arc;
    use time::macros::date;

    fn market(as_of: Date) -> MarketContext {
        let discount = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (1.0, (-0.04_f64).exp())])
            .build()
            .expect("valid discount");
        let foreign = DiscountCurve::builder("EUR-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (1.0, (-0.02_f64).exp())])
            .build()
            .expect("valid foreign");
        let vol = VolSurface::builder("EURUSD-VOL")
            .expiries(&[1.0])
            .strikes(&[1.1])
            .row(&[0.20])
            .build()
            .expect("valid vol");
        let provider = Arc::new(SimpleFxProvider::new());
        provider
            .set_quote(Currency::EUR, Currency::USD, 1.1)
            .expect("valid rate");
        let fx = FxMatrix::new(provider);
        MarketContext::new()
            .insert(discount)
            .insert(foreign)
            .insert_surface(vol)
            .insert_fx(fx)
            .insert_price(
                "EURUSD-SPOT",
                MarketScalar::Price(Money::new(1.1, Currency::USD).expect("valid money fixture")),
            )
    }

    #[test]
    fn shared_fx_option_inputs_reconstruct_date_based_discount_rates() {
        let as_of = date!(2026 - 01 - 01);
        let expiry = date!(2027 - 01 - 01);
        let market = market(as_of);

        let inputs = collect_fx_option_inputs(FxOptionInputRequest {
            market: &market,
            as_of,
            base_currency: Currency::EUR,
            quote_currency: Currency::USD,
            expiry,
            day_count: DayCount::Act365F,
            domestic_discount_curve_id: &CurveId::new("USD-OIS"),
            foreign_discount_curve_id: &CurveId::new("EUR-OIS"),
            vol_surface_id: "EURUSD-VOL",
            strike: 1.1,
            instrument_pricing_overrides: &InstrumentPricingOverrides::default(),
            spot_source: FxSpotSource::Matrix,
            rate_context: "test",
        })
        .expect("inputs should resolve");

        let domestic_df = market
            .get_discount("USD-OIS")
            .expect("discount")
            .df_between_dates(as_of, expiry)
            .expect("df");
        let foreign_df = market
            .get_discount("EUR-OIS")
            .expect("foreign")
            .df_between_dates(as_of, expiry)
            .expect("df");

        assert!(((-inputs.r_domestic * inputs.t).exp() - domestic_df).abs() < 1e-12);
        assert!(((-inputs.r_foreign * inputs.t).exp() - foreign_df).abs() < 1e-12);
        assert_eq!(inputs.spot, 1.1);
        assert_eq!(inputs.sigma, 0.20);
    }

    #[test]
    fn shared_fx_option_inputs_can_source_spot_from_scalar_id() {
        let as_of = date!(2026 - 01 - 01);
        let expiry = date!(2027 - 01 - 01);
        let market = market(as_of);
        let spot_id = finstack_quant_core::types::PriceId::new("EURUSD-SPOT");

        let inputs = collect_fx_option_inputs(FxOptionInputRequest {
            market: &market,
            as_of,
            base_currency: Currency::EUR,
            quote_currency: Currency::USD,
            expiry,
            day_count: DayCount::Act365F,
            domestic_discount_curve_id: &CurveId::new("USD-OIS"),
            foreign_discount_curve_id: &CurveId::new("EUR-OIS"),
            vol_surface_id: "EURUSD-VOL",
            strike: 1.1,
            instrument_pricing_overrides: &InstrumentPricingOverrides::default(),
            spot_source: FxSpotSource::ScalarId(Some(&spot_id)),
            rate_context: "test",
        })
        .expect("inputs should resolve");

        assert_eq!(inputs.spot, 1.1);
    }
}
