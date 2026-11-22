//! Unrounded schedule PV helper for rates calibration and risk.

use finstack_quant_core::cashflow::npv_amounts;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;

/// Schedule → PV helper that uses the curve's own day count convention (raw `f64`).
///
/// Returns unrounded NPV for high-precision calibration and risk.
/// Empty remaining schedules return zero only when the instrument defines a
/// notional, matching the known-currency boundary of `schedule_pv`.
///
/// # Arguments
///
/// * `instrument` - The instrument providing cashflows.
/// * `market` - Market data context.
/// * `as_of` - Valuation date used as the cashflow cutoff and the discounting
///   anchor on the resolved discount curve.
/// * `discount_curve_id` - ID of the discount curve to use.
/// * `include_today` - Whether cash dated `as_of` is still owed; chosen by the
///   product, with no default.
pub fn schedule_pv_raw<S>(
    instrument: &S,
    market: &MarketContext,
    as_of: Date,
    discount_curve_id: &finstack_quant_core::types::CurveId,
    include_today: bool,
) -> finstack_quant_core::Result<f64>
where
    S: finstack_quant_cashflows::CashflowProvider,
{
    let flows = S::dated_cashflows(instrument, market, as_of)?;
    if flows.is_empty() {
        instrument
            .notional()?
            .ok_or(finstack_quant_core::InputError::TooFewPoints)?;
        return Ok(0.0);
    }
    let disc = market.get_discount(discount_curve_id.as_str())?;

    let amounts = flows
        .into_iter()
        .map(|(date, amount)| (date, amount.amount()))
        .collect::<Vec<_>>();
    npv_amounts(disc.as_ref(), as_of, &amounts, include_today)
}

#[cfg(test)]
mod tests {
    use super::schedule_pv_raw;
    use finstack_quant_cashflows::CashflowScheduleSource;
    use finstack_quant_cashflows::builder::CashFlowSchedule;
    use finstack_quant_core::cashflow::CFKind;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::InterpStyle;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::CurveId;
    use time::macros::date;

    struct SingleFlowProvider;

    impl CashflowScheduleSource for SingleFlowProvider {
        fn notional(&self) -> finstack_quant_core::Result<Option<Money>> {
            Ok(Some(Money::from((100_i64, Currency::USD))))
        }

        fn raw_cashflow_schedule(
            &self,
            _market: &MarketContext,
            as_of: Date,
        ) -> finstack_quant_core::Result<CashFlowSchedule> {
            let flow_date = as_of + time::Duration::days(180);
            finstack_quant_cashflows::schedule_from_dated_flows(
                vec![
                    (as_of, Money::from((1_000_i64, Currency::USD))),
                    (flow_date, Money::from((100_i64, Currency::USD))),
                ],
                CFKind::Fixed,
                DayCount::Act365F,
                finstack_quant_cashflows::ScheduleBuildOpts::default(),
            )
        }
    }

    struct EmptyNoNotionalProvider;

    impl CashflowScheduleSource for EmptyNoNotionalProvider {
        fn notional(&self) -> finstack_quant_core::Result<Option<Money>> {
            Ok(None)
        }

        fn raw_cashflow_schedule(
            &self,
            _market: &MarketContext,
            _as_of: Date,
        ) -> finstack_quant_core::Result<CashFlowSchedule> {
            Ok(finstack_quant_cashflows::schedule_from_classified_flows(
                Vec::new(),
                DayCount::Act365F,
                finstack_quant_cashflows::ScheduleBuildOpts::default(),
            ))
        }
    }

    #[test]
    fn schedule_pv_raw_excludes_as_of_cash() -> finstack_quant_core::Result<()> {
        let as_of = date!(2024 - 01 - 01);
        let market = MarketContext::new()
            .try_insert(
                DiscountCurve::builder("DISC")
                    .base_date(as_of)
                    .knots([(0.0, 1.0), (1.0, 0.95)])
                    .interp(InterpStyle::Linear)
                    .day_count(DayCount::Act365F)
                    .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
                    .build()?,
            )
            .expect("insert market curve");

        let pv = schedule_pv_raw(
            &SingleFlowProvider,
            &market,
            as_of,
            &CurveId::new("DISC"),
            false,
        )?;

        assert!(pv > 0.0);
        assert!(pv < 100.0);
        Ok(())
    }

    #[test]
    fn schedule_pv_raw_rejects_empty_schedule_without_known_currency() {
        let market = MarketContext::new();
        let curve_id = CurveId::new("DISC");
        let as_of = date!(2025 - 01 - 16);
        assert!(matches!(
            schedule_pv_raw(&EmptyNoNotionalProvider, &market, as_of, &curve_id, false),
            Err(finstack_quant_core::Error::Input(
                finstack_quant_core::InputError::TooFewPoints
            ))
        ));
    }
}
