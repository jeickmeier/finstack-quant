//! Fee cashflow emission (periodic and fixed fees on the builder pipeline).

use crate::primitives::{CFKind, CashFlow};
use finstack_quant_core::cashflow::CashFlowAccrual;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use finstack_quant_core::InputError;
use rust_decimal::Decimal;

use super::super::compiler::PeriodicFee;
use super::super::specs::{FeeAccrualBasis, FeeBase};
use super::{decimal_to_f64, f64_to_decimal};

/// Conversion factor from basis points to rate (1 bp = 0.0001).
const BP_TO_RATE: Decimal = Decimal::from_parts(1, 0, 0, false, 4);

/// Emit fee cashflows on a specific date.
///
/// Processes both periodic fees (based on drawn/undrawn balances) and fixed
/// fees (explicit amounts) that fall on the given date.
///
/// For periodic fees, computes the fee amount as `base * bp * year_fraction`
/// where base is either the drawn balance or the undrawn balance (facility_limit - outstanding).
///
/// When a fee's `accrual_basis` is `PointInTime`, the outstanding balance is
/// sampled at the period's accrual start from `outstanding_history` (falling
/// back to the live `outstanding` only when no history exists). With
/// `TimeWeightedAverage`, each constant-balance interval uses the contractual
/// day count and emits its own accrual metadata. This preserves leap-year,
/// intraperiod balance changes, and undrawn-limit clipping semantics.
///
/// Any non-zero fee amount is emitted; negative fees (rebates) are preserved
/// as negative cashflows for both periodic and fixed fees.
pub(in crate::builder) fn emit_fees_on(
    d: Date,
    periodic_fees: &[PeriodicFee],
    fixed_fees: &[(Date, Money)],
    outstanding: Decimal,
    outstanding_history: &[(Date, Decimal)],
    ccy: Currency,
    new_flows: &mut Vec<CashFlow>,
) -> finstack_quant_core::Result<()> {
    for pf in periodic_fees {
        let first = pf.dates.partition_point(|date| {
            pf.prev
                .get(date)
                .is_none_or(|period| period.accrual_end < d)
        });
        for period in pf.dates[first..]
            .iter()
            .filter_map(|date| pf.prev.get(date))
            .take_while(|period| period.accrual_end == d)
        {
            let mut segments = super::balances::balance_segments(
                outstanding_history,
                period.accrual_start,
                period.accrual_end,
                outstanding,
            );
            if pf.accrual_basis == FeeAccrualBasis::PointInTime {
                if let Some(first) = segments.first().copied() {
                    segments = vec![(period.accrual_start, period.accrual_end, first.2)];
                }
            }
            for (accrual_start, accrual_end, effective_outstanding) in segments {
                let is_termination_date = pf.terminal_accrual_end == Some(accrual_end);
                let coupon_period = crate::builder::date_generation::icma_coupon_period(
                    period.unadjusted_start,
                    period.unadjusted_end,
                    pf.frequency,
                    pf.stub,
                    false,
                );
                let yf = pf.day_count.year_fraction(
                    accrual_start,
                    accrual_end,
                    finstack_quant_core::dates::DayCountContext {
                        calendar: Some(pf.calendar),
                        frequency: Some(pf.frequency),
                        bus_basis: None,
                        coupon_period,
                        end_is_termination_date: is_termination_date,
                    },
                )?;

                let base_amt = match &pf.base {
                    FeeBase::Drawn => effective_outstanding,
                    FeeBase::Undrawn { facility_limit } => {
                        if facility_limit.currency() != ccy {
                            return Err(InputError::Invalid.into());
                        }
                        let facility_limit_dec = f64_to_decimal(facility_limit.amount())?;
                        (facility_limit_dec - effective_outstanding).max(Decimal::ZERO)
                    }
                };

                let yf_dec = f64_to_decimal(yf)?;
                let fee_amt_dec = base_amt * pf.bp * BP_TO_RATE * yf_dec;
                let fee_amt = decimal_to_f64(fee_amt_dec)?;

                let rate_dec = pf.bp * BP_TO_RATE;
                let rate = decimal_to_f64(rate_dec)?;

                if fee_amt != 0.0 {
                    new_flows.push(
                        CashFlow::new(
                            period.payment_date,
                            None,
                            Money::new(fee_amt, ccy)?,
                            CFKind::Fee,
                            yf,
                            Some(rate),
                        )
                        .with_accrual(CashFlowAccrual {
                            coupon_period,
                            end_is_termination_date: is_termination_date,
                            calendar_id: Some(pf.calendar_id.clone()),
                            start: accrual_start,
                            end: accrual_end,
                            day_count: pf.day_count,
                            projected_index_rate: None,
                        }),
                    );
                }
            }
        }
    }

    for (fd, amt) in fixed_fees {
        if *fd == d && amt.amount() != 0.0 {
            new_flows.push(CashFlow::new(d, None, *amt, CFKind::Fee, 0.0, None));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::compiler::PeriodicFee;
    use crate::builder::periods::SchedulePeriod;
    use crate::builder::specs::{FeeAccrualBasis, FeeBase};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{DayCount, Tenor};
    use rust_decimal_macros::dec;
    use time::Month;

    /// Helper to build a simple PeriodicFee with one period.
    fn make_periodic_fee(
        accrual_start: Date,
        accrual_end: Date,
        payment_date: Date,
        bp: Decimal,
        accrual_basis: FeeAccrualBasis,
        base: FeeBase,
    ) -> PeriodicFee {
        let mut prev = finstack_quant_core::HashMap::default();
        prev.insert(
            payment_date,
            SchedulePeriod {
                accrual_start,
                accrual_end,
                payment_date,
                reset_date: None,
                accrual_year_fraction: 0.0,
                unadjusted_start: accrual_start,
                unadjusted_end: accrual_end,
            },
        );
        PeriodicFee {
            calendar_id: "weekends_only".to_owned(),
            base,
            bp,
            day_count: DayCount::Act360,
            frequency: Tenor::quarterly(),
            stub: finstack_quant_core::dates::StubKind::ShortBack,
            calendar: crate::builder::calendar::resolve_calendar_strict("weekends_only")
                .expect("weekends_only calendar should resolve"),
            dates: vec![payment_date],
            prev,
            accrual_basis,
            terminal_accrual_end: Some(accrual_end),
        }
    }

    #[test]
    fn b14_fee_uses_contract_daycount_on_each_balance_segment() {
        let start = time::macros::date!(2023 - 12 - 01);
        let event = time::macros::date!(2023 - 12 - 31);
        let end = time::macros::date!(2024 - 03 - 01);
        let mut fee = make_periodic_fee(
            start,
            end,
            end,
            dec!(50),
            FeeAccrualBasis::TimeWeightedAverage,
            FeeBase::Drawn,
        );
        fee.day_count = DayCount::ActAct;
        let mut flows = Vec::new();
        emit_fees_on(
            end,
            &[fee],
            &[],
            Decimal::ZERO,
            &[(start, dec!(1000000)), (event, Decimal::ZERO)],
            Currency::USD,
            &mut flows,
        )
        .expect("valid fee accrual");
        let amount: f64 = flows.iter().map(|flow| flow.amount.amount()).sum();
        assert!((amount - 1_000_000.0 * 0.005 * 30.0 / 365.0).abs() < 1e-8);
    }

    #[test]
    fn point_in_time_matches_original_behavior() {
        let start = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
        let end = Date::from_calendar_date(2025, Month::April, 15).expect("valid date");
        let payment = end;

        let pf = make_periodic_fee(
            start,
            end,
            payment,
            dec!(50),
            FeeAccrualBasis::PointInTime,
            FeeBase::Drawn,
        );

        let outstanding = dec!(1000000);
        let history: Vec<(Date, Decimal)> = Vec::new();
        let mut flows = Vec::new();

        emit_fees_on(
            payment,
            &[pf],
            &[],
            outstanding,
            &history,
            Currency::USD,
            &mut flows,
        )
        .expect("valid date");

        assert_eq!(flows.len(), 1);
        let fee = flows[0].amount.amount();
        assert!((fee - 1250.0).abs() < 0.01, "Expected ~1250.0, got {}", fee);
        let accrual = flows[0]
            .accrual
            .as_ref()
            .expect("periodic fee should own its accrual metadata");
        assert_eq!((accrual.start, accrual.end), (start, end));
        assert_eq!(accrual.day_count, DayCount::Act360);
    }

    #[test]
    fn twa_with_constant_outstanding_matches_point_in_time() {
        let start = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
        let end = Date::from_calendar_date(2025, Month::April, 15).expect("valid date");
        let payment = end;
        let outstanding = dec!(1000000);

        let history: Vec<(Date, Decimal)> = vec![(start, outstanding)];

        let pf_pit = make_periodic_fee(
            start,
            end,
            payment,
            dec!(50),
            FeeAccrualBasis::PointInTime,
            FeeBase::Drawn,
        );
        let mut flows_pit = Vec::new();
        emit_fees_on(
            payment,
            &[pf_pit],
            &[],
            outstanding,
            &history,
            Currency::USD,
            &mut flows_pit,
        )
        .expect("valid date");

        let pf_twa = make_periodic_fee(
            start,
            end,
            payment,
            dec!(50),
            FeeAccrualBasis::TimeWeightedAverage,
            FeeBase::Drawn,
        );
        let mut flows_twa = Vec::new();
        emit_fees_on(
            payment,
            &[pf_twa],
            &[],
            outstanding,
            &history,
            Currency::USD,
            &mut flows_twa,
        )
        .expect("valid date");

        assert_eq!(flows_pit.len(), 1);
        assert_eq!(flows_twa.len(), 1);
        assert!(
            (flows_pit[0].amount.amount() - flows_twa[0].amount.amount()).abs() < 1e-10,
            "PIT={} vs TWA={}",
            flows_pit[0].amount.amount(),
            flows_twa[0].amount.amount()
        );
    }

    #[test]
    fn twa_with_varying_outstanding_computes_weighted_average() {
        let start = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
        let mid = Date::from_calendar_date(2025, Month::February, 14).expect("valid date");
        let end = Date::from_calendar_date(2025, Month::April, 15).expect("valid date");
        let payment = end;

        let history: Vec<(Date, Decimal)> = vec![(start, dec!(1000000)), (mid, dec!(500000))];

        let pf = make_periodic_fee(
            start,
            end,
            payment,
            dec!(50),
            FeeAccrualBasis::TimeWeightedAverage,
            FeeBase::Drawn,
        );

        let mut flows = Vec::new();
        emit_fees_on(
            payment,
            &[pf],
            &[],
            dec!(500000),
            &history,
            Currency::USD,
            &mut flows,
        )
        .expect("valid date");

        assert_eq!(flows.len(), 2);
        let fee: f64 = flows.iter().map(|flow| flow.amount.amount()).sum();
        let expected_twa = (1_000_000.0 * 30.0 + 500_000.0 * 60.0) / 90.0;
        let expected_fee = expected_twa * 0.005 * (90.0 / 360.0);
        assert!(
            (fee - expected_fee).abs() < 0.02,
            "Expected ~{:.2}, got {:.2}",
            expected_fee,
            fee
        );
    }

    #[test]
    fn twa_undrawn_base_uses_weighted_average() {
        let start = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
        let mid = Date::from_calendar_date(2025, Month::February, 14).expect("valid date");
        let end = Date::from_calendar_date(2025, Month::April, 15).expect("valid date");
        let payment = end;
        let facility_limit = 2_000_000.0;

        let history: Vec<(Date, Decimal)> = vec![(start, dec!(1000000)), (mid, dec!(500000))];

        let pf = make_periodic_fee(
            start,
            end,
            payment,
            dec!(50),
            FeeAccrualBasis::TimeWeightedAverage,
            FeeBase::Undrawn {
                facility_limit: Money::new(facility_limit, Currency::USD)
                    .expect("valid money fixture"),
            },
        );

        let mut flows = Vec::new();
        emit_fees_on(
            payment,
            &[pf],
            &[],
            dec!(500000),
            &history,
            Currency::USD,
            &mut flows,
        )
        .expect("valid date");

        assert_eq!(flows.len(), 2);
        let twa_outstanding = (1_000_000.0 * 30.0 + 500_000.0 * 60.0) / 90.0;
        let undrawn = facility_limit - twa_outstanding;
        let expected_fee = undrawn * 0.005 * (90.0 / 360.0);
        let fee: f64 = flows.iter().map(|flow| flow.amount.amount()).sum();
        assert!(
            (fee - expected_fee).abs() < 0.02,
            "Expected ~{:.2}, got {:.2}",
            expected_fee,
            fee
        );
    }

    #[test]
    fn point_in_time_uses_period_start_balance_on_amortizing_schedule() {
        // The balance amortizes from 1,000,000 (period start) to 800,000 (live
        // balance on the payment date). PointInTime must price off the
        // period-start balance, not the post-amortization payment-date balance.
        let start = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
        let end = Date::from_calendar_date(2025, Month::April, 15).expect("valid date");
        let payment = end;

        let history: Vec<(Date, Decimal)> = vec![(start, dec!(1000000))];

        let pf = make_periodic_fee(
            start,
            end,
            payment,
            dec!(50),
            FeeAccrualBasis::PointInTime,
            FeeBase::Drawn,
        );

        let mut flows = Vec::new();
        emit_fees_on(
            payment,
            &[pf],
            &[],
            dec!(800000), // live balance after same-date amortization
            &history,
            Currency::USD,
            &mut flows,
        )
        .expect("valid fee inputs");

        assert_eq!(flows.len(), 1);
        let fee = flows[0].amount.amount();
        // 1,000,000 * 50bp * 90/360 = 1250.0 (golden, period-start base)
        assert!(
            (fee - 1250.0).abs() < 0.01,
            "PointInTime fee must use period-start balance: expected 1250.0, got {fee}"
        );
    }

    #[test]
    fn negative_periodic_fee_emitted_as_rebate() {
        let start = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
        let end = Date::from_calendar_date(2025, Month::April, 15).expect("valid date");
        let payment = end;

        let history: Vec<(Date, Decimal)> = vec![(start, dec!(1000000))];

        let pf = make_periodic_fee(
            start,
            end,
            payment,
            dec!(-50), // negative bp: rebate
            FeeAccrualBasis::PointInTime,
            FeeBase::Drawn,
        );

        let mut flows = Vec::new();
        emit_fees_on(
            payment,
            &[pf],
            &[],
            dec!(1000000),
            &history,
            Currency::USD,
            &mut flows,
        )
        .expect("valid fee inputs");

        assert_eq!(flows.len(), 1, "negative fee must not be dropped");
        let fee = flows[0].amount.amount();
        assert!(
            (fee + 1250.0).abs() < 0.01,
            "Expected -1250.0 rebate, got {fee}"
        );
    }
}
