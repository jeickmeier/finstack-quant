//! Contract specifications and general parameter types.

use serde::{Deserialize, Serialize};

/// Contract size information for derivatives
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ContractSpec {
    /// Number of units per contract
    pub contract_size: f64,
    /// Optional multiplier for pricing
    pub multiplier: Option<f64>,
}

impl ContractSpec {
    /// Create a new contract specification
    pub fn new(contract_size: f64) -> Self {
        Self {
            contract_size,
            multiplier: None,
        }
    }

    /// Standard single-unit contract
    pub fn unit() -> Self {
        Self::new(1.0)
    }

    /// Set the contract multiplier
    pub fn with_multiplier(mut self, multiplier: f64) -> Self {
        self.multiplier = Some(multiplier);
        self
    }
}

impl Default for ContractSpec {
    fn default() -> Self {
        Self::unit()
    }
}

/// Schedule specification for payment periods
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ScheduleSpec {
    /// Start date for the schedule
    #[schemars(with = "String")]
    pub start: Date,
    /// End date for the schedule
    #[schemars(with = "String")]
    pub end: Date,
    /// Payment frequency
    pub frequency: Tenor,
    /// Stub period handling
    #[serde(default = "crate::serde_defaults::stub_short_front")]
    pub stub: StubKind,
    /// Business day convention
    #[serde(default = "crate::serde_defaults::bdc_modified_following")]
    pub bdc: BusinessDayConvention,
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
            bdc: BusinessDayConvention::Following,
            calendar_id: None,
        }
    }

    /// Set business day convention
    pub fn with_bdc(mut self, bdc: BusinessDayConvention) -> Self {
        self.bdc = bdc;
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
use finstack_core::dates::{BusinessDayConvention, Date, StubKind, Tenor};

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

    fn sample_dates() -> (Date, Date) {
        (date!(2025 - 01 - 02), date!(2026 - 01 - 02))
    }

    #[test]
    fn contract_spec_builders_set_expected_fields() {
        let plain = ContractSpec::new(1000.0);
        let unit = ContractSpec::unit();
        let custom = ContractSpec::new(50.0).with_multiplier(2.5);

        assert_eq!(plain.contract_size, 1000.0);
        assert_eq!(plain.multiplier, None);
        assert_eq!(unit.contract_size, 1.0);
        assert_eq!(unit.multiplier, None);
        assert_eq!(custom.contract_size, 50.0);
        assert_eq!(custom.multiplier, Some(2.5));
        assert_eq!(ContractSpec::default().contract_size, 1.0);
    }

    #[test]
    fn schedule_spec_builders_apply_overrides() {
        let (start, end) = sample_dates();
        let schedule = ScheduleSpec::new(start, end, Tenor::quarterly())
            .with_bdc(BusinessDayConvention::ModifiedFollowing)
            .with_stub(StubKind::ShortBack)
            .with_calendar("nyse");

        assert_eq!(schedule.start, start);
        assert_eq!(schedule.end, end);
        assert_eq!(schedule.frequency, Tenor::quarterly());
        assert_eq!(schedule.stub, StubKind::ShortBack);
        assert_eq!(schedule.bdc, BusinessDayConvention::ModifiedFollowing);
        assert_eq!(schedule.calendar_id, Some("nyse"));
    }

    #[test]
    fn schedule_spec_new_uses_local_defaults() {
        let (start, end) = sample_dates();
        let schedule = ScheduleSpec::new(start, end, Tenor::monthly());

        assert_eq!(schedule.stub, StubKind::ShortFront);
        assert_eq!(schedule.bdc, BusinessDayConvention::Following);
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
            assert_eq!(schedule.bdc, BusinessDayConvention::ModifiedFollowing);
            assert_eq!(schedule.frequency, Tenor::quarterly());
        }
    }
}
