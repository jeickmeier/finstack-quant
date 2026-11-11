//! WASM bindings for schedule generation from [`finstack_quant_core::dates`]:
//! `Schedule` and `ScheduleBuilder`. Stub rules, grid modes and business-day
//! conventions cross as their snake_case labels (Rust `FromStr`).
//!
//! Schedule dates are **epoch days** (`Int32Array`, days since 1970-01-01);
//! the JSON wire form (`toJson`, `fromSpec`, `toSpec`) uses ISO-8601 strings.

use crate::utils::input::{from_js_json, js_epoch_days, js_int, js_string};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_core::dates::{
    BusinessDayConvention, Date, GridMode, Schedule, ScheduleBuilder, ScheduleSpec, StubKind,
    Tenor, days_since_epoch,
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
///   .stubRule("none")
///   .grid("natural")
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
    /// Start a schedule builder for an accrual range (Rust `Schedule::builder`).
    ///
    /// The builder has no financial defaults: choose the frequency, stub
    /// rule, grid and payment convention before `build()`.
    ///
    /// # Arguments
    ///
    /// * `start` - First unadjusted accrual date, as days since 1970-01-01.
    /// * `end` - Last unadjusted accrual date, as days since 1970-01-01; must
    ///   be after `start`.
    ///
    /// @returns An empty `ScheduleBuilder` for the range.
    /// @throws `TypeError` if a date is not an integer; `FinstackError` (kind
    /// `validation`) if a date is out of range or `start` is not before `end`.
    /// @param start - First unadjusted accrual date, as days since 1970-01-01.
    /// @param end - Last unadjusted accrual date, as days since 1970-01-01; must be after `start`.
    #[wasm_bindgen(js_name = builder)]
    pub fn builder(
        #[wasm_bindgen(unchecked_param_type = "number")] start: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] end: JsValue,
    ) -> Result<JsScheduleBuilder, JsValue> {
        Schedule::builder(js_epoch_days(&start, "start")?, js_epoch_days(&end, "end")?)
            .map(|inner| JsScheduleBuilder { inner })
            .map_err(to_js_err)
    }

    /// Build a schedule from a persisted specification (Rust `ScheduleBuilder::build`).
    ///
    /// # Arguments
    ///
    /// * `spec` - `ScheduleSpec` JSON text or plain object, such as
    ///   `ScheduleBuilder.toSpec()` output: ISO `start`/`end`, `frequency`
    ///   (a `{ count, unit }` tenor), `stub`, `grid`, `cds_imm_mode`,
    ///   `business_day_convention`, `calendar_id`, and the optional
    ///   `payment_lag_days` and `fixing_lag_business_days`. Unknown fields are
    ///   rejected.
    ///
    /// @returns The generated `Schedule`.
    /// @throws `TypeError` if `spec` is not a JSON string or plain object;
    /// `FinstackError` (kind `validation`) for an invalid spec (missing choice,
    /// CDS IMM on a non-natural grid, a convention without a calendar) or a
    /// generation failure; kind `not_found` for an unknown calendar.
    /// @param spec - `ScheduleSpec` JSON text or plain object, such as `ScheduleBuilder.toSpec()` output: ISO `start`/`end`, `frequency` (a `{ count, unit }` tenor), `stub`, `grid`, `cds_imm_mode`, `business_day_convention`, `calendar_id`, and the optional `payment_lag_days` and `fixing_lag_business_days`. Unknown fields are rejected.
    #[wasm_bindgen(js_name = fromSpec)]
    pub fn from_spec(
        #[wasm_bindgen(unchecked_param_type = "ScheduleSpec | string")] spec: JsValue,
    ) -> Result<JsSchedule, JsValue> {
        ScheduleBuilder::from(from_js_json::<ScheduleSpec>(&spec, "spec")?)
            .build()
            .map(|inner| JsSchedule { inner })
            .map_err(to_js_err)
    }

    /// Unadjusted accrual grid as epoch days: the period start plus each period end.
    #[wasm_bindgen(getter, js_name = dates)]
    pub fn dates(&self) -> Box<[i32]> {
        epoch_day_array(self.inner.get_dates())
    }

    /// Payment date of each accrual period as epoch days (one per period end).
    #[wasm_bindgen(getter, js_name = paymentDates)]
    pub fn payment_dates(&self) -> Box<[i32]> {
        epoch_day_array(self.inner.get_payment_dates())
    }

    /// Fixing date of each accrual period as epoch days; empty when no fixing lag is set.
    #[wasm_bindgen(getter, js_name = fixingDates)]
    pub fn fixing_dates(&self) -> Box<[i32]> {
        epoch_day_array(self.inner.get_fixing_dates())
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
    ///   `payment_dates` and `fixing_dates`.
    ///
    /// @returns The parsed `Schedule`.
    /// @throws `TypeError` if `json` is not a JSON string or plain object;
    /// `FinstackError` (kind `validation`) if it does not match the schema
    /// (all three arrays are required) or its shape is invalid: fewer than 2
    /// or more than 100,000 grid dates, grid dates not strictly increasing,
    /// not exactly one payment date per period, or a non-empty fixing series
    /// of the wrong length.
    /// @param json - Schedule JSON text or plain object with ISO `dates`, `payment_dates` and `fixing_dates`.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(
        #[wasm_bindgen(unchecked_param_type = "JsonInput")] json: JsValue,
    ) -> Result<JsSchedule, JsValue> {
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
/// ).frequency("1M").stubRule("none").grid("natural");
/// // A payment lag is counted in business days, so it needs a calendar.
/// const schedule = monthly.adjustWith("following", "nyse").paymentLagDays(2).build();
/// schedule.paymentDates.length; // 6
/// ```
///
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const monthly = core.Schedule.builder(
///   core.createDate(2025, 1, 15),
///   core.createDate(2025, 7, 15),
/// )
///   .frequency("1M")
///   .stubRule("none")
///   .grid("natural")
///   .adjustWith("modified_following", "nyse");
/// const schedule = monthly.paymentLagDays(2).build();
/// schedule.paymentDates.length; // 6
/// ```
///
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const monthly = core.Schedule.builder(
///   core.createDate(2025, 1, 15),
///   core.createDate(2025, 7, 15),
/// )
///   .frequency("1M")
///   .stubRule("none")
///   .grid("natural")
///   .adjustWith("modified_following", "nyse");
/// const schedule = monthly.paymentLagDays(2).build();
/// schedule.paymentDates.length; // 6
/// ```
#[wasm_bindgen(js_name = ScheduleBuilder)]
#[derive(Clone, Debug)]
pub struct JsScheduleBuilder {
    inner: ScheduleBuilder,
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
    /// @param frequency - Tenor text such as `"3M"`, `"6M"` or `"1Y"` (use `tenor.toString()` for a `Tenor`).
    #[wasm_bindgen(js_name = frequency)]
    pub fn frequency(
        &mut self,
        #[wasm_bindgen(unchecked_param_type = "string")] frequency: JsValue,
    ) -> Result<(), JsValue> {
        let frequency = Tenor::parse(&js_string(&frequency, "frequency")?).map_err(to_js_err)?;
        self.inner = self.inner.clone().frequency(frequency);
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
    /// @param stub - Stub name: `"none"`, `"short_front"`, `"short_back"`, `"long_front"` or `"long_back"`, in any ASCII case.
    #[wasm_bindgen(js_name = stubRule)]
    pub fn stub_rule(
        &mut self,
        #[wasm_bindgen(unchecked_param_type = "string")] stub: JsValue,
    ) -> Result<(), JsValue> {
        let stub = js_string(&stub, "stub")?
            .parse::<StubKind>()
            .map_err(to_js_err)?;
        self.inner = self.inner.clone().stub_rule(stub);
        Ok(())
    }

    /// Select the regular grid.
    ///
    /// # Arguments
    ///
    /// * `grid` - Grid name: `"natural"`, `"end_of_month"` (month/year
    ///   frequency, month-end regular anchor) or `"quarterly_imm"` (whole
    ///   quarters, IMM-date regular anchor), in any ASCII case.
    ///
    /// @returns This builder, updated in place.
    /// @throws `TypeError` if `grid` is not a string; `FinstackError` (kind
    /// `validation`) if no grid matches.
    /// @param grid - Grid name: `"natural"`, `"end_of_month"` (month/year frequency, month-end regular anchor) or `"quarterly_imm"` (whole quarters, IMM-date regular anchor), in any ASCII case.
    #[wasm_bindgen(js_name = grid)]
    pub fn grid(
        &mut self,
        #[wasm_bindgen(unchecked_param_type = "string")] grid: JsValue,
    ) -> Result<(), JsValue> {
        let grid = js_string(&grid, "grid")?
            .parse::<GridMode>()
            .map_err(to_js_err)?;
        self.inner = self.inner.clone().grid(grid);
        Ok(())
    }

    /// Keep payment dates on the raw period ends, with no calendar.
    ///
    /// Selects the `unadjusted` convention and clears any calendar; a
    /// positive payment or fixing lag then makes `build` throw.
    ///
    /// @returns This builder, updated in place.
    #[wasm_bindgen(js_name = unadjusted)]
    pub fn unadjusted(&mut self) {
        self.inner = self.inner.clone().unadjusted();
    }

    /// Adjust payment dates with a business-day convention and calendar.
    ///
    /// # Arguments
    ///
    /// * `convention` - Business-day convention name such as
    ///   `"modified_following"`.
    /// * `calendar` - Registered holiday-calendar id (for example `"nyse"`);
    ///   it is resolved strictly when the schedule is built.
    ///
    /// @returns This builder, updated in place.
    /// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
    /// `validation`) if the convention is unknown.
    /// @param convention - Business-day convention name such as `"modified_following"`.
    /// @param calendar - Registered holiday-calendar id (for example `"nyse"`); it is resolved strictly when the schedule is built.
    #[wasm_bindgen(js_name = adjustWith)]
    pub fn adjust_with(
        &mut self,
        #[wasm_bindgen(unchecked_param_type = "string")] convention: JsValue,
        #[wasm_bindgen(unchecked_param_type = "string")] calendar: JsValue,
    ) -> Result<(), JsValue> {
        let convention = js_string(&convention, "convention")?
            .parse::<BusinessDayConvention>()
            .map_err(to_js_err)?;
        let calendar = js_string(&calendar, "calendar")?;
        self.inner = self.inner.clone().adjust_with(convention, &calendar);
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
    /// @param lag - Signed number of business days; `0` pays on the period end. A non-zero lag needs a calendar (`adjustWith`), or `build` fails.
    #[wasm_bindgen(js_name = paymentLagDays)]
    pub fn payment_lag_days(
        &mut self,
        #[wasm_bindgen(unchecked_param_type = "number")] lag: JsValue,
    ) -> Result<(), JsValue> {
        self.inner = self.inner.clone().payment_lag_days(js_int(&lag, "lag")?);
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
    /// @param lag - Number of business days the fixing precedes the accrual start; it needs a calendar (`adjustWith`), or `build` fails.
    #[wasm_bindgen(js_name = fixingLagBusinessDays)]
    pub fn fixing_lag_business_days(
        &mut self,
        #[wasm_bindgen(unchecked_param_type = "number")] lag: JsValue,
    ) -> Result<(), JsValue> {
        self.inner = self
            .inner
            .clone()
            .fixing_lag_business_days(js_int(&lag, "lag")?);
        Ok(())
    }

    /// Use CDS IMM dates (the 20th of March, June, September and December).
    ///
    /// Same semantics as the Rust `ScheduleBuilder::cds_imm`: sets the
    /// frequency to quarterly and the stub rule to `short_back` and turns CDS
    /// IMM mode on (it requires the natural grid). A later `frequency` or
    /// `stubRule` call wins, and `toSpec()` reports what `build()` uses.
    ///
    /// @returns This builder, with CDS IMM mode on.
    #[wasm_bindgen(js_name = cdsImm)]
    pub fn cds_imm(&mut self) {
        self.inner = self.inner.clone().cds_imm();
    }

    /// The complete specification these choices describe.
    ///
    /// @returns A plain `ScheduleSpec` object accepted by `Schedule.fromSpec`.
    /// @throws `FinstackError` (kind `validation`) naming every missing choice
    /// (frequency, stub, grid, payment convention) or an inconsistent
    /// combination.
    #[wasm_bindgen(js_name = toSpec, unchecked_return_type = "ScheduleSpec")]
    pub fn to_spec(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.to_spec().map_err(to_js_err)?)
    }

    /// Generate the schedule (Rust `ScheduleBuilder::build`).
    ///
    /// @returns The generated `Schedule`.
    /// @throws `FinstackError` (kind `validation`) for a missing choice, an
    /// inconsistent specification or a generation failure; kind `not_found`
    /// for an unknown calendar.
    #[wasm_bindgen(js_name = build)]
    pub fn build(&self) -> Result<JsSchedule, JsValue> {
        self.inner
            .clone()
            .build()
            .map(|inner| JsSchedule { inner })
            .map_err(to_js_err)
    }
}
