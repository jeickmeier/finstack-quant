//! Context values used by day-count calculations.

use time::Date;

use crate::dates::{calendar_by_id_strict, HolidayCalendar, Tenor};

/// Optional context for day-count year-fraction calculations.
///
/// Certain conventions require additional information:
/// - `Bus/252` requires a holiday `calendar`.
/// - `Act/Act (ISMA)` requires the coupon `frequency`.
/// - `Act/365L` requires `frequency` and the enclosing `coupon_period`.
#[derive(Clone, Copy, Default)]
pub struct DayCountContext<'a> {
    /// Holiday calendar for business day conventions
    pub calendar: Option<&'a dyn HolidayCalendar>,
    /// Payment frequency (required for ACT/ACT ISMA and ACT/365L).
    pub frequency: Option<Tenor>,
    /// Business day convention (required for Bus/252)
    pub bus_basis: Option<u16>,
    /// Unadjusted regular reference coupon period for ACT/ACT ISMA, or the
    /// enclosing contractual coupon period for ACT/365L, as `(start, end)`.
    ///
    /// When set, the ISMA year fraction uses this explicit reference period
    /// instead of re-anchoring from the accrual start date. Required for
    /// correct accrued interest calculations on mid-coupon dates or
    /// irregular first/last coupons. Its endpoints must share the nominal
    /// month grid; adjusted payment dates are not ICMA reference boundaries.
    /// ACT/365L requires the actual enclosing coupon boundaries for both
    /// full and partial accrual, keeping its 365/366 denominator constant.
    pub coupon_period: Option<(Date, Date)>,
    /// Whether `end` is the instrument termination date.
    ///
    /// Required by 30E/360 ISDA for its end-of-February termination exception.
    pub end_is_termination_date: bool,
}

/// Reject coupon boundaries unless `start < end`.
fn validate_coupon_period(start: Date, end: Date) -> crate::Result<()> {
    if start >= end {
        return Err(crate::Error::Validation(format!(
            "coupon period start must be before end, got start={start} end={end}"
        )));
    }
    Ok(())
}

impl<'a> DayCountContext<'a> {
    /// Return a copy with validated coupon boundaries for ACT/ACT ICMA or ACT/365L.
    ///
    /// # Arguments
    ///
    /// * `start` - Inclusive start: the unadjusted regular reference start for
    ///   ICMA, or the actual contractual coupon start for ACT/365L.
    /// * `end` - Exclusive end: the unadjusted regular reference end for ICMA,
    ///   or the actual contractual coupon end for ACT/365L. Must be after
    ///   `start`; ICMA reference endpoints must share one nominal month grid.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` unless `start < end`.
    pub fn with_coupon_period(mut self, start: Date, end: Date) -> crate::Result<Self> {
        validate_coupon_period(start, end)?;
        self.coupon_period = Some((start, end));
        Ok(self)
    }
}

impl<'a> std::fmt::Debug for DayCountContext<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DayCountContext")
            .field("calendar", &self.calendar.map(|_| "HolidayCalendar"))
            .field("frequency", &self.frequency)
            .field("bus_basis", &self.bus_basis)
            .field("coupon_period", &self.coupon_period)
            .field("end_is_termination_date", &self.end_is_termination_date)
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
/// Serializable snapshot of [`DayCountContext`] state for persistence and interchange.
///
/// This struct captures the optional context parameters (calendar, frequency, business-day basis)
/// needed to reconstruct a [`DayCountContext`] at runtime using the built-in calendar lookup.
pub struct DayCountContextState {
    /// Optional calendar code (e.g. "target2").
    pub calendar_id: Option<String>,
    /// Optional coupon frequency, required for ACT/ACT ISMA and ACT/365L.
    pub frequency: Option<Tenor>,
    /// Optional custom business-day divisor (defaults to 252 when `None`).
    pub bus_basis: Option<u16>,
    /// Optional unadjusted regular ICMA reference period or ACT/365L enclosing
    /// coupon `(start, end)`, serialized as two ISO dates.
    ///
    /// Required for exact ICMA accrual on round-trip. `None` selects the
    /// frequency-only ICMA calculation path; ACT/365L requires these dates.
    #[serde(default, with = "crate::wire::optional_date_pair")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<(crate::wire::DateWire, crate::wire::DateWire)>")
    )]
    pub coupon_period: Option<(Date, Date)>,
    /// Whether the accrual end is the instrument termination date.
    pub end_is_termination_date: bool,
}

impl DayCountContextState {
    /// Build a validated state snapshot.
    ///
    /// This is the single validation point shared by every host binding: an
    /// inverted `coupon_period` is rejected here rather than at year-fraction
    /// time. The calendar id is *not* resolved (that happens in
    /// [`Self::to_ctx`]) so a snapshot can be persisted before the registry
    /// is consulted.
    ///
    /// # Arguments
    ///
    /// * `calendar_id` - Optional registered holiday-calendar id (for example
    ///   `"usny"`), required by `Bus/252`.
    /// * `frequency` - Coupon frequency required by ACT/ACT ICMA and ACT/365L.
    /// * `bus_basis` - Optional business-day divisor for `Bus/252`; `None`
    ///   selects 252.
    /// * `coupon_period` - Optional `(start, end)` unadjusted regular ACT/ACT ICMA
    ///   reference period or ACT/365L enclosing coupon; `start` must precede
    ///   `end`. ICMA validates the nominal month grid at calculation time.
    /// * `end_is_termination_date` - Whether the accrual end is the
    ///   instrument termination date (30E/360 ISDA February rule).
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when `coupon_period` is not strictly
    /// increasing.
    pub fn try_new(
        calendar_id: Option<String>,
        frequency: Option<Tenor>,
        bus_basis: Option<u16>,
        coupon_period: Option<(Date, Date)>,
        end_is_termination_date: bool,
    ) -> crate::Result<Self> {
        if let Some((start, end)) = coupon_period {
            validate_coupon_period(start, end)?;
        }
        Ok(Self {
            calendar_id,
            frequency,
            bus_basis,
            coupon_period,
            end_is_termination_date,
        })
    }

    /// Build a runtime [`DayCountContext`] using the built-in calendar lookup.
    ///
    /// # Errors
    ///
    /// Returns an error when `calendar_id` names an unknown calendar or when
    /// a deserialized `coupon_period` is inverted.
    pub fn to_ctx(&self) -> crate::Result<DayCountContext<'static>> {
        if let Some((start, end)) = self.coupon_period {
            validate_coupon_period(start, end)?;
        }
        let calendar = self
            .calendar_id
            .as_deref()
            .map(calendar_by_id_strict)
            .transpose()?;
        Ok(DayCountContext {
            calendar,
            frequency: self.frequency,
            bus_basis: self.bus_basis,
            coupon_period: self.coupon_period,
            end_is_termination_date: self.end_is_termination_date,
        })
    }
}

impl<'a> From<DayCountContext<'a>> for DayCountContextState {
    fn from(value: DayCountContext<'a>) -> Self {
        let calendar_id = value
            .calendar
            .and_then(|cal| cal.metadata().map(|meta| meta.id.to_string()));
        Self {
            calendar_id,
            frequency: value.frequency,
            bus_basis: value.bus_basis,
            coupon_period: value.coupon_period,
            end_is_termination_date: value.end_is_termination_date,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

    #[test]
    fn try_new_rejects_inverted_coupon_period() {
        let err = DayCountContextState::try_new(
            None,
            None,
            None,
            Some((date!(2025 - 07 - 01), date!(2025 - 01 - 01))),
            false,
        )
        .expect_err("inverted");
        assert!(err
            .to_string()
            .contains("coupon period start must be before end"));
        assert!(err.to_string().contains("2025-07-01"));
        assert!(DayCountContextState::try_new(
            None,
            None,
            None,
            Some((date!(2025 - 01 - 01), date!(2025 - 07 - 01))),
            false,
        )
        .is_ok());
    }

    #[test]
    fn to_ctx_rejects_inverted_deserialized_coupon_period() {
        let state = DayCountContextState {
            calendar_id: None,
            frequency: None,
            bus_basis: None,
            coupon_period: Some((date!(2025 - 07 - 01), date!(2025 - 01 - 01))),
            end_is_termination_date: false,
        };
        assert!(state.to_ctx().is_err());
    }
}
