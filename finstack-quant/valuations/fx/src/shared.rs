//! Shared pricing and metric helpers for FX instruments.
//!
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::money::fx::FxQuery;
use finstack_quant_core::types::{CurveId, PriceId};
use finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides;

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
    pub(crate) quoted_spot: Option<f64>,
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
        return Err(finstack_quant_core::Error::Validation(
            "FX forward maturity must not precede the valuation date".to_string(),
        ));
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
    let spot = if let Some(spot) = request.quoted_spot {
        spot
    } else {
        let matrix = request.market.fx_required()?;
        matrix
            .rate(FxQuery::new(
                request.base_currency,
                request.quote_currency,
                request.as_of,
                Default::default(),
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
                    Default::default(),
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
    let r_domestic =
        finstack_quant_valuations_core::instruments::common_impl::helpers::zero_rate_from_df(
            df_d,
            t,
            &format!("{} domestic discount", request.rate_context),
        )?;
    let r_foreign =
        finstack_quant_valuations_core::instruments::common_impl::helpers::zero_rate_from_df(
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
        });
    }

    let sigma =
        finstack_quant_valuations_core::instruments::common_impl::vol_resolution::resolve_sigma_at(
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
        df_domestic: no_vol.df_domestic,
    })
}

/// `true` once `expiry` is reached on the instrument's `day_count` basis, where
/// option Greeks are reported as zero.
pub(crate) fn greeks_expired(
    day_count: DayCount,
    as_of: Date,
    expiry: Date,
) -> finstack_quant_core::Result<bool> {
    Ok(day_count.year_fraction(as_of, expiry, DayCountContext::default())? <= 0.0)
}

/// One-sided PV change per basis point of a parallel bump of `curve_id`.
pub(crate) fn discount_rho_bp<I>(
    instrument: &I,
    market: &MarketContext,
    as_of: Date,
    curve_id: &CurveId,
    bump_bp: f64,
) -> finstack_quant_core::Result<f64>
where
    I: finstack_quant_valuations_core::instruments::Instrument,
{
    let base_pv = instrument.value(market, as_of)?.amount();
    let bumped = finstack_quant_valuations_core::metrics::bump_discount_curve_parallel(
        market, curve_id, bump_bp,
    )?;
    let pv_bumped = instrument.value(&bumped, as_of)?.amount();
    Ok((pv_bumped - base_pv) / bump_bp)
}

/// Market with the FX matrix `base/quote` rate bumped relatively by `bump_pct`.
///
/// Uses the matrix's own relative bump, which keeps its `FxConfig` (pivot,
/// triangulation) and pinned quotes and never mutates the shared provider.
pub(crate) fn fx_matrix_bumped_market(
    market: &MarketContext,
    base_currency: Currency,
    quote_currency: Currency,
    as_of: Date,
    bump_pct: f64,
) -> finstack_quant_core::Result<MarketContext> {
    Ok(market
        .clone()
        .insert_fx(market.fx_required()?.with_bumped_rate(
            base_currency,
            quote_currency,
            bump_pct,
            as_of,
        )?))
}

/// Central spot delta `(PV(S·(1+h)) − PV(S·(1−h))) / (2·S·h)`.
///
/// `bumped_market(h)` returns the market with the instrument's spot source
/// bumped relatively by `h`.
pub(crate) fn spot_delta<I, F>(
    instrument: &I,
    as_of: Date,
    spot: f64,
    bump_pct: f64,
    bumped_market: F,
) -> finstack_quant_core::Result<f64>
where
    I: finstack_quant_valuations_core::instruments::Instrument,
    F: Fn(f64) -> finstack_quant_core::Result<MarketContext>,
{
    let up = bumped_market(bump_pct)?;
    let down = bumped_market(-bump_pct)?;
    let pv_up = instrument.value(&up, as_of)?.amount();
    let pv_dn = instrument.value(&down, as_of)?.amount();
    finstack_quant_valuations_core::metrics::central_diff_by_width(
        pv_up,
        pv_dn,
        2.0 * spot * bump_pct,
    )
}

/// Central spot gamma `(PV(S·(1+h)) − 2·PV(S) + PV(S·(1−h))) / (S·h)²`.
///
/// Returns `0.0` when the bump width `S·h` is not positive. `bumped_market(h)`
/// returns the market with the instrument's spot source bumped relatively by `h`.
pub(crate) fn spot_gamma<I, F>(
    instrument: &I,
    as_of: Date,
    base_pv: f64,
    spot: f64,
    bump_pct: f64,
    bumped_market: F,
) -> finstack_quant_core::Result<f64>
where
    I: finstack_quant_valuations_core::instruments::Instrument,
    F: Fn(f64) -> finstack_quant_core::Result<MarketContext>,
{
    let bump_size = spot * bump_pct;
    if bump_size <= 0.0 {
        return Ok(0.0);
    }
    let up = bumped_market(bump_pct)?;
    let pv_up = instrument.value(&up, as_of)?.amount();
    let down = bumped_market(-bump_pct)?;
    let pv_dn = instrument.value(&down, as_of)?.amount();
    Ok((pv_up - 2.0 * base_pv + pv_dn) / (bump_size * bump_size))
}

/// Quote-per-base FX policy stamp: whether the matrix triangulated the pair.
///
/// `spot_overridden` marks a spot taken from outside the matrix (a quoted
/// spot or an explicit spot scalar), where triangulation does not apply.
pub(crate) fn fx_valuation_details(
    market: &MarketContext,
    base_currency: Currency,
    quote_currency: Currency,
    as_of: Date,
    spot_overridden: bool,
) -> finstack_quant_valuations_core::results::ValuationDetails {
    let fx_triangulated = if spot_overridden {
        None
    } else {
        market
            .fx()
            .and_then(|fx| {
                fx.rate(FxQuery::new(
                    base_currency,
                    quote_currency,
                    as_of,
                    Default::default(),
                ))
                .ok()
            })
            .map(|q| q.triangulated)
    };
    finstack_quant_valuations_core::results::ValuationDetails::Fx(
        finstack_quant_valuations_core::results::FxValuationDetails { fx_triangulated },
    )
}

/// Empty schedule of a contract whose cashflows have all settled.
pub(crate) fn settled_schedule(
    notional_hint: Option<finstack_quant_core::money::Money>,
) -> finstack_quant_core::Result<finstack_quant_cashflows::builder::CashFlowSchedule> {
    finstack_quant_cashflows::schedule_from_classified_flows(
        Vec::new(),
        DayCount::Act365F,
        finstack_quant_cashflows::ScheduleBuildOpts {
            notional_hint,
            meta: finstack_quant_cashflows::builder::CashFlowMeta {
                representation:
                    finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual,
                ..Default::default()
            },
        },
    )
}

/// One volatility scenario: the instrument and market to reprice.
pub(crate) type VolScenario<I> = (I, MarketContext);

/// Applied half-width with the up and down volatility scenarios.
pub(crate) type VolScenarios<I> = (f64, VolScenario<I>, VolScenario<I>);

/// Central volatility bump on the source pricing reads.
///
/// An instrument with an `implied_volatility` override is bumped on that
/// quote (the market is left unchanged); otherwise the named surface is bumped
/// in parallel. With an override the half-width is capped at half the quoted
/// vol so the down scenario stays positive.
///
/// `overrides` must be the instrument's own pricing overrides; they decide
/// whether the quote or the surface is the active source.
///
/// Returns `(bump_abs, up, down)`, where `bump_abs` is the half-width actually
/// applied (absolute vol, `0.01` = one vol point).
pub(crate) fn active_vol_scenarios<I>(
    instrument: &I,
    overrides: &InstrumentPricingOverrides,
    market: &MarketContext,
    surface_id: &str,
    requested_bump_abs: f64,
) -> finstack_quant_core::Result<VolScenarios<I>>
where
    I: finstack_quant_valuations_core::instruments::Instrument + Clone,
{
    use finstack_quant_valuations_core::metrics::bump_active_volatility;

    let bump_abs = overrides
        .market_quotes
        .implied_volatility
        .map_or(requested_bump_abs, |vol| requested_bump_abs.min(vol * 0.5));
    let up = bump_active_volatility(instrument, market, surface_id, bump_abs)?;
    let down = bump_active_volatility(instrument, market, surface_id, -bump_abs)?;
    Ok((bump_abs, up, down))
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
            .set_quotes(&[(Currency::EUR, Currency::USD, 1.1)])
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
            spot_id: None,
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
            spot_id: Some(&spot_id),
            rate_context: "test",
        })
        .expect("inputs should resolve");

        assert_eq!(inputs.spot, 1.1);
    }
}
