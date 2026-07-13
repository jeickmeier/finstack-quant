//! JSON bridge for constructing and validating cashflow schedules.
//!
//! This module is intentionally small and serde-first. It gives bindings a
//! stable string-based surface while preserving the Rust builder and schedule
//! types as the canonical schema.

use crate::accrual::{accrued_interest_amount, AccrualConfig};
use crate::builder::schedule::sort_schedule_with_metadata;
use crate::builder::{
    CashFlowSchedule, CouponType, FeeSpec, FixedCouponSpec, FixedWindow, FloatingCouponSpec,
    Notional, ScheduleParams, StepUpCouponSpec,
};
use crate::primitives::{is_cash_settlement_kind, CFKind};
use finstack_quant_core::config::{rounding_context_from, FinstackConfig, RoundingContext};
use finstack_quant_core::dates::{Date, DateExt};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::{Error, Result};
use rust_decimal::Decimal;
use serde::Deserializer;

/// Schema version used by stamped cashflow schedule envelopes.
pub const CASHFLOW_SCHEDULE_SCHEMA_VERSION: &str = "finstack_quant.cashflows.schedule/1";

/// Specification for building a [`CashFlowSchedule`] from JSON.
#[derive(Debug, Clone, serde::Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CashflowScheduleBuildSpec {
    /// Principal amount and amortization behavior.
    pub notional: Notional,
    /// Contract issue date.
    #[schemars(with = "String")]
    pub issue: Date,
    /// Contract maturity date.
    #[schemars(with = "String")]
    pub maturity: Date,
    /// Coupon instructions, applied in order through the canonical builder.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub coupon_program: Vec<CouponLegSpec>,
    /// Payment-split instructions, applied after coupon instructions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub payment_program: Vec<PaymentProgramSpec>,
    /// Fee legs to add to the schedule.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fees: Vec<FeeSpec>,
    /// Explicit principal events to add after the base principal setup.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub principal_events: Vec<PrincipalEventSpec>,
}

/// One canonical coupon-program instruction.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CouponLegSpec {
    /// Full-horizon fixed coupon leg.
    Fixed {
        /// Canonical fixed coupon specification.
        spec: FixedCouponSpec,
    },
    /// Full-horizon floating coupon leg.
    Floating {
        /// Canonical floating coupon specification.
        spec: FloatingCouponSpec,
    },
    /// Full-horizon fixed step-up coupon leg.
    StepUp {
        /// Canonical step-up coupon specification.
        spec: StepUpCouponSpec,
    },
    /// Fixed coupon over an explicit half-open date window.
    FixedWindow {
        #[schemars(with = "String")]
        /// Inclusive window start.
        start: Date,
        #[schemars(with = "String")]
        /// Exclusive window end.
        end: Date,
        /// Fixed coupon specification for the window.
        spec: FixedCouponSpec,
    },
    /// Floating coupon over an explicit half-open date window.
    FloatingWindow {
        #[schemars(with = "String")]
        /// Inclusive window start.
        start: Date,
        #[schemars(with = "String")]
        /// Exclusive window end.
        end: Date,
        /// Floating coupon specification for the window.
        spec: FloatingCouponSpec,
    },
    /// Fixed coupons followed by floating coupons at `switch`.
    FixedToFloat {
        #[schemars(with = "String")]
        /// Date on which the floating leg begins.
        switch: Date,
        /// Fixed-rate quote and schedule before the switch.
        fixed: FixedWindow,
        /// Floating coupon specification after the switch.
        floating: FloatingCouponSpec,
        /// Settlement behavior for the fixed leg.
        fixed_split: CouponType,
    },
    /// Consecutive fixed-rate windows driven by dated rate steps.
    FixedRateProgram {
        /// Dated rate steps in strictly increasing order.
        steps: Vec<RateStepSpec>,
        /// Shared schedule conventions for every fixed-rate window.
        schedule: ScheduleParams,
        #[serde(default)]
        /// Default settlement behavior for the generated windows.
        default_split: CouponType,
    },
    /// Consecutive floating-rate windows driven by dated margin steps.
    FloatingMarginProgram {
        /// Dated floating-margin steps in strictly increasing order.
        steps: Vec<RateStepSpec>,
        /// Base floating specification whose spread is replaced by each step.
        base: FloatingCouponSpec,
    },
}

/// A dated decimal step used by coupon programs.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RateStepSpec {
    /// Boundary date at which the step ends or changes.
    #[schemars(with = "String")]
    pub date: Date,
    /// Fixed rate or floating margin, according to the parent instruction.
    pub rate: Decimal,
}

/// One canonical payment-split instruction.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PaymentProgramSpec {
    /// Apply a split over one explicit half-open date window.
    Window {
        #[schemars(with = "String")]
        /// Inclusive window start.
        start: Date,
        #[schemars(with = "String")]
        /// Exclusive window end.
        end: Date,
        /// Settlement behavior active in the window.
        split: CouponType,
    },
    /// Apply consecutive payment splits from dated boundaries.
    Program {
        /// Dated settlement steps in strictly increasing order.
        steps: Vec<PaymentStepSpec>,
    },
}

/// A dated payment-split step.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PaymentStepSpec {
    /// Boundary date for the payment split.
    #[schemars(with = "String")]
    pub date: Date,
    /// Settlement behavior active for the step.
    pub split: CouponType,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CanonicalBuildSpec {
    notional: Notional,
    issue: Date,
    maturity: Date,
    #[serde(default)]
    coupon_program: Vec<CouponLegSpec>,
    #[serde(default)]
    payment_program: Vec<PaymentProgramSpec>,
    #[serde(default)]
    fees: Vec<FeeSpec>,
    #[serde(default)]
    principal_events: Vec<PrincipalEventSpec>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyBuildSpec {
    notional: Notional,
    issue: Date,
    maturity: Date,
    #[serde(default)]
    fixed_coupons: Vec<FixedCouponSpec>,
    #[serde(default)]
    floating_coupons: Vec<FloatingCouponSpec>,
    #[serde(default)]
    fees: Vec<FeeSpec>,
    #[serde(default)]
    principal_events: Vec<PrincipalEventSpec>,
}

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum BuildSpecInput {
    Canonical(CanonicalBuildSpec),
    Legacy(LegacyBuildSpec),
}

impl<'de> serde::Deserialize<'de> for CashflowScheduleBuildSpec {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let input = BuildSpecInput::deserialize(deserializer)?;
        Ok(match input {
            BuildSpecInput::Canonical(spec) => Self {
                notional: spec.notional,
                issue: spec.issue,
                maturity: spec.maturity,
                coupon_program: spec.coupon_program,
                payment_program: spec.payment_program,
                fees: spec.fees,
                principal_events: spec.principal_events,
            },
            BuildSpecInput::Legacy(spec) => {
                let mut coupon_program =
                    Vec::with_capacity(spec.fixed_coupons.len() + spec.floating_coupons.len());
                coupon_program.extend(
                    spec.fixed_coupons
                        .into_iter()
                        .map(|spec| CouponLegSpec::Fixed { spec }),
                );
                coupon_program.extend(
                    spec.floating_coupons
                        .into_iter()
                        .map(|spec| CouponLegSpec::Floating { spec }),
                );
                Self {
                    notional: spec.notional,
                    issue: spec.issue,
                    maturity: spec.maturity,
                    coupon_program,
                    payment_program: Vec::new(),
                    fees: spec.fees,
                    principal_events: spec.principal_events,
                }
            }
        })
    }
}

/// JSON representation of an explicit principal event.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PrincipalEventSpec {
    /// Event date.
    #[schemars(with = "String")]
    pub date: Date,
    /// Outstanding balance delta. Positive increases outstanding, negative repays.
    pub delta: Money,
    /// Optional cash leg. When omitted, the cash leg equals `delta`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cash: Option<Money>,
    /// Cashflow classification to emit.
    pub kind: CFKind,
}

/// JSON-friendly dated flow item.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DatedFlowJson {
    /// Flow date.
    #[schemars(with = "String")]
    pub date: Date,
    /// Dated amount.
    pub amount: Money,
}

/// Version-stamped cashflow schedule payload for durable JSON examples.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CashflowScheduleEnvelope {
    /// Envelope schema version.
    pub schema_version: String,
    /// Rounding and tolerance context active at serialization time.
    pub rounding_context: RoundingContext,
    /// Canonical cashflow schedule.
    pub schedule: CashFlowSchedule,
}

impl CashflowScheduleEnvelope {
    /// Build an envelope from a canonical schedule using deterministic default config.
    pub fn from_schedule(schedule: CashFlowSchedule) -> Self {
        let config = FinstackConfig::default();
        Self {
            schema_version: CASHFLOW_SCHEDULE_SCHEMA_VERSION.to_string(),
            rounding_context: rounding_context_from(&config),
            schedule,
        }
    }

    fn validate_schema_version(&self) -> Result<()> {
        if self.schema_version != CASHFLOW_SCHEDULE_SCHEMA_VERSION {
            return Err(Error::Validation(format!(
                "unsupported cashflow schedule envelope schema_version '{}'; expected '{}'",
                self.schema_version, CASHFLOW_SCHEDULE_SCHEMA_VERSION
            )));
        }
        Ok(())
    }
}

impl CashflowScheduleBuildSpec {
    /// Build a canonical cashflow schedule.
    ///
    /// This applies the same builder pipeline used by Rust callers: principal
    /// setup, amortization, fixed coupons, floating coupons, fees, principal
    /// events, validation, and deterministic sorting.
    ///
    /// # Arguments
    ///
    /// * `market` - Optional market context used for floating-rate projection.
    ///   Fixed-rate schedules can pass `None`.
    ///
    /// # Returns
    ///
    /// Fully materialized [`CashFlowSchedule`] with canonical metadata and
    /// sorted cashflows.
    ///
    /// # Errors
    ///
    /// Returns an error when the specification is internally inconsistent or
    /// when floating coupons require market data that is unavailable.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_cashflows::json::CashflowScheduleBuildSpec;
    /// use finstack_quant_core::dates::Date;
    /// use time::Month;
    ///
    /// let spec_json = r#"{
    ///   "notional": {
    ///     "initial": { "amount": "1000000", "currency": "USD" },
    ///     "amort": "None"
    ///   },
    ///   "issue": "2024-08-31",
    ///   "maturity": "2025-08-31",
    ///   "fixed_coupons": [{
    ///     "coupon_type": "Cash",
    ///     "rate": "0.06",
    ///     "freq": { "count": 12, "unit": "months" },
    ///     "dc": "Thirty360",
    ///     "bdc": "following",
    ///     "calendar_id": "weekends_only",
    ///     "stub": "None",
    ///     "end_of_month": false,
    ///     "payment_lag_days": 0
    ///   }]
    /// }"#;
    ///
    /// let spec: CashflowScheduleBuildSpec = serde_json::from_str(spec_json).expect("valid spec");
    /// let schedule = spec.build(None)?;
    /// assert_eq!(
    ///     schedule.meta.issue_date,
    ///     Some(Date::from_calendar_date(2024, Month::August, 31).expect("valid date"))
    /// );
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    pub fn build(&self, market: Option<&MarketContext>) -> Result<CashFlowSchedule> {
        let mut builder = CashFlowSchedule::builder();
        let _ = builder
            .principal(self.notional.initial, self.issue, self.maturity)
            .amortization(self.notional.amort.clone());

        for instruction in &self.coupon_program {
            match instruction {
                CouponLegSpec::Fixed { spec } => {
                    let _ = builder.fixed_cf(spec.clone());
                }
                CouponLegSpec::Floating { spec } => {
                    let _ = builder.floating_cf(spec.clone());
                }
                CouponLegSpec::StepUp { spec } => {
                    let _ = builder.step_up_cf(spec.clone());
                }
                CouponLegSpec::FixedWindow { start, end, spec } => {
                    let _ = builder.add_fixed_window(*start, *end, spec.clone());
                }
                CouponLegSpec::FloatingWindow { start, end, spec } => {
                    let _ = builder.add_floating_window(*start, *end, spec.clone());
                }
                CouponLegSpec::FixedToFloat {
                    switch,
                    fixed,
                    floating,
                    fixed_split,
                } => {
                    let _ = builder.fixed_to_float(
                        *switch,
                        fixed.clone(),
                        floating.clone(),
                        *fixed_split,
                    );
                }
                CouponLegSpec::FixedRateProgram {
                    steps,
                    schedule,
                    default_split,
                } => {
                    let steps: Vec<_> = steps.iter().map(|step| (step.date, step.rate)).collect();
                    let _ = builder.fixed_stepup_decimal(&steps, schedule.clone(), *default_split);
                }
                CouponLegSpec::FloatingMarginProgram { steps, base } => {
                    let steps: Vec<_> = steps.iter().map(|step| (step.date, step.rate)).collect();
                    let _ = builder.float_margin_stepup_decimal(&steps, base.clone());
                }
            }
        }
        for instruction in &self.payment_program {
            match instruction {
                PaymentProgramSpec::Window { start, end, split } => {
                    let _ = builder.add_payment_window(*start, *end, *split);
                }
                PaymentProgramSpec::Program { steps } => {
                    let steps: Vec<_> = steps.iter().map(|step| (step.date, step.split)).collect();
                    let _ = builder.payment_split_program(&steps);
                }
            }
        }
        for spec in &self.fees {
            let _ = builder.fee(spec.clone());
        }
        for event in &self.principal_events {
            let _ = builder.add_principal_event(event.date, event.delta, event.cash, event.kind);
        }

        builder.build_with_curves(market)
    }
}

/// Build a schedule from JSON and return canonical schedule JSON.
///
/// The input must be a JSON-encoded [`CashflowScheduleBuildSpec`]. The output
/// is the canonical serde representation of [`CashFlowSchedule`], including
/// builder-populated metadata such as `meta.issue_date` and deterministic
/// cashflow ordering. The payload is not wrapped in a schema envelope; callers
/// that store versioned examples should track that version outside this bridge.
///
/// # Arguments
///
/// * `spec_json` - JSON-encoded [`CashflowScheduleBuildSpec`].
/// * `market_json` - Optional JSON-encoded [`MarketContext`] used for floating
///   coupon projection.
///
/// # Returns
///
/// Canonical JSON string for the generated [`CashFlowSchedule`].
///
/// # Errors
///
/// Returns an error if the input JSON cannot be parsed, the market JSON is
/// invalid, the build spec is inconsistent, or the output cannot be serialized.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_cashflows::build_cashflow_schedule_json;
///
/// let spec_json = r#"{
///   "notional": {
///     "initial": { "amount": "1000000", "currency": "USD" },
///     "amort": "None"
///   },
///   "issue": "2024-08-31",
///   "maturity": "2025-08-31",
///   "fixed_coupons": [{
///     "coupon_type": "Cash",
///     "rate": "0.06",
///     "freq": { "count": 12, "unit": "months" },
///     "dc": "Thirty360",
///     "bdc": "following",
///     "calendar_id": "weekends_only",
///     "stub": "None",
///     "end_of_month": false,
///     "payment_lag_days": 0
///   }]
/// }"#;
///
/// let schedule_json = build_cashflow_schedule_json(spec_json, None)?;
/// assert!(schedule_json.contains("\"flows\""));
/// assert!(schedule_json.contains("\"issue_date\":\"2024-08-31\""));
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn build_cashflow_schedule_json(spec_json: &str, market_json: Option<&str>) -> Result<String> {
    let spec: CashflowScheduleBuildSpec = serde_json::from_str(spec_json).map_err(|err| {
        Error::Validation(format!("invalid cashflow schedule build spec JSON: {err}"))
    })?;
    let market = parse_optional_market(market_json)?;
    let schedule = spec.build(market.as_ref())?;
    serialize_json(&schedule, "cashflow schedule")
}

/// Build a stamped schedule envelope from JSON and return canonical envelope JSON.
pub fn build_cashflow_schedule_envelope_json(
    spec_json: &str,
    market_json: Option<&str>,
) -> Result<String> {
    let spec: CashflowScheduleBuildSpec = serde_json::from_str(spec_json).map_err(|err| {
        Error::Validation(format!("invalid cashflow schedule build spec JSON: {err}"))
    })?;
    let market = parse_optional_market(market_json)?;
    let schedule = spec.build(market.as_ref())?;
    let envelope = CashflowScheduleEnvelope::from_schedule(schedule);
    serialize_json(&envelope, "cashflow schedule envelope")
}

/// Validate a schedule JSON payload and return canonical schedule JSON.
///
/// Canonicalization parses the payload as [`CashFlowSchedule`] and serializes
/// it back with the Rust serde model. This verifies the shape and normalizes
/// serialization, but it does not rebuild or regenerate cashflows from an
/// economic spec.
///
/// # Pre-Issue Flows
///
/// When `meta.issue_date` is set, interest-bearing flows (coupons, PIK,
/// stubs) dated before the issue date are rejected. Principal-type flows
/// (notional exchanges, amortization, prepayments) and fees may predate the
/// issue date: the builder produces such schedules for delayed-funding
/// structures.
///
/// # Arguments
///
/// * `schedule_json` - JSON-encoded [`CashFlowSchedule`].
///
/// # Returns
///
/// Canonical JSON string for the parsed schedule.
///
/// # Errors
///
/// Returns an error if the input is not a valid [`CashFlowSchedule`] JSON value.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_cashflows::{build_cashflow_schedule_json, validate_cashflow_schedule_json};
///
/// let spec_json = r#"{
///   "notional": {
///     "initial": { "amount": "1000000", "currency": "USD" },
///     "amort": "None"
///   },
///   "issue": "2024-08-31",
///   "maturity": "2025-08-31",
///   "fixed_coupons": []
/// }"#;
///
/// let schedule_json = build_cashflow_schedule_json(spec_json, None)?;
/// let canonical = validate_cashflow_schedule_json(&schedule_json)?;
/// assert_eq!(serde_json::from_str::<serde_json::Value>(&canonical).unwrap()["meta"]["issue_date"], "2024-08-31");
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn validate_cashflow_schedule_json(schedule_json: &str) -> Result<String> {
    let mut schedule = parse_schedule(schedule_json)?;
    validate_cashflow_schedule(&mut schedule)?;
    serialize_json(&schedule, "cashflow schedule")
}

/// Validate and canonicalize an in-memory schedule for all public consumers.
pub fn validate_cashflow_schedule(schedule: &mut CashFlowSchedule) -> Result<()> {
    sort_schedule_with_metadata(schedule);
    schedule.validate()?;
    validate_schedule_economic_invariants(schedule)
}

/// Validate a stamped schedule envelope JSON payload and return canonical JSON.
pub fn validate_cashflow_schedule_envelope_json(envelope_json: &str) -> Result<String> {
    let envelope: CashflowScheduleEnvelope =
        serde_json::from_str(envelope_json).map_err(|err| {
            Error::Validation(format!("invalid cashflow schedule envelope JSON: {err}"))
        })?;
    envelope.validate_schema_version()?;
    let mut envelope = envelope;
    validate_cashflow_schedule(&mut envelope.schedule)?;
    serialize_json(&envelope, "cashflow schedule envelope")
}

/// Extract dated amounts from a schedule JSON payload.
///
/// The returned JSON is an array of [`DatedFlowJson`] values. Each entry
/// contains the cashflow date and currency-tagged amount. Non-cash state rows
/// (`PIK` and `DefaultedNotional`) are excluded so the result is safe to sum
/// as dated settlement cash.
///
/// # Arguments
///
/// * `schedule_json` - JSON-encoded [`CashFlowSchedule`].
///
/// # Returns
///
/// JSON array of dated amount objects.
///
/// # Errors
///
/// Returns an error if the schedule JSON is invalid or the output cannot be
/// serialized.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_cashflows::{build_cashflow_schedule_json, dated_flows_json};
///
/// let spec_json = r#"{
///   "notional": {
///     "initial": { "amount": "1000000", "currency": "USD" },
///     "amort": "None"
///   },
///   "issue": "2024-08-31",
///   "maturity": "2025-08-31",
///   "fixed_coupons": []
/// }"#;
///
/// let schedule_json = build_cashflow_schedule_json(spec_json, None)?;
/// let flows_json = dated_flows_json(&schedule_json)?;
/// let flows: Vec<serde_json::Value> = serde_json::from_str(&flows_json).unwrap();
/// assert!(!flows.is_empty());
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn dated_flows_json(schedule_json: &str) -> Result<String> {
    let mut schedule = parse_schedule(schedule_json)?;
    validate_cashflow_schedule(&mut schedule)?;
    let flows: Vec<DatedFlowJson> = schedule
        .flows
        .iter()
        .filter(|flow| is_cash_settlement_kind(flow.kind))
        .map(|flow| DatedFlowJson {
            date: flow.date,
            amount: flow.amount,
        })
        .collect();
    serialize_json(&flows, "dated flows")
}

/// Compute accrued interest from a schedule JSON payload.
///
/// The schedule is parsed as [`CashFlowSchedule`], `as_of` is parsed as an
/// ISO-8601 date, and `config_json` is parsed as [`AccrualConfig`] when
/// supplied. When `config_json` is `None`, [`AccrualConfig::default`] is used.
///
/// # Arguments
///
/// * `schedule_json` - JSON-encoded [`CashFlowSchedule`].
/// * `as_of` - ISO-8601 date string such as `"2025-02-28"`.
/// * `config_json` - Optional JSON-encoded [`AccrualConfig`].
///
/// # Returns
///
/// Scalar accrued-interest amount in the schedule's currency space.
///
/// # Errors
///
/// Returns an error if the schedule, as-of date, or optional accrual config JSON
/// cannot be parsed.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_cashflows::{accrued_interest_json, build_cashflow_schedule_json};
///
/// let spec_json = r#"{
///   "notional": {
///     "initial": { "amount": "1000000", "currency": "USD" },
///     "amort": "None"
///   },
///   "issue": "2024-08-31",
///   "maturity": "2025-08-31",
///   "fixed_coupons": [{
///     "coupon_type": "Cash",
///     "rate": "0.06",
///     "freq": { "count": 12, "unit": "months" },
///     "dc": "Thirty360",
///     "bdc": "following",
///     "calendar_id": "weekends_only",
///     "stub": "None",
///     "end_of_month": false,
///     "payment_lag_days": 0
///   }]
/// }"#;
///
/// let schedule_json = build_cashflow_schedule_json(spec_json, None)?;
/// let accrued = accrued_interest_json(&schedule_json, "2025-02-28", None)?;
/// assert!(accrued > 0.0);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn accrued_interest_json(
    schedule_json: &str,
    as_of: &str,
    config_json: Option<&str>,
) -> Result<f64> {
    let mut schedule = parse_schedule(schedule_json)?;
    validate_cashflow_schedule(&mut schedule)?;
    let as_of = parse_iso_date(as_of)?;
    let config = match config_json {
        Some(json) => serde_json::from_str::<AccrualConfig>(json)
            .map_err(|err| Error::Validation(format!("invalid accrual config JSON: {err}")))?,
        None => AccrualConfig::default(),
    };
    accrued_interest_amount(&schedule, as_of, &config)
}

fn parse_schedule(schedule_json: &str) -> Result<CashFlowSchedule> {
    serde_json::from_str(schedule_json)
        .map_err(|err| Error::Validation(format!("invalid cashflow schedule JSON: {err}")))
}

fn validate_schedule_economic_invariants(schedule: &CashFlowSchedule) -> Result<()> {
    let initial = schedule.notional.initial;
    let expected_currency = initial.currency();
    let initial_amount = initial.amount().abs();
    // Scale tolerance with notional to absorb serde/f64 roundoff; this is not
    // a permission to over-amortize economically.
    let epsilon = (initial_amount * 1e-8).max(1e-6);
    let mut total_amortization = 0.0_f64;

    for flow in &schedule.flows {
        if flow.kind == CFKind::Amortization {
            if flow.amount.currency() != expected_currency {
                return Err(Error::Validation(format!(
                    "amortization flow currency ({}) must match initial notional currency ({})",
                    flow.amount.currency(),
                    expected_currency
                )));
            }
            total_amortization += flow.amount.amount().max(0.0);
        }
    }

    if total_amortization > initial_amount + epsilon {
        return Err(Error::Validation(format!(
            "total amortization ({total_amortization:.6}) exceeds initial notional ({initial_amount:.6})"
        )));
    }

    if let Some(issue_date) = schedule.meta.issue_date {
        let long_horizon = issue_date.add_months(1200);
        for flow in &schedule.flows {
            // Principal-type flows (Notional/Amortization/PrePayment and the
            // revolver variants) and fees MAY predate the issue date: the
            // builder itself emits them for delayed-funding structures.
            // Interest-bearing kinds cannot accrue before issue, so they are
            // still rejected.
            let interest_bearing = matches!(
                flow.kind,
                CFKind::Fixed
                    | CFKind::FloatReset
                    | CFKind::InflationCoupon
                    | CFKind::PIK
                    | CFKind::Stub
            );
            if flow.date < issue_date && interest_bearing {
                return Err(Error::Validation(format!(
                    "interest-bearing cashflow ({:?}) dated {} is before issue date {}",
                    flow.kind, flow.date, issue_date
                )));
            }
            if flow.date > long_horizon {
                tracing::warn!(
                    flow_date = %flow.date,
                    issue_date = %issue_date,
                    horizon_date = %long_horizon,
                    "cashflow schedule contains a flow more than 100 years after issue date"
                );
            }
        }
    }

    Ok(())
}

fn parse_optional_market(market_json: Option<&str>) -> Result<Option<MarketContext>> {
    market_json
        .map(|json| {
            serde_json::from_str::<MarketContext>(json)
                .map_err(|err| Error::Validation(format!("invalid market context JSON: {err}")))
        })
        .transpose()
}

fn parse_iso_date(value: &str) -> Result<Date> {
    let format = time::format_description::well_known::Iso8601::DEFAULT;
    Date::parse(value, &format)
        .map_err(|err| Error::Validation(format!("invalid ISO date '{value}': {err}")))
}

fn serialize_json<T: serde::Serialize>(value: &T, label: &str) -> Result<String> {
    serde_json::to_string(value)
        .map_err(|err| Error::Validation(format!("failed to serialize {label} JSON: {err}")))
}
