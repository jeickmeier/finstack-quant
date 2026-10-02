//! Schedule anchor generation.

use super::*;
use time::Month;

pub(super) type Buffer = SmallVec<[Date; 32]>;

pub(super) const MAX_SCHEDULE_ANCHORS: usize = 100_000;

fn schedule_too_large_error() -> crate::Error {
    crate::Error::Validation(format!(
        "schedule generation exceeded {MAX_SCHEDULE_ANCHORS} anchors; \
         check date range and tenor frequency"
    ))
}

fn check_anchor_count(len: usize) -> crate::Result<()> {
    if len > MAX_SCHEDULE_ANCHORS {
        return Err(schedule_too_large_error());
    }
    Ok(())
}

fn next_roll_index(i: i32) -> crate::Result<i32> {
    i.checked_add(1).ok_or_else(schedule_too_large_error)
}

#[inline]
fn maybe_eom(eom: bool, d: Date) -> Date {
    if eom {
        d.end_of_month()
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
/// Generate IMM dates (third Wednesday of Mar/Jun/Sep/Dec) within the given range.
///
/// Unlike regular schedule generation which adds fixed intervals, this function
/// computes the actual third Wednesday of each quarterly month to handle the
/// variable day-of-month correctly.
pub(super) fn generate_imm_dates(start: Date, end: Date) -> crate::Result<Vec<Date>> {
    let mut dates = Vec::new();
    let last_imm = crate::dates::third_wednesday(Month::December, Date::MAX.year())?;
    if start > last_imm {
        return Ok(dates);
    }

    let first_imm = if crate::dates::imm::is_imm_date(start) {
        start
    } else {
        next_imm(start)?
    };

    if first_imm > end {
        return Ok(dates);
    }

    dates.push(first_imm);

    let mut current = first_imm;
    while current < end && current < last_imm {
        let next = next_imm(current)?;
        if next > end {
            break;
        }
        dates.push(next);
        current = next;
    }

    Ok(dates)
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

// BuilderInternal – raw date sequence generator

#[derive(Clone, Copy)]
pub(super) struct BuilderInternal {
    pub start: Date,
    pub end: Date,
    pub frequency: Tenor,
    pub stub: StubKind,
    pub eom: bool,
}

impl BuilderInternal {
    pub(super) fn generate(self) -> crate::Result<Vec<Date>> {
        if self.start >= self.end {
            return Err(crate::error::InputError::InvalidDateRange.into());
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
    /// Returns `None` when the roll lies beyond the finite date range; a stub
    /// may terminate at its contractual endpoint without constructing that
    /// comparison anchor. Invalid tenor arithmetic still returns an error.
    pub(super) fn nth_tenor(self, anchor: Date, n: i32) -> crate::Result<Option<Date>> {
        let tenor = self.frequency;
        if n == 0 {
            return Ok(Some(anchor));
        }
        let count_i32 =
            i32::try_from(tenor.count()).map_err(|_| crate::error::InputError::InvalidTenor {
                tenor: tenor.to_string(),
                reason: format!("count {} exceeds i32::MAX", tenor.count()),
            })?;
        let checked_mul_i32 = |lhs: i32, rhs: i32| -> crate::Result<i32> {
            lhs.checked_mul(rhs).ok_or_else(|| {
                crate::Error::from(crate::error::InputError::InvalidTenor {
                    tenor: tenor.to_string(),
                    reason: "tenor multiplication overflowed while generating schedule".to_string(),
                })
            })
        };
        let checked_mul_i64 = |lhs: i64, rhs: i64| -> crate::Result<i64> {
            lhs.checked_mul(rhs).ok_or_else(|| {
                crate::Error::from(crate::error::InputError::InvalidTenor {
                    tenor: tenor.to_string(),
                    reason: "tenor multiplication overflowed while generating schedule".to_string(),
                })
            })
        };
        let add_months = |months: i32| -> crate::Result<Option<Date>> {
            let month_index = |date: Date| i64::from(date.year()) * 12 + date.month() as i64 - 1;
            let target = month_index(anchor) + i64::from(months);
            if !(month_index(Date::MIN)..=month_index(Date::MAX)).contains(&target) {
                return Ok(None);
            }
            anchor.add_months(months).map(Some)
        };
        let add_days = |days: i64| -> crate::Result<Option<Date>> {
            let origin = i64::from(anchor.to_julian_day());
            let minimum = i64::from(Date::MIN.to_julian_day()) - origin;
            let maximum = i64::from(Date::MAX.to_julian_day()) - origin;
            if !(minimum..=maximum).contains(&days) {
                return Ok(None);
            }
            anchor.add_days(days).map(Some)
        };
        match tenor.unit() {
            crate::dates::TenorUnit::Months => add_months(checked_mul_i32(n, count_i32)?),
            crate::dates::TenorUnit::Years => {
                let years = checked_mul_i32(n, count_i32)?;
                add_months(checked_mul_i32(years, 12)?)
            }
            crate::dates::TenorUnit::Weeks => add_days(checked_mul_i64(
                checked_mul_i64(i64::from(n), i64::from(tenor.count()))?,
                7,
            )?),
            crate::dates::TenorUnit::Days => {
                add_days(checked_mul_i64(i64::from(n), i64::from(tenor.count()))?)
            }
        }
    }

    // EOM convention: `maybe_eom` snaps only the COMPUTED intermediate roll
    // dates to month-end. The user-provided `start` and `end` dates are
    // contractual and are emitted verbatim (QuantLib-style); snapping them
    // (the previous behavior) silently moved the effective date and maturity.

    pub(super) fn gen_regular(self) -> crate::Result<Vec<Date>> {
        let mut buf: Buffer = Buffer::new();
        buf.push(self.start);
        let mut i = 1;
        loop {
            // Alignment is against the actual roll grid, including EOM.
            // A contractual maturity between grid points needs an explicit stub.
            let Some(raw) = self.nth_tenor(self.start, i)? else {
                return Err(crate::error::InputError::NonIntegerScheduleTenor.into());
            };
            let snapped = maybe_eom(self.eom, raw);
            if snapped == self.end {
                push_if_new(&mut buf, self.end);
                check_anchor_count(buf.len())?;
                break;
            }
            if snapped > self.end {
                return Err(crate::error::InputError::NonIntegerScheduleTenor.into());
            }
            if snapped < self.end {
                push_if_new(&mut buf, snapped);
                check_anchor_count(buf.len())?;
            }
            i = next_roll_index(i)?;
        }
        Ok(buf.into_vec())
    }

    pub(super) fn gen_short_back(self) -> crate::Result<Vec<Date>> {
        let mut buf: Buffer = Buffer::new();
        buf.push(self.start);
        let mut i = 1;
        loop {
            let dt = match self
                .nth_tenor(self.start, i)?
                .map(|raw| maybe_eom(self.eom, raw))
            {
                Some(dt) if dt < self.end => dt,
                _ => {
                    push_if_new(&mut buf, self.end);
                    check_anchor_count(buf.len())?;
                    break;
                }
            };
            push_if_new(&mut buf, dt);
            check_anchor_count(buf.len())?;
            i = next_roll_index(i)?;
        }
        Ok(buf.into_vec())
    }

    pub(super) fn gen_short_front(self) -> crate::Result<Vec<Date>> {
        let mut buf: Buffer = Buffer::new();
        let anchor = self.end;
        push_if_new(&mut buf, anchor);
        let mut i = 1;
        loop {
            let dt = match self
                .nth_tenor(anchor, -i)?
                .map(|raw| maybe_eom(self.eom, raw))
            {
                Some(dt) if dt > self.start => dt,
                _ => {
                    push_if_new(&mut buf, self.start);
                    check_anchor_count(buf.len())?;
                    break;
                }
            };
            let snapped = maybe_eom(self.eom, dt);
            if snapped > self.start && snapped < self.end {
                push_if_new(&mut buf, snapped);
                check_anchor_count(buf.len())?;
            }
            i = next_roll_index(i)?;
        }
        buf.as_mut_slice().reverse();
        Ok(buf.into_vec())
    }

    pub(super) fn gen_long_front(self) -> crate::Result<Vec<Date>> {
        // Regular anchors backward from `end`; `aligned` records whether the
        // lowest anchor lands exactly on `start` (no stub at all).
        let mut anchors: Vec<Date> = vec![self.end];
        let mut i = 1;
        let aligned = loop {
            let Some(dt) = self
                .nth_tenor(self.end, -i)?
                .map(|raw| maybe_eom(self.eom, raw))
            else {
                break false;
            };
            if dt <= self.start {
                break dt == self.start;
            }
            anchors.push(dt);
            check_anchor_count(anchors.len())?;
            i = next_roll_index(i)?;
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

    pub(super) fn gen_long_back(self) -> crate::Result<Vec<Date>> {
        let mut buf: Buffer = Buffer::new();
        let anchor = self.start;
        buf.push(anchor);
        let mut i = 1;
        loop {
            let Some(next) = self
                .nth_tenor(anchor, i)?
                .map(|raw| maybe_eom(self.eom, raw))
            else {
                push_if_new(&mut buf, self.end);
                check_anchor_count(buf.len())?;
                break;
            };
            // An aligned contractual end needs no successor for comparison.
            if next >= self.end {
                push_if_new(&mut buf, self.end);
                check_anchor_count(buf.len())?;
                break;
            }
            let next_after = self
                .nth_tenor(anchor, next_roll_index(i)?)?
                .map(|raw| maybe_eom(self.eom, raw));
            if next_after.is_none_or(|date| date > self.end) {
                push_if_new(&mut buf, self.end);
                check_anchor_count(buf.len())?;
                break;
            }
            let dt = maybe_eom(self.eom, next);
            if dt < self.end {
                push_if_new(&mut buf, dt);
                check_anchor_count(buf.len())?;
            }
            i = next_roll_index(i)?;
        }
        Ok(buf.into_vec())
    }
}
