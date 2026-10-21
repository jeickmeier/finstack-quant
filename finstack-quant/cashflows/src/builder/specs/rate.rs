//! Contractual coupon of a loan facility or note: fixed all-in rate or floating index plus spread.

use super::coupon::FloatingRateSpec;
use finstack_quant_core::dates::Date;
use finstack_quant_core::types::Rate;

/// Contractual coupon of a loan facility or note: a fixed all-in rate or a
/// floating index plus spread.
///
/// Shared by the valuations term loan, revolving credit facility, asset-backed
/// facility and structured-credit tranche coupons.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_cashflows::builder::RateSpec;
///
/// let fixed = RateSpec::Fixed { rate: 0.06 }; // 6% all-in
/// assert!(matches!(fixed, RateSpec::Fixed { .. }));
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[allow(clippy::large_enum_variant)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum RateSpec {
    /// Fixed all-in annual rate.
    Fixed {
        /// Annual rate as a decimal (`0.06` = 6%).
        rate: f64,
    },
    /// Floating index plus spread, with the canonical floor, cap, gearing and
    /// reset conventions of [`FloatingRateSpec`](super::FloatingRateSpec).
    Floating(FloatingRateSpec),
}

impl RateSpec {
    /// Fixed-rate spec from a typed rate.
    ///
    /// # Arguments
    ///
    /// * `rate` - All-in annual rate; stored as its decimal value.
    pub fn fixed_rate(rate: Rate) -> Self {
        Self::Fixed {
            rate: rate.as_decimal(),
        }
    }

    /// Contractual rate for an explicit accrual period, as a decimal.
    ///
    /// A floating rate whose reset date (accrual start less `reset_lag_days`
    /// business days on the fixing calendar) is on or before `as_of` reads the
    /// fixing series; later resets project from the forward curve.
    ///
    /// # Arguments
    ///
    /// * `accrual_start` - Start of the accrual period, also the effective date
    ///   used to sample a projected term-index quote.
    /// * `accrual_end` - Exclusive end of the full contractual coupon period.
    /// * `accrual_year_fraction` - Positive finite contractual coupon accrual,
    ///   used to convert the curve-basis quote before spreads and rate bounds.
    /// * `as_of` - Valuation date separating observed fixings from projections.
    /// * `market` - Market holding the forward curve and fixings.
    ///
    /// # Errors
    ///
    /// Returns an error if the fixing calendar is unknown, a required fixing
    /// is missing, a seasoned compounded-overnight coupon is requested, or
    /// the projection fails.
    pub fn try_rate_for_period(
        &self,
        accrual_start: Date,
        accrual_end: Date,
        accrual_year_fraction: f64,
        as_of: Date,
        market: &finstack_quant_core::market_data::context::MarketContext,
    ) -> finstack_quant_core::Result<f64> {
        match self {
            Self::Fixed { rate } => Ok(*rate),
            Self::Floating(spec) => {
                let params = crate::builder::FloatingRateParams::try_from(spec)?;
                let calendar_id = spec
                    .fixing_calendar_id
                    .as_deref()
                    .unwrap_or(finstack_quant_core::dates::WEEKENDS_ONLY_ID);
                let calendar = finstack_quant_core::dates::calendar_by_id_strict(calendar_id)?;
                let reset_date = finstack_quant_core::dates::DateExt::add_business_days(
                    accrual_start,
                    -spec.reset_lag_days,
                    calendar,
                )?;
                // Tranche coupons treat a reset on the valuation date as fixed:
                // it needs the day's recorded fixing and is never projected.
                if reset_date <= as_of {
                    if spec.compounding.is_some() {
                        return Err(finstack_quant_core::Error::Validation(
                            "seasoned compounded-overnight tranche coupons require a canonical compounded fixing schedule"
                                .into(),
                        ));
                    }
                    let fixings = finstack_quant_core::market_data::fixings::get_fixing_series(
                        market,
                        spec.forward_curve_id.as_str(),
                    )?;
                    let raw =
                        finstack_quant_core::market_data::fixings::require_fixing_value_exact(
                            Some(fixings),
                            spec.forward_curve_id.as_str(),
                            reset_date,
                            as_of,
                        )?;
                    return Ok(crate::builder::calculate_floating_rate(raw, &params));
                }
                let fwd = market.get_forward(spec.forward_curve_id.as_str())?;
                crate::builder::project_floating_rate(
                    accrual_start,
                    fwd.as_ref(),
                    &params,
                    accrual_start,
                    accrual_end,
                    accrual_year_fraction,
                )
            }
        }
    }
}
