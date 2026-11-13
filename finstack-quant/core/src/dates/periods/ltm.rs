//! Last-twelve-months (LTM) windows over a validated period plan.

use super::{PeriodId, PeriodKind, PeriodPlan};
use crate::dates::date_extensions::DateExt;

/// The plan periods a last-twelve-months sum reads, ending at one period.
///
/// Built by [`ltm_window`]. A window is complete only when no expected slot
/// is missing; it says nothing about the values, which callers check.
/// Callers never annualise, rescale or partially sum an incomplete window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LtmWindow {
    /// Plan periods of the window, in chronological order, ending with the
    /// target period.
    pub slots: Vec<PeriodId>,
    /// Expected canonical slots of the window that the plan does not hold.
    pub missing: u32,
}

impl LtmWindow {
    /// Number of slots a complete window holds: present plus missing.
    #[must_use]
    pub fn required(&self) -> usize {
        self.slots.len() + self.missing as usize
    }

    /// Whether every expected slot of the window is present in the plan.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.missing == 0
    }
}

/// Trailing-twelve-month window of plan periods ending at (and including) `target`.
///
/// | Frequency | Window |
/// | --- | --- |
/// | Monthly, quarterly, semi-annual, annual | The last 12, 4, 2 or 1 canonical reporting periods ending at `target`, stepped in the plan's calendar |
/// | Daily | Every canonical calendar day starting in `[target.end - 12 months, target.end)`, with the month-end start clamped |
/// | Weekly | Rejected: 52 weeks is 364 days and a fiscal week 53 may be short, so neither is twelve months |
///
/// Daily means full calendar-day coverage: a sparse business-day model is
/// not a complete daily window, and an absent row does not prove a weekend or
/// holiday (record CORE-CONVENTIONS-014 #11). The window reads only the plan;
/// it does not filter actual or forecast periods.
///
/// # Arguments
///
/// * `target` - Reporting period that ends the window (inclusive); it must
///   be a period of `plan`.
/// * `plan` - Validated period plan whose calendar context steps the window.
///
/// # Returns
///
/// The present slots, oldest first, and the count of expected slots absent
/// from the plan.
///
/// # Errors
///
/// Returns `Error::Validation` when `target` is not in `plan`, `target` is
/// weekly, or stepping back leaves the supported date range. These are
/// errors, not missing slots.
///
/// # Example
/// ```
/// use finstack_quant_core::dates::{build_periods, ltm_window, PeriodId};
///
/// let plan = build_periods("2024Q1..2025Q2", None)?;
/// let window = ltm_window(PeriodId::quarter(2025, 1)?, &plan)?;
/// assert!(window.is_complete());
/// assert_eq!(window.slots.first(), Some(&PeriodId::quarter(2024, 2)?));
/// assert_eq!(window.slots.len(), 4);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn ltm_window(target: PeriodId, plan: &PeriodPlan) -> crate::Result<LtmWindow> {
    let target_period = plan
        .periods
        .iter()
        .find(|period| period.id == target)
        .ok_or_else(|| {
            crate::Error::Validation(format!("LTM target {target} is not a period of the plan"))
        })?;
    let present = |id: &PeriodId| plan.periods.iter().any(|period| period.id == *id);
    match target.kind() {
        PeriodKind::Weekly => Err(crate::Error::Validation(format!(
            "LTM is not defined for weekly period {target}: 52 weeks is 364 days and a fiscal \
             week 53 may be short, so neither is twelve months"
        ))),
        PeriodKind::Daily => {
            let end = target_period.end;
            let start = end.add_months(-12)?;
            let slots: Vec<PeriodId> = plan
                .periods
                .iter()
                .filter(|period| period.start >= start && period.start < end)
                .map(|period| period.id)
                .collect();
            let required = (end - start).whole_days();
            let missing = u32::try_from(required - slots.len() as i64).map_err(|_| {
                crate::Error::Validation(format!("LTM window for {target} is out of range"))
            })?;
            Ok(LtmWindow { slots, missing })
        }
        kind => {
            let count = i32::from(kind.periods_per_year());
            let mut window = LtmWindow {
                slots: Vec::with_capacity(count as usize),
                missing: 0,
            };
            for back in (0..count).rev() {
                let slot = target.shift(-back, plan.calendar)?;
                if present(&slot) {
                    window.slots.push(slot);
                } else {
                    window.missing += 1;
                }
            }
            Ok(window)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dates::{
        Date, FiscalConfig, Period, PeriodCalendar, build_fiscal_periods, build_periods,
        create_date,
    };
    use time::Month;

    fn day(date: Date) -> Period {
        let id = PeriodId::day(date.year(), date.ordinal()).expect("day id");
        Period {
            id,
            start: date,
            end: date.next_day().expect("next day"),
            is_actual: true,
        }
    }

    fn daily_plan(from: Date, to: Date, business_only: bool) -> PeriodPlan {
        let mut out = Vec::new();
        let mut d = from;
        while d <= to {
            if !business_only || d.weekday().number_days_from_monday() < 5 {
                out.push(day(d));
            }
            d = d.next_day().expect("next day");
        }
        PeriodPlan::try_new(PeriodCalendar::Gregorian, out).expect("daily plan")
    }

    #[test]
    fn reporting_windows_take_the_last_canonical_periods_of_the_plan() {
        let monthly = build_periods("2024M01..2025M06", None).expect("plan");
        let window = ltm_window(PeriodId::month(2025, 3).unwrap(), &monthly).unwrap();
        assert!(window.is_complete());
        assert_eq!(window.slots.len(), 12);
        assert_eq!(window.slots[0], PeriodId::month(2024, 4).unwrap());

        let quarterly = build_periods("2024Q1..2025Q4", None).expect("plan");
        let window = ltm_window(PeriodId::quarter(2024, 4).unwrap(), &quarterly).unwrap();
        assert_eq!(window.slots.len(), 4);
        assert!(window.is_complete());

        let annual = build_periods("2023..2025", None).expect("plan");
        let year = PeriodId::annual(2024).expect("valid annual period");
        let window = ltm_window(year, &annual).unwrap();
        assert_eq!(window.slots, vec![year]);
        assert!(window.is_complete());
    }

    #[test]
    fn leading_history_absent_from_the_plan_is_missing_not_named() {
        let quarterly = build_periods("2025Q1..2025Q4", None).expect("plan");
        let window = ltm_window(PeriodId::quarter(2025, 2).unwrap(), &quarterly).unwrap();
        assert_eq!(
            window.slots,
            vec![
                PeriodId::quarter(2025, 1).unwrap(),
                PeriodId::quarter(2025, 2).unwrap()
            ]
        );
        assert_eq!(window.missing, 2);
        assert_eq!(window.required(), 4);
    }

    #[test]
    fn fiscal_reporting_windows_step_in_the_plan_calendar() {
        let plan = build_fiscal_periods(
            "FY2024Q1..FY2025Q4",
            FiscalConfig::new(10, 1).expect("valid fiscal start"),
            None,
        )
        .expect("plan");
        let target: PeriodId = "FY2025Q2".parse().unwrap();
        let window = ltm_window(target, &plan).unwrap();
        assert!(window.is_complete());
        assert_eq!(window.slots.first(), Some(&"FY2024Q3".parse().unwrap()));
    }

    #[test]
    fn target_absent_from_the_plan_and_weekly_targets_are_errors() {
        let quarterly = build_periods("2024Q1..2024Q4", None).expect("plan");
        assert!(ltm_window(PeriodId::quarter(2025, 1).unwrap(), &quarterly).is_err());
        let weekly = build_periods("2024W1..2025W10", None).expect("plan");
        let err = ltm_window(PeriodId::week(2025, 5).unwrap(), &weekly).unwrap_err();
        assert!(err.to_string().contains("weekly"), "{err}");
        let fiscal_weekly = build_fiscal_periods(
            "FY2024W1..FY2025W53",
            FiscalConfig::new(10, 1).expect("valid fiscal start"),
            None,
        )
        .expect("plan");
        assert!(ltm_window("FY2025W10".parse().unwrap(), &fiscal_weekly).is_err());
    }

    #[test]
    fn calendar_day_window_spans_a_full_year_not_252_days() {
        let plan = daily_plan(
            create_date(2024, Month::January, 1).unwrap(),
            create_date(2025, Month::December, 31).unwrap(),
            false,
        );
        let at = PeriodId::day(2025, 365).unwrap();
        let window = ltm_window(at, &plan).unwrap();
        assert!(window.is_complete());
        assert_eq!(window.slots.len(), 365);
        assert_eq!(window.slots[0], PeriodId::day(2025, 1).unwrap());
        assert_eq!(window.slots.last(), Some(&at));
    }

    /// A business-day grid is not full calendar-day coverage: absent rows do
    /// not prove weekends or holidays, so the window is incomplete.
    #[test]
    fn business_day_grid_is_not_a_complete_daily_window() {
        let plan = daily_plan(
            create_date(2024, Month::January, 1).unwrap(),
            create_date(2025, Month::December, 31).unwrap(),
            true,
        );
        let window = ltm_window(PeriodId::day(2025, 365).unwrap(), &plan).unwrap();
        assert!(!window.is_complete());
        // 2025 has 261 weekdays; the other 104 calendar days are missing.
        assert_eq!(window.slots.len(), 261);
        assert_eq!(window.missing, 104);
    }

    #[test]
    fn daily_window_before_a_year_of_history_is_incomplete() {
        let plan = daily_plan(
            create_date(2025, Month::March, 1).unwrap(),
            create_date(2025, Month::December, 31).unwrap(),
            false,
        );
        let window = ltm_window(PeriodId::day(2025, 365).unwrap(), &plan).unwrap();
        assert!(!window.is_complete());
        // January and February 2025 are uncovered.
        assert_eq!(window.missing, 59);
        assert_eq!(window.required(), 365);
    }
}
