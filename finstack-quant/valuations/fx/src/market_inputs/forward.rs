//! FX spot-basis conventions, date guards, and forward-rate input collection.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::money::fx::FxQuery;
use finstack_quant_core::types::CurveId;

/// Far-future expiry shared by long-dated FX `example()` constructors (`2030-06-21`).
pub(crate) const FAR_EXPIRY: Date = time::macros::date!(2030 - 06 - 21);

/// Return whether an FX contractual event is complete under the end-of-day policy.
///
/// FX fixing, expiry, delivery, and settlement cashflows remain live on their
/// event date and are extinguished on the following valuation date.
#[inline]
pub(crate) fn event_has_occurred(event_date: Date, as_of: Date) -> bool {
    event_date < as_of
}

/// Value-date basis of the selected ordered FX quote.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum FxSpotBasis {
    /// The selected quote is cash FX at the valuation date.
    Cash,
    /// The selected outright is quoted for a stated value date.
    ValueDate {
        /// Fixed value date of the spot quote.
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::DateWire")
        )]
        date: Date,
    },
}

impl FxSpotBasis {
    pub(crate) fn cash_rate(
        self,
        spot: f64,
        domestic: &impl Discounting,
        foreign: &impl Discounting,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        let cash = match self {
            Self::Cash => spot,
            Self::ValueDate { date } => {
                spot * (domestic.df_between_dates(as_of, date)?
                    / foreign.df_between_dates(as_of, date)?)
            }
        };
        finstack_quant_core::validation::validate_f64_positive(cash, "FX cash-equivalent rate")?;
        Ok(cash)
    }
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
    /// Value-date basis of the spot number.
    pub(crate) spot_basis: FxSpotBasis,
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
                finstack_quant_core::money::fx::FxConversionPolicy::CashflowDate,
            ))?
            .rate
    };
    if !spot.is_finite() || spot <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{} spot must be finite and positive, got {spot}",
            request.context
        )));
    }
    let spot =
        request
            .spot_basis
            .cash_rate(spot, domestic.as_ref(), foreign.as_ref(), request.as_of)?;
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
