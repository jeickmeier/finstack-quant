//! Schedule anchor generation.

use super::*;
use crate::dates::imm::{imm_on_or_after, imm_on_or_before, is_imm_date};

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

/// Day-of-month rule applied to the computed (non-contractual) roll dates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RollDay {
    /// Keep the day the tenor arithmetic produces.
    Natural,
    /// Snap to the last day of the month (`end_of_month`).
    EndOfMonth,
    /// Snap to the third Wednesday of the month (standard IMM roll).
    ThirdWednesday,
}

impl RollDay {
    fn apply(self, d: Date) -> crate::Result<Date> {
        match self {
            Self::Natural => Ok(d),
            Self::EndOfMonth => Ok(d.end_of_month()),
            Self::ThirdWednesday => crate::dates::third_wednesday(d.month(), d.year()),
        }
    }
}

#[inline]
fn push_if_new(buf: &mut Buffer, d: Date) {
    if buf.last().copied() != Some(d) {
        buf.push(d)
    }
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
    pub roll: RollDay,
}

impl BuilderInternal {
    pub(super) fn generate(self) -> crate::Result<Vec<Date>> {
        if self.start >= self.end {
            return Err(crate::InputError::InvalidDateRange.into());
        }
        match self.stub {
            StubKind::ShortFront => self.gen_short_front(),
            StubKind::LongFront => self.gen_long_front(),
            StubKind::LongBack => self.gen_long_back(),
            StubKind::None => self.gen_forward(true),
            StubKind::ShortBack => self.gen_forward(false),
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
        // `Tenor::new` bounds the count (at most 73,200 days), so the cast is exact.
        let count_i32 = tenor.count() as i32;
        let checked_mul_i32 = |lhs: i32, rhs: i32| -> crate::Result<i32> {
            lhs.checked_mul(rhs).ok_or_else(|| {
                crate::Error::from(crate::InputError::InvalidTenor {
                    tenor: tenor.to_string(),
                    reason: "tenor multiplication overflowed while generating schedule".to_string(),
                })
            })
        };
        let checked_mul_i64 = |lhs: i64, rhs: i64| -> crate::Result<i64> {
            lhs.checked_mul(rhs).ok_or_else(|| {
                crate::Error::from(crate::InputError::InvalidTenor {
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
            crate::dates::TenorUnit::Years => add_months(checked_mul_i32(n, count_i32 * 12)?),
            crate::dates::TenorUnit::Weeks => add_days(checked_mul_i64(
                checked_mul_i64(i64::from(n), i64::from(tenor.count()))?,
                7,
            )?),
            crate::dates::TenorUnit::Days => {
                add_days(checked_mul_i64(i64::from(n), i64::from(tenor.count()))?)
            }
        }
    }

    /// The `n`-th roll date from `anchor` with the roll-day rule applied.
    ///
    /// Only these COMPUTED intermediate dates move to month-end or the third
    /// Wednesday. The user-provided `start` and `end` dates are contractual
    /// and are emitted verbatim (QuantLib-style); snapping them silently moves
    /// the effective date and maturity.
    fn rolled(self, anchor: Date, n: i32) -> crate::Result<Option<Date>> {
        self.nth_tenor(anchor, n)?
            .map(|raw| self.roll.apply(raw))
            .transpose()
    }

    /// Standard IMM schedule: a regular grid on the quarterly IMM cycle
    /// (third Wednesday of Mar/Jun/Sep/Dec) at `frequency`, between the
    /// contractual `start` and `end`, which are always kept.
    ///
    /// This is the IMM roll convention (FpML `RollConventionEnum` `IMM`)
    /// applied to an ordinary schedule:
    ///
    /// - `frequency` must be a whole number of quarters (3M, 6M, 1Y, ...).
    /// - Back-stub kinds (`ShortBack`, `LongBack`, `None`) anchor the grid on
    ///   the first IMM date on or after `start` and generate forward, so an
    ///   off-cycle `end` closes a final stub. Front-stub kinds (`ShortFront`,
    ///   `LongFront`) anchor on the last IMM date on or before `end` and
    ///   generate backward, so an off-cycle `start` opens an initial stub.
    /// - An off-cycle endpoint on the anchor side becomes a short stub to the
    ///   nearest IMM date, because the regular periods must start and end on
    ///   IMM dates. With quarterly frequency every IMM date inside the range
    ///   is a roll date, so both directions give the same dates.
    /// - A range with no IMM date strictly inside it is a single period.
    /// - `StubKind::None` requires `start` and `end` on IMM dates and a whole
    ///   number of periods between them.
    pub(super) fn generate_imm(self) -> crate::Result<Vec<Date>> {
        if self.start >= self.end {
            return Err(crate::InputError::InvalidDateRange.into());
        }
        let whole_quarters = match self.frequency.unit() {
            crate::dates::TenorUnit::Months => self.frequency.count().is_multiple_of(3),
            crate::dates::TenorUnit::Years => true,
            crate::dates::TenorUnit::Weeks | crate::dates::TenorUnit::Days => false,
        };
        if !whole_quarters {
            return Err(crate::InputError::InvalidTenor {
                tenor: self.frequency.to_string(),
                reason: "IMM schedules roll on the quarterly IMM cycle, so the frequency \
                         must be a whole number of quarters"
                    .to_string(),
            }
            .into());
        }
        if self.stub == StubKind::None && !(is_imm_date(self.start) && is_imm_date(self.end)) {
            return Err(crate::Error::Validation(format!(
                "an IMM schedule without stubs needs start ({}) and end ({}) on IMM dates",
                self.start, self.end
            )));
        }
        let grid = |start: Date, end: Date| BuilderInternal { start, end, ..self }.generate();
        if matches!(self.stub, StubKind::ShortFront | StubKind::LongFront) {
            let Some(last_roll) = imm_on_or_before(self.end)
                .ok()
                .filter(|&roll| roll > self.start)
            else {
                return Ok(vec![self.start, self.end]);
            };
            let mut dates = grid(self.start, last_roll)?;
            if self.end > last_roll {
                dates.push(self.end);
            }
            Ok(dates)
        } else {
            let Some(first_roll) = imm_on_or_after(self.start)
                .ok()
                .filter(|&roll| roll < self.end)
            else {
                return Ok(vec![self.start, self.end]);
            };
            let mut dates = grid(first_roll, self.end)?;
            if self.start < first_roll {
                dates.insert(0, self.start);
            }
            Ok(dates)
        }
    }

    /// Forward roll grid from `start`, closed by `end`; with `aligned`
    /// (`StubKind::None`), a grid that overshoots or runs out before `end` is
    /// an error. Alignment is against the actual roll grid, including EOM; a
    /// contractual maturity between grid points needs an explicit stub.
    fn gen_forward(self, aligned: bool) -> crate::Result<Vec<Date>> {
        let mut buf: Buffer = Buffer::new();
        buf.push(self.start);
        let mut i = 1;
        loop {
            let dt = match self.rolled(self.start, i)? {
                Some(dt) if dt < self.end => dt,
                last => {
                    if aligned && last != Some(self.end) {
                        return Err(crate::InputError::NonIntegerScheduleTenor.into());
                    }
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
            let dt = match self.rolled(anchor, -i)? {
                Some(dt) if dt > self.start => dt,
                _ => {
                    push_if_new(&mut buf, self.start);
                    check_anchor_count(buf.len())?;
                    break;
                }
            };
            if dt < self.end {
                push_if_new(&mut buf, dt);
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
            let Some(dt) = self.rolled(self.end, -i)? else {
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
        // anchors[0] is the user-provided end date; the rest are already snapped.
        for (idx, &dt) in anchors.iter().enumerate().rev() {
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
            let Some(next) = self.rolled(anchor, i)? else {
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
            let next_after = self.rolled(anchor, next_roll_index(i)?)?;
            if next_after.is_none_or(|date| date > self.end) {
                push_if_new(&mut buf, self.end);
                check_anchor_count(buf.len())?;
                break;
            }
            if next < self.end {
                push_if_new(&mut buf, next);
                check_anchor_count(buf.len())?;
            }
            i = next_roll_index(i)?;
        }
        Ok(buf.into_vec())
    }
}
