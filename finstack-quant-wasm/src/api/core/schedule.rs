//! WASM bindings for schedule generation from [`finstack_quant_core::dates`]:
//! `Schedule` and `ScheduleBuilder`. Stub rules, business-day conventions and
//! error policies cross as their snake_case labels (Rust `FromStr`).
//!
//! Schedule dates are **epoch days** (`Int32Array`, days since 1970-01-01);
//! the JSON wire form (`toJson`, `fromSpec`, `toSpec`) uses ISO-8601 strings.

use crate::utils::input::{from_js_json, js_bool, js_epoch_days, js_int, js_string};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_core::dates::{
    days_since_epoch, BusinessDayConvention, Date, Schedule, ScheduleBuilder, ScheduleSpec,
    StubKind, Tenor,
};
use wasm_bindgen::prelude::*;

/// Convert dates to an epoch-day array.
fn epoch_day_array(dates: &[Date]) -> Box<[i32]> {
    dates.iter().copied().map(days_since_epoch).collect()
}

/// A generated schedule: the accrual grid plus payment and fixing dates.
///
/// Build one with `Schedule.builder(start, end)` or `Schedule.fromSpec(spec)`.
///
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const schedule = core.Schedule.builder(
///   core.createDate(2025, 1, 15),
///   core.createDate(2026, 1, 15),
/// )
///   .frequency("3M")
///   .adjustWith("modified_following", "nyse")
///   .build();
/// schedule.dates.length; // 5 accrual boundaries
/// schedule.paymentDates.length; // 4 payments
/// ```
#[wasm_bindgen(js_name = Schedule)]
#[derive(Clone, Debug)]
pub struct JsSchedule {
    pub(crate) inner: Schedule,
}

#[wasm_bindgen(js_class = Schedule)]
impl JsSchedule {
    /// Start a schedule builder for an accrual range (Rust `ScheduleSpec::new`).
    ///
    /// # Arguments
    ///
    /// * `start` - First unadjusted accrual date, as days since 1970-01-01.
    /// * `end` - Last unadjusted accrual date, as days since 1970-01-01; must
    ///   be on or after `start`.
    ///
    /// @returns A `ScheduleBuilder` with the Rust defaults: monthly
    /// frequency, no stub, no business-day adjustment, strict error policy.
    /// @throws `TypeError` if a date is not an integer; `FinstackError` (kind
    /// `validation`) if a date is out of range or `start` is after `end`.
    #[wasm_bindgen(js_name = builder)]
    pub fn builder(start: JsValue, end: JsValue) -> Result<JsScheduleBuilder, JsValue> {
        ScheduleSpec::new(js_epoch_days(&start, "start")?, js_epoch_days(&end, "end")?)
            .map(|spec| JsScheduleBuilder { spec })
            .map_err(to_js_err)
    }

    /// Build a schedule from a persisted specification (Rust `ScheduleBuilder::build`).
    ///
    /// # Arguments
    ///
    /// * `spec` - `ScheduleSpec` JSON text or plain object, such as
    ///   `ScheduleBuilder.toSpec()` output: ISO `start`/`end`, `frequency`
    ///   (a `{ count, unit }` tenor), `stub`, `business_day_convention`, `calendar_id`,
    ///   `end_of_month`, `imm_mode`, `cds_imm_mode`, `error_policy`,
    ///   `payment_lag_days` and `fixing_lag_business_days`. Unknown fields are
    ///   rejected.
    ///
    /// @returns The generated `Schedule`.
    /// @throws `TypeError` if `spec` is not a JSON string or plain object;
    /// `FinstackError` (kind `validation`) for an invalid spec, both IMM modes
    /// together, or a generation failure; kind `not_found` for an unknown
    /// calendar under the strict policy.
    #[wasm_bindgen(js_name = fromSpec)]
    pub fn from_spec(spec: JsValue) -> Result<JsSchedule, JsValue> {
        ScheduleBuilder::from(from_js_json::<ScheduleSpec>(&spec, "spec")?)
            .build()
            .map(|inner| JsSchedule { inner })
            .map_err(to_js_err)
    }

    /// Unadjusted accrual grid as epoch days: the period start plus each period end.
    #[wasm_bindgen(getter, js_name = dates)]
    pub fn dates(&self) -> Box<[i32]> {
        epoch_day_array(&self.inner.dates)
    }

    /// Payment date of each accrual period as epoch days (one per period end).
    #[wasm_bindgen(getter, js_name = paymentDates)]
    pub fn payment_dates(&self) -> Box<[i32]> {
        epoch_day_array(&self.inner.payment_dates)
    }

    /// Fixing date of each accrual period as epoch days; empty when no fixing lag is set.
    #[wasm_bindgen(getter, js_name = fixingDates)]
    pub fn fixing_dates(&self) -> Box<[i32]> {
        epoch_day_array(&self.inner.fixing_dates)
    }

    /// Whether schedule construction produced any warning.
    ///
    /// @returns `true` when `warnings` is non-empty.
    #[wasm_bindgen(js_name = hasWarnings)]
    pub fn has_warnings(&self) -> bool {
        self.inner.has_warnings()
    }

    /// Whether a graceful-fallback policy suppressed a construction error.
    ///
    /// @returns `true` when a `graceful_fallback` warning is present.
    #[wasm_bindgen(js_name = usedGracefulFallback)]
    pub fn used_graceful_fallback(&self) -> bool {
        self.inner.used_graceful_fallback()
    }

    /// Construction warnings in the Rust `ScheduleWarning` wire form (an
    /// array of single-key objects such as `{ graceful_fallback: { … } }`).
    #[wasm_bindgen(getter, js_name = warnings)]
    pub fn warnings(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.warnings)
    }

    /// Serialize to the canonical JSON wire form shared with Python `Schedule.to_json`.
    ///
    /// @returns Compact JSON text with ISO-8601 dates.
    /// @throws If serialization fails (not expected for a valid schedule).
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.inner).map_err(to_js_err)
    }

    /// Deserialize from the canonical JSON wire form produced by `toJson`.
    ///
    /// # Arguments
    ///
    /// * `json` - Schedule JSON text or plain object with ISO `dates`,
    ///   `payment_dates`, `fixing_dates` and optional `warnings`.
    ///
    /// @returns The parsed `Schedule`.
    /// @throws `TypeError` if `json` is not a JSON string or plain object;
    /// `FinstackError` (kind `validation`) if it does not match the schema.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<JsSchedule, JsValue> {
        from_js_json::<Schedule>(&json, "json").map(|inner| JsSchedule { inner })
    }
}

/// Fluent builder for a `Schedule`.
///
/// Every setter updates this builder in place and returns it, so settings
/// can be chained or applied one statement at a time, exactly as in Python.
/// wasm-bindgen cannot return the receiver, so the setters are bound without
/// a return value and `exports/core.js` returns `this` from each.
///
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const monthly = core.Schedule.builder(
///   core.createDate(2025, 1, 15),
///   core.createDate(2025, 7, 15),
/// ).frequency("1M");
/// // A payment lag is counted in business days, so it needs a calendar.
/// const schedule = monthly.adjustWith("following", "nyse").paymentLagDays(2).build();
/// schedule.paymentDates.length; // 6
/// ```
#[wasm_bindgen(js_name = ScheduleBuilder)]
#[derive(Clone, Debug)]
pub struct JsScheduleBuilder {
    spec: ScheduleSpec,
}

#[wasm_bindgen(js_class = ScheduleBuilder)]
impl JsScheduleBuilder {
    /// Set the period frequency.
    ///
    /// # Arguments
    ///
    /// * `frequency` - Tenor text such as `"3M"`, `"6M"` or `"1Y"` (use
    ///   `tenor.toString()` for a `Tenor`).
    ///
    /// @returns This builder, updated in place.
    /// @throws `TypeError` if `frequency` is not a string; `FinstackError`
    /// (kind `validation`) if it is not a tenor.
    #[wasm_bindgen(js_name = frequency)]
    pub fn frequency(&mut self, frequency: JsValue) -> Result<(), JsValue> {
        self.spec.frequency =
            Tenor::parse(&js_string(&frequency, "frequency")?).map_err(to_js_err)?;
        Ok(())
    }

    /// Set the stub rule.
    ///
    /// # Arguments
    ///
    /// * `stub` - Stub name: `"none"`, `"short_front"`, `"short_back"`,
    ///   `"long_front"` or `"long_back"`, in any ASCII case.
    ///
    /// @returns This builder, updated in place.
    /// @throws `TypeError` if `stub` is not a string; `FinstackError` (kind
    /// `validation`) if no stub rule matches.
    #[wasm_bindgen(js_name = stubRule)]
    pub fn stub_rule(&mut self, stub: JsValue) -> Result<(), JsValue> {
        self.spec.stub = js_string(&stub, "stub")?
            .parse::<StubKind>()
            .map_err(to_js_err)?;
        Ok(())
    }

    /// Adjust payment dates with a business-day convention and calendar.
    ///
    /// # Arguments
    ///
    /// * `convention` - Business-day convention name such as
    ///   `"modified_following"`.
    /// * `calendar` - Registered holiday-calendar id (for example `"nyse"`);
    ///   it is resolved when the schedule is built, under the error policy.
    ///
    /// @returns This builder, updated in place.
    /// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
    /// `validation`) if the convention is unknown.
    #[wasm_bindgen(js_name = adjustWith)]
    pub fn adjust_with(&mut self, convention: JsValue, calendar: JsValue) -> Result<(), JsValue> {
        let convention = js_string(&convention, "convention")?
            .parse::<BusinessDayConvention>()
            .map_err(to_js_err)?;
        let calendar = js_string(&calendar, "calendar")?;
        self.spec.business_day_convention = Some(convention);
        self.spec.calendar_id = Some(calendar);
        Ok(())
    }

    /// Set the payment lag in business days after each adjusted period end.
    ///
    /// # Arguments
    ///
    /// * `lag` - Signed number of business days; `0` pays on the period end.
    ///   A non-zero lag needs a calendar (`adjustWith`), or `build` fails.
    ///
    /// @returns This builder, updated in place.
    /// @throws `TypeError` if `lag` is not an integer.
    #[wasm_bindgen(js_name = paymentLagDays)]
    pub fn payment_lag_days(&mut self, lag: JsValue) -> Result<(), JsValue> {
        self.spec.payment_lag_days = js_int(&lag, "lag")?;
        Ok(())
    }

    /// Set the fixing lag in business days before each period's accrual start.
    ///
    /// # Arguments
    ///
    /// * `lag` - Number of business days the fixing precedes the accrual
    ///   start; it needs a calendar (`adjustWith`), or `build` fails.
    ///
    /// @returns This builder, updated in place.
    /// @throws `TypeError` if `lag` is not an integer.
    #[wasm_bindgen(js_name = fixingLagBusinessDays)]
    pub fn fixing_lag_business_days(&mut self, lag: JsValue) -> Result<(), JsValue> {
        self.spec.fixing_lag_business_days = Some(js_int(&lag, "lag")?);
        Ok(())
    }

    /// Enable or disable end-of-month rolling.
    ///
    /// # Arguments
    ///
    /// * `eom` - `true` keeps period ends on the last day of the month when
    ///   the anchor date is a month end.
    ///
    /// @returns This builder, updated in place.
    /// @throws `TypeError` if `eom` is not a boolean.
    #[wasm_bindgen(js_name = endOfMonth)]
    pub fn end_of_month(&mut self, eom: JsValue) -> Result<(), JsValue> {
        self.spec.end_of_month = js_bool(&eom, "eom")?;
        Ok(())
    }

    /// Use CDS IMM dates (the 20th of March, June, September and December).
    ///
    /// Same semantics as the Rust `ScheduleBuilder::cds_imm`: sets the
    /// frequency to quarterly and the stub rule to `short_back`. A later
    /// `frequency` or `stubRule` call wins, and `toSpec()` reports what
    /// `build()` uses.
    ///
    /// @returns This builder, with CDS IMM mode on and standard IMM mode off.
    #[wasm_bindgen(js_name = cdsImm)]
    pub fn cds_imm(&mut self) {
        self.spec = self.spec.clone().with_cds_imm();
    }

    /// Roll on standard IMM dates (the third Wednesday of Mar, Jun, Sep, Dec).
    ///
    /// Same semantics as the Rust `ScheduleBuilder::imm`: sets the IMM
    /// defaults (quarterly frequency, `short_back` stub rule). A later
    /// `frequency` or `stubRule` call wins. `build()` keeps `start` and `end`
    /// as the first and last dates and lays the regular periods on IMM dates
    /// at the frequency, which must be a whole number of quarters: forward
    /// from the first IMM date on or after `start` for back-stub rules,
    /// backward from the last IMM date on or before `end` for front-stub
    /// rules. With no IMM date strictly between `start` and `end` the schedule
    /// is the single period `[start, end]`.
    ///
    /// @returns This builder, with standard IMM mode on and CDS IMM mode off.
    #[wasm_bindgen(js_name = imm)]
    pub fn imm(&mut self) {
        self.spec = self.spec.clone().with_imm();
    }

    /// Set the policy for recoverable construction errors.
    ///
    /// # Arguments
    ///
    /// * `policy` - Policy name: `"strict"`, `"missing_calendar_warning"` or
    ///   `"graceful_empty"`, in any ASCII case.
    ///
    /// @returns This builder, updated in place.
    /// @throws `TypeError` if `policy` is not a string; `FinstackError` (kind
    /// `validation`) if no policy matches.
    #[wasm_bindgen(js_name = errorPolicy)]
    pub fn error_policy(&mut self, policy: JsValue) -> Result<(), JsValue> {
        self.spec.error_policy = js_string(&policy, "policy")?.parse().map_err(to_js_err)?;
        Ok(())
    }

    /// The persisted specification this builder holds.
    ///
    /// @returns A plain `ScheduleSpec` object accepted by `Schedule.fromSpec`.
    /// @throws If the specification cannot be serialized (not expected).
    #[wasm_bindgen(js_name = toSpec)]
    pub fn to_spec(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.spec)
    }

    /// Generate the schedule (Rust `ScheduleBuilder::build`).
    ///
    /// @returns The generated `Schedule`.
    /// @throws `FinstackError` (kind `validation`) for an invalid frequency or
    /// a generation failure; kind `not_found` for an unknown calendar under
    /// the strict policy.
    #[wasm_bindgen(js_name = build)]
    pub fn build(&self) -> Result<JsSchedule, JsValue> {
        ScheduleBuilder::from(self.spec.clone())
            .build()
            .map(|inner| JsSchedule { inner })
            .map_err(to_js_err)
    }
}
