//! Shared clean redemption for dated loan call provisions.

use crate::cashflow::builder::CashFlowSchedule;
use crate::instruments::fixed_income::bond::pricing::engine::tree::BondValuator;
use crate::instruments::fixed_income::term_loan::{LoanCall, LoanCallType, TermLoan};
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::Result;

/// Remaining principal and cash accrual after scheduled exercise-date events.
/// Earned but not yet capitalized PIK belongs to the principal exercise claim.
pub(crate) fn exercise_claim(
    loan: &TermLoan,
    schedule: &CashFlowSchedule,
    exercise: Date,
) -> Result<(f64, f64)> {
    let principal = schedule
        .outstanding_by_date()?
        .iter()
        .take_while(|(date, _)| *date <= exercise)
        .last()
        .map_or(0.0, |(_, amount)| amount.amount());
    let mut config = loan.accrual_config();
    let cash = crate::cashflow::accrual::accrued_interest_amount(schedule, exercise, &config)?;
    config.include_pik = true;
    let total = crate::cashflow::accrual::accrued_interest_amount(schedule, exercise, &config)?;
    let capitalized_pending: f64 = schedule
        .get_flows()
        .iter()
        .filter(|flow| {
            flow.kind == CFKind::Pik && flow.get_balance_date() <= exercise && exercise < flow.date
        })
        .map(|flow| flow.amount.amount())
        .sum();
    Ok((principal + total - cash - capitalized_pending, cash))
}

/// Clean call redemption; the caller adds accrued cash interest once.
///
/// # Arguments
///
/// * `call` - Effective call provision, including its clean percentage-of-par floor.
/// * `schedule` - Canonical loan schedule; PIK capitalization is not cash consideration.
/// * `market` - Reference discount curve for a make-whole provision.
/// * `exercise` - Exercise date, which excludes already-due flows from the reference PV.
/// * `outstanding` - Remaining principal after scheduled events plus earned PIK,
///   in loan currency. Scheduled cash principal is paid separately.
/// * `accrued` - Cash interest accrued to exercise, subtracted before applying the clean floor.
pub(crate) fn clean_call_price(
    call: &LoanCall,
    schedule: &CashFlowSchedule,
    market: &MarketContext,
    exercise: Date,
    outstanding: f64,
    accrued: f64,
) -> Result<f64> {
    let floor = outstanding * call.price_pct_of_par / 100.0;
    match &call.call_type {
        LoanCallType::MakeWhole(spec) => {
            let reference = market.get_discount(&spec.reference_curve_id)?;
            let flows = schedule
                .get_flows()
                .iter()
                .filter(|flow| flow.kind != CFKind::Pik)
                .map(|flow| (flow.date, flow.amount))
                .collect::<Vec<_>>();
            BondValuator::make_whole_call_price(
                spec,
                reference.as_ref(),
                exercise,
                &flows,
                floor,
                accrued,
            )
        }
        _ => Ok(floor),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cashflow::builder::CouponType;
    use crate::instruments::fixed_income::term_loan::{AmortizationSpec, RateSpec};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{BusinessDayConvention, DayCount, Tenor};
    use finstack_quant_core::money::Money;
    use time::macros::date;

    #[test]
    fn call_claim_includes_pik_once_after_scheduled_amortization() {
        let mut loan = TermLoan::example().expect("loan");
        loan.issue_date = date!(2025 - 01 - 01);
        loan.maturity = date!(2027 - 01 - 01);
        loan.notional_limit = Money::new(100.0, Currency::USD).expect("principal");
        loan.rate = RateSpec::Fixed { rate: 0.10 };
        loan.frequency = Tenor::annual();
        loan.day_count = DayCount::Thirty360;
        loan.business_day_convention = BusinessDayConvention::Unadjusted;
        loan.coupon_type = CouponType::Pik;
        loan.amortization = AmortizationSpec::None;
        let market = MarketContext::new();
        let schedule = crate::instruments::fixed_income::term_loan::cashflows::generate_cashflows(
            &loan, &market,
        )
        .expect("schedule");
        assert_eq!(
            exercise_claim(&loan, &schedule, date!(2026 - 01 - 01)).expect("anniversary"),
            (110.0, 0.0)
        );
        let (principal, cash) =
            exercise_claim(&loan, &schedule, date!(2025 - 07 - 01)).expect("off-cycle");
        assert!((principal - 105.0).abs() < 1e-10);
        assert_eq!(cash, 0.0);
        loan.amortization = AmortizationSpec::PercentOfRemainingPerPeriod { pct: 0.5 };
        let schedule = crate::instruments::fixed_income::term_loan::cashflows::generate_cashflows(
            &loan, &market,
        )
        .expect("schedule");
        assert_eq!(
            exercise_claim(&loan, &schedule, date!(2026 - 01 - 01)).expect("after amort"),
            (55.0, 0.0)
        );
    }
}
