//! The structured-credit month count that predates CORE-CONVENTIONS-007 #4.
//!
//! `finstack_quant_core::dates::DateExt::months_until` now returns completed
//! months by original-anchor calendar arithmetic and rejects reversed dates.
//! Structured-credit sites use months for distinct meanings (seasoning,
//! simulation indices, remaining term, lock-out windows). Until each site's
//! meaning is stated and it moves to the right measure (reconciliation slice
//! E2b, owner resolution O13), those sites keep this named, site-local count
//! instead of silently adopting completed months.

use finstack_quant_core::dates::Date;

/// Clamped calendar-month count kept for structured-credit sites pending E2b.
pub(crate) trait ClampedMonths {
    /// `(Y₂-Y₁)·12 + (M₂-M₁)`, minus one when the start day exceeds the end
    /// day unless both are month-ends; zero when `other` precedes `self`.
    fn clamped_months_until(self, other: Date) -> u32;
}

impl ClampedMonths for Date {
    fn clamped_months_until(self, other: Date) -> u32 {
        let mut months =
            (other.year() - self.year()) * 12 + (other.month() as i32 - self.month() as i32);
        if self.day() > other.day() {
            let self_eom = self.day() == self.month().length(self.year());
            let other_eom = other.day() == other.month().length(other.year());
            if !(self_eom && other_eom) {
                months -= 1;
            }
        }
        u32::try_from(months).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::ClampedMonths;
    use time::macros::date;

    #[test]
    fn keeps_the_pre_007_count() {
        assert_eq!(
            date!(2020 - 01 - 15).clamped_months_until(date!(2022 - 03 - 10)),
            25
        );
        assert_eq!(
            date!(2025 - 01 - 30).clamped_months_until(date!(2025 - 02 - 28)),
            0
        );
        assert_eq!(
            date!(2025 - 01 - 31).clamped_months_until(date!(2025 - 02 - 28)),
            1
        );
        assert_eq!(
            date!(2025 - 03 - 01).clamped_months_until(date!(2025 - 01 - 01)),
            0
        );
    }
}
