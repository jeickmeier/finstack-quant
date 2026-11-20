//! Dollar roll pricing.
//!
//! Dollar roll value is the net of the two TBA legs plus any
//! implied carry benefit or cost.

use super::DollarRoll;
use crate::tba::pricer::price_tba;
use finstack_quant_core::Result;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;

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

    Money::from_model_value(value, roll.notional.currency())
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::InterpStyle;
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
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .expect("valid curve");

        MarketContext::new()
            .try_insert(disc)
            .expect("insert market curve")
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

    #[test]
    fn roll_mark_and_cash_reconcile_with_the_delivered_pool() {
        use finstack_quant_valuations_core::instruments::Instrument;
        use finstack_quant_valuations_core::metrics::collect_period_cash;

        let roll = DollarRoll::example().expect("dollar roll");
        let cases = [
            (roll.front_leg().expect("front leg"), -1.0),
            (roll.back_leg().expect("back leg"), 1.0),
        ];
        let base = time::macros::date!(2024 - 01 - 01);
        let discount = DiscountCurve::builder("USD-OIS")
            .base_date(base)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .interp(InterpStyle::LogLinear)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .knots([(0.0, 1.0), (40.0, 1.0)])
            .build()
            .expect("zero-rate curve");
        let market = MarketContext::new()
            .try_insert(discount)
            .expect("discount curve");
        assert!(!roll.includes_valuation_date_cashflows());
        for (leg, delivered_sign) in cases {
            let payment = leg.effective_settlement_date().expect("settlement date");
            let start = payment.previous_day().expect("day before delivery");
            let currency = roll.notional.currency();
            let opening = roll.value(&market, start).expect("open dollar roll");
            let closing = roll.value(&market, payment).expect("remaining dollar roll");
            let cash = collect_period_cash(&roll, &market, start, payment, currency)
                .expect("settlement invoice");
            let pool = crate::tba::pricer::resolve_assumed_pool(&leg).expect("delivery pool");
            let delivery = pool.value(&market, payment).expect("delivered pool value");
            let closing_position = closing.amount() + delivered_sign * delivery.amount();
            assert_ne!(cash.total.amount(), 0.0);
            assert!((opening.amount() - closing_position - cash.total.amount()).abs() < 1e-6);
        }
    }
}
