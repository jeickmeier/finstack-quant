//! Roundtrip serialization tests for various types.
//!
//! These tests verify that types serialize and deserialize correctly,
//! and that the serialized form can be used to reconstruct working objects.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{
    BusinessDayConvention, DayCount, DayCountContextState, GridMode, Schedule, ScheduleBuilder,
    ScheduleSpec, StubKind, Tenor, TenorUnit,
};
use finstack_quant_core::explain::ExplainOpts;
use finstack_quant_core::{Error, InputError};
use time::{Date, Month};

#[test]
fn error_serialization_roundtrip() {
    let input = Error::Input(InputError::NotFound {
        id: "curve:USD-OIS".to_string(),
    });
    let currency_mismatch = Error::CurrencyMismatch {
        expected: Currency::USD,
        actual: Currency::EUR,
    };

    let json_input = serde_json::to_string(&input).unwrap();
    let json_currency = serde_json::to_string(&currency_mismatch).unwrap();

    let roundtrip_input: Error = serde_json::from_str(&json_input).unwrap();
    let roundtrip_currency: Error = serde_json::from_str(&json_currency).unwrap();

    assert!(matches!(
        roundtrip_input,
        Error::Input(InputError::NotFound { .. })
    ));
    if let Error::CurrencyMismatch { expected, actual } = roundtrip_currency {
        assert_eq!(expected, Currency::USD);
        assert_eq!(actual, Currency::EUR);
    } else {
        panic!("expected currency mismatch variant");
    }
}

#[test]
fn explain_opts_roundtrip() {
    let opts = ExplainOpts {
        enabled: true,
        max_entries: Some(128),
    };
    let json = serde_json::to_string(&opts).unwrap();
    let restored: ExplainOpts = serde_json::from_str(&json).unwrap();
    assert!(restored.enabled);
    assert_eq!(restored.max_entries, Some(128));
}

#[test]
fn daycount_ctx_state_roundtrip() {
    let state = DayCountContextState {
        calendar_id: Some("target2".to_string()),
        frequency: Some(Tenor::quarterly()),
        bus_basis: Some(260),
        coupon_period: None,
        end_is_termination_date: Some(false),
    };
    let ctx = state.to_ctx().expect("calendar state hydrates");
    let start = Date::from_calendar_date(2025, Month::January, 2).unwrap();
    let end = Date::from_calendar_date(2025, Month::February, 2).unwrap();
    let yf = DayCount::Bus252.year_fraction(start, end, ctx).unwrap();
    assert!(yf > 0.0);

    let roundtrip_state = DayCountContextState::try_from(ctx).expect("snapshot context");
    assert_eq!(roundtrip_state.calendar_id.as_deref(), Some("target2"));
    assert_eq!(roundtrip_state.frequency, Some(Tenor::quarterly()));
    assert_eq!(roundtrip_state.bus_basis, Some(260));
}

#[test]
fn schedule_spec_builds_expected_dates() {
    let start = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let end = Date::from_calendar_date(2025, Month::July, 15).unwrap();
    let spec = ScheduleSpec {
        start,
        end,
        frequency: Tenor::new(1, TenorUnit::Months).expect("valid tenor fixture"),
        stub: StubKind::None,
        grid: GridMode::Natural,
        cds_imm_mode: false,
        business_day_convention: BusinessDayConvention::Following,
        calendar_id: Some("target2".to_string()),
        payment_lag_days: 0,
        fixing_lag_business_days: None,
    };

    let json = serde_json::to_string(&spec).unwrap();
    assert!(!json.contains("error_policy"));
    assert!(!json.contains("\"graceful\""));
    assert!(!json.contains("\"allow_missing_calendar\""));
    let restored: ScheduleSpec = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, spec);
    let schedule = ScheduleBuilder::from(restored).build().unwrap();
    assert_eq!(schedule.get_dates().len(), 7);

    // Cross-check with builder directly
    let builder_schedule = Schedule::builder(start, end)
        .unwrap()
        .frequency(Tenor::new(1, TenorUnit::Months).expect("valid tenor fixture"))
        .stub_rule(StubKind::None)
        .grid(GridMode::Natural)
        .adjust_with(BusinessDayConvention::Following, "target2")
        .build()
        .unwrap();

    assert_eq!(schedule.get_dates(), builder_schedule.get_dates());
}

/// A canonical spec body without its closing brace, for field tests.
const SPEC_BASE: &str = r#"{
        "start":"2025-01-15",
        "end":"2025-09-30",
        "frequency":{"count":3,"unit":"months"},
        "stub":"short_back",
        "grid":"natural",
        "cds_imm_mode":false,
        "business_day_convention":"unadjusted",
        "calendar_id":"weekends_only""#;

#[test]
fn schedule_spec_rejects_retired_policy_booleans() {
    for retired in [
        "\"allow_missing_calendar\":true",
        "\"end_of_month\":false",
        "\"imm_mode\":false",
    ] {
        assert!(serde_json::from_str::<ScheduleSpec>(&format!("{SPEC_BASE},{retired}}}")).is_err());
    }
}

#[test]
fn schedule_spec_requires_every_financial_choice() {
    // Record CORE-CONVENTIONS-008 #2: no frequency, stub, grid or payment
    // convention default; the operation defaults (lags) may be omitted.
    let canonical: ScheduleSpec = serde_json::from_str(&format!("{SPEC_BASE}}}")).unwrap();
    assert_eq!(
        (
            canonical.payment_lag_days,
            canonical.fixing_lag_business_days
        ),
        (0, None)
    );
    for field in [
        "frequency",
        "stub",
        "grid",
        "cds_imm_mode",
        "business_day_convention",
    ] {
        let mut value: serde_json::Value = serde_json::from_str(&format!("{SPEC_BASE}}}")).unwrap();
        value.as_object_mut().unwrap().remove(field);
        assert!(
            serde_json::from_value::<ScheduleSpec>(value).is_err(),
            "{field}"
        );
    }
    // A convention other than Unadjusted needs a calendar.
    let mut value: serde_json::Value = serde_json::from_str(&format!("{SPEC_BASE}}}")).unwrap();
    value["business_day_convention"] = "following".into();
    value["calendar_id"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<ScheduleSpec>(value).is_err());
    // Equal endpoints are not a schedule.
    let mut value: serde_json::Value = serde_json::from_str(&format!("{SPEC_BASE}}}")).unwrap();
    value["end"] = "2025-01-15".into();
    assert!(serde_json::from_value::<ScheduleSpec>(value).is_err());
}

#[test]
// schema-rejection-test: ScheduleSpec `payment_lag_business_days` (now `payment_lag_days`)
fn schedule_spec_rejects_retired_payment_lag_business_days() {
    let canonical: ScheduleSpec =
        serde_json::from_str(&format!("{SPEC_BASE},\"payment_lag_days\":2}}")).unwrap();
    assert_eq!(canonical.payment_lag_days, 2);
    let err = serde_json::from_str::<ScheduleSpec>(&format!(
        "{SPEC_BASE},\"payment_lag_business_days\":2}}"
    ))
    .unwrap_err();
    assert!(
        err.to_string().contains("payment_lag_business_days"),
        "{err}"
    );
}

#[test]
fn schedule_spec_rejects_cds_imm_on_a_non_natural_grid() {
    let mut value: serde_json::Value = serde_json::from_str(&format!("{SPEC_BASE}}}")).unwrap();
    value["grid"] = "quarterly_imm".into();
    value["cds_imm_mode"] = true.into();
    assert!(serde_json::from_value::<ScheduleSpec>(value).is_err());

    let spec = ScheduleSpec {
        start: Date::from_calendar_date(2025, Month::March, 19).unwrap(),
        end: Date::from_calendar_date(2025, Month::September, 30).unwrap(),
        frequency: Tenor::quarterly(),
        stub: StubKind::ShortBack,
        grid: GridMode::QuarterlyImm,
        cds_imm_mode: true,
        business_day_convention: BusinessDayConvention::Unadjusted,
        calendar_id: None,
        payment_lag_days: 0,
        fixing_lag_business_days: None,
    };
    assert!(ScheduleBuilder::from(spec).build().is_err());
}

/// `ScheduleBuilder::to_spec` captures exactly what `build` uses, including
/// `cds_imm`'s quarterly/ShortBack settings and a later override.
#[test]
fn builder_to_spec_matches_build_semantics() {
    let start = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let end = Date::from_calendar_date(2026, Month::January, 15).unwrap();
    let choices = |builder: ScheduleBuilder| builder.grid(GridMode::Natural).unadjusted();

    // Bare CDS IMM: the spec carries quarterly/ShortBack.
    let builder = choices(Schedule::builder(start, end).unwrap().cds_imm());
    let spec = builder.to_spec().unwrap();
    assert_eq!(spec.frequency, Tenor::quarterly());
    assert_eq!(spec.stub, StubKind::ShortBack);
    assert_eq!(
        ScheduleBuilder::from(spec).build().unwrap().get_dates(),
        builder.build().unwrap().get_dates()
    );

    // CDS IMM then monthly: the later frequency wins on both paths.
    let builder = choices(
        Schedule::builder(start, end)
            .unwrap()
            .cds_imm()
            .frequency(Tenor::monthly()),
    );
    let spec = builder.to_spec().unwrap();
    assert_eq!(spec.frequency, Tenor::monthly());
    let built = ScheduleBuilder::from(spec)
        .build()
        .unwrap()
        .get_dates()
        .to_vec();
    assert_eq!(built, builder.build().unwrap().get_dates());
    assert!(built.len() > 6, "monthly CDS schedule, got {built:?}");

    // Standard IMM from an on-cycle start.
    let imm_start = Date::from_calendar_date(2025, Month::March, 19).unwrap();
    let builder = Schedule::builder(imm_start, end)
        .unwrap()
        .frequency(Tenor::quarterly())
        .stub_rule(StubKind::ShortBack)
        .grid(GridMode::QuarterlyImm)
        .unadjusted();
    let spec = builder.to_spec().unwrap();
    assert_eq!(spec.grid, GridMode::QuarterlyImm);
    assert_eq!(
        ScheduleBuilder::from(spec).build().unwrap().get_dates(),
        builder.build().unwrap().get_dates()
    );

    // An incomplete builder names what is missing.
    let error = Schedule::builder(start, end)
        .unwrap()
        .to_spec()
        .unwrap_err();
    for choice in ["frequency", "stub", "grid", "payment convention"] {
        assert!(error.to_string().contains(choice), "{error}");
    }
}

// Moved from phase2_strictness.rs: nested strictness.
mod period_strictness {
    use finstack_quant_core::dates::{Period, build_periods};

    #[test]
    fn period_rejects_unknown_fields() {
        let period = build_periods("2025Q1..Q1", None)
            .expect("valid period plan")
            .periods
            .into_iter()
            .next()
            .expect("one period");
        let mut value = serde_json::to_value(period).expect("period serializes");
        value
            .as_object_mut()
            .expect("period is an object")
            .insert("__nonexistent__".into(), serde_json::json!(true));

        let error = serde_json::from_value::<Period>(value).expect_err("unknown fields must fail");
        assert!(error.to_string().contains("unknown field"), "{error}");
    }
}
