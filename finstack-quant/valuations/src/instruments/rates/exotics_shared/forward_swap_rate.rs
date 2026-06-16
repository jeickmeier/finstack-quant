use crate::instruments::common_impl::pricing::time::{
    rate_period_on_dates, relative_df_discount_curve,
};
use finstack_quant_core::dates::{
    BusinessDayConvention, Date, DayCount, DayCountContext, StubKind, Tenor,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::Result;

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
    pub fixed_freq: Tenor,
    /// Fixed leg day-count convention.
    pub fixed_day_count: DayCount,
    /// Floating leg payment/reset frequency.
    pub float_freq: Tenor,
    /// Floating leg day-count convention.
    pub float_day_count: DayCount,
}

/// Calculate forward swap rate and annuity for a swap running from `start` to `end`.
///
/// Uses curve-consistent time mapping:
/// - Discount factors use the discount curve's own day-count basis
/// - Forward rates use the forward curve's own time basis
/// - Accruals use the supplied fixed and floating leg day-count conventions
pub fn calculate_forward_swap_rate(inputs: ForwardSwapRateInputs<'_>) -> Result<(f64, f64)> {
    let disc = inputs
        .market
        .get_discount(inputs.discount_curve_id.as_ref())?;

    let sched_fixed = crate::cashflow::builder::build_dates(
        inputs.start,
        inputs.end,
        inputs.fixed_freq,
        StubKind::None,
        BusinessDayConvention::ModifiedFollowing,
        false,
        0,
        crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID,
    )?;

    let mut annuity = 0.0;
    let mut prev_date = inputs.start;
    for &d in sched_fixed.dates.iter().skip(1) {
        let accrual =
            inputs
                .fixed_day_count
                .year_fraction(prev_date, d, DayCountContext::default())?;
        let df = relative_df_discount_curve(disc.as_ref(), inputs.as_of, d)?;
        annuity += accrual * df;
        prev_date = d;
    }

    if annuity.abs() < 1e-10 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Annuity is near-zero ({}) for swap from {} to {}; check curve or schedule configuration",
            annuity, inputs.start, inputs.end
        )));
    }

    if inputs.forward_curve_id == inputs.discount_curve_id {
        let df_start = relative_df_discount_curve(disc.as_ref(), inputs.as_of, inputs.start)?;
        let df_end = relative_df_discount_curve(disc.as_ref(), inputs.as_of, inputs.end)?;
        let rate = (df_start - df_end) / annuity;
        Ok((rate, annuity))
    } else {
        let fwd_curve = inputs
            .market
            .get_forward(inputs.forward_curve_id.as_ref())?;
        let sched_float = crate::cashflow::builder::build_dates(
            inputs.start,
            inputs.end,
            inputs.float_freq,
            StubKind::None,
            BusinessDayConvention::ModifiedFollowing,
            false,
            0,
            crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID,
        )?;

        let mut pv_float = 0.0;
        let mut prev_date = inputs.start;
        for &d in &sched_float.dates {
            if d == inputs.start {
                continue;
            }
            let accrual =
                inputs
                    .float_day_count
                    .year_fraction(prev_date, d, DayCountContext::default())?;
            let fwd_rate = rate_period_on_dates(fwd_curve.as_ref(), prev_date, d)?;
            let df = relative_df_discount_curve(disc.as_ref(), inputs.as_of, d)?;
            pv_float += fwd_rate * accrual * df;
            prev_date = d;
        }

        let rate = pv_float / annuity;
        Ok((rate, annuity))
    }
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
            fixed_freq: "1Y".parse().expect("tenor"),
            fixed_day_count: DayCount::Act365F,
            float_freq: "1Y".parse().expect("tenor"),
            float_day_count: DayCount::Act365F,
        })
        .expect("forward swap rate");

        assert!(annuity > 0.0);
        assert!((rate - 0.051271096).abs() < 1e-3, "rate={rate}");
    }
}
