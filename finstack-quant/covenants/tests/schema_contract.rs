//! Generated covenant schemas describe exactly what serde writes.
//!
//! [`ThresholdSchedule`] has a hand-written `Deserialize` (constructor
//! validation over `[date, threshold]` pairs); an engine carrying one is
//! serialized, validated against the registered schema and read back.

use finstack_quant_core::dates::{Date, Tenor};
use finstack_quant_covenants::schema::ARTIFACTS;
use finstack_quant_covenants::{
    Covenant, CovenantEngine, CovenantMetricId, CovenantSpec, CovenantType, ThresholdSchedule,
};
use serde_json::{json, Value};
use time::Month;

fn schema(type_name: &str) -> Value {
    ARTIFACTS
        .iter()
        .find(|artifact| artifact.type_name() == type_name)
        .unwrap_or_else(|| panic!("{type_name} is not registered"))
        .generate()
        .expect("schema renders")
}

fn validator(type_name: &str) -> jsonschema::Validator {
    jsonschema::options()
        .should_validate_formats(true)
        .build(&schema(type_name))
        .expect("schema compiles")
}

fn date(month: Month, day: u8) -> Date {
    Date::from_calendar_date(2025, month, day).expect("valid date")
}

#[test]
fn engine_with_threshold_schedule_validates_and_round_trips() {
    let schedule = ThresholdSchedule::new(vec![
        (date(Month::January, 1), 5.0),
        (date(Month::July, 1), 4.5),
    ])
    .expect("valid schedule");
    let mut engine = CovenantEngine::new();
    engine.add_spec(
        CovenantSpec::with_metric(
            Covenant::new(
                CovenantType::MaxDebtToEbitda { threshold: 5.0 },
                Tenor::quarterly(),
                "max_leverage",
            ),
            CovenantMetricId::from("debt_to_ebitda"),
        )
        .with_threshold_schedule(schedule),
    );

    let wire = serde_json::to_value(&engine).expect("serialize");
    let errors: Vec<String> = validator("CovenantEngine")
        .iter_errors(&wire)
        .map(|error| error.to_string())
        .collect();
    assert!(errors.is_empty(), "{errors:?}\n{wire}");
    let back: CovenantEngine = serde_json::from_value(wire.clone()).expect("deserialize");
    assert_eq!(serde_json::to_value(&back).expect("reserialize"), wire);
}

#[test]
fn threshold_schedule_schema_rejects_object_entries() {
    let schedule =
        ThresholdSchedule::new(vec![(date(Month::January, 1), 5.0)]).expect("valid schedule");
    let mut engine = CovenantEngine::new();
    engine.add_spec(
        CovenantSpec::with_metric(
            Covenant::new(
                CovenantType::MaxDebtToEbitda { threshold: 5.0 },
                Tenor::quarterly(),
                "max_leverage",
            ),
            CovenantMetricId::from("debt_to_ebitda"),
        )
        .with_threshold_schedule(schedule),
    );
    let mut wire = serde_json::to_value(&engine).expect("serialize");
    assert_eq!(
        wire["specs"][0]["threshold_schedule"],
        json!([["2025-01-01", 5.0]])
    );
    wire["specs"][0]["threshold_schedule"] = json!([{ "date": "2025-01-01", "threshold": 5.0 }]);
    assert!(!validator("CovenantEngine").is_valid(&wire));
    assert!(serde_json::from_value::<CovenantEngine>(wire).is_err());
}
