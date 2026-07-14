//! Theta calculator for interest rate options.
//!
//! Computes theta via a bump-and-reprice approach: reprice the instrument
//! at `as_of + period` (default 1D) holding market curves and vol surface fixed.

use crate::instruments::rates::cap_floor::pricing::projection::resolve_optioned_coupon;
use crate::instruments::rates::cap_floor::CapFloor;
use crate::metrics::calculate_theta_date;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::dates::Date;
use finstack_quant_core::Result;

/// Theta calculator (bump-and-reprice with customizable period)
pub(crate) struct ThetaCalculator;

impl MetricCalculator for ThetaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &CapFloor = context.instrument_as()?;

        // Get theta period from pricing overrides, default to "1D"
        let period_str = context
            .get_metric_overrides()
            .and_then(|po| po.theta_period.as_deref())
            .unwrap_or("1D");

        let expiry_date = next_option_theta_expiry(option, context.curves.as_ref(), context.as_of)?;

        let rolled_date = calculate_theta_date(context.as_of, period_str, expiry_date)?;

        // If already expired or rolling to same date, theta is zero
        if rolled_date <= context.as_of {
            return Ok(0.0);
        }

        // Base PV from context
        let base_pv = context.base_value.amount();

        // Reprice at rolled date with same market context
        let bumped = context.instrument_value_with_scenario(&context.curves, rolled_date)?;

        Ok(bumped.amount() - base_pv)
    }
}

fn next_option_theta_expiry(
    option: &CapFloor,
    market: &finstack_quant_core::market_data::context::MarketContext,
    as_of: Date,
) -> Result<Option<Date>> {
    let mut final_payment = None;
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
        if fixing_date >= as_of {
            // Under the start-of-day policy a same-day fixing is still unpublished,
            // but rolling beyond it would require a fixing that is not in today's
            // market snapshot. Cap the standard theta horizon at the fixing itself.
            return Ok(Some(fixing_date));
        }
    }
    Ok(final_payment)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::rates::cap_floor::{
        OvernightCouponConvention, OvernightSpreadCompounding,
    };
    use crate::instruments::rates::irs::FloatingLegCompounding;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::ForwardCurve;
    use finstack_quant_core::money::Money;
    use time::macros::date;

    #[test]
    fn theta_expiry_uses_shared_rfr_cutoff_fixing() {
        let as_of = date!(2024 - 01 - 02);
        let mut option = CapFloor::new_caplet(
            "RFR-THETA-FIXING",
            Money::new(1_000_000.0, Currency::USD),
            0.05,
            date!(2024 - 01 - 03),
            date!(2024 - 04 - 03),
            DayCount::Act360,
            "USD-OIS",
            "USD-SOFR-OIS",
            "USD-CAP-VOL",
        )
        .expect("caplet");
        option.overnight_coupon = Some(OvernightCouponConvention {
            compounding: FloatingLegCompounding::CompoundedWithRateCutoff { cutoff_days: 1 },
            payment_delay_days: 2,
            fixing_calendar_id: Some("usny".into()),
            payment_calendar_id: Some("usny".into()),
            spread_compounding: OvernightSpreadCompounding::Exclude,
        });
        let market = MarketContext::new().insert(
            ForwardCurve::builder("USD-SOFR-OIS", 1.0 / 360.0)
                .base_date(as_of)
                .day_count(DayCount::Act360)
                .knots([(0.0, 0.05), (1.0, 0.05)])
                .build()
                .expect("forward"),
        );

        let next = next_option_theta_expiry(&option, &market, as_of)
            .expect("theta expiry")
            .expect("next fixing");

        assert_eq!(next, date!(2024 - 04 - 01));
    }

    #[test]
    fn theta_skips_paid_period_before_resolving_fixing() {
        let as_of = date!(2025 - 04 - 03);
        let option = CapFloor::new_caplet(
            "PAID-THETA",
            Money::new(1_000_000.0, Currency::USD),
            0.05,
            date!(2025 - 01 - 02),
            date!(2025 - 04 - 02),
            DayCount::Act360,
            "USD-OIS",
            "TEST-TERM-3M",
            "USD-CAP-VOL",
        )
        .expect("caplet");

        let next = next_option_theta_expiry(&option, &MarketContext::new(), as_of)
            .expect("paid period should not resolve missing fixings");

        assert_eq!(next, None);
    }

    #[test]
    fn theta_uses_delayed_payment_after_contractual_maturity() {
        let as_of = date!(2025 - 04 - 03);
        let mut option = CapFloor::new_caplet(
            "DELAYED-THETA",
            Money::new(1_000_000.0, Currency::USD),
            0.05,
            date!(2025 - 01 - 02),
            date!(2025 - 04 - 02),
            DayCount::Act360,
            "USD-OIS",
            "USD-SOFR-OIS",
            "USD-CAP-VOL",
        )
        .expect("caplet");
        option.overnight_coupon = Some(OvernightCouponConvention {
            compounding: FloatingLegCompounding::CompoundedWithRateCutoff { cutoff_days: 1 },
            payment_delay_days: 2,
            fixing_calendar_id: Some("usny".into()),
            payment_calendar_id: Some("usny".into()),
            spread_compounding: OvernightSpreadCompounding::Exclude,
        });

        let next = next_option_theta_expiry(&option, &MarketContext::new(), as_of)
            .expect("fixed delayed period should not re-resolve fixings");

        assert_eq!(next, Some(date!(2025 - 04 - 04)));
    }
}
