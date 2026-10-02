//! Pricing and metric helpers for interest-rate instruments.
//!
use crate::instruments::common_impl::pricing::time::{
    rate_between_on_dates, relative_df_discount_curve,
};
use finstack_quant_core::dates::{
    BusinessDayConvention, Date, DayCount, DayCountContext, StubKind, Tenor,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::ForwardCurve;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::Result;

/// Validate that a term-index curve represents the instrument's contractual
/// reset tenor.
pub(crate) fn validate_term_curve_tenor(
    curve: &ForwardCurve,
    tenor: Tenor,
    instrument: &str,
) -> Result<()> {
    let expected = tenor.to_years();
    if (curve.tenor() - expected).abs() > 1e-8 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{instrument} index_tenor {expected} does not match forward curve '{}' tenor {}",
            curve.id(),
            curve.tenor()
        )));
    }
    Ok(())
}

/// Fixed-tenor index fixing at a reset date.
pub(crate) fn term_fixing_on_date(curve: &ForwardCurve, date: Date) -> Result<f64> {
    let t = curve.day_count().signed_year_fraction(
        curve.base_date(),
        date,
        DayCountContext::default(),
    )?;
    Ok(curve.rate(t))
}

/// Inputs for forward swap rate calculation.
pub struct ForwardSwapRateInputs<'a> {
    /// Market context containing the referenced curves.
    pub market: &'a MarketContext,
    /// Discount curve used for annuity and discount-factor calculations.
    pub discount_curve_id: &'a CurveId,
    /// Forward/projection curve used for floating-leg forward rates.
    pub forward_curve_id: &'a CurveId,
    /// Valuation date.
    pub as_of: Date,
    /// Swap effective/start date.
    pub start: Date,
    /// Swap maturity/end date.
    pub end: Date,
    /// Fixed leg payment frequency.
    pub fixed_frequency: Tenor,
    /// Fixed leg day-count convention.
    pub fixed_day_count: DayCount,
    /// Floating leg payment/reset frequency.
    pub float_frequency: Tenor,
    /// Floating leg day-count convention.
    pub float_day_count: DayCount,
    /// Calendar used to adjust reference-swap schedules.
    pub calendar_id: &'a str,
    /// Business-day convention for both legs.
    pub business_day_convention: BusinessDayConvention,
    /// Stub rule for irregular reference swaps.
    pub stub: StubKind,
    /// Preserve end-of-month rolls.
    pub end_of_month: bool,
    /// Payment lag in business days.
    pub payment_lag_days: i32,
    /// Require a term projection curve whose tenor matches `float_frequency`.
    /// Disable for overnight-compounded reference swaps.
    pub enforce_forward_tenor: bool,
}

/// Calculate forward swap rate and annuity for a swap running from `start` to `end`.
///
/// Uses curve-consistent time mapping:
/// - Discount factors use the discount curve's own day-count basis
/// - Forward rates use the forward curve's own time basis
/// - Accruals use the supplied fixed and floating leg day-count conventions
///
/// # Arguments
///
/// * `inputs` - Complete reference-swap definition: market context, curve IDs,
///   valuation/start/end dates, leg frequencies/day counts, calendar,
///   business-day/stub/EOM rules, payment lag, and tenor-enforcement policy.
pub fn calculate_forward_swap_rate(inputs: ForwardSwapRateInputs<'_>) -> Result<(f64, f64)> {
    let disc = inputs
        .market
        .get_discount(inputs.discount_curve_id.as_ref())?;

    let sched_fixed = crate::cashflow::builder::periods::build_periods(
        crate::cashflow::builder::periods::BuildPeriodsParams {
            start: inputs.start,
            end: inputs.end,
            frequency: inputs.fixed_frequency,
            stub: inputs.stub,
            business_day_convention: inputs.business_day_convention,
            calendar_id: inputs.calendar_id,
            end_of_month: inputs.end_of_month,
            day_count: inputs.fixed_day_count,
            payment_lag_days: inputs.payment_lag_days,
            reset_lag_days: None,
            adjust_accrual_dates: false,
            roll_rule: crate::cashflow::builder::specs::RollRule::None,
        },
    )?;

    let annuity = crate::instruments::rates::irs::cashflow::fixed_leg_annuity(
        &sched_fixed,
        disc.as_ref(),
        inputs.as_of,
        inputs.as_of,
    )?;

    if annuity.abs() < 1e-10 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Annuity is near-zero ({}) for swap from {} to {}; check curve or schedule configuration",
            annuity, inputs.start, inputs.end
        )));
    }

    let fwd_curve = if inputs.forward_curve_id == inputs.discount_curve_id {
        None
    } else {
        let curve = inputs
            .market
            .get_forward(inputs.forward_curve_id.as_ref())?;
        if inputs.enforce_forward_tenor {
            validate_term_curve_tenor(
                curve.as_ref(),
                inputs.float_frequency,
                "CMS reference swap",
            )?;
        }
        Some(curve)
    };
    let sched_float = crate::cashflow::builder::periods::build_periods(
        crate::cashflow::builder::periods::BuildPeriodsParams {
            start: inputs.start,
            end: inputs.end,
            frequency: inputs.float_frequency,
            stub: inputs.stub,
            business_day_convention: inputs.business_day_convention,
            calendar_id: inputs.calendar_id,
            end_of_month: inputs.end_of_month,
            day_count: inputs.float_day_count,
            payment_lag_days: inputs.payment_lag_days,
            reset_lag_days: None,
            adjust_accrual_dates: false,
            roll_rule: crate::cashflow::builder::specs::RollRule::None,
        },
    )?;

    let mut pv_float = 0.0;
    for period in &sched_float {
        let accrual = period.accrual_year_fraction;
        let coupon = if let Some(curve) = &fwd_curve {
            rate_between_on_dates(
                curve.as_ref(),
                period.accrual_start,
                period.accrual_end,
                accrual,
            )? * accrual
        } else {
            disc.df_on_date_curve(period.accrual_start)?
                / disc.df_on_date_curve(period.accrual_end)?
                - 1.0
        };
        let df = relative_df_discount_curve(disc.as_ref(), inputs.as_of, period.payment_date)?;
        pv_float += coupon * df;
    }

    let rate = pv_float / annuity;
    Ok((rate, annuity))
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::types::CurveId;
    use time::Month;

    #[test]
    fn shared_forward_swap_rate_matches_flat_single_curve_formula() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let discount_curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([
                (0.0, 1.0),
                (1.0, (-0.05_f64).exp()),
                (10.0, (-0.5_f64).exp()),
            ])
            .build()
            .expect("discount curve");
        let market = MarketContext::new().insert(discount_curve);
        let start = Date::from_calendar_date(2026, Month::January, 1).expect("valid date");
        let end = Date::from_calendar_date(2031, Month::January, 1).expect("valid date");

        let (rate, annuity) = calculate_forward_swap_rate(ForwardSwapRateInputs {
            market: &market,
            discount_curve_id: &CurveId::from("USD-OIS"),
            forward_curve_id: &CurveId::from("USD-OIS"),
            as_of,
            start,
            end,
            fixed_frequency: "1Y".parse().expect("tenor"),
            fixed_day_count: DayCount::Act365F,
            float_frequency: "1Y".parse().expect("tenor"),
            float_day_count: DayCount::Act365F,
            calendar_id: crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
            enforce_forward_tenor: false,
        })
        .expect("forward swap rate");

        assert!(annuity > 0.0);
        assert!((rate - 0.051271096).abs() < 1e-3, "rate={rate}");
    }
    #[test]
    fn single_curve_forward_prices_actual_adjusted_and_lagged_payments() {
        use finstack_quant_core::dates::{adjust, calendar_by_id, DateExt};
        let as_of = Date::from_calendar_date(2025, Month::January, 2).expect("date");
        let start = Date::from_calendar_date(2025, Month::January, 31).expect("date");
        let end = Date::from_calendar_date(2027, Month::January, 31).expect("date");
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (1.0, 0.97), (2.0, 0.89), (3.0, 0.83)])
            .build()
            .expect("curve");
        let market = MarketContext::new().insert(curve);
        let disc = market.get_discount("USD-OIS").expect("curve");
        let calendar_id = crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID;
        let calendar = calendar_by_id(calendar_id).expect("calendar");
        for business_day_convention in [
            BusinessDayConvention::Unadjusted,
            BusinessDayConvention::Following,
            BusinessDayConvention::ModifiedFollowing,
        ] {
            for lag in [0, 2, 20] {
                let (rate, annuity) = calculate_forward_swap_rate(ForwardSwapRateInputs {
                    market: &market,
                    discount_curve_id: &CurveId::from("USD-OIS"),
                    forward_curve_id: &CurveId::from("USD-OIS"),
                    as_of,
                    start,
                    end,
                    fixed_frequency: Tenor::quarterly(),
                    fixed_day_count: DayCount::Thirty360,
                    float_frequency: Tenor::semi_annual(),
                    float_day_count: DayCount::Act360,
                    calendar_id,
                    business_day_convention,
                    stub: StubKind::None,
                    end_of_month: true,
                    payment_lag_days: lag,
                    enforce_forward_tenor: false,
                })
                .expect("forward swap rate");
                let mut expected = 0.0;
                for month in [0, 6, 12, 18] {
                    let a = start.add_months(month).expect("start");
                    let b = start.add_months(month + 6).expect("end");
                    let payment = adjust(b, business_day_convention, calendar)
                        .expect("adjusted payment")
                        .add_business_days(lag, calendar)
                        .expect("lagged payment");
                    expected += (disc.df_on_date_curve(a).expect("df")
                        / disc.df_on_date_curve(b).expect("df")
                        - 1.0)
                        * relative_df_discount_curve(disc.as_ref(), as_of, payment)
                            .expect("payment df");
                }
                assert!(
                    (rate * annuity - expected).abs() < 1e-12,
                    "business_day_convention={business_day_convention:?}, lag={lag}"
                );
                if business_day_convention == BusinessDayConvention::Unadjusted && lag == 0 {
                    let telescope = relative_df_discount_curve(disc.as_ref(), as_of, start)
                        .expect("df")
                        - relative_df_discount_curve(disc.as_ref(), as_of, end).expect("df");
                    assert!((expected - telescope).abs() < 1e-12);
                }
            }
        }
    }
}
