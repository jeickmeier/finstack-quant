//! Theta calculator for interest rate options.
//!
//! Computes theta via a bump-and-reprice approach: reprice the instrument
//! at `as_of + period` (default 1D) holding market curves and vol surface fixed.

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

        let expiry_date = next_option_theta_expiry(option, context.as_of)?;

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

fn next_option_theta_expiry(option: &CapFloor, as_of: Date) -> Result<Option<Date>> {
    let next_fixing = option
        .pricing_periods()?
        .into_iter()
        .map(|period| option.option_fixing_date(&period))
        .find(|fixing_date| *fixing_date > as_of);
    Ok(next_fixing.or(Some(option.maturity)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::rates::cap_floor::{CapFloorVolType, RateOptionType};
    use crate::instruments::{ExerciseStyle, SettlementType};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
    use finstack_quant_core::money::Money;
    use rust_decimal::Decimal;
    use time::macros::date;

    #[test]
    fn theta_expiry_uses_rfr_accrual_end_fixing() {
        let option = CapFloor {
            id: "RFR-THETA-FIXING".into(),
            rate_option_type: RateOptionType::Cap,
            notional: Money::new(1_000_000.0, Currency::USD),
            strike: Decimal::try_from(0.05).expect("valid decimal"),
            start_date: date!(2024 - 01 - 03),
            maturity: date!(2024 - 07 - 03),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            stub: StubKind::None,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            exercise_style: ExerciseStyle::European,
            settlement: SettlementType::Cash,
            discount_curve_id: "USD-OIS".into(),
            forward_curve_id: "USD-SOFR-OIS".into(),
            vol_surface_id: "USD-CAP-VOL".into(),
            vol_type: CapFloorVolType::Lognormal,
            vol_shift: 0.0,
            pricing_overrides: crate::instruments::PricingOverrides::default(),
            attributes: Default::default(),
        };

        let next = next_option_theta_expiry(&option, date!(2024 - 02 - 01))
            .expect("theta expiry")
            .expect("next fixing");

        assert_eq!(next, date!(2024 - 04 - 03));
    }
}
