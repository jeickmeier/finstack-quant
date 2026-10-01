//! Embedded JSON Schema resources owned by the cashflows crate.

#[cfg(feature = "jsonschema-validate")]
use finstack_quant_core::Error;
use finstack_quant_core::Result;
use serde_json::Value;

/// Stable base URI for cashflow component schemas.
#[cfg(feature = "jsonschema-validate")]
pub const CASHFLOW_SCHEMA_BASE: &str = "https://finstack_quant.dev/schemas/cashflow/1/";

/// Typed cashflow definitions eligible for assertion-checked externalization.
pub const CASHFLOW_SCHEMA_DEFINITIONS: &[finstack_quant_core::schema::ExternalSchemaDefinition] = &[
    finstack_quant_core::schema::ExternalSchemaDefinition::new::<crate::builder::DefaultModelSpec>(
        "DefaultModelSpec",
        "https://finstack_quant.dev/schemas/cashflow/1/default_model_spec.schema.json",
    ),
    finstack_quant_core::schema::ExternalSchemaDefinition::new::<crate::builder::FeeSpec>(
        "FeeSpec",
        "https://finstack_quant.dev/schemas/cashflow/1/fee_specs.schema.json",
    ),
    finstack_quant_core::schema::ExternalSchemaDefinition::new::<crate::builder::FixedCouponSpec>(
        "FixedCouponSpec",
        "https://finstack_quant.dev/schemas/cashflow/1/coupon_specs.schema.json",
    ),
    finstack_quant_core::schema::ExternalSchemaDefinition::new::<crate::builder::PrepaymentModelSpec>(
        "PrepaymentModelSpec",
        "https://finstack_quant.dev/schemas/cashflow/1/prepayment_model_spec.schema.json",
    ),
    finstack_quant_core::schema::ExternalSchemaDefinition::new::<crate::builder::RecoveryModelSpec>(
        "RecoveryModelSpec",
        "https://finstack_quant.dev/schemas/cashflow/1/recovery_model_spec.schema.json",
    ),
    finstack_quant_core::schema::ExternalSchemaDefinition::new::<crate::builder::ScheduleParams>(
        "ScheduleParams",
        "https://finstack_quant.dev/schemas/cashflow/1/schedule_params.schema.json",
    ),
];

/// Package a derived cashflow schema using canonical shared definitions.
///
/// This pass changes only reference placement: shared definitions are
/// replaced by their equivalent published `$id`, then newly unreachable local
/// definitions are removed.
///
/// # Arguments
///
/// * `schema` - Complete schema generated from a cashflow serde type.
#[doc(hidden)]
pub fn package_cashflow_schema(schema: &mut Value) -> Result<()> {
    finstack_quant_core::schema::externalize_schema_definitions(
        schema,
        finstack_quant_core::schema::COMMON_SCHEMA_DEFINITIONS,
    )
}

#[cfg(feature = "jsonschema-validate")]
const SCHEMAS: [(&str, &str); 14] = [
    (
        "accrual_config.schema.json",
        include_str!("../schemas/cashflow/1/accrual_config.schema.json"),
    ),
    (
        "amortization_spec.schema.json",
        include_str!("../schemas/cashflow/1/amortization_spec.schema.json"),
    ),
    (
        "cashflow_schedule.schema.json",
        include_str!("../schemas/cashflow/1/cashflow_schedule.schema.json"),
    ),
    (
        "cashflow_schedule_build_spec.schema.json",
        include_str!("../schemas/cashflow/1/cashflow_schedule_build_spec.schema.json"),
    ),
    (
        "coupon_specs.schema.json",
        include_str!("../schemas/cashflow/1/coupon_specs.schema.json"),
    ),
    (
        "default_model_spec.schema.json",
        include_str!("../schemas/cashflow/1/default_model_spec.schema.json"),
    ),
    (
        "fee_specs.schema.json",
        include_str!("../schemas/cashflow/1/fee_specs.schema.json"),
    ),
    (
        "floating_coupon_spec.schema.json",
        include_str!("../schemas/cashflow/1/floating_coupon_spec.schema.json"),
    ),
    (
        "period_aggregation.schema.json",
        include_str!("../schemas/cashflow/1/period_aggregation.schema.json"),
    ),
    (
        "prepayment_model_spec.schema.json",
        include_str!("../schemas/cashflow/1/prepayment_model_spec.schema.json"),
    ),
    (
        "recovery_model_spec.schema.json",
        include_str!("../schemas/cashflow/1/recovery_model_spec.schema.json"),
    ),
    (
        "schedule_build_opts.schema.json",
        include_str!("../schemas/cashflow/1/schedule_build_opts.schema.json"),
    ),
    (
        "schedule_params.schema.json",
        include_str!("../schemas/cashflow/1/schedule_params.schema.json"),
    ),
    (
        "step_up_coupon_spec.schema.json",
        include_str!("../schemas/cashflow/1/step_up_coupon_spec.schema.json"),
    ),
];

/// Parse the embedded schemas once per process.
///
/// Errors are cached as `String` because `finstack_quant_core::Error` is not
/// `Clone`.
#[cfg(feature = "jsonschema-validate")]
fn parsed_schemas() -> &'static std::result::Result<Vec<(String, jsonschema::Resource)>, String> {
    static CACHE: std::sync::OnceLock<
        std::result::Result<Vec<(String, jsonschema::Resource)>, String>,
    > = std::sync::OnceLock::new();

    CACHE.get_or_init(|| {
        SCHEMAS
            .into_iter()
            .map(|(filename, raw)| {
                let schema = serde_json::from_str::<Value>(raw)
                    .map_err(|err| format!("invalid cashflow schema JSON at {filename}: {err}"))?;
                let resource = jsonschema::Resource::from_contents(schema).map_err(|err| {
                    format!("invalid cashflow schema resource at {filename}: {err}")
                })?;
                Ok((format!("{CASHFLOW_SCHEMA_BASE}{filename}"), resource))
            })
            .collect()
    })
}

/// Return the embedded cashflow schemas as JSON-Schema resolver resources.
///
/// # Errors
///
/// Returns a validation error if a checked-in schema is malformed.
#[cfg(feature = "jsonschema-validate")]
pub fn resources() -> Result<Vec<(String, jsonschema::Resource)>> {
    match parsed_schemas() {
        Ok(entries) => Ok(entries.clone()),
        Err(err) => Err(Error::Validation(err.clone())),
    }
}

/// Canonical `FixedCouponSpec`: a 4.25% semi-annual coupon.
fn coupon_examples() -> finstack_quant_core::Result<Vec<serde_json::Value>> {
    serialize_example(&example_fixed_coupon()?, "fixed coupon spec")
}

/// The coupon shared by the coupon and build-spec examples.
fn example_fixed_coupon() -> finstack_quant_core::Result<crate::builder::FixedCouponSpec> {
    Ok(crate::builder::FixedCouponSpec {
        coupon_type: Default::default(),
        rate: "0.0425".parse().map_err(|error| {
            finstack_quant_core::Error::Internal(format!("parse example coupon rate: {error}"))
        })?,
        schedule: example_schedule()?,
    })
}

/// Canonical `PrepaymentModelSpec`: a flat 6% annual CPR.
fn prepayment_examples() -> finstack_quant_core::Result<Vec<serde_json::Value>> {
    serialize_example(
        &crate::builder::PrepaymentModelSpec {
            cpr: 0.06,
            curve: None,
        },
        "prepayment model spec",
    )
}

/// Canonical `FeeSpec` payloads, one per shape.
fn fee_examples() -> finstack_quant_core::Result<Vec<serde_json::Value>> {
    let date = finstack_quant_core::dates::Date::from_calendar_date(2024, time::Month::January, 15)
        .map_err(|error| {
            finstack_quant_core::Error::Internal(format!("build example fee date: {error}"))
        })?;
    let fixed = crate::builder::FeeSpec::Fixed {
        date,
        amount: finstack_quant_core::money::Money::from((
            25000_i64,
            finstack_quant_core::currency::Currency::USD,
        )),
    };
    serialize_example(&fixed, "fee spec")
}

/// Canonical `ScheduleParams`: a semi-annual USD bond schedule.
fn schedule_params_examples() -> finstack_quant_core::Result<Vec<serde_json::Value>> {
    serialize_example(&example_schedule()?, "schedule params")
}

/// The schedule shared by the schedule and coupon examples.
fn example_schedule() -> finstack_quant_core::Result<crate::builder::ScheduleParams> {
    Ok(crate::builder::ScheduleParams {
        frequency: finstack_quant_core::dates::Tenor::parse("6M").map_err(|error| {
            finstack_quant_core::Error::Internal(format!("parse example frequency: {error}"))
        })?,
        day_count: finstack_quant_core::dates::DayCount::ActActIsma,
        business_day_convention: finstack_quant_core::dates::BusinessDayConvention::Following,
        calendar_id: "sifma".into(),
        stub: Default::default(),
        end_of_month: false,
        payment_lag_days: 0,
        adjust_accrual_dates: false,
        roll_rule: Default::default(),
    })
}

/// Canonical `RecoveryModelSpec`: 40% recovery, three-month lag.
fn recovery_model_examples() -> finstack_quant_core::Result<Vec<serde_json::Value>> {
    serialize_example(
        &crate::builder::RecoveryModelSpec {
            rate: 0.40,
            recovery_lag: 3,
            severity_vector: None,
        },
        "recovery model spec",
    )
}

/// Canonical `AmortizationSpec`: the bullet default.
fn amortization_examples() -> finstack_quant_core::Result<Vec<serde_json::Value>> {
    serialize_example(
        &crate::builder::AmortizationSpec::default(),
        "amortization spec",
    )
}

/// Canonical `DefaultModelSpec`: a flat 2% annual CDR.
fn default_model_examples() -> finstack_quant_core::Result<Vec<serde_json::Value>> {
    serialize_example(
        &crate::builder::DefaultModelSpec {
            cdr: 0.02,
            curve: None,
        },
        "default model spec",
    )
}

/// Canonical `AccrualConfig`: the linear, PIK-inclusive default.
fn accrual_config_examples() -> finstack_quant_core::Result<Vec<serde_json::Value>> {
    serialize_example(&crate::accrual::AccrualConfig::default(), "accrual config")
}

/// A one-year, USD 1mm, 4.25% semi-annual bullet bond.
fn example_build_spec() -> finstack_quant_core::Result<crate::json::CashflowScheduleBuildSpec> {
    let date = |year, month, day| {
        finstack_quant_core::dates::Date::from_calendar_date(year, month, day).map_err(|error| {
            finstack_quant_core::Error::Internal(format!("build example schedule date: {error}"))
        })
    };
    Ok(crate::json::CashflowScheduleBuildSpec {
        notional: crate::builder::Notional {
            initial: finstack_quant_core::money::Money::from((
                1_000_000_i64,
                finstack_quant_core::currency::Currency::USD,
            )),
            amort: Default::default(),
        },
        issue_date: date(2025, time::Month::January, 15)?,
        maturity: date(2026, time::Month::January, 15)?,
        coupon_program: vec![crate::json::CouponLegSpec::Fixed {
            spec: example_fixed_coupon()?,
        }],
        payment_program: Vec::new(),
        fees: Vec::new(),
        principal_events: Vec::new(),
        principal_exchange: Default::default(),
    })
}

/// Canonical `CashflowScheduleBuildSpec`: the inputs of [`cashflow_schedule_examples`].
fn build_spec_examples() -> finstack_quant_core::Result<Vec<serde_json::Value>> {
    serialize_example(&example_build_spec()?, "cashflow schedule build spec")
}

/// Canonical `CashFlowSchedule`: the schedule built from [`example_build_spec`].
fn cashflow_schedule_examples() -> finstack_quant_core::Result<Vec<serde_json::Value>> {
    serialize_example(&example_build_spec()?.build(None)?, "cashflow schedule")
}

/// Canonical `PeriodAggregation`: [`example_build_spec`]'s flows by calendar quarter.
fn period_aggregation_examples() -> finstack_quant_core::Result<Vec<serde_json::Value>> {
    let schedule = example_build_spec()?.build(None)?;
    let flows: Vec<crate::DatedFlow> = schedule
        .flows
        .iter()
        .map(|flow| (flow.date, flow.amount))
        .collect();
    let plan = finstack_quant_core::dates::build_periods("2025Q1..2026Q1", None)?;
    serialize_example(
        &crate::aggregation::aggregate_by_period(&flows, &plan.periods)?,
        "period aggregation",
    )
}

/// Canonical `ScheduleBuildOpts`: a notional hint over default metadata.
fn schedule_build_opts_examples() -> finstack_quant_core::Result<Vec<serde_json::Value>> {
    serialize_example(
        &crate::traits::ScheduleBuildOpts {
            notional_hint: Some(finstack_quant_core::money::Money::from((
                1_000_000_i64,
                finstack_quant_core::currency::Currency::USD,
            ))),
            meta: Default::default(),
        },
        "schedule build opts",
    )
}

/// Canonical `FloatingCouponSpec`: quarterly SOFR plus 150 bp.
fn floating_coupon_examples() -> finstack_quant_core::Result<Vec<serde_json::Value>> {
    let spec: crate::builder::FloatingCouponSpec = serde_json::from_value(serde_json::json!({
        "rate_spec": {
            "forward_curve_id": "USD-SOFR-3M",
            "spread_bp": "150",
            "reset_frequency": {"count": 3, "unit": "months"}
        },
        "frequency": {"count": 3, "unit": "months"},
        "day_count": "act_360",
        "business_day_convention": "modified_following",
        "calendar_id": "nyse",
        "stub": "none",
        "end_of_month": false,
        "payment_lag_days": 0
    }))
    .map_err(|error| {
        finstack_quant_core::Error::Internal(format!("parse example floating coupon: {error}"))
    })?;
    serialize_example(&spec, "floating coupon spec")
}

/// Canonical `StepUpCouponSpec`: 4.25% stepping to 5.25% after one year.
fn step_up_coupon_examples() -> finstack_quant_core::Result<Vec<serde_json::Value>> {
    let parse = |rate: &str| {
        rate.parse().map_err(|error| {
            finstack_quant_core::Error::Internal(format!("parse example coupon rate: {error}"))
        })
    };
    let step_date =
        finstack_quant_core::dates::Date::from_calendar_date(2026, time::Month::January, 15)
            .map_err(|error| {
                finstack_quant_core::Error::Internal(format!("build example step date: {error}"))
            })?;
    let spec = crate::builder::StepUpCouponSpec {
        coupon_type: Default::default(),
        initial_rate: parse("0.0425")?,
        step_schedule: vec![(step_date, parse("0.0525")?)],
        schedule: example_schedule()?,
    };
    serialize_example(&spec, "step-up coupon spec")
}

/// Serialize one spec as a single-element example list.
fn serialize_example<T: serde::Serialize>(
    value: &T,
    label: &str,
) -> finstack_quant_core::Result<Vec<serde_json::Value>> {
    let value = serde_json::to_value(value).map_err(|error| {
        finstack_quant_core::Error::Internal(format!("serialize example {label}: {error}"))
    })?;
    Ok(vec![value])
}

/// The crate's complete schema registry, sorted by artifact path.
///
/// This lives in the library, not the generator binary, so the generator, the
/// contract tests and the bindings all render from one definition. Render an
/// entry with [`finstack_quant_core::schema::SchemaArtifact::generate`].
pub const ARTIFACTS: &[finstack_quant_core::schema::SchemaArtifact] = &[
    finstack_quant_core::schema::SchemaArtifact::new::<crate::accrual::AccrualConfig>(
        "schemas/cashflow/1/accrual_config.schema.json",
        "https://finstack_quant.dev/schemas/cashflow/1/accrual_config.schema.json",
        "AccrualConfig",
        "Schedule-driven accrued-interest configuration.",
    )
    .with_kind(finstack_quant_core::schema::SchemaKind::Input)
    .with_packager(package_cashflow_schema)
    .with_summary(
        "Accrual method, ex-coupon rule, PIK inclusion and coupon frequency for accrued interest.",
    )
    .with_examples(accrual_config_examples),
    finstack_quant_core::schema::SchemaArtifact::new::<crate::builder::AmortizationSpec>(
        "schemas/cashflow/1/amortization_spec.schema.json",
        "https://finstack_quant.dev/schemas/cashflow/1/amortization_spec.schema.json",
        "AmortizationSpec",
        "Cashflow amortization specification.",
    )
    .with_packager(package_cashflow_schema)
    .with_summary("How principal is repaid over the schedule.")
    .with_examples(amortization_examples),
    finstack_quant_core::schema::SchemaArtifact::new::<crate::builder::CashFlowSchedule>(
        "schemas/cashflow/1/cashflow_schedule.schema.json",
        "https://finstack_quant.dev/schemas/cashflow/1/cashflow_schedule.schema.json",
        "CashFlowSchedule",
        "Canonical cashflow schedule.",
    )
    .with_kind(finstack_quant_core::schema::SchemaKind::Output)
    .with_packager(package_cashflow_schema)
    .with_summary(
        "Ordered classified cashflows with their notional, day count and schedule metadata.",
    )
    .with_examples(cashflow_schedule_examples),
    finstack_quant_core::schema::SchemaArtifact::new::<crate::json::CashflowScheduleBuildSpec>(
        "schemas/cashflow/1/cashflow_schedule_build_spec.schema.json",
        "https://finstack_quant.dev/schemas/cashflow/1/cashflow_schedule_build_spec.schema.json",
        "CashflowScheduleBuildSpec",
        "Cashflow schedule build specification.",
    )
    .with_kind(finstack_quant_core::schema::SchemaKind::Input)
    .with_packager(package_cashflow_schema)
    .with_summary(
        "Notional, dates, coupon program, fees and principal events that build a schedule.",
    )
    .with_examples(build_spec_examples),
    finstack_quant_core::schema::SchemaArtifact::new::<crate::builder::FixedCouponSpec>(
        "schemas/cashflow/1/coupon_specs.schema.json",
        "https://finstack_quant.dev/schemas/cashflow/1/coupon_specs.schema.json",
        "FixedCouponSpec",
        "Fixed and floating coupon specification.",
    )
    .with_packager(package_cashflow_schema)
    .with_summary("Coupon rate, frequency, day count and stub handling for one leg.")
    .with_examples(coupon_examples),
    finstack_quant_core::schema::SchemaArtifact::new::<crate::builder::DefaultModelSpec>(
        "schemas/cashflow/1/default_model_spec.schema.json",
        "https://finstack_quant.dev/schemas/cashflow/1/default_model_spec.schema.json",
        "DefaultModelSpec",
        "Cashflow default-model specification.",
    )
    .with_packager(package_cashflow_schema)
    .with_summary("Hazard or CDR assumption driving credit losses.")
    .with_examples(default_model_examples),
    finstack_quant_core::schema::SchemaArtifact::new::<crate::builder::FeeSpec>(
        "schemas/cashflow/1/fee_specs.schema.json",
        "https://finstack_quant.dev/schemas/cashflow/1/fee_specs.schema.json",
        "FeeSpec",
        "Cashflow fee specification.",
    )
    .with_packager(package_cashflow_schema)
    .with_summary("Recurring or one-off fees attached to a schedule.")
    .with_examples(fee_examples),
    finstack_quant_core::schema::SchemaArtifact::new::<crate::builder::FloatingCouponSpec>(
        "schemas/cashflow/1/floating_coupon_spec.schema.json",
        "https://finstack_quant.dev/schemas/cashflow/1/floating_coupon_spec.schema.json",
        "FloatingCouponSpec",
        "Floating coupon specification.",
    )
    .with_kind(finstack_quant_core::schema::SchemaKind::Component)
    .with_packager(package_cashflow_schema)
    .with_summary("Index, spread, caps and floors, reset rules and schedule for one floating leg.")
    .with_examples(floating_coupon_examples),
    finstack_quant_core::schema::SchemaArtifact::new::<crate::aggregation::PeriodAggregation>(
        "schemas/cashflow/1/period_aggregation.schema.json",
        "https://finstack_quant.dev/schemas/cashflow/1/period_aggregation.schema.json",
        "PeriodAggregation",
        "Currency-preserving cashflow totals by reporting period.",
    )
    .with_kind(finstack_quant_core::schema::SchemaKind::Output)
    .with_packager(package_cashflow_schema)
    .with_summary("Per-period, per-currency cashflow totals keyed by period code.")
    .with_examples(period_aggregation_examples),
    finstack_quant_core::schema::SchemaArtifact::new::<crate::builder::PrepaymentModelSpec>(
        "schemas/cashflow/1/prepayment_model_spec.schema.json",
        "https://finstack_quant.dev/schemas/cashflow/1/prepayment_model_spec.schema.json",
        "PrepaymentModelSpec",
        "Cashflow prepayment-model specification.",
    )
    .with_packager(package_cashflow_schema)
    .with_summary("CPR, PSA or custom vector driving voluntary prepayments.")
    .with_examples(prepayment_examples),
    finstack_quant_core::schema::SchemaArtifact::new::<crate::builder::RecoveryModelSpec>(
        "schemas/cashflow/1/recovery_model_spec.schema.json",
        "https://finstack_quant.dev/schemas/cashflow/1/recovery_model_spec.schema.json",
        "RecoveryModelSpec",
        "Cashflow recovery-model specification.",
    )
    .with_packager(package_cashflow_schema)
    .with_summary("Recovery rate and lag applied to defaulted balances.")
    .with_examples(recovery_model_examples),
    finstack_quant_core::schema::SchemaArtifact::new::<crate::traits::ScheduleBuildOpts>(
        "schemas/cashflow/1/schedule_build_opts.schema.json",
        "https://finstack_quant.dev/schemas/cashflow/1/schedule_build_opts.schema.json",
        "ScheduleBuildOpts",
        "Schedule-level inputs for building a schedule from existing flows.",
    )
    .with_kind(finstack_quant_core::schema::SchemaKind::Input)
    .with_packager(package_cashflow_schema)
    .with_summary(
        "Notional hint and metadata stamped on a schedule built from dated or classified flows.",
    )
    .with_examples(schedule_build_opts_examples),
    finstack_quant_core::schema::SchemaArtifact::new::<crate::builder::ScheduleParams>(
        "schemas/cashflow/1/schedule_params.schema.json",
        "https://finstack_quant.dev/schemas/cashflow/1/schedule_params.schema.json",
        "ScheduleParams",
        "Cashflow schedule construction parameters.",
    )
    .with_packager(package_cashflow_schema)
    .with_summary("Start, end, frequency, calendar and roll rules for a payment schedule.")
    .with_examples(schedule_params_examples),
    finstack_quant_core::schema::SchemaArtifact::new::<crate::builder::StepUpCouponSpec>(
        "schemas/cashflow/1/step_up_coupon_spec.schema.json",
        "https://finstack_quant.dev/schemas/cashflow/1/step_up_coupon_spec.schema.json",
        "StepUpCouponSpec",
        "Step-up fixed coupon specification.",
    )
    .with_kind(finstack_quant_core::schema::SchemaKind::Component)
    .with_packager(package_cashflow_schema)
    .with_summary("Initial coupon rate, dated rate steps and schedule for one step-up leg.")
    .with_examples(step_up_coupon_examples),
];
