//! Theta calculator for interest rate options.
//!
//! Computes theta via a bump-and-reprice approach: reprice the instrument
//! at `as_of + period` (default 1D) holding market curves and vol surface fixed.

use crate::cap_floor::CapFloor;
use crate::cap_floor::pricing::projection::resolve_optioned_coupon;
use finstack_quant_core::Result;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::fixings::{fixing_series_id, validate_fixing_index_id};
use finstack_quant_core::market_data::scalars::{
    ScalarTimeSeries, SeriesExtrapolation, SeriesInterpolation,
};
use finstack_quant_valuations_core::metrics::sensitivities::theta::calculate_theta_date;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// Theta calculator (bump-and-reprice with customizable period)
pub(crate) struct ThetaCalculator;

impl MetricCalculator for ThetaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &CapFloor = context.instrument_as()?;

        // Get theta period from pricing overrides, default to "1D"
        let period =
            finstack_quant_valuations_core::metrics::sensitivities::theta::theta_period(context);

        let expiry_date = next_option_theta_expiry(option, context.curves.as_ref(), context.as_of)?;

        let rolled_date = calculate_theta_date(context.as_of, period, expiry_date)?;

        // If already expired or rolling to same date, theta is zero
        if rolled_date <= context.as_of {
            return Ok(0.0);
        }

        // Base PV from context
        let base_pv = context.base_value.amount();

        // When rolling past a term cap/floor's same-day reset on a multi-period
        // contract, carry today's projected index rate into FIXING:<index> if no
        // historical fixing was supplied yet.
        let rolled = if let Some(rolled_market) = rolled_market_with_same_day_fixings(
            option,
            context.curves.as_ref(),
            context.as_of,
            rolled_date,
        )? {
            context.reprice_money(&rolled_market, rolled_date)?
        } else {
            context.reprice_money(&context.curves, rolled_date)?
        };

        Ok(rolled.amount() - base_pv)
    }
}

fn term_fixing_series_id(option: &CapFloor) -> Result<String> {
    let index_id = validate_fixing_index_id(option.forward_curve_id.as_str())?;
    Ok(fixing_series_id(index_id))
}

fn rolled_market_with_same_day_fixings(
    option: &CapFloor,
    market: &finstack_quant_core::market_data::context::MarketContext,
    as_of: Date,
    rolled_date: Date,
) -> Result<Option<std::sync::Arc<finstack_quant_core::market_data::context::MarketContext>>> {
    if option.overnight_coupon.is_some() {
        return Ok(None);
    }
    let series_id = term_fixing_series_id(option)?;
    let existing = match market.get_series(&series_id) {
        Ok(series) => Some(series),
        Err(err) if err.kind() == finstack_quant_core::ErrorKind::NotFound => None,
        Err(err) => return Err(err),
    };
    let mut additions = Vec::new();
    for period in option.pricing_periods()? {
        if period.payment_date <= rolled_date {
            continue;
        }
        let fixing_date = period.reset_date.unwrap_or(period.accrual_start);
        if fixing_date < as_of || fixing_date >= rolled_date {
            continue;
        }
        let already_recorded = existing
            .as_ref()
            .is_some_and(|series| series.get_value_exact(fixing_date).is_ok())
            || additions.iter().any(|&(d, _)| d == fixing_date);
        if !already_recorded {
            let projected = resolve_optioned_coupon(option, &period, market, as_of)?;
            additions.push((fixing_date, projected.forward - projected.additive_spread));
        }
    }
    if additions.is_empty() {
        return Ok(None);
    }
    let mut observations = existing
        .as_ref()
        .map_or_else(Vec::new, |s| s.observations());
    observations.extend(additions);
    let series = ScalarTimeSeries::new(
        series_id,
        observations,
        Some(option.notional.currency()),
        SeriesInterpolation::Step,
        SeriesExtrapolation::Error,
    )?;
    Ok(Some(std::sync::Arc::new(
        market.clone().insert_series(series),
    )))
}

fn next_option_theta_expiry(
    option: &CapFloor,
    market: &finstack_quant_core::market_data::context::MarketContext,
    as_of: Date,
) -> Result<Option<Date>> {
    let mut final_payment = None;
    let mut has_unpublished_same_day = false;
    for period in option.pricing_periods()? {
        if period.payment_date <= as_of {
            continue;
        }
        final_payment = Some(final_payment.map_or(period.payment_date, |current: Date| {
            current.max(period.payment_date)
        }));
        let fixed_before_valuation = if option.overnight_coupon.is_some() {
            period.accrual_end < as_of
        } else {
            period.reset_date.unwrap_or(period.accrual_start) < as_of
        };
        if fixed_before_valuation {
            continue;
        }
        let fixing_date = resolve_optioned_coupon(option, &period, market, as_of)?.fixing_date;
        if option.overnight_coupon.is_some() {
            if fixing_date >= as_of {
                return Ok(Some(fixing_date));
            }
        } else if fixing_date > as_of {
            return Ok(Some(fixing_date));
        } else if fixing_date == as_of {
            let series_id = term_fixing_series_id(option)?;
            let is_published = market
                .get_series(&series_id)
                .is_ok_and(|series| series.get_value_exact(fixing_date).is_ok());
            if !is_published {
                has_unpublished_same_day = true;
            }
        }
    }
    if has_unpublished_same_day {
        // Only cap theta at `as_of` when no later period with a future fixing
        // remains (e.g. a single caplet resetting today with no FIXING series).
        return Ok(Some(as_of));
    }
    Ok(final_payment)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cap_floor::RateOptionType;
    use crate::cap_floor::{OvernightCouponConvention, OvernightSpreadCompounding};
    use finstack_quant_cashflows::builder::FloatingLegCompounding;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::ForwardCurve;
    use finstack_quant_core::money::Money;
    use time::macros::date;

    #[test]
    fn theta_expiry_uses_shared_rfr_cutoff_fixing() {
        let as_of = date!(2024 - 01 - 02);
        let mut option = CapFloor::new(crate::cap_floor::CapFloorParams {
            id: "RFR-THETA-FIXING".into(),
            rate_option_type: RateOptionType::Caplet,
            notional: Money::from((1_000_000_i64, Currency::USD)),
            strike: 0.05,
            start_date: date!(2024 - 01 - 03),
            maturity: date!(2024 - 04 - 03),
            frequency: None,
            day_count: DayCount::Act360,
            discount_curve_id: "USD-OIS".into(),
            forward_curve_id: "USD-SOFR-OIS".into(),
            vol_surface_id: "USD-CAP-VOL".into(),
        })
        .expect("caplet");
        option.overnight_coupon = Some(OvernightCouponConvention {
            compounding: FloatingLegCompounding::CompoundedWithRateCutoff { cutoff_days: 1 },
            payment_lag_days: 2,
            fixing_calendar_id: Some("usny".into()),
            payment_calendar_id: Some("usny".into()),
            spread_compounding: OvernightSpreadCompounding::Exclude,
        });
        let market = MarketContext::new()
            .try_insert(
                ForwardCurve::flat("USD-SOFR-OIS", as_of, 0.05, DayCount::Act360).expect("forward"),
            )
            .expect("insert market curve");

        let next = next_option_theta_expiry(&option, &market, as_of)
            .expect("theta expiry")
            .expect("next fixing");

        assert_eq!(next, date!(2024 - 04 - 01));
    }

    #[test]
    fn theta_skips_paid_period_before_resolving_fixing() {
        let as_of = date!(2025 - 04 - 03);
        let option = CapFloor::new(crate::cap_floor::CapFloorParams {
            id: "PAID-THETA".into(),
            rate_option_type: RateOptionType::Caplet,
            notional: Money::from((1_000_000_i64, Currency::USD)),
            strike: 0.05,
            start_date: date!(2025 - 01 - 02),
            maturity: date!(2025 - 04 - 02),
            frequency: None,
            day_count: DayCount::Act360,
            discount_curve_id: "USD-OIS".into(),
            forward_curve_id: "TEST-TERM-3M".into(),
            vol_surface_id: "USD-CAP-VOL".into(),
        })
        .expect("caplet");

        let next = next_option_theta_expiry(&option, &MarketContext::new(), as_of)
            .expect("paid period should not resolve missing fixings");

        assert_eq!(next, None);
    }

    #[test]
    fn theta_uses_delayed_payment_after_contractual_maturity() {
        let as_of = date!(2025 - 04 - 03);
        let mut option = CapFloor::new(crate::cap_floor::CapFloorParams {
            id: "DELAYED-THETA".into(),
            rate_option_type: RateOptionType::Caplet,
            notional: Money::from((1_000_000_i64, Currency::USD)),
            strike: 0.05,
            start_date: date!(2025 - 01 - 02),
            maturity: date!(2025 - 04 - 02),
            frequency: None,
            day_count: DayCount::Act360,
            discount_curve_id: "USD-OIS".into(),
            forward_curve_id: "USD-SOFR-OIS".into(),
            vol_surface_id: "USD-CAP-VOL".into(),
        })
        .expect("caplet");
        option.overnight_coupon = Some(OvernightCouponConvention {
            compounding: FloatingLegCompounding::CompoundedWithRateCutoff { cutoff_days: 1 },
            payment_lag_days: 2,
            fixing_calendar_id: Some("usny".into()),
            payment_calendar_id: Some("usny".into()),
            spread_compounding: OvernightSpreadCompounding::Exclude,
        });

        let next = next_option_theta_expiry(&option, &MarketContext::new(), as_of)
            .expect("fixed delayed period should not re-resolve fixings");

        assert_eq!(next, Some(date!(2025 - 04 - 04)));
    }

    #[test]
    fn theta_expiry_does_not_freeze_on_reset_day_of_multi_period_cap() {
        use finstack_quant_core::market_data::surfaces::{
            VolInterpolationMode, VolQuoteType, VolSurface, VolSurfaceAxis,
        };
        use finstack_quant_core::market_data::term_structures::DiscountCurve;
        use finstack_quant_valuations_core::instruments::{Instrument, PricingOptions};
        use finstack_quant_valuations_core::metrics::MetricId;

        let as_of = date!(2024 - 01 - 02);
        let option = CapFloor::new(crate::cap_floor::CapFloorParams {
            id: "RESET-DAY-CAP".into(),
            rate_option_type: RateOptionType::Cap,
            notional: Money::from((1_000_000_i64, Currency::USD)),
            strike: 0.05,
            start_date: as_of,
            maturity: date!(2024 - 07 - 02),
            frequency: Some(finstack_quant_core::dates::Tenor::quarterly()),
            day_count: DayCount::Act360,
            discount_curve_id: "USD-OIS".into(),
            forward_curve_id: "TEST-TERM-3M".into(),
            vol_surface_id: "USD-CAP-VOL".into(),
        })
        .expect("cap");
        let surface = VolSurface::builder("USD-CAP-VOL")
            .secondary_axis(VolSurfaceAxis::Strike)
            .quote_type(VolQuoteType::BlackLognormal)
            .interpolation_mode(VolInterpolationMode::Vol)
            .expiries(&[0.25, 0.5, 1.0])
            .secondary_values(&[0.03, 0.05, 0.08])
            .row(&[0.25, 0.25, 0.25])
            .row(&[0.25, 0.25, 0.25])
            .row(&[0.25, 0.25, 0.25])
            .build()
            .expect("surface");
        let market = MarketContext::new()
            .try_insert(
                DiscountCurve::flat("USD-OIS", as_of, 0.05, DayCount::Act365F).expect("disc"),
            )
            .expect("insert disc")
            .try_insert(
                ForwardCurve::flat("TEST-TERM-3M", as_of, 0.05, DayCount::Act360).expect("fwd"),
            )
            .expect("insert fwd")
            .insert_surface(surface);

        let next = next_option_theta_expiry(&option, &market, as_of)
            .expect("theta expiry")
            .expect("future reset or payment date");
        assert!(
            next > as_of,
            "same-day fixing on multi-period cap must not clamp theta horizon to as_of, got {next}"
        );

        let theta = option
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::Theta],
                PricingOptions::default(),
            )
            .expect("multi-period cap theta on reset date")
            .measures["theta"];
        assert!(
            theta.is_finite() && theta.abs() > 0.0,
            "multi-period cap theta on reset day must reflect live caplet decay, got {theta}"
        );
    }
}
