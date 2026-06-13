//! Dollar roll pricing.
//!
//! Dollar roll value is the net of the two TBA legs plus any
//! implied carry benefit or cost.

use super::DollarRoll;
use crate::instruments::fixed_income::tba::pricer::price_tba;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;
use finstack_core::Result;

/// Price a dollar roll.
///
/// The dollar roll value is computed as:
/// - Front leg: Short TBA (receive price at front settlement)
/// - Back leg: Long TBA (pay price at back settlement)
///
/// Net value = Back leg value - Front leg value
///
/// Positive value means the back purchase is more valuable than the front
/// sale from the buyer's perspective.
pub(crate) fn price_dollar_roll(
    roll: &DollarRoll,
    market: &MarketContext,
    as_of: Date,
) -> Result<Money> {
    let front_leg = roll.front_leg()?;
    let back_leg = roll.back_leg()?;

    let front_value = price_tba(&front_leg, market, as_of)?;
    let back_value = price_tba(&back_leg, market, as_of)?;

    // TBA value from buyer's perspective = pool_value - trade_payment.
    // Dollar roll is short front (sell) + long back (buy), so:
    //   roll_value = back_value - front_value
    let value = back_value.amount() - front_value.amount();

    Ok(Money::new(value, roll.notional.currency()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_core::math::interp::InterpStyle;
    use time::Month;

    fn create_test_market(as_of: Date) -> MarketContext {
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([
                (0.0, 1.0),
                (0.25, 0.99),
                (1.0, 0.96),
                (5.0, 0.80),
                (30.0, 0.30),
            ])
            .interp(InterpStyle::Linear)
            .build()
            .expect("valid curve");

        MarketContext::new().insert(disc)
    }

    #[test]
    fn test_price_dollar_roll() {
        let roll = DollarRoll::example().expect("DollarRoll example is valid");
        let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid");
        let market = create_test_market(as_of);

        let pv = price_dollar_roll(&roll, &market, as_of).expect("should price");

        // Value should be reasonable
        assert!(pv.amount().abs() < roll.notional.amount());
    }
}
