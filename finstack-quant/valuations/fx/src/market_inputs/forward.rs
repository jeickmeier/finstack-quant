//! FX spot-basis conventions, date guards, and forward-rate input collection.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DateExt, HolidayCalendar, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::money::fx::FxQuery;
use finstack_quant_core::types::CurveId;

/// Look up the strict composite holiday calendar for a currency pair.
pub(crate) fn joint_calendar(
    base_calendar_id: &str,
    quote_calendar_id: &str,
) -> finstack_quant_core::Result<&'static dyn HolidayCalendar> {
    finstack_quant_core::dates::calendar_by_id_strict(&format!(
        "{base_calendar_id}+{quote_calendar_id}"
    ))
}

/// Spot date of an FX trade under the product's explicit rule:
/// `trade_date` plus `settlement_days` business days on `calendar`, which
/// must be open on the result (record CORE-CONVENTIONS-011 #6).
pub(crate) fn spot_date_from_lag(
    trade_date: Date,
    settlement_days: u32,
    calendar: &dyn HolidayCalendar,
) -> finstack_quant_core::Result<Date> {
    let lag = i32::try_from(settlement_days).map_err(|_| {
        finstack_quant_core::Error::Validation(format!(
            "FX spot lag {settlement_days} is out of range"
        ))
    })?;
    let spot = trade_date.add_business_days(lag, calendar)?;
    if !calendar.is_business_day(spot)? {
        return Err(finstack_quant_core::Error::Validation(format!(
            "FX spot date {spot} is not a business day of the settlement calendar"
        )));
    }
    Ok(spot)
}

/// Roll `trade_date` to `(spot_date, maturity)` on `calendar` via spot lag and `fx_forward_date`.
pub(crate) fn roll_fx_spot_and_maturity(
    trade_date: Date,
    settlement_days: u32,
    tenor: Tenor,
    calendar: &dyn HolidayCalendar,
    business_day_convention: BusinessDayConvention,
    preserve_business_eom: bool,
) -> finstack_quant_core::Result<(Date, Date)> {
    let spot_date = spot_date_from_lag(trade_date, settlement_days, calendar)?;
    let maturity = finstack_quant_core::dates::fx::fx_forward_date(
        spot_date,
        tenor,
        business_day_convention,
        preserve_business_eom,
        calendar,
    )?;
    Ok((spot_date, maturity))
}

/// Resolve an ordered FX spot quote from an explicit override or the market's `FxMatrix`.
pub(crate) fn resolve_fx_matrix_spot(
    market: &MarketContext,
    from_currency: Currency,
    to_currency: Currency,
    as_of: Date,
    quoted_spot: Option<f64>,
    missing_matrix_message: Option<&str>,
) -> finstack_quant_core::Result<f64> {
    if let Some(spot) = quoted_spot {
        return Ok(spot);
    }
    let matrix = match (market.fx(), missing_matrix_message) {
        (Some(matrix), _) => matrix,
        (None, Some(message)) => {
            return Err(finstack_quant_core::Error::Validation(message.to_string()));
        }
        (None, None) => market.fx_required()?,
    };
    Ok((**matrix)
        .rate(FxQuery::new(
            from_currency,
            to_currency,
            as_of,
            finstack_quant_core::money::fx::FxConversionPolicy::CashflowDate,
        ))?
        .rate)
}

/// Checked covered-interest-parity forward `spot * df_numerator / df_denominator`.
pub(crate) fn cip_forward_from_dfs(
    spot: f64,
    df_numerator: f64,
    df_denominator: f64,
    context: &str,
) -> finstack_quant_core::Result<f64> {
    if !df_denominator.is_finite() || df_denominator < 1e-12 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{context} denominator discount factor ({df_denominator}) cannot produce a finite FX forward"
        )));
    }
    if !df_numerator.is_finite() || df_numerator <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{context} numerator discount factor ({df_numerator}) cannot produce a finite FX forward"
        )));
    }
    let forward = spot * df_numerator / df_denominator;
    if !forward.is_finite() || forward <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{context} FX forward must be positive and finite, got {forward}"
        )));
    }
    Ok(forward)
}

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
    let raw_spot = resolve_fx_matrix_spot(
        request.market,
        request.base_currency,
        request.quote_currency,
        request.as_of,
        request.quoted_spot,
        None,
    )?;
    if !raw_spot.is_finite() || raw_spot <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{} spot must be finite and positive, got {raw_spot}",
            request.context
        )));
    }
    let spot = request.spot_basis.cash_rate(
        raw_spot,
        domestic.as_ref(),
        foreign.as_ref(),
        request.as_of,
    )?;
    let _ = cip_forward_from_dfs(spot, df_foreign, df_domestic, request.context)?;
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
