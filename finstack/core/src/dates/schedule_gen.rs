//! Internal schedule date generation machinery.
//!
//! Contains [`BuilderInternal`] and helper functions for producing raw date
//! sequences from a frequency / stub / EOM specification.  This is consumed
//! exclusively by the public [`ScheduleBuilder`](super::ScheduleBuilder).

use smallvec::SmallVec;
use time::{Date, Duration};

use super::next_imm;
use crate::dates::date_extensions::DateExt;
use crate::dates::schedule_iter::StubKind;
use crate::dates::Tenor;

/// Small helper alias when we need to pre-buffer (used only for `ShortFront`).
type Buffer = SmallVec<[Date; 32]>;

/// Apply End-of-Month (EOM) convention to a date.
fn apply_eom(date: Date) -> Date {
    date.end_of_month()
}

#[inline]
fn maybe_eom(eom: bool, d: Date) -> Date {
    if eom {
        apply_eom(d)
    } else {
        d
    }
}

#[inline]
fn push_if_new(buf: &mut Buffer, d: Date) {
    if buf.last().copied() != Some(d) {
        buf.push(d)
    }
}

/// Check if a date is a CDS roll date (20th of Mar/Jun/Sep/Dec).
pub(super) fn is_cds_roll_date(date: Date) -> bool {
    crate::dates::imm::is_cds_date(date)
}

/// Check if a date is a standard IMM date (third Wednesday of Mar/Jun/Sep/Dec).
pub(super) fn is_imm_roll_date(date: Date) -> bool {
    crate::dates::imm::is_imm_date(date)
}

/// Generate IMM dates (third Wednesday of Mar/Jun/Sep/Dec) within the given range.
///
/// Unlike regular schedule generation which adds fixed intervals, this function
/// computes the actual third Wednesday of each quarterly month to handle the
/// variable day-of-month correctly.
pub(super) fn generate_imm_dates(start: Date, end: Date) -> Vec<Date> {
    let mut dates = Vec::new();

    let first_imm = if is_imm_roll_date(start) {
        start
    } else {
        next_imm(start)
    };

    if first_imm > end {
        return dates;
    }

    dates.push(first_imm);

    let mut current = first_imm;
    loop {
        let next = next_imm(current);
        if next > end {
            break;
        }
        dates.push(next);
        current = next;
    }

    dates
}

/// Enforce strictly increasing, duplicate-free dates while preserving original order.
/// Drops any consecutive duplicates and any dates that would not increase.
pub(super) fn enforce_monotonic_and_dedup(dates: &mut Vec<Date>) {
    if dates.is_empty() {
        return;
    }
    let mut write = 0;
    for read in 1..dates.len() {
        if dates[read] > dates[write] {
            write += 1;
            if read != write {
                dates[write] = dates[read];
            }
        }
    }
    dates.truncate(write + 1);
}

/// Like [`enforce_monotonic_and_dedup`] but NEVER drops the terminal
/// (maturity) date.
///
/// Used after business-day adjustment: when an interior anchor adjusts onto
/// or past the adjusted terminal date (e.g. Modified Following pushing the
/// penultimate anchor forward while Preceding pulls maturity back), the
/// forward-scanning dedup would keep the interior date and silently drop
/// maturity — truncating the final accrual period. This variant scans
/// backward so collisions merge into the earlier period and the terminal
/// date always survives.
pub(super) fn enforce_monotonic_keep_terminal(dates: &mut Vec<Date>) {
    if dates.len() < 2 {
        return;
    }
    let mut keep: Vec<Date> = Vec::with_capacity(dates.len());
    let mut upper: Option<Date> = None;
    for &d in dates.iter().rev() {
        if upper.is_none_or(|u| d < u) {
            keep.push(d);
            upper = Some(d);
        }
    }
    keep.reverse();
    *dates = keep;
}

#[cfg(test)]
mod tests {
    use super::enforce_monotonic_keep_terminal;
    use time::{Date, Month};

    fn d(year: i32, month: u8, day: u8) -> Date {
        Date::from_calendar_date(year, Month::try_from(month).expect("month"), day)
            .expect("valid date")
    }

    #[test]
    fn keep_terminal_drops_interior_collision_not_maturity() {
        // Penultimate adjusted past the adjusted terminal: terminal survives.
        let mut dates = vec![
            d(2024, 1, 31),
            d(2024, 2, 29),
            d(2024, 3, 29),
            d(2024, 3, 28),
        ];
        enforce_monotonic_keep_terminal(&mut dates);
        assert_eq!(dates, vec![d(2024, 1, 31), d(2024, 2, 29), d(2024, 3, 28)]);
    }

    #[test]
    fn keep_terminal_dedups_equal_runs() {
        let mut dates = vec![d(2024, 1, 31), d(2024, 3, 29), d(2024, 3, 29)];
        enforce_monotonic_keep_terminal(&mut dates);
        assert_eq!(dates, vec![d(2024, 1, 31), d(2024, 3, 29)]);
    }

    #[test]
    fn keep_terminal_noop_on_sorted_input() {
        let mut dates = vec![d(2024, 1, 31), d(2024, 2, 29), d(2024, 3, 29)];
        let expected = dates.clone();
        enforce_monotonic_keep_terminal(&mut dates);
        assert_eq!(dates, expected);
    }
}

// ---------------------------------------------------------------------------
// BuilderInternal – raw date sequence generator
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub(super) struct BuilderInternal {
    pub start: Date,
    pub end: Date,
    pub freq: Tenor,
    pub stub: StubKind,
    pub eom: bool,
}

impl BuilderInternal {
    pub(super) fn generate(self) -> crate::Result<Vec<Date>> {
        if self.start >= self.end {
            return Err(crate::error::InputError::InvalidScheduleRange {
                start: self.start,
                end: self.end,
            }
            .into());
        }
        match self.stub {
            StubKind::ShortFront => self.gen_short_front(),
            StubKind::LongFront => self.gen_long_front(),
            StubKind::LongBack => self.gen_long_back(),
            StubKind::None => self.gen_regular(),
            StubKind::ShortBack => self.gen_short_back(),
        }
    }

    /// The `n`-th roll date from a fixed `anchor` (`n` may be negative for
    /// backward generation).
    ///
    /// Every date is computed as `anchor + n·tenor` directly from the anchor
    /// (QuantLib-style), so month-end clamping in short months never
    /// propagates: backward semi-annual from Aug 31 yields Feb 28/29 and then
    /// Aug **31** again, not Aug 28. Chaining `prev + tenor` (the previous
    /// implementation) drifted the roll day by 1–3 days per short month.
    fn nth_tenor(self, anchor: Date, n: i32) -> crate::Result<Date> {
        let tenor = self.freq;
        if n == 0 {
            return Ok(anchor);
        }
        let count_i32 =
            i32::try_from(tenor.count).map_err(|_| crate::error::InputError::InvalidTenor {
                tenor: tenor.to_string(),
                reason: format!("count {} exceeds i32::MAX", tenor.count),
            })?;
        Ok(match tenor.unit {
            crate::dates::TenorUnit::Months => anchor.add_months(n * count_i32),
            crate::dates::TenorUnit::Years => anchor.add_months(n * count_i32 * 12),
            crate::dates::TenorUnit::Weeks => {
                anchor + Duration::weeks(i64::from(n) * i64::from(tenor.count))
            }
            crate::dates::TenorUnit::Days => {
                anchor + Duration::days(i64::from(n) * i64::from(tenor.count))
            }
        })
    }

    // EOM convention: `maybe_eom` snaps only the COMPUTED intermediate roll
    // dates to month-end. The user-provided `start` and `end` dates are
    // contractual and are emitted verbatim (QuantLib-style); snapping them
    // (the previous behavior) silently moved the effective date and maturity.

    fn gen_regular(self) -> crate::Result<Vec<Date>> {
        let mut buf: Buffer = Buffer::new();
        buf.push(self.start);
        let mut i = 1;
        loop {
            // Alignment with `end` is judged on the raw (un-snapped) roll so
            // that `StubKind::None` still accepts schedules whose endpoints
            // are not month-ends. Under the EOM convention the effective roll
            // is the month-end snap, so an `end` anywhere in `(raw, snapped]`
            // (e.g. the last business day of the roll month) is also aligned.
            let raw = self.nth_tenor(self.start, i)?;
            let snapped = maybe_eom(self.eom, raw);
            if raw == self.end || (raw < self.end && self.end <= snapped) {
                push_if_new(&mut buf, self.end);
                break;
            }
            if raw > self.end {
                return Err(crate::error::InputError::NonIntegerScheduleTenor.into());
            }
            if snapped < self.end {
                push_if_new(&mut buf, snapped);
            }
            i += 1;
        }
        Ok(buf.into_vec())
    }

    fn gen_short_back(self) -> crate::Result<Vec<Date>> {
        let mut buf: Buffer = Buffer::new();
        buf.push(self.start);
        let mut i = 1;
        loop {
            let dt = maybe_eom(self.eom, self.nth_tenor(self.start, i)?);
            if dt >= self.end {
                push_if_new(&mut buf, self.end);
                break;
            }
            push_if_new(&mut buf, dt);
            i += 1;
        }
        Ok(buf.into_vec())
    }

    fn gen_short_front(self) -> crate::Result<Vec<Date>> {
        let mut buf: Buffer = Buffer::new();
        let anchor = self.end;
        push_if_new(&mut buf, anchor);
        let mut i = 1;
        loop {
            let dt = self.nth_tenor(anchor, -i)?;
            if dt <= self.start {
                push_if_new(&mut buf, self.start);
                break;
            }
            let snapped = maybe_eom(self.eom, dt);
            if snapped > self.start && snapped < self.end {
                push_if_new(&mut buf, snapped);
            }
            i += 1;
        }
        buf.as_mut_slice().reverse();
        Ok(buf.into_vec())
    }

    fn gen_long_front(self) -> crate::Result<Vec<Date>> {
        // Regular anchors backward from `end`; `aligned` records whether the
        // lowest anchor lands exactly on `start` (no stub at all).
        let mut anchors: Vec<Date> = vec![self.end];
        let mut i = 1;
        let aligned = loop {
            let dt = self.nth_tenor(self.end, -i)?;
            if dt <= self.start {
                break dt == self.start;
            }
            anchors.push(dt);
            i += 1;
        };
        // Long front stub: merge the residual short stub with the first
        // regular period by dropping the lowest anchor. Skipping this merge
        // (the previous behavior) made LongFront identical to ShortFront.
        if !aligned && anchors.len() > 1 {
            anchors.pop();
        }
        let mut buf: Buffer = Buffer::new();
        buf.push(self.start);
        for (idx, &a) in anchors.iter().enumerate().rev() {
            // anchors[0] is the user-provided end date: never snap it.
            let dt = if idx == 0 { a } else { maybe_eom(self.eom, a) };
            if dt > self.start && (idx == 0 || dt < self.end) {
                push_if_new(&mut buf, dt);
            }
        }
        Ok(buf.into_vec())
    }

    fn gen_long_back(self) -> crate::Result<Vec<Date>> {
        let mut buf: Buffer = Buffer::new();
        let anchor = self.start;
        buf.push(anchor);
        let mut i = 1;
        loop {
            let next = self.nth_tenor(anchor, i)?;
            let next_after = self.nth_tenor(anchor, i + 1)?;
            if next_after > self.end {
                push_if_new(&mut buf, self.end);
                break;
            }
            let dt = maybe_eom(self.eom, next);
            if dt < self.end {
                push_if_new(&mut buf, dt);
            }
            i += 1;
        }
        Ok(buf.into_vec())
    }
}
