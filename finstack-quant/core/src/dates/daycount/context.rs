//! Context values used by day-count calculations.

use time::Date;

use crate::dates::{HolidayCalendar, Tenor, calendar_by_id_strict, canonical_calendar_id};

/// Optional context for day-count year-fraction calculations.
///
/// Certain conventions require additional information:
/// - `Bus/252` requires a holiday `calendar`.
/// - `Act/Act (ISMA)` requires the coupon `frequency`.
/// - `Act/365L` requires the enclosing `coupon_period`.
#[derive(Clone, Copy, Default)]
pub struct DayCountContext<'a> {
    /// Holiday calendar for business day conventions
    pub calendar: Option<&'a dyn HolidayCalendar>,
    /// Payment frequency (required for ACT/ACT ISMA).
    pub frequency: Option<Tenor>,
    /// Business-day divisor for Bus/252 (for example 252); `None` uses 252.
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
    /// Whether the chronologically later accrual endpoint is the
    /// contractual termination date; `None` means unspecified.
    ///
    /// 30E/360 ISDA requires an explicit `Some(true)` or `Some(false)` for
    /// its end-of-February termination exception (including for equal
    /// dates); other conventions ignore it. The signed API applies it to the
    /// later endpoint whatever the argument order.
    pub end_is_termination_date: Option<bool>,
}

/// Reject a zero business-day basis and coupon boundaries unless
/// `start < end`; shared by runtime contexts and state snapshots.
fn validate_fields(
    bus_basis: Option<u16>,
    coupon_period: Option<(Date, Date)>,
) -> crate::Result<()> {
    if bus_basis == Some(0) {
        return Err(crate::InputError::InvalidBusBasis { basis: 0 }.into());
    }
    if let Some((start, end)) = coupon_period
        && start >= end
    {
        return Err(crate::Error::Validation(format!(
            "coupon period start must be before end, got start={start} end={end}"
        )));
    }
    Ok(())
}

impl DayCountContext<'_> {
    /// Structural validation of every supplied field, independent of the
    /// convention; see [`super::DayCount::validate_context`].
    pub(super) fn validate_structure(&self) -> crate::Result<()> {
        validate_fields(self.bus_basis, self.coupon_period)
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

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields, try_from = "DayCountContextStateFields")]
#[cfg_attr(feature = "json-schema", schemars(!try_from, deny_unknown_fields))]
/// Serializable snapshot of [`DayCountContext`] state for persistence and interchange.
///
/// This struct captures the optional context parameters (calendar, frequency, business-day basis)
/// needed to reconstruct a [`DayCountContext`] at runtime using the built-in calendar lookup.
/// `Default` is the empty context (no calendar, frequency, basis, coupon
/// period or termination status), the snapshot of
/// [`DayCountContext::default`].
pub struct DayCountContextState {
    /// Optional calendar code (e.g. "target2").
    pub calendar_id: Option<String>,
    /// Optional coupon frequency, required for ACT/ACT ISMA.
    pub frequency: Option<Tenor>,
    /// Optional custom business-day divisor (defaults to 252 when `None`).
    pub bus_basis: Option<u16>,
    /// Optional unadjusted regular ICMA reference period or ACT/365L enclosing
    /// coupon `(start, end)`, serialized as two ISO dates.
    ///
    /// Required for exact ICMA accrual on round-trip. `None` selects the
    /// frequency-only ICMA calculation path; ACT/365L requires these dates.
    #[serde(default)]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<(crate::wire::DateWire, crate::wire::DateWire)>")
    )]
    pub coupon_period: Option<(Date, Date)>,
    /// Whether the later accrual endpoint is the contractual termination
    /// date; `None` (omitted or `null` on the wire) means unspecified, which
    /// 30E/360 ISDA rejects.
    #[serde(default)]
    pub end_is_termination_date: Option<bool>,
}

/// Wire fields of [`DayCountContextState`]; deserialization validates them
/// through [`DayCountContextState::try_new`].
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DayCountContextStateFields {
    calendar_id: Option<String>,
    frequency: Option<Tenor>,
    bus_basis: Option<u16>,
    #[serde(default)]
    coupon_period: Option<(Date, Date)>,
    #[serde(default)]
    end_is_termination_date: Option<bool>,
}

impl TryFrom<DayCountContextStateFields> for DayCountContextState {
    type Error = crate::Error;

    fn try_from(fields: DayCountContextStateFields) -> crate::Result<Self> {
        Self::try_new(
            fields.calendar_id,
            fields.frequency,
            fields.bus_basis,
            fields.coupon_period,
            fields.end_is_termination_date,
        )
    }
}

impl DayCountContextState {
    /// Build a validated state snapshot.
    ///
    /// This is the single validation point shared by every host binding and
    /// by deserialization: an inverted `coupon_period` is rejected here rather
    /// than at year-fraction time. The calendar id is *not* resolved (that happens in
    /// [`Self::to_ctx`]) so a snapshot can be persisted before the registry
    /// is consulted.
    ///
    /// # Arguments
    ///
    /// * `calendar_id` - Optional registered holiday-calendar id (for example
    ///   `"usny"`), required by `Bus/252`.
    /// * `frequency` - Coupon frequency required by ACT/ACT ICMA.
    /// * `bus_basis` - Optional business-day divisor for `Bus/252`, a positive
    ///   count of business days per year; `None` selects 252.
    /// * `coupon_period` - Optional `(start, end)` unadjusted regular ACT/ACT ICMA
    ///   reference period or ACT/365L enclosing coupon; `start` must precede
    ///   `end`. ICMA validates the nominal month grid at calculation time.
    /// * `end_is_termination_date` - Whether the later accrual endpoint is the
    ///   contractual termination date (30E/360 ISDA February rule); `None`
    ///   means unspecified, which 30E/360 ISDA rejects at calculation time.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when `coupon_period` is not strictly
    /// increasing, and `InputError::InvalidBusBasis` when `bus_basis` is zero.
    pub fn try_new(
        calendar_id: Option<String>,
        frequency: Option<Tenor>,
        bus_basis: Option<u16>,
        coupon_period: Option<(Date, Date)>,
        end_is_termination_date: Option<bool>,
    ) -> crate::Result<Self> {
        validate_fields(bus_basis, coupon_period)?;
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
    /// The fields are revalidated first, because public fields can be set
    /// after [`Self::try_new`].
    ///
    /// # Errors
    ///
    /// Returns the [`Self::try_new`] errors for invalid fields, and an error
    /// when `calendar_id` names an unknown calendar.
    pub fn to_ctx(&self) -> crate::Result<DayCountContext<'static>> {
        validate_fields(self.bus_basis, self.coupon_period)?;
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

impl<'a> TryFrom<DayCountContext<'a>> for DayCountContextState {
    type Error = crate::Error;

    fn try_from(value: DayCountContext<'a>) -> crate::Result<Self> {
        let calendar_id = value
            .calendar
            .map(|calendar| {
                let id = calendar.calendar_id().ok_or_else(|| {
                    crate::Error::Validation(
                        "day-count calendar has no registered identifier and cannot be restored"
                            .into(),
                    )
                })?;
                canonical_calendar_id(&id)
            })
            .transpose()?;
        Ok(Self {
            calendar_id,
            frequency: value.frequency,
            bus_basis: value.bus_basis,
            coupon_period: value.coupon_period,
            end_is_termination_date: value.end_is_termination_date,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dates::{HolidayCalendar, calendar_by_id_strict};
    use time::macros::date;

    #[test]
    fn default_state_is_the_default_context_snapshot() {
        assert_eq!(
            DayCountContextState::default(),
            DayCountContextState::try_from(DayCountContext::default()).expect("default snapshot")
        );
    }

    #[test]
    fn composite_calendar_id_round_trips_in_context_state() {
        let calendar = calendar_by_id_strict("usny+target2").expect("registered union");
        let context = DayCountContext {
            calendar: Some(calendar),
            ..DayCountContext::default()
        };
        let state = DayCountContextState::try_from(context).expect("snapshot composite id");
        assert_eq!(state.calendar_id.as_deref(), Some("target2+usny"));
        let restored = state.to_ctx().expect("restore composite");
        let restored_state = DayCountContextState::try_from(restored).expect("resnapshot");
        assert_eq!(restored_state.calendar_id.as_deref(), Some("target2+usny"));
    }

    #[test]
    fn context_snapshot_rejects_unregistered_custom_calendar() {
        struct CustomCalendar;
        impl HolidayCalendar for CustomCalendar {
            fn is_holiday(&self, date: Date) -> crate::Result<bool> {
                Ok(date.month() == time::Month::June && date.day() == 1)
            }

            fn is_business_day(&self, date: Date) -> crate::Result<bool> {
                Ok(!crate::dates::DateExt::is_weekend(date)
                    && !self.is_holiday(date).expect("calendar lookup"))
            }
        }

        let calendar = CustomCalendar;
        let context = DayCountContext {
            calendar: Some(&calendar),
            ..DayCountContext::default()
        };
        let error = DayCountContextState::try_from(context)
            .expect_err("custom calendar cannot be reconstructed from registry");
        assert!(error.to_string().contains("cannot be restored"));
    }

    #[test]
    fn try_new_rejects_inverted_coupon_period() {
        let err = DayCountContextState::try_new(
            None,
            None,
            None,
            Some((date!(2025 - 07 - 01), date!(2025 - 01 - 01))),
            None,
        )
        .expect_err("inverted");
        assert!(
            err.to_string()
                .contains("coupon period start must be before end")
        );
        assert!(err.to_string().contains("2025-07-01"));
        assert!(
            DayCountContextState::try_new(
                None,
                None,
                None,
                Some((date!(2025 - 01 - 01), date!(2025 - 07 - 01))),
                None,
            )
            .is_ok()
        );
    }

    #[test]
    fn try_new_rejects_a_zero_bus_basis() {
        let err = DayCountContextState::try_new(None, None, Some(0), None, None)
            .expect_err("a zero Bus/252 divisor must be rejected at construction");
        assert!(err.to_string().contains("Invalid Bus/252 basis"), "{err}");
        assert!(DayCountContextState::try_new(None, None, Some(252), None, None).is_ok());
    }

    #[test]
    fn deserialization_validates_through_try_new() {
        let valid = DayCountContextState::try_new(
            None,
            None,
            None,
            Some((date!(2025 - 01 - 01), date!(2025 - 07 - 01))),
            None,
        )
        .expect("valid state");
        let mut json = serde_json::to_value(&valid).expect("serializable");
        assert_eq!(
            serde_json::from_value::<DayCountContextState>(json.clone()).expect("round-trips"),
            valid
        );
        let period = json["coupon_period"].as_array_mut().expect("two dates");
        period.swap(0, 1);
        let err = serde_json::from_value::<DayCountContextState>(json.clone())
            .expect_err("inverted coupon period is rejected at load time");
        assert!(
            err.to_string()
                .contains("coupon period start must be before end")
        );
        json["coupon_period"] = serde_json::Value::Null;
        json["bus_basis"] = serde_json::json!(0);
        assert!(serde_json::from_value::<DayCountContextState>(json).is_err());
    }
}
