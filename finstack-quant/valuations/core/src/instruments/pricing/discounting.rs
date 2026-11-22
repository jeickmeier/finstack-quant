//! Discount-factor conversion and cashflow-schedule present-value helpers.

use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;

/// Convert a discount factor to an effective continuously-compounded zero rate.
///
/// Returns `r` such that `exp(-r * t) = df`. Returns `Ok(0.0)` at expiry
/// (`t <= 0`), which is the correct mathematical limit and matches the
/// behaviour required by callers that short-circuit on `t <= 0` before using
/// the returned rate.
///
/// # Arguments
///
/// * `df` - Finite positive unitless discount factor.
/// * `t` - Finite year fraction on the discount curve's clock.
/// * `context` - Pricing operation reported in input validation errors.
///
/// # Errors
///
/// Returns a `Validation` error for a non-finite time or a discount factor
/// that is non-finite or non-positive, including at expiry.
#[inline]
pub fn zero_rate_from_df(df: f64, t: f64, context: &str) -> finstack_quant_core::Result<f64> {
    if !df.is_finite() || df <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{context}: discount factor must be finite and > 0, got {df:.6e} for t={t:.6}"
        )));
    }
    if !t.is_finite() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{context}: discount time must be finite, got {t}"
        )));
    }
    if t <= 0.0 {
        return Ok(0.0);
    }
    Ok(-df.ln() / t)
}

/// Schedule -> PV helper that uses the curve's own day count convention.
///
/// This variant ensures consistency between:
/// - Metric calculations (e.g., par rate using `df_on_date`)
/// - NPV calculations
///
/// Routes through `core::cashflow::npv` (record CORE-CONVENTIONS-016):
/// flows before `as_of` are excluded and a flow dated `as_of` counts at
/// factor 1 exactly when `include_today` is true.
/// An empty remaining schedule returns zero in the instrument's notional
/// currency; an instrument without a defined notional still returns an error.
///
/// # Arguments
///
/// * `instrument` - Cashflow-providing instrument whose remaining schedule is discounted.
/// * `market` - Market context supplying the discount curve and any curves needed to build cashflows.
/// * `as_of` - Valuation anchor date; cashflows strictly before `as_of` are excluded and flows on `as_of` are governed by `include_today`.
/// * `discount_curve_id` - Identifier of the discount curve whose own day-count axis discounts the flows.
/// * `include_today` - Whether cash dated `as_of` is still owed; chosen by the
///   product, with no default.
pub fn schedule_pv<S>(
    instrument: &S,
    market: &MarketContext,
    as_of: Date,
    discount_curve_id: &finstack_quant_core::types::CurveId,
    include_today: bool,
) -> finstack_quant_core::Result<Money>
where
    S: finstack_quant_cashflows::CashflowProvider,
{
    use finstack_quant_core::cashflow::npv;

    let flows = S::dated_cashflows(instrument, market, as_of)?;
    if flows.is_empty() {
        let notional = instrument
            .notional()?
            .ok_or(finstack_quant_core::InputError::TooFewPoints)?;
        return Ok(Money::from((0_i64, notional.currency())));
    }
    let disc = market.get_discount(discount_curve_id.as_str())?;
    // Discount on the curve's own time axis, consistent with curve metrics.
    npv(disc.as_ref(), as_of, &flows, include_today)
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_cashflows::builder::CashFlowSchedule;
    use finstack_quant_cashflows::{ScheduleBuildOpts, schedule_from_dated_flows};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
    use finstack_quant_core::types::CurveId;
    use time::Duration;
    use time::macros::date;

    struct EmptyFlowProvider;

    crate::impl_empty_cashflow_provider!(
        EmptyFlowProvider,
        finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
    );

    struct SingleFlowProvider;

    impl finstack_quant_cashflows::CashflowScheduleSource for SingleFlowProvider {
        fn notional(&self) -> finstack_quant_core::Result<Option<Money>> {
            Ok(Some(Money::from((100_i64, Currency::USD))))
        }

        fn raw_cashflow_schedule(
            &self,
            _curves: &MarketContext,
            as_of: Date,
        ) -> finstack_quant_core::Result<CashFlowSchedule> {
            schedule_from_dated_flows(
                vec![
                    (as_of, Money::from((10_000_i64, Currency::USD))),
                    (
                        as_of + Duration::days(30),
                        Money::from((100_i64, Currency::USD)),
                    ),
                ],
                finstack_quant_core::cashflow::CFKind::Fixed,
                DayCount::Act365F,
                ScheduleBuildOpts {
                    notional_hint: self.notional().expect("valid notional"),
                    ..Default::default()
                },
            )
        }
    }

    #[test]
    fn zero_rate_validates_inputs_before_expiry_return() {
        for df in [f64::NAN, 0.0] {
            assert!(zero_rate_from_df(df, 0.0, "test").is_err());
        }
        for time in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(zero_rate_from_df(1.0, time, "test").is_err());
        }
        assert_eq!(zero_rate_from_df(1.0, -1.0, "test").unwrap(), 0.0);
    }

    #[test]
    fn empty_schedule_without_known_currency_remains_an_error() {
        let empty = EmptyFlowProvider;
        let market = MarketContext::new();
        let curve_id = CurveId::new("DISC");
        let as_of = date!(2025 - 01 - 16);
        assert!(matches!(
            schedule_pv(&empty, &market, as_of, &curve_id, false),
            Err(finstack_quant_core::Error::Input(
                finstack_quant_core::InputError::TooFewPoints
            ))
        ));
    }

    #[test]
    fn schedule_pv_excludes_as_of_cash() -> finstack_quant_core::Result<()> {
        let as_of = date!(2024 - 01 - 01);
        let market = MarketContext::new()
            .try_insert(
                DiscountCurve::builder("DISC")
                    .base_date(as_of)
                    .knots([(0.0, 1.0), (1.0, 0.95)])
                    .interp(InterpStyle::Linear)
                    .day_count(DayCount::Act365F)
                    .extrapolation(ExtrapolationPolicy::Extend)
                    .build()?,
            )
            .expect("insert market curve");

        let pv = schedule_pv(
            &SingleFlowProvider,
            &market,
            as_of,
            &CurveId::new("DISC"),
            false,
        )?;

        assert!(pv.amount() > 0.0);
        assert!(pv.amount() < 100.0);
        assert_eq!(pv.currency(), Currency::USD);
        Ok(())
    }
}
