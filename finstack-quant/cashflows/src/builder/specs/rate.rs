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
    ///
    /// [`RateSpec::try_rate_for_period`] aligns `compounding` (`None` or
    /// `Simple`), `index_tenor` validation, and `fallback` for missing market
    /// sources with the cashflow builder; single-period calls still treat
    /// `reset_date <= as_of` as a required historical fixing and use the
    /// weekends-only calendar when `fixing_calendar_id` is omitted.
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
    /// fixing series (consulting `fallback` when the series itself is absent);
    /// later resets project from the forward curve at the reset date, enforcing
    /// `index_tenor` against `ForwardCurve::index_tenor()` and consulting `fallback`
    /// only when the curve is absent or does not reach `reset_date`.
    ///
    /// # Differences from the cashflow builder
    ///
    /// This path serves loan and tranche coupons and keeps two intentional
    /// single-period conventions:
    ///
    /// - With no `fixing_calendar_id`, the reset lag counts weekends-only
    ///   business days; the builder uses the coupon schedule calendar.
    /// - A reset on `as_of` requires that day's recorded fixing when a fixing
    ///   series is present; the builder uses it when published and otherwise
    ///   projects.
    /// - Overnight compounding (`SimpleAverage` / `CompoundedInArrears` /
    ///   `CompoundedWithObservationShift` / `CompoundedWithRateCutoff`) is
    ///   rejected because a single-period call carries no observation schedule;
    ///   `None` and `Simple` are both accepted.
    ///
    /// # Arguments
    ///
    /// * `accrual_start` - Start of the accrual period; the reset date is this
    ///   date less `reset_lag_days` business days.
    /// * `accrual_end` - Exclusive end of the full contractual coupon period.
    /// * `accrual_year_fraction` - Positive finite contractual coupon accrual,
    ///   used to convert the curve-basis quote before spreads and rate bounds.
    /// * `as_of` - Valuation date separating observed fixings from projections.
    /// * `market` - Market holding the forward curve and fixings.
    ///
    /// # Errors
    ///
    /// Returns an error if a fixed rate is not finite, `reset_lag_days` is
    /// negative, an overnight `compounding` method is set, `index_tenor`
    /// disagrees with the resolved forward curve tenor by more than 10%, the
    /// fixing calendar is unknown, a required observation in a supplied fixing
    /// series is missing, or curve projection fails.
    pub fn try_rate_for_period(
        &self,
        accrual_start: Date,
        accrual_end: Date,
        accrual_year_fraction: f64,
        as_of: Date,
        market: &finstack_quant_core::market_data::context::MarketContext,
    ) -> finstack_quant_core::Result<f64> {
        match self {
            Self::Fixed { rate } => {
                if !rate.is_finite() {
                    return Err(finstack_quant_core::Error::Validation(
                        "fixed rate must be finite".into(),
                    ));
                }
                Ok(*rate)
            }
            Self::Floating(spec) => {
                if spec.reset_lag_days < 0 {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "reset_lag_days must be non-negative (0 = on accrual start, >0 = prior business days); got {}",
                        spec.reset_lag_days
                    )));
                }
                if let Some(compounding) = spec.compounding.filter(|c| c.is_overnight()) {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "RateSpec::try_rate_for_period supports only term-rate coupons \
                         (`compounding` unset or `Simple`); got {compounding:?}"
                    )));
                }
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
                // an existing fixing series must carry that day's fixing; only an
                // absent series consults `fallback`.
                if reset_date <= as_of {
                    match finstack_quant_core::market_data::fixings::get_fixing_series(
                        market,
                        spec.index_id.as_str(),
                    ) {
                        Ok(fixings) => {
                            let raw = finstack_quant_core::market_data::fixings::require_fixing_value_exact(
                                Some(fixings),
                                spec.index_id.as_str(),
                                reset_date,
                            )?;
                            return Ok(crate::builder::calculate_floating_rate(raw, &params));
                        }
                        Err(err) => {
                            if !matches!(spec.fallback, crate::builder::FloatingRateFallback::Error)
                            {
                                tracing::warn!(
                                    reset_date = %reset_date,
                                    spread_bp = %spec.spread_bp,
                                    fallback = ?spec.fallback,
                                    error = %err,
                                    "Floating fixing series unavailable, using configured fallback"
                                );
                            }
                            let raw = spec.fallback.index_rate(err)?;
                            return Ok(crate::builder::calculate_floating_rate(raw, &params));
                        }
                    }
                }
                match market.get_forward(spec.forward_curve_id.as_str()) {
                    Ok(fwd) => {
                        crate::builder::pipeline::validate_index_tenor_match(
                            spec.index_tenor,
                            fwd.as_ref(),
                            spec.forward_curve_id.as_str(),
                        )?;
                        if reset_date < fwd.base_date() {
                            let err = finstack_quant_core::Error::Validation(format!(
                                "floating-rate observation date {} is before the '{}' curve base date {}",
                                reset_date,
                                fwd.id(),
                                fwd.base_date(),
                            ));
                            if !matches!(spec.fallback, crate::builder::FloatingRateFallback::Error)
                            {
                                tracing::warn!(
                                    reset_date = %reset_date,
                                    spread_bp = %spec.spread_bp,
                                    fallback = ?spec.fallback,
                                    error = %err,
                                    "Floating reset precedes curve base date, using configured fallback"
                                );
                            }
                            let raw = spec.fallback.index_rate(err)?;
                            return Ok(crate::builder::calculate_floating_rate(raw, &params));
                        }
                        crate::builder::project_floating_rate(
                            reset_date,
                            fwd.as_ref(),
                            &params,
                            accrual_start,
                            accrual_end,
                            accrual_year_fraction,
                        )
                    }
                    Err(err)
                        if matches!(
                            err,
                            finstack_quant_core::Error::Input(
                                finstack_quant_core::InputError::NotFound { .. }
                                    | finstack_quant_core::InputError::MissingCurve { .. }
                            )
                        ) =>
                    {
                        if !matches!(spec.fallback, crate::builder::FloatingRateFallback::Error) {
                            tracing::warn!(
                                reset_date = %reset_date,
                                spread_bp = %spec.spread_bp,
                                fallback = ?spec.fallback,
                                error = %err,
                                "Forward curve unavailable, using configured fallback"
                            );
                        }
                        let raw = spec.fallback.index_rate(err)?;
                        Ok(crate::builder::calculate_floating_rate(raw, &params))
                    }
                    Err(err) => Err(err),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::{FloatingLegCompounding, FloatingRateFallback, FloatingRateSpec};
    use finstack_quant_core::dates::{DayCount, Tenor};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::ForwardCurve;
    use finstack_quant_core::types::CurveId;
    use rust_decimal::Decimal;
    use time::Month;

    #[test]
    fn past_coupon_uses_the_contractual_index_after_projection_curve_replacement() {
        use finstack_quant_core::market_data::scalars::{
            ScalarTimeSeries, SeriesExtrapolation, SeriesInterpolation,
        };
        let start = Date::from_calendar_date(2025, Month::January, 2).expect("reset");
        let end = Date::from_calendar_date(2025, Month::April, 2).expect("payment");
        let observed = |index, rate| {
            ScalarTimeSeries::new(
                index,
                vec![(start, rate)],
                None,
                SeriesInterpolation::Step,
                SeriesExtrapolation::Error,
            )
            .expect("observed series")
        };
        let proxy_only = MarketContext::new().insert_series(observed("FIXING:PROXY", 0.05));
        let market = proxy_only
            .clone()
            .insert_series(observed("FIXING:EUR-EURIBOR-3M", 0.04));
        let mut spec = FloatingRateSpec::euribor_3m(Decimal::ZERO);
        spec.index_id = "EUR-EURIBOR-3M".into();
        spec.reset_lag_days = 0;
        for projection in ["EUR-EURIBOR-3M", "PROXY"] {
            spec.forward_curve_id = projection.into();
            let coupon = RateSpec::Floating(spec.clone());
            let rate = coupon
                .try_rate_for_period(start, end, 0.25, end, &market)
                .expect("historical rate");
            assert!((100_000_000.0 * 0.25 * rate - 1_000_000.0).abs() < 1e-8);
            assert!(
                coupon
                    .try_rate_for_period(start, end, 0.25, end, &proxy_only)
                    .is_err()
            );
        }
    }

    #[test]
    fn try_rate_for_period_validates_inputs_and_enforces_reset_date_cutoff() {
        let base = Date::from_calendar_date(2025, Month::April, 4).expect("valid date");
        let start = Date::from_calendar_date(2025, Month::April, 7).expect("valid date");
        let end = Date::from_calendar_date(2025, Month::July, 7).expect("valid date");
        let fwd = ForwardCurve::from_zero_rates(
            "USD-SOFR-3M",
            base,
            &[(0.0, 0.03), (0.25, 0.05), (1.0, 0.08)],
            finstack_quant_core::math::Compounding::Continuous,
            DayCount::Act360,
            finstack_quant_core::math::interp::InterpStyle::LogLinear,
            finstack_quant_core::math::interp::ExtrapolationPolicy::Extend,
        )
        .expect("valid curve");
        let expected_proj = fwd
            .rate_between_dates(start, end, 91.0 / 360.0)
            .expect("factor projection");
        let market = MarketContext::new().insert(fwd);

        assert!(
            RateSpec::Fixed { rate: f64::NAN }
                .try_rate_for_period(start, end, 0.25, base, &market)
                .is_err()
        );

        let base_float = FloatingRateSpec {
            index_id: "USD-SOFR-3M".into(),
            forward_curve_id: CurveId::new("USD-SOFR-3M"),
            spread_bp: Decimal::ZERO,
            gearing: Decimal::ONE,
            gearing_includes_spread: true,
            index_floor_bp: None,
            index_cap_bp: None,
            all_in_floor_bp: None,
            all_in_cap_bp: None,
            overnight_index_constraints: Default::default(),
            reset_frequency: Tenor::quarterly(),
            index_tenor: None,
            reset_lag_days: 2,
            fixing_calendar_id: None,
            compounding: None,
            overnight_basis: None,
            fallback: FloatingRateFallback::Error,
        };

        let mut neg_lag = base_float.clone();
        neg_lag.reset_lag_days = -2;
        assert!(
            RateSpec::Floating(neg_lag)
                .try_rate_for_period(start, end, 0.25, base, &market)
                .is_err()
        );

        let mut compounded = base_float.clone();
        compounded.compounding =
            Some(FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 });
        assert!(
            RateSpec::Floating(compounded)
                .try_rate_for_period(start, end, 0.25, base, &market)
                .is_err()
        );

        let mut zero_lag = base_float.clone();
        zero_lag.reset_lag_days = 0;

        // With reset_lag_days = 2, reset_date precedes curve base_date (2025-04-04) and errors without FIXING:
        assert!(
            RateSpec::Floating(base_float)
                .try_rate_for_period(start, end, 91.0 / 360.0, base, &market)
                .is_err(),
            "reset_lag_days = 2 shifts reset before base_date and requires observed fixing"
        );

        let r_lag0 = RateSpec::Floating(zero_lag)
            .try_rate_for_period(start, end, 91.0 / 360.0, base, &market)
            .expect("lag 0 projection");
        assert!((r_lag0 - expected_proj).abs() < 1e-14);
    }
}
