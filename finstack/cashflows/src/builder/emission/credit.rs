//! Credit event cashflow emission (defaults, prepayments, recoveries).

use crate::primitives::{CFKind, CashFlow};
use finstack_core::currency::Currency;
use finstack_core::dates::CalendarRegistry;
use finstack_core::dates::DateExt;
use finstack_core::dates::{adjust, Date};
use finstack_core::money::Money;

use super::super::specs::DefaultEvent;

/// Emit default and recovery cashflows on a specific date.
///
/// For each default event on date `d`:
/// 1. Emit DefaultedNotional cashflow (reduces outstanding by full defaulted amount)
/// 2. Emit Recovery cashflow on future date (does NOT affect outstanding until that date)
///
/// The recovery cashflow is emitted at a future date based on `recovery_lag`, but the
/// outstanding balance is only reduced by the full defaulted amount at default time.
/// This ensures interest calculations between default and recovery dates use the
/// correct (reduced) outstanding balance.
///
/// # Arguments
///
/// * `d` - Current date to check for default events
/// * `default_events` - Slice of default event specifications
/// * `outstanding` - Mutable reference to outstanding notional balance
/// * `ccy` - Currency for cashflows
///
/// # Returns
///
/// Vector of cashflows (0, 1, or 2 per matching event)
///
/// # Errors
///
/// Returns an error if any matching event fails validation (see
/// [`DefaultEvent::validate`]), if `recovery_lag` exceeds `i32::MAX` months,
/// or if recovery-date adjustment fails (unknown calendar, adjustment error).
///
/// Emission is atomic across all events on date `d`: every event is
/// validated and its recovery date resolved before any cashflow is pushed
/// or `outstanding` is mutated, so a failing event leaves no partial state.
///
/// # Examples
///
/// ```
/// use finstack_cashflows::builder::emit_default_on;
/// use finstack_cashflows::builder::specs::DefaultEvent;
/// use finstack_core::currency::Currency;
/// use finstack_core::dates::Date;
/// use time::Month;
///
/// let d = Date::from_calendar_date(2025, Month::March, 1).expect("valid date");
/// let event = DefaultEvent {
///     default_date: d,
///     defaulted_amount: 100_000.0,
///     recovery_rate: 0.40,
///     recovery_lag: 12,
///     recovery_bdc: None,
///     recovery_calendar_id: None,
///     accrued_on_default: None,
/// };
/// let mut outstanding = 1_000_000.0;
/// let mut flows = Vec::new();
/// emit_default_on(d, &[event], &mut outstanding, Currency::USD, &mut flows).expect("should succeed");
///
/// // Outstanding is reduced by full defaulted amount (recovery is future cash inflow only)
/// assert_eq!(outstanding, 900_000.0);
/// assert_eq!(flows.len(), 2); // Default + Recovery cashflows
/// ```
pub fn emit_default_on(
    d: Date,
    default_events: &[DefaultEvent],
    outstanding: &mut f64,
    ccy: Currency,
    out: &mut Vec<CashFlow>,
) -> finstack_core::Result<()> {
    let matching: Vec<&DefaultEvent> = default_events
        .iter()
        .filter(|e| e.default_date == d)
        .collect();

    // Pass 1: validate every event and resolve every recovery date before
    // any mutation, so a failure on event k cannot leave events 1..k-1
    // partially applied (cross-event atomicity).
    let mut recovery_dates: Vec<Option<Date>> = Vec::with_capacity(matching.len());
    for event in &matching {
        // Validate event parameters (recovery_rate in [0,1], finite defaulted_amount >= 0)
        event.validate()?;

        let recovery_date = if event.defaulted_amount > 0.0 && event.recovery_rate > 0.0 {
            let recovery_lag_months = i32::try_from(event.recovery_lag).map_err(|_| {
                finstack_core::Error::Validation(format!(
                    "recovery_lag = {} exceeds i32::MAX months",
                    event.recovery_lag
                ))
            })?;
            let base_recovery_date = d.add_months(recovery_lag_months);

            // Apply optional business-day adjustment if both BDC and calendar are provided.
            Some(
                if let (Some(bdc), Some(ref cal_id)) =
                    (event.recovery_bdc, &event.recovery_calendar_id)
                {
                    let cal = CalendarRegistry::global()
                        .resolve_str(cal_id.as_str())
                        .ok_or_else(|| {
                            finstack_core::Error::Input(finstack_core::InputError::NotFound {
                                id: cal_id.clone(),
                            })
                        })?;
                    adjust(base_recovery_date, bdc, cal)?
                } else {
                    base_recovery_date
                },
            )
        } else {
            None
        };
        recovery_dates.push(recovery_date);
    }

    // Pass 2: apply mutations; all fallible work happened in pass 1.
    for (event, recovery_date) in matching.iter().zip(recovery_dates) {
        // Clamp defaulted amount to outstanding to prevent negative balances.
        // Similar to how prepayments clamp to outstanding.
        let defaulted = event.defaulted_amount.min(*outstanding).max(0.0);
        if defaulted <= 0.0 {
            continue;
        }

        let recovery_amt = defaulted * event.recovery_rate;

        // Default cashflow
        out.push(CashFlow {
            date: d,
            reset_date: None,
            amount: Money::new(defaulted, ccy),
            kind: CFKind::DefaultedNotional,
            accrual_factor: 0.0,
            rate: None,
        });
        *outstanding -= defaulted;

        // Recovery cashflow (on future date)
        if let Some(recovery_date) = recovery_date {
            out.push(CashFlow {
                date: recovery_date,
                reset_date: None,
                amount: Money::new(recovery_amt, ccy),
                kind: CFKind::Recovery,
                accrual_factor: 0.0,
                rate: None,
            });
            // Note: We do NOT add recovery back to outstanding here.
            // Recovery is a future cash inflow that doesn't restore the
            // principal base for interest calculations. The outstanding
            // balance should only be updated when processing flows as-of
            // their actual dates (e.g., via outstanding_by_date()).
        }

        // Accrued-on-default: ISDA standard CDS convention — protection buyer
        // pays accrued premium from last payment date to default date.
        if let Some(accrued_amt) = event.accrued_on_default {
            if accrued_amt > 0.0 {
                out.push(CashFlow {
                    date: d,
                    reset_date: None,
                    amount: Money::new(accrued_amt, ccy),
                    kind: CFKind::AccruedOnDefault,
                    accrual_factor: 0.0,
                    rate: None,
                });
            }
        }
    }

    Ok(())
}

/// Emit prepayment cashflow on a specific date.
///
/// Reduces outstanding balance by prepayment amount.
/// Prepayments are unscheduled principal reductions, typically
/// driven by behavioral models (CPR/PSA for mortgages, etc.).
///
/// # Arguments
///
/// * `d` - Payment date
/// * `prepayment_amount` - Amount prepaid
/// * `outstanding` - Mutable reference to outstanding balance
/// * `ccy` - Currency
///
/// # Returns
///
/// Vector containing zero or one cashflow
///
/// # Errors
///
/// Returns `Error::Validation` if `prepayment_amount` is non-finite
/// (NaN/∞). A non-finite amount would otherwise silently clamp to the full
/// outstanding balance via `min`, liquidating the position.
///
/// # Examples
///
/// ```
/// use finstack_cashflows::builder::emit_prepayment_on;
/// use finstack_core::currency::Currency;
/// use finstack_core::dates::Date;
/// use time::Month;
///
/// let d = Date::from_calendar_date(2025, Month::March, 1).expect("valid date");
/// let mut outstanding = 1_000_000.0;
/// let mut flows = Vec::new();
/// emit_prepayment_on(d, 50_000.0, &mut outstanding, Currency::USD, &mut flows)
///     .expect("finite prepayment");
///
/// assert_eq!(outstanding, 950_000.0);
/// assert_eq!(flows.len(), 1);
/// ```
pub fn emit_prepayment_on(
    d: Date,
    prepayment_amount: f64,
    outstanding: &mut f64,
    ccy: Currency,
    out: &mut Vec<CashFlow>,
) -> finstack_core::Result<()> {
    if !prepayment_amount.is_finite() {
        return Err(finstack_core::Error::Validation(format!(
            "prepayment amount ({prepayment_amount}) must be finite"
        )));
    }
    if prepayment_amount <= 0.0 {
        return Ok(());
    }

    let amount = prepayment_amount.min(*outstanding);
    if amount > 0.0 {
        *outstanding -= amount;
        out.push(CashFlow {
            date: d,
            reset_date: None,
            amount: Money::new(amount, ccy),
            kind: CFKind::PrePayment,
            accrual_factor: 0.0,
            rate: None,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::dates::BusinessDayConvention;
    use time::Month;

    #[test]
    fn emit_default_rejects_recovery_lag_that_exceeds_i32() {
        let d = Date::from_calendar_date(2025, Month::March, 1).expect("valid date");
        let event = DefaultEvent {
            default_date: d,
            defaulted_amount: 100_000.0,
            recovery_rate: 0.4,
            recovery_lag: u32::MAX,
            recovery_bdc: None,
            recovery_calendar_id: None,
            accrued_on_default: None,
        };
        let mut outstanding = 1_000_000.0;
        let mut flows = Vec::new();

        let err = emit_default_on(d, &[event], &mut outstanding, Currency::USD, &mut flows)
            .expect_err("oversized recovery lag should fail");

        assert!(err.to_string().contains("recovery_lag"));
        assert_eq!(outstanding, 1_000_000.0);
        assert!(flows.is_empty());
    }

    #[test]
    fn emit_default_rejects_unknown_recovery_calendar_without_partial_mutation() {
        let d = Date::from_calendar_date(2025, Month::March, 1).expect("valid date");
        let event = DefaultEvent {
            default_date: d,
            defaulted_amount: 100_000.0,
            recovery_rate: 0.4,
            recovery_lag: 1,
            recovery_bdc: Some(BusinessDayConvention::Following),
            recovery_calendar_id: Some("missing-calendar".to_string()),
            accrued_on_default: None,
        };
        let mut outstanding = 1_000_000.0;
        let mut flows = Vec::new();

        let err = emit_default_on(d, &[event], &mut outstanding, Currency::USD, &mut flows)
            .expect_err("unknown recovery calendar should fail");

        assert!(err.to_string().contains("missing-calendar"));
        assert_eq!(outstanding, 1_000_000.0);
        assert!(flows.is_empty());
    }
}
