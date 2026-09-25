//! Schedule specification parameter type.

use serde::{Deserialize, Serialize};

/// Schedule specification for payment periods
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct ScheduleSpec {
    /// Start date for the schedule
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub start: Date,
    /// End date for the schedule
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub end: Date,
    /// Payment frequency
    pub frequency: Tenor,
    /// Stub period handling
    #[serde(default = "crate::serde_defaults::stub_short_front")]
    pub stub: StubKind,
    /// Business day convention
    #[serde(default = "crate::serde_defaults::bdc_modified_following")]
    pub business_day_convention: BusinessDayConvention,
    /// Optional calendar for adjustments
    pub calendar_id: Option<&'static str>,
}

impl ScheduleSpec {
    /// Create a new schedule specification
    pub fn new(start: Date, end: Date, frequency: Tenor) -> Self {
        Self {
            start,
            end,
            frequency,
            stub: StubKind::ShortFront,
            business_day_convention: BusinessDayConvention::Following,
            calendar_id: None,
        }
    }

    /// Set business day convention
    pub fn with_business_day_convention(
        mut self,
        business_day_convention: BusinessDayConvention,
    ) -> Self {
        self.business_day_convention = business_day_convention;
        self
    }

    /// Set stub handling
    pub fn with_stub(mut self, stub: StubKind) -> Self {
        self.stub = stub;
        self
    }

    /// Set calendar for adjustments
    pub fn with_calendar(mut self, calendar_id: &'static str) -> Self {
        self.calendar_id = Some(calendar_id);
        self
    }
}

// Need to import these for the ScheduleSpec
use finstack_quant_core::dates::{BusinessDayConvention, Date, StubKind, Tenor};

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

    fn sample_dates() -> (Date, Date) {
        (date!(2025 - 01 - 02), date!(2026 - 01 - 02))
    }

    #[test]
    fn schedule_spec_builders_apply_overrides() {
        let (start, end) = sample_dates();
        let schedule = ScheduleSpec::new(start, end, Tenor::quarterly())
            .with_business_day_convention(BusinessDayConvention::ModifiedFollowing)
            .with_stub(StubKind::ShortBack)
            .with_calendar("nyse");

        assert_eq!(schedule.start, start);
        assert_eq!(schedule.end, end);
        assert_eq!(schedule.frequency, Tenor::quarterly());
        assert_eq!(schedule.stub, StubKind::ShortBack);
        assert_eq!(
            schedule.business_day_convention,
            BusinessDayConvention::ModifiedFollowing
        );
        assert_eq!(schedule.calendar_id, Some("nyse"));
    }

    #[test]
    fn schedule_spec_new_uses_local_defaults() {
        let (start, end) = sample_dates();
        let schedule = ScheduleSpec::new(start, end, Tenor::monthly());

        assert_eq!(schedule.stub, StubKind::ShortFront);
        assert_eq!(
            schedule.business_day_convention,
            BusinessDayConvention::Following
        );
        assert_eq!(schedule.calendar_id, None);
    }

    #[test]
    fn schedule_spec_serde_defaults_match_annotations() {
        let json = r#"{
            "start":"2025-01-02",
            "end":"2026-01-02",
            "frequency":{"count":3,"unit":"months"},
            "calendar_id":null
        }"#;
        let schedule = serde_json::from_str::<ScheduleSpec>(json);
        assert!(schedule.is_ok(), "schedule should deserialize");
        if let Ok(schedule) = schedule {
            assert_eq!(schedule.stub, StubKind::ShortFront);
            assert_eq!(
                schedule.business_day_convention,
                BusinessDayConvention::ModifiedFollowing
            );
            assert_eq!(schedule.frequency, Tenor::quarterly());
        }
    }
}
