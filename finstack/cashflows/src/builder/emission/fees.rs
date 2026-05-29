//! Fee cashflow emission (periodic, commitment, usage, facility).

use crate::primitives::{CFKind, CashFlow};
use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::money::Money;
use finstack_core::InputError;
use rust_decimal::Decimal;

use super::super::compiler::PeriodicFee;
use super::super::specs::{FeeAccrualBasis, FeeBase};

/// Conversion factor from basis points to rate (1 bp = 0.0001).
const BP_TO_RATE: Decimal = Decimal::from_parts(1, 0, 0, false, 4); // 0.0001

// Shared f64 ↔ Decimal conversion helpers from the parent `emission` module.
// These propagate errors on NaN/Inf instead of silently collapsing to zero.
use super::{decimal_to_f64, f64_to_decimal};

/// Emit a single revolving-credit fee cashflow.
///
/// Creates a single fee cashflow with the specified kind if the computed fee amount
/// is positive, otherwise returns an empty vector.
///
/// Uses `Decimal` arithmetic throughout for consistency with the periodic fee
/// emission path, avoiding f64 precision differences for large notionals.
fn emit_revolving_fee_on(
    d: Date,
    base_amount: f64,
    fee_bp: f64,
    year_fraction: f64,
    ccy: Currency,
    kind: CFKind,
) -> finstack_core::Result<Option<CashFlow>> {
    // Use Decimal for consistent precision with emit_fees_on
    let base_dec = f64_to_decimal(base_amount)?;
    let fee_bp_dec = f64_to_decimal(fee_bp)?;
    let yf_dec = f64_to_decimal(year_fraction)?;

    let fee_amt_dec = base_dec * fee_bp_dec * BP_TO_RATE * yf_dec;
    let fee_amt = decimal_to_f64(fee_amt_dec)?;
    let rate = decimal_to_f64(fee_bp_dec * BP_TO_RATE)?;

    if fee_amt > 0.0 {
        Ok(Some(CashFlow {
            date: d,
            reset_date: None,
            amount: Money::new(fee_amt, ccy),
            kind,
            accrual_factor: year_fraction,
            rate: Some(rate),
        }))
    } else {
        Ok(None)
    }
}

/// Compute the time-weighted average of a value over a date range using a history map.
///
/// Given a history of `(date, outstanding)` snapshots, computes:
///
/// ```text
/// TWA = sum(outstanding_i * delta_t_i) / sum(delta_t_i)
/// ```
///
/// where `outstanding_i` is the outstanding at each snapshot date that falls within
/// `[accrual_start, accrual_end)`, and `delta_t_i` is the number of days until the
/// next snapshot or `accrual_end`.
///
/// If no history entries exist for the period, returns the `fallback` value.
fn compute_time_weighted_average(
    outstanding_history: &finstack_core::HashMap<Date, Decimal>,
    accrual_start: Date,
    accrual_end: Date,
    fallback: Decimal,
    entries_buf: &mut Vec<(Date, Decimal)>,
) -> Decimal {
    // Collect entries that are relevant to the accrual period:
    // any date < accrual_end (we need entries before start to carry forward).
    entries_buf.clear();
    entries_buf.extend(
        outstanding_history
            .iter()
            .filter(|(date, _)| **date < accrual_end)
            .map(|(date, val)| (*date, *val)),
    );

    if entries_buf.is_empty() {
        return fallback;
    }

    entries_buf.sort_by_key(|(d, _)| *d);
    let entries = entries_buf;

    // Find the outstanding at accrual_start: the most recent entry at or before accrual_start
    let start_idx = match entries.binary_search_by_key(&accrual_start, |(d, _)| *d) {
        Ok(i) => i,
        Err(i) => {
            if i == 0 {
                // No entry at or before accrual_start; use fallback for the initial value
                entries.insert(0, (accrual_start, fallback));
                0
            } else {
                // The entry just before the insertion point is the most recent before accrual_start.
                // Create a synthetic entry at accrual_start with that value.
                let val = entries[i - 1].1;
                entries.insert(i, (accrual_start, val));
                i
            }
        }
    };

    // Compute TWA from start_idx onward, clamped to [accrual_start, accrual_end)
    let mut weighted_sum = Decimal::ZERO;
    let mut total_days = 0i64;

    for i in start_idx..entries.len() {
        let (date_i, val_i) = entries[i];
        if date_i >= accrual_end {
            break;
        }
        // Next boundary: either the next entry's date or accrual_end
        let next_date = if i + 1 < entries.len() {
            entries[i + 1].0.min(accrual_end)
        } else {
            accrual_end
        };
        let days = (next_date - date_i).whole_days();
        if days > 0 {
            weighted_sum += val_i * Decimal::from(days);
            total_days += days;
        }
    }

    if total_days > 0 {
        weighted_sum / Decimal::from(total_days)
    } else {
        fallback
    }
}

/// Emit fee cashflows on a specific date.
///
/// Processes both periodic fees (based on drawn/undrawn balances) and fixed
/// fees (explicit amounts) that fall on the given date.
///
/// For periodic fees, computes the fee amount as `base * bps * year_fraction`
/// where base is either the drawn balance or the undrawn balance (facility_limit - outstanding).
///
/// When a fee's `accrual_basis` is `TimeWeightedAverage`, the outstanding balance used
/// for the base amount is computed as a time-weighted average over the accrual period,
/// using the `outstanding_history` map. This is useful for commitment fees on revolving
/// facilities where the outstanding changes within the fee accrual period.
pub(in crate::builder) fn emit_fees_on(
    d: Date,
    periodic_fees: &[PeriodicFee],
    fixed_fees: &[(Date, Money)],
    outstanding: Decimal,
    outstanding_history: &finstack_core::HashMap<Date, Decimal>,
    ccy: Currency,
    new_flows: &mut Vec<CashFlow>,
) -> finstack_core::Result<()> {
    // Lazily allocated: only `TimeWeightedAverage` fees touch this buffer (via
    // `compute_time_weighted_average`, which clears + extends it itself). For
    // the common cases — no periodic fees, point-in-time fees, or a date with
    // no matching fee period — `Vec::new()` never allocates, avoiding an
    // O(history) allocation on every build date.
    let mut twa_buf: Vec<(Date, Decimal)> = Vec::new();

    for pf in periodic_fees {
        if let Some(period) = pf.prev.get(&d) {
            // Use proper DayCountContext with calendar and frequency so that
            // conventions like Bus/252 and Act/Act ISMA compute correctly.
            let yf = pf.dc.year_fraction(
                period.accrual_start,
                period.accrual_end,
                finstack_core::dates::DayCountContext {
                    calendar: Some(pf.calendar),
                    frequency: Some(pf.freq),
                    bus_basis: None,
                    coupon_period: None,
                },
            )?;

            // Determine the outstanding to use based on accrual basis
            let effective_outstanding = match pf.accrual_basis {
                FeeAccrualBasis::PointInTime => outstanding,
                FeeAccrualBasis::TimeWeightedAverage => compute_time_weighted_average(
                    outstanding_history,
                    period.accrual_start,
                    period.accrual_end,
                    outstanding,
                    &mut twa_buf,
                ),
            };

            let base_amt = match &pf.base {
                FeeBase::Drawn => effective_outstanding,
                FeeBase::Undrawn { facility_limit } => {
                    if facility_limit.currency() != ccy {
                        return Err(InputError::Invalid.into());
                    }
                    let facility_limit_dec = f64_to_decimal(facility_limit.amount())?;
                    let undrawn = facility_limit_dec - effective_outstanding;
                    if undrawn > Decimal::ZERO {
                        undrawn
                    } else {
                        Decimal::ZERO
                    }
                }
            };

            let yf_dec = f64_to_decimal(yf)?;
            let fee_amt_dec = base_amt * pf.bps * BP_TO_RATE * yf_dec;
            let fee_amt = decimal_to_f64(fee_amt_dec)?;

            // Convert rate from bps to decimal for storage
            let rate_dec = pf.bps * BP_TO_RATE;
            let rate = decimal_to_f64(rate_dec)?;

            if fee_amt > 0.0 {
                new_flows.push(CashFlow {
                    date: d,
                    reset_date: None,
                    amount: Money::new(fee_amt, ccy),
                    kind: CFKind::Fee,
                    accrual_factor: yf,
                    rate: Some(rate),
                });
            }
        }
    }

    for (fd, amt) in fixed_fees {
        if *fd == d && amt.amount() != 0.0 {
            new_flows.push(CashFlow {
                date: d,
                reset_date: None,
                amount: *amt,
                kind: CFKind::Fee,
                // Fixed fees don't have an accrual period - use 0.0
                accrual_factor: 0.0,
                rate: None,
            });
        }
    }
    Ok(())
}

/// Parameters for emitting revolving-credit fee cashflows for one accrual period.
#[derive(Debug, Clone, Copy)]
pub struct RevolvingFeeEmissionConfig {
    /// Payment date for all emitted fee cashflows.
    pub payment_date: Date,
    /// Drawn balance used as the base for usage fees.
    pub drawn_balance: f64,
    /// Undrawn balance used as the base for commitment fees.
    pub undrawn_balance: f64,
    /// Total commitment amount used as the base for facility fees.
    pub commitment_amount: f64,
    /// Commitment fee quote in basis points.
    pub commitment_fee_bp: f64,
    /// Usage fee quote in basis points.
    pub usage_fee_bp: f64,
    /// Facility fee quote in basis points.
    pub facility_fee_bp: f64,
    /// Accrual factor for the period, expressed in years.
    pub year_fraction: f64,
    /// Currency applied to all emitted fee cashflows.
    pub currency: Currency,
}

/// Emit all revolving-credit fee cashflows for a single accrual period.
pub fn emit_revolving_credit_fees(
    flows: &mut Vec<CashFlow>,
    cfg: &RevolvingFeeEmissionConfig,
) -> finstack_core::Result<()> {
    if let Some(cf) = emit_revolving_fee_on(
        cfg.payment_date,
        cfg.undrawn_balance,
        cfg.commitment_fee_bp,
        cfg.year_fraction,
        cfg.currency,
        CFKind::CommitmentFee,
    )? {
        flows.push(cf);
    }

    if let Some(cf) = emit_revolving_fee_on(
        cfg.payment_date,
        cfg.drawn_balance,
        cfg.usage_fee_bp,
        cfg.year_fraction,
        cfg.currency,
        CFKind::UsageFee,
    )? {
        flows.push(cf);
    }

    if let Some(cf) = emit_revolving_fee_on(
        cfg.payment_date,
        cfg.commitment_amount,
        cfg.facility_fee_bp,
        cfg.year_fraction,
        cfg.currency,
        CFKind::FacilityFee,
    )? {
        flows.push(cf);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::compiler::PeriodicFee;
    use crate::builder::date_generation::SchedulePeriod;
    use crate::builder::specs::{FeeAccrualBasis, FeeBase};
    use finstack_core::currency::Currency;
    use finstack_core::dates::{DayCount, Tenor};
    use rust_decimal_macros::dec;
    use time::Month;

    /// Helper to build a simple PeriodicFee with one period.
    fn make_periodic_fee(
        accrual_start: Date,
        accrual_end: Date,
        payment_date: Date,
        bps: Decimal,
        accrual_basis: FeeAccrualBasis,
        base: FeeBase,
    ) -> PeriodicFee {
        let mut prev = finstack_core::HashMap::default();
        prev.insert(
            payment_date,
            SchedulePeriod {
                accrual_start,
                accrual_end,
                payment_date,
                reset_date: None,
                accrual_year_fraction: 0.0,
            },
        );
        PeriodicFee {
            base,
            bps,
            dc: DayCount::Act360,
            freq: Tenor::quarterly(),
            calendar: crate::builder::calendar::resolve_calendar_strict("weekends_only")
                .expect("weekends_only calendar should resolve"),
            dates: vec![accrual_start, accrual_end],
            prev,
            accrual_basis,
        }
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
        let history = finstack_core::HashMap::default();
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
    }

    #[test]
    fn twa_with_constant_outstanding_matches_point_in_time() {
        let start = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
        let end = Date::from_calendar_date(2025, Month::April, 15).expect("valid date");
        let payment = end;
        let outstanding = dec!(1000000);

        let mut history = finstack_core::HashMap::default();
        history.insert(start, outstanding);

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

        let mut history = finstack_core::HashMap::default();
        history.insert(start, dec!(1000000));
        history.insert(mid, dec!(500000));

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

        assert_eq!(flows.len(), 1);
        let fee = flows[0].amount.amount();
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

        let mut history = finstack_core::HashMap::default();
        history.insert(start, dec!(1000000));
        history.insert(mid, dec!(500000));

        let pf = make_periodic_fee(
            start,
            end,
            payment,
            dec!(50),
            FeeAccrualBasis::TimeWeightedAverage,
            FeeBase::Undrawn {
                facility_limit: Money::new(facility_limit, Currency::USD),
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

        assert_eq!(flows.len(), 1);
        let twa_outstanding = (1_000_000.0 * 30.0 + 500_000.0 * 60.0) / 90.0;
        let undrawn = facility_limit - twa_outstanding;
        let expected_fee = undrawn * 0.005 * (90.0 / 360.0);
        let fee = flows[0].amount.amount();
        assert!(
            (fee - expected_fee).abs() < 0.02,
            "Expected ~{:.2}, got {:.2}",
            expected_fee,
            fee
        );
    }

    #[test]
    fn compute_twa_no_history_returns_fallback() {
        let start = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
        let end = Date::from_calendar_date(2025, Month::April, 15).expect("valid date");
        let history = finstack_core::HashMap::default();
        let mut buf = Vec::new();
        let result = compute_time_weighted_average(&history, start, end, dec!(42), &mut buf);
        assert_eq!(result, dec!(42));
    }

    #[test]
    fn compute_twa_single_entry_at_start() {
        let start = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
        let end = Date::from_calendar_date(2025, Month::April, 15).expect("valid date");
        let mut history = finstack_core::HashMap::default();
        history.insert(start, dec!(1000000));
        let mut buf = Vec::new();
        let result = compute_time_weighted_average(&history, start, end, dec!(0), &mut buf);
        assert_eq!(result, dec!(1000000), "Expected 1M, got {}", result);
    }

    #[test]
    fn compute_twa_entry_before_start() {
        let before = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let start = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
        let end = Date::from_calendar_date(2025, Month::April, 15).expect("valid date");
        let mut history = finstack_core::HashMap::default();
        history.insert(before, dec!(1000000));
        let mut buf = Vec::new();
        let result = compute_time_weighted_average(&history, start, end, dec!(0), &mut buf);
        assert_eq!(result, dec!(1000000), "Expected 1M, got {}", result);
    }

    #[test]
    fn emits_all_non_zero_revolving_fee_kinds() {
        let payment_date = Date::from_calendar_date(2025, Month::March, 31).expect("valid date");
        let mut flows = Vec::new();

        emit_revolving_credit_fees(
            &mut flows,
            &RevolvingFeeEmissionConfig {
                payment_date,
                drawn_balance: 400_000.0,
                undrawn_balance: 600_000.0,
                commitment_amount: 1_000_000.0,
                commitment_fee_bp: 25.0,
                usage_fee_bp: 15.0,
                facility_fee_bp: 10.0,
                year_fraction: 0.25,
                currency: Currency::USD,
            },
        )
        .expect("finite fee inputs");

        assert_eq!(flows.len(), 3);
        assert_eq!(flows[0].kind, CFKind::CommitmentFee);
        assert_eq!(flows[1].kind, CFKind::UsageFee);
        assert_eq!(flows[2].kind, CFKind::FacilityFee);
    }
}
