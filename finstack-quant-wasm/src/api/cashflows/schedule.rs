//! `CashFlowSchedule` and `CashFlowBuilder` handles and the typed schedule
//! constructors.
//!
//! The schedule is held parsed, so analytics run without re-reading JSON; its
//! parts (flows, notional, metadata) and every spec argument cross as wire
//! values typed by the generated `cashflows` TypeScript declarations.

use crate::api::core::market_context::JsMarketContext;
use crate::utils::input::{js_f64, js_f64_seq, js_string, json_text};
use crate::utils::wire::{js_date, js_opt_wire, js_wire};
use crate::utils::{date_to_iso, to_js_err, to_js_value};
use finstack_quant_cashflows::builder::schedule::merge_cashflow_schedules;
use finstack_quant_cashflows::builder::{
    CashFlowBuilder, CashFlowMeta, CashFlowSchedule, CashflowRepresentation, CouponType,
    FixedCouponSpec, FloatingCouponSpec, Notional,
};
use finstack_quant_cashflows::primitives::{CFKind, CashFlow};
use finstack_quant_cashflows::{CashflowScheduleBuildSpec, DatedFlowJson, ScheduleBuildOpts};
use finstack_quant_core::dates::{Date, DayCount, Period};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::wire::DecimalWire;
use wasm_bindgen::prelude::*;

/// `[date, value]` pairs with ISO dates, as the coupon programs carry them.
#[derive(serde::Deserialize)]
struct DatedPair<T>(#[serde(with = "finstack_quant_core::wire::date")] Date, T);

/// Dated amounts as `{ date, amount }` objects.
fn dated_flows_wire(flows: Vec<(Date, Money)>) -> Result<JsValue, JsValue> {
    to_js_value(
        &flows
            .into_iter()
            .map(|(date, amount)| DatedFlowJson { date, amount })
            .collect::<Vec<_>>(),
    )
}

/// Read `{ date, amount }` objects as dated amounts.
pub(crate) fn dated_flows_arg(value: &JsValue, label: &str) -> Result<Vec<(Date, Money)>, JsValue> {
    Ok(js_wire::<Vec<DatedFlowJson>>(value, label)?
        .into_iter()
        .map(|flow| (flow.date, flow.amount))
        .collect())
}

/// Canonical cashflow schedule: classified dated flows with their notional,
/// day count and metadata.
///
/// Obtain one from `CashFlowSchedule.builder()`, `buildCashflowSchedule`,
/// `CashFlowSchedule.fromJson` or `CashFlowSchedule.fromParts`. Methods that
/// return a schedule return a new handle and leave this one unchanged.
#[wasm_bindgen(js_name = CashFlowSchedule)]
#[derive(Clone, Debug)]
pub struct JsCashFlowSchedule {
    pub(crate) inner: CashFlowSchedule,
}

impl JsCashFlowSchedule {
    pub(crate) fn from_inner(inner: CashFlowSchedule) -> Self {
        Self { inner }
    }
}

#[wasm_bindgen(js_class = CashFlowSchedule)]
impl JsCashFlowSchedule {
    /// Start an empty schedule builder.
    ///
    /// @returns A `CashFlowBuilder`; set the principal first, then add coupons, fees and principal events.
    #[wasm_bindgen]
    pub fn builder() -> JsCashFlowBuilder {
        JsCashFlowBuilder {
            inner: CashFlowBuilder::default(),
        }
    }

    /// Assemble a schedule from existing flows, notional, day count and metadata.
    ///
    /// Flows are put into the canonical schedule order; nothing is validated
    /// until `validate()` or an analytic runs.
    ///
    /// @param flows - `CashFlow` wire objects.
    /// @param notional - `Notional` wire object: initial balance and amortization rule.
    /// @param day_count - `DayCount` wire string (for example `"act_360"`) used for accrual and year fractions.
    /// @param meta - `CashFlowMeta` wire object: representation, calendar ids, commitment, issue date and maturity.
    /// @returns The assembled `CashFlowSchedule`.
    /// @throws If an argument does not match its wire type (kind `validation`).
    #[wasm_bindgen(js_name = fromParts)]
    pub fn from_parts(
        flows: JsValue,
        notional: JsValue,
        day_count: JsValue,
        meta: JsValue,
    ) -> Result<JsCashFlowSchedule, JsValue> {
        Ok(Self::from_inner(CashFlowSchedule::from_parts(
            js_wire::<Vec<CashFlow>>(&flows, "flows")?,
            js_wire::<Notional>(&notional, "notional")?,
            js_wire::<DayCount>(&day_count, "dayCount")?,
            js_wire::<CashFlowMeta>(&meta, "meta")?,
        )))
    }

    /// Assemble a schedule from existing flows, with default metadata unless given.
    ///
    /// Same as `fromParts` with `meta` optional. (Python's `from_flows` also
    /// reads a pandas DataFrame; WASM takes the flow list.)
    ///
    /// @param flows - `CashFlow` wire objects.
    /// @param notional - `Notional` wire object: initial balance and amortization rule.
    /// @param day_count - `DayCount` wire string used for accrual and year fractions.
    /// @param meta - Optional `CashFlowMeta` wire object; omitted means the default metadata (contractual representation, no issue date).
    /// @returns The assembled `CashFlowSchedule`.
    /// @throws If an argument does not match its wire type (kind `validation`).
    #[wasm_bindgen(js_name = fromFlows)]
    pub fn from_flows(
        flows: JsValue,
        notional: JsValue,
        day_count: JsValue,
        meta: Option<JsValue>,
    ) -> Result<JsCashFlowSchedule, JsValue> {
        Ok(Self::from_inner(CashFlowSchedule::from_parts(
            js_wire::<Vec<CashFlow>>(&flows, "flows")?,
            js_wire::<Notional>(&notional, "notional")?,
            js_wire::<DayCount>(&day_count, "dayCount")?,
            js_opt_wire::<CashFlowMeta>(meta.as_ref(), "meta")?.unwrap_or_default(),
        )))
    }

    /// Parse a schedule from its canonical JSON.
    ///
    /// @param json - `CashFlowSchedule` JSON string or plain object; unknown fields are rejected.
    /// @returns The parsed `CashFlowSchedule`; it is not validated until `validate()` or an analytic runs.
    /// @throws If `json` is neither a string nor a plain object (kind `invalid_type`) or does not match the schedule schema (kind `validation`).
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<JsCashFlowSchedule, JsValue> {
        serde_json::from_str(&json_text(&json, "json")?)
            .map(Self::from_inner)
            .map_err(to_js_err)
    }

    /// Serialize the schedule to canonical JSON.
    ///
    /// @returns `CashFlowSchedule` JSON accepted by `fromJson` and by every `scheduleJson` argument.
    /// @throws If the schedule cannot be serialized.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.inner).map_err(to_js_err)
    }

    /// Copy of the schedule tagged with a different representation.
    ///
    /// @param representation - `"contractual"`, `"projected"`, `"placeholder"` or `"no_residual"`: what the flows mean to pricing and waterfall policy.
    /// @returns A new `CashFlowSchedule` with `meta.representation` replaced.
    /// @throws If `representation` is not one of the listed strings (kind `validation`).
    #[wasm_bindgen(js_name = withRepresentation)]
    pub fn with_representation(
        &self,
        representation: JsValue,
    ) -> Result<JsCashFlowSchedule, JsValue> {
        Ok(Self::from_inner(self.inner.clone().with_representation(
            js_wire::<CashflowRepresentation>(&representation, "representation")?,
        )))
    }

    /// Copy of the schedule with a different notional.
    ///
    /// @param notional - `Notional` wire object replacing the schedule's initial balance and amortization rule.
    /// @returns A new `CashFlowSchedule`; flows are unchanged.
    /// @throws If `notional` is not a `Notional` (kind `validation`).
    #[wasm_bindgen(js_name = withNotional)]
    pub fn with_notional(&self, notional: JsValue) -> Result<JsCashFlowSchedule, JsValue> {
        Ok(Self::from_inner(self.inner.clone().with_notional(
            js_wire::<Notional>(&notional, "notional")?,
        )))
    }

    /// All flows in canonical schedule order.
    ///
    /// @returns `CashFlow` wire objects: coupons, fees, principal and state rows.
    /// @throws If the flows cannot be serialized.
    #[wasm_bindgen(js_name = getFlows)]
    pub fn get_flows(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.get_flows())
    }

    /// Coupon flows only (fixed, floating, inflation and stub interest).
    ///
    /// @returns `CashFlow` wire objects of interest-like kinds, in schedule order.
    /// @throws If the flows cannot be serialized.
    #[wasm_bindgen]
    pub fn coupons(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.coupons().collect::<Vec<_>>())
    }

    /// Unique flow dates in ascending order.
    ///
    /// @returns ISO-8601 dates on which at least one flow falls.
    /// @throws If the dates cannot be serialized.
    #[wasm_bindgen]
    pub fn dates(&self) -> Result<JsValue, JsValue> {
        to_js_value(
            &self
                .inner
                .dates()
                .into_iter()
                .map(date_to_iso)
                .collect::<Vec<_>>(),
        )
    }

    /// Notional of the schedule.
    ///
    /// @returns `Notional` wire object: initial balance and amortization rule.
    /// @throws If the notional cannot be serialized.
    #[wasm_bindgen(js_name = getNotional)]
    pub fn get_notional(&self) -> Result<JsValue, JsValue> {
        to_js_value(self.inner.get_notional())
    }

    /// Day count attached to the schedule.
    ///
    /// @returns `DayCount` wire string such as `"act_360"`.
    /// @throws If the day count cannot be serialized.
    #[wasm_bindgen(js_name = getDayCount)]
    pub fn get_day_count(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.get_day_count())
    }

    /// Schedule-level metadata.
    ///
    /// @returns `CashFlowMeta` wire object: representation, calendar ids, commitment, issue date, maturity and projected fixings.
    /// @throws If the metadata cannot be serialized.
    #[wasm_bindgen(js_name = getMeta)]
    pub fn get_meta(&self) -> Result<JsValue, JsValue> {
        to_js_value(self.inner.get_meta())
    }

    /// Check the schedule's invariants.
    ///
    /// @throws If the notional or a flow is invalid, flow dates are out of order, or funding, outstanding balances, currencies and the schedule metadata do not reconcile (kind `validation`).
    #[wasm_bindgen]
    pub fn validate(&self) -> Result<(), JsValue> {
        self.inner.validate().map_err(to_js_err)
    }

    /// Copy of the schedule with every flow amount multiplied by a factor.
    ///
    /// Scales principal, interest, fee and recovery amounts; the
    /// representative notional, flow kinds and dates are unchanged.
    ///
    /// @param scale - Finite multiplier applied to every flow amount (for example a position quantity; a negative value reverses the cashflow direction).
    /// @returns A new, scaled `CashFlowSchedule`.
    /// @throws If `scale` is not a number (kind `invalid_type`) or is NaN or infinite (kind `validation`).
    #[wasm_bindgen(js_name = scaleAmounts)]
    pub fn scale_amounts(&self, scale: JsValue) -> Result<JsCashFlowSchedule, JsValue> {
        self.inner
            .clone()
            .scale_amounts(js_f64(&scale, "scale")?)
            .map(Self::from_inner)
            .map_err(to_js_err)
    }

    /// Weighted average life in years from a date.
    ///
    /// WAL over the positive principal flows (amortization, notional and
    /// prepayment) dated after `asOf`, with time measured as actual days / 365
    /// (the SIFMA convention), not the schedule's accrual day count.
    ///
    /// @param as_of - ISO-8601 measurement date; only principal flows strictly after it count.
    /// @returns WAL in years; `0` when no principal flow falls after `asOf`.
    /// @throws If `asOf` is not an ISO-8601 string (kind `invalid_type` or `validation`) or a year fraction cannot be computed.
    #[wasm_bindgen]
    pub fn wal(&self, as_of: JsValue) -> Result<f64, JsValue> {
        self.inner.wal(js_date(&as_of, "asOf")?).map_err(to_js_err)
    }

    /// Outstanding principal balance after each unique balance date.
    ///
    /// Principal flows (amortization, PIK, draws and repayments) are replayed
    /// from the initial notional.
    ///
    /// @returns `{ date, amount }` entries in date order; `amount` is the outstanding balance after that date's flows.
    /// @throws If `meta.issue_date` is unset or principal flows mix currencies (kind `validation`).
    #[wasm_bindgen(js_name = outstandingByDate)]
    pub fn outstanding_by_date(&self) -> Result<JsValue, JsValue> {
        dated_flows_wire(self.inner.outstanding_by_date().map_err(to_js_err)?)
    }

    /// Present value of the flows grouped by reporting period and currency.
    ///
    /// Each flow is discounted on the named curve from `base`; with a credit
    /// curve the flows are also weighted by survival and recovery.
    ///
    /// @param periods - `Period` wire objects (`{ id, start, end, is_actual }`) defining the reporting buckets.
    /// @param market - `MarketContext` handle holding the discount (and credit) curve.
    /// @param disc_curve_id - Identifier of the discount curve in `market`, for example `"USD-OIS"`.
    /// @param base - ISO-8601 valuation date that discounting starts from.
    /// @param day_count - Optional `DayCount` wire string for discounting year fractions; omitted means `"act_365f"`.
    /// @param credit_curve_id - Optional identifier of a hazard curve in `market`; omitted means no credit adjustment.
    /// @returns `PeriodAggregation`: `{ periodId: { currency: Money } }` of PV per period and currency.
    /// @throws If a curve is missing from `market` (kind `not_found`), or an argument is malformed or a discount factor cannot be computed (kind `invalid_type` or `validation`).
    #[wasm_bindgen(js_name = pvByPeriod)]
    pub fn pv_by_period(
        &self,
        periods: JsValue,
        market: &JsMarketContext,
        disc_curve_id: JsValue,
        base: JsValue,
        day_count: Option<JsValue>,
        credit_curve_id: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let periods = js_wire::<Vec<Period>>(&periods, "periods")?;
        let disc_curve_id = CurveId::from(js_string(&disc_curve_id, "discCurveId")?.as_str());
        let credit_curve_id =
            crate::utils::input::js_opt_string(credit_curve_id.as_ref(), "creditCurveId")?
                .map(|id| CurveId::from(id.as_str()));
        let aggregation = self
            .inner
            .pv_by_period(
                &periods,
                market.inner(),
                &disc_curve_id,
                credit_curve_id.as_ref(),
                js_date(&base, "base")?,
                js_opt_wire::<DayCount>(day_count.as_ref(), "dayCount")?,
            )
            .map_err(to_js_err)?;
        to_js_value(&aggregation)
    }

    /// Calendar-year non-principal / principal / PV ladder.
    ///
    /// @param pvs - Present value of each flow, one per flow in schedule order, in flow-amount units.
    /// @returns `{ year, non_principal, principal, pv }` rows in ascending year order. (Python returns the same rows as a DataFrame.)
    /// @throws If `pvs` is not an array of numbers (kind `invalid_type`), does not have one entry per flow, or holds a non-finite value (kind `validation`).
    #[wasm_bindgen(js_name = calendarYearLadder)]
    pub fn calendar_year_ladder(&self, pvs: JsValue) -> Result<JsValue, JsValue> {
        to_js_value(
            &self
                .inner
                .calendar_year_ladder(&js_f64_seq(&pvs, "pvs")?)
                .map_err(to_js_err)?,
        )
    }
}

/// Fluent builder for a `CashFlowSchedule`.
///
/// Created by `CashFlowSchedule.builder()`. Every setter consumes the builder
/// and returns it, so calls chain; a call on a builder that was already
/// consumed throws. Setter errors are recorded and reported by `build`.
#[wasm_bindgen(js_name = CashFlowBuilder)]
#[derive(Clone, Debug)]
pub struct JsCashFlowBuilder {
    pub(crate) inner: CashFlowBuilder,
}

#[wasm_bindgen(js_class = CashFlowBuilder)]
impl JsCashFlowBuilder {
    /// Set the initial principal and the issue and maturity dates.
    ///
    /// @param initial - `Money` wire object: initial outstanding balance; its currency is the schedule currency.
    /// @param issue_date - ISO-8601 issue date; accrual starts here.
    /// @param maturity - ISO-8601 maturity date; the final principal is repaid here.
    /// @returns The builder, for chaining.
    /// @throws If an argument does not match its wire type (kind `invalid_type` or `validation`).
    #[wasm_bindgen]
    pub fn principal(
        mut self,
        initial: JsValue,
        issue_date: JsValue,
        maturity: JsValue,
    ) -> Result<JsCashFlowBuilder, JsValue> {
        let _ = self.inner.principal(
            js_wire::<Money>(&initial, "initial")?,
            js_date(&issue_date, "issueDate")?,
            js_date(&maturity, "maturity")?,
        );
        Ok(self)
    }

    /// Choose whether the initial funding and final redemption are emitted as flows.
    ///
    /// @param exchange - `"none"` or `"initial_and_final"` (the default when never called).
    /// @returns The builder, for chaining.
    /// @throws If `exchange` is not one of the listed strings (kind `validation`).
    #[wasm_bindgen(js_name = principalExchange)]
    pub fn principal_exchange(mut self, exchange: JsValue) -> Result<JsCashFlowBuilder, JsValue> {
        let _ = self
            .inner
            .principal_exchange(js_wire(&exchange, "exchange")?);
        Ok(self)
    }

    /// Set the amortization rule of the principal.
    ///
    /// @param spec - `AmortizationSpec` wire value, for example `"none"` or `{ linear_to: { final_notional } }`.
    /// @returns The builder, for chaining.
    /// @throws If `spec` is not an `AmortizationSpec` (kind `validation`).
    #[wasm_bindgen]
    pub fn amortization(mut self, spec: JsValue) -> Result<JsCashFlowBuilder, JsValue> {
        let _ = self.inner.amortization(js_wire(&spec, "spec")?);
        Ok(self)
    }

    /// Add a dated principal event (draw, repayment or other balance change).
    ///
    /// @param date - ISO-8601 economic date on which the outstanding balance changes.
    /// @param payment_date - ISO-8601 cash settlement date of the event.
    /// @param delta - `Money` wire object: change in outstanding balance (positive increases it, negative repays).
    /// @param kind - `CFKind` wire string of the emitted flow, for example `"amortization"` or `"revolving_draw"`.
    /// @param cash - Optional `Money` wire object: cash paid or received when it differs from `delta` (OID, fees); omitted means equal to `delta`.
    /// @returns The builder, for chaining.
    /// @throws If an argument does not match its wire type (kind `invalid_type` or `validation`).
    #[wasm_bindgen(js_name = addPrincipalEvent)]
    pub fn add_principal_event(
        mut self,
        date: JsValue,
        payment_date: JsValue,
        delta: JsValue,
        kind: JsValue,
        cash: Option<JsValue>,
    ) -> Result<JsCashFlowBuilder, JsValue> {
        let _ = self.inner.add_principal_event(
            js_date(&date, "date")?,
            js_date(&payment_date, "paymentDate")?,
            js_wire::<Money>(&delta, "delta")?,
            js_opt_wire::<Money>(cash.as_ref(), "cash")?,
            js_wire::<CFKind>(&kind, "kind")?,
        );
        Ok(self)
    }

    /// Add a fixed-rate coupon leg over the whole life.
    ///
    /// @param spec - `FixedCouponSpec` wire object: decimal `rate`, coupon type and schedule parameters.
    /// @returns The builder, for chaining.
    /// @throws If `spec` is not a `FixedCouponSpec` (kind `validation`).
    #[wasm_bindgen(js_name = fixedCf)]
    pub fn fixed_cf(mut self, spec: JsValue) -> Result<JsCashFlowBuilder, JsValue> {
        let _ = self.inner.fixed_cf(js_wire(&spec, "spec")?);
        Ok(self)
    }

    /// Add a floating-rate coupon leg over the whole life.
    ///
    /// @param spec - `FloatingCouponSpec` wire object: rate specification, coupon type and schedule parameters.
    /// @returns The builder, for chaining.
    /// @throws If `spec` is not a `FloatingCouponSpec` (kind `validation`).
    #[wasm_bindgen(js_name = floatingCf)]
    pub fn floating_cf(mut self, spec: JsValue) -> Result<JsCashFlowBuilder, JsValue> {
        let _ = self.inner.floating_cf(js_wire(&spec, "spec")?);
        Ok(self)
    }

    /// Add a fixed coupon leg whose rate steps up on listed dates.
    ///
    /// @param spec - `StepUpCouponSpec` wire object: initial decimal rate, dated rate steps, coupon type and schedule parameters.
    /// @returns The builder, for chaining.
    /// @throws If `spec` is not a `StepUpCouponSpec` (kind `validation`).
    #[wasm_bindgen(js_name = stepUpCf)]
    pub fn step_up_cf(mut self, spec: JsValue) -> Result<JsCashFlowBuilder, JsValue> {
        let _ = self.inner.step_up_cf(js_wire(&spec, "spec")?);
        Ok(self)
    }

    /// Add a one-off or recurring fee.
    ///
    /// @param spec - `FeeSpec` wire value: `{ fixed: { date, amount } }` or `{ periodic_bp: { ... } }`.
    /// @returns The builder, for chaining.
    /// @throws If `spec` is not a `FeeSpec` (kind `validation`).
    #[wasm_bindgen]
    pub fn fee(mut self, spec: JsValue) -> Result<JsCashFlowBuilder, JsValue> {
        let _ = self.inner.fee(js_wire(&spec, "spec")?);
        Ok(self)
    }

    /// Add a fixed coupon leg that applies only inside a date window.
    ///
    /// @param start - ISO-8601 start of the window (inclusive).
    /// @param end - ISO-8601 end of the window (exclusive).
    /// @param spec - `FixedCouponSpec` wire object for the window.
    /// @returns The builder, for chaining.
    /// @throws If an argument does not match its wire type (kind `invalid_type` or `validation`).
    #[wasm_bindgen(js_name = addFixedWindow)]
    pub fn add_fixed_window(
        mut self,
        start: JsValue,
        end: JsValue,
        spec: JsValue,
    ) -> Result<JsCashFlowBuilder, JsValue> {
        let _ = self.inner.add_fixed_window(
            js_date(&start, "start")?,
            js_date(&end, "end")?,
            js_wire(&spec, "spec")?,
        );
        Ok(self)
    }

    /// Add a floating coupon leg that applies only inside a date window.
    ///
    /// @param start - ISO-8601 start of the window (inclusive).
    /// @param end - ISO-8601 end of the window (exclusive).
    /// @param spec - `FloatingCouponSpec` wire object for the window.
    /// @returns The builder, for chaining.
    /// @throws If an argument does not match its wire type (kind `invalid_type` or `validation`).
    #[wasm_bindgen(js_name = addFloatingWindow)]
    pub fn add_floating_window(
        mut self,
        start: JsValue,
        end: JsValue,
        spec: JsValue,
    ) -> Result<JsCashFlowBuilder, JsValue> {
        let _ = self.inner.add_floating_window(
            js_date(&start, "start")?,
            js_date(&end, "end")?,
            js_wire(&spec, "spec")?,
        );
        Ok(self)
    }

    /// Set the cash/PIK split of coupons paid inside a date window.
    ///
    /// @param start - ISO-8601 start of the window (inclusive).
    /// @param end - ISO-8601 end of the window (exclusive).
    /// @param split - `CouponType` wire value: `"cash"`, `"pik"` or `{ split: { cash_fraction, pik_fraction } }`.
    /// @returns The builder, for chaining.
    /// @throws If an argument does not match its wire type (kind `invalid_type` or `validation`).
    #[wasm_bindgen(js_name = addPaymentWindow)]
    pub fn add_payment_window(
        mut self,
        start: JsValue,
        end: JsValue,
        split: JsValue,
    ) -> Result<JsCashFlowBuilder, JsValue> {
        let _ = self.inner.add_payment_window(
            js_date(&start, "start")?,
            js_date(&end, "end")?,
            js_wire::<CouponType>(&split, "split")?,
        );
        Ok(self)
    }

    /// Set a sequence of cash/PIK splits, each applying until its end date.
    ///
    /// @param steps - `[isoDate, CouponType]` pairs ordered by date: each split applies up to (not including) its date; periods after the last step pay cash.
    /// @returns The builder, for chaining.
    /// @throws If `steps` is not an array of `[isoDate, CouponType]` pairs (kind `validation`).
    #[wasm_bindgen(js_name = paymentSplitProgram)]
    pub fn payment_split_program(mut self, steps: JsValue) -> Result<JsCashFlowBuilder, JsValue> {
        let steps: Vec<(Date, CouponType)> =
            js_wire::<Vec<DatedPair<CouponType>>>(&steps, "steps")?
                .into_iter()
                .map(|DatedPair(date, split)| (date, split))
                .collect();
        let _ = self.inner.payment_split_program(&steps);
        Ok(self)
    }

    /// Pay a fixed coupon until a switch date, then a floating coupon.
    ///
    /// @param switch_date - ISO-8601 date on which the leg switches from fixed to floating (Rust and Python name it `switch`, a reserved word in JavaScript).
    /// @param fixed - `FixedCouponSpec` wire object used before the switch.
    /// @param floating - `FloatingCouponSpec` wire object used from the switch.
    /// @returns The builder, for chaining.
    /// @throws If an argument does not match its wire type (kind `invalid_type` or `validation`).
    #[wasm_bindgen(js_name = fixedToFloat)]
    pub fn fixed_to_float(
        mut self,
        switch_date: JsValue,
        fixed: JsValue,
        floating: JsValue,
    ) -> Result<JsCashFlowBuilder, JsValue> {
        let _ = self.inner.fixed_to_float(
            js_date(&switch_date, "switchDate")?,
            js_wire::<FixedCouponSpec>(&fixed, "fixed")?,
            js_wire::<FloatingCouponSpec>(&floating, "floating")?,
        );
        Ok(self)
    }

    /// Add a floating leg whose margin steps on listed dates.
    ///
    /// @param steps - `[isoDate, decimalString]` pairs with strictly increasing dates: the spread in basis points in force from each date.
    /// @param base_spec - `FloatingCouponSpec` wire object supplying index, schedule and conventions; its own spread applies before the first step.
    /// @returns The builder, for chaining.
    /// @throws If an argument does not match its wire type (kind `validation`).
    #[wasm_bindgen(js_name = floatMarginSteps)]
    pub fn float_margin_steps(
        mut self,
        steps: JsValue,
        base_spec: JsValue,
    ) -> Result<JsCashFlowBuilder, JsValue> {
        let steps: Vec<_> = js_wire::<Vec<DatedPair<DecimalWire>>>(&steps, "steps")?
            .into_iter()
            .map(|DatedPair(date, margin)| (date, margin.0))
            .collect();
        let _ = self
            .inner
            .float_margin_steps(&steps, js_wire(&base_spec, "baseSpec")?);
        Ok(self)
    }

    /// Build the schedule, projecting floating coupons from market curves when a market is given.
    ///
    /// The builder is not consumed and can be built again.
    ///
    /// This export is the no-market call; the facade routes a supplied market to `buildWithMarket`.
    ///
    /// @param market - Optional `MarketContext` handle holding the forward curves and fixings the floating legs reference; omitted means floating coupons that need a forward curve use their fallback policy.
    /// @returns The built `CashFlowSchedule` handle; release it with free().
    /// @throws If a setter recorded an error, the principal is unset, a referenced curve or fixing is missing (kind `not_found`), or schedule generation fails (kind `validation`).
    #[wasm_bindgen]
    pub fn build(&self) -> Result<JsCashFlowSchedule, JsValue> {
        self.inner
            .build(None)
            .map(JsCashFlowSchedule::from_inner)
            .map_err(to_js_err)
    }

    /// Build the schedule with market data; the published `build(market)` calls this.
    ///
    /// @param market - `MarketContext` handle holding the forward curves and fixings the floating legs reference.
    /// @returns The built `CashFlowSchedule` handle; release it with free().
    /// @throws If a setter recorded an error, the principal is unset, a referenced curve or fixing is missing (kind `not_found`), or schedule generation fails (kind `validation`).
    #[wasm_bindgen(js_name = buildWithMarket)]
    pub fn build_with_market(
        &self,
        market: &JsMarketContext,
    ) -> Result<JsCashFlowSchedule, JsValue> {
        self.inner
            .build(Some(market.inner()))
            .map(JsCashFlowSchedule::from_inner)
            .map_err(to_js_err)
    }
}

/// Build a cashflow schedule from a build spec.
///
/// Typed twin of `buildCashflowScheduleJson`: the same spec, returning a
/// `CashFlowSchedule` handle.
///
/// This export is the no-market call; the facade routes a supplied market to `buildCashflowScheduleWithMarket`.
///
/// @param spec - `CashflowScheduleBuildSpec` wire object or JSON: notional, issue and maturity dates, coupon program, fees and principal events.
/// @param market - Optional `MarketContext` handle holding the forward curves and fixings the floating legs reference; omitted means floating coupons that need a forward curve use their fallback policy.
/// @returns The built `CashFlowSchedule` handle; release it with free().
/// @throws If `spec` does not match the build-spec schema (kind `invalid_type` or `validation`), a referenced curve or fixing is missing (kind `not_found`), or schedule construction fails (kind `validation`).
#[wasm_bindgen(js_name = buildCashflowSchedule)]
pub fn build_cashflow_schedule(spec: JsValue) -> Result<JsCashFlowSchedule, JsValue> {
    crate::utils::input::from_js_json::<CashflowScheduleBuildSpec>(&spec, "spec")?
        .build(None)
        .map(JsCashFlowSchedule::from_inner)
        .map_err(to_js_err)
}

/// Build a cashflow schedule from a build spec with market data; the published
/// `buildCashflowSchedule(spec, market)` calls this.
///
/// @param spec - `CashflowScheduleBuildSpec` wire object or JSON.
/// @param market - `MarketContext` handle holding the forward curves and fixings the floating legs reference.
/// @returns The built `CashFlowSchedule` handle.
/// @throws If `spec` does not match the build-spec schema, a referenced curve or fixing is missing (kind `not_found`), or schedule construction fails (kind `validation`).
#[wasm_bindgen(js_name = buildCashflowScheduleWithMarket)]
pub fn build_cashflow_schedule_with_market(
    spec: JsValue,
    market: &JsMarketContext,
) -> Result<JsCashFlowSchedule, JsValue> {
    crate::utils::input::from_js_json::<CashflowScheduleBuildSpec>(&spec, "spec")?
        .build(Some(market.inner()))
        .map(JsCashFlowSchedule::from_inner)
        .map_err(to_js_err)
}

/// Settlement cash flows of a schedule as dated amounts.
///
/// Validates the schedule and keeps only cash-settling rows (`pik` and
/// `defaulted_notional` state rows are excluded), so the result is safe to
/// sum per currency. Typed twin of `datedFlowsJson`.
///
/// @param schedule - `CashFlowSchedule` handle.
/// @returns `{ date, amount }` entries in schedule order. (Python returns `(date, Money)` tuples.)
/// @throws If the schedule fails validation (kind `validation`).
#[wasm_bindgen(js_name = datedFlows)]
pub fn dated_flows(schedule: &JsCashFlowSchedule) -> Result<JsValue, JsValue> {
    dated_flows_wire(finstack_quant_cashflows::dated_flows(&schedule.inner).map_err(to_js_err)?)
}

/// Build a schedule from dated amounts that all share one cashflow kind.
///
/// @param flows - `{ date, amount }` entries: ISO-8601 date and `Money` amount of each flow.
/// @param kind - `CFKind` wire string stamped on every flow, for example `"fixed"`.
/// @param day_count - `DayCount` wire string attached to the schedule for downstream accrual and yield calculations.
/// @param opts - Optional `ScheduleBuildOpts` wire object (`{ notional_hint?, meta? }`); omitted means a zero notional in the first flow's currency and default metadata.
/// @returns The `CashFlowSchedule` handle, flows in canonical order.
/// @throws If an argument does not match its wire type (kind `validation`).
#[wasm_bindgen(js_name = scheduleFromDatedFlows)]
pub fn schedule_from_dated_flows(
    flows: JsValue,
    kind: JsValue,
    day_count: JsValue,
    opts: Option<JsValue>,
) -> Result<JsCashFlowSchedule, JsValue> {
    Ok(JsCashFlowSchedule::from_inner(
        finstack_quant_cashflows::schedule_from_dated_flows(
            dated_flows_arg(&flows, "flows")?,
            js_wire::<CFKind>(&kind, "kind")?,
            js_wire::<DayCount>(&day_count, "dayCount")?,
            js_opt_wire::<ScheduleBuildOpts>(opts.as_ref(), "opts")?.unwrap_or_default(),
        ),
    ))
}

/// Build a schedule from flows that already carry their cashflow kind.
///
/// @param flows - `CashFlow` wire objects; each flow's kind is kept as is.
/// @param day_count - `DayCount` wire string attached to the schedule for downstream accrual and yield calculations.
/// @param opts - Optional `ScheduleBuildOpts` wire object (`{ notional_hint?, meta? }`); omitted means a zero notional in the first flow's currency and default metadata.
/// @returns The `CashFlowSchedule` handle, flows in canonical order.
/// @throws If an argument does not match its wire type (kind `validation`).
#[wasm_bindgen(js_name = scheduleFromClassifiedFlows)]
pub fn schedule_from_classified_flows(
    flows: JsValue,
    day_count: JsValue,
    opts: Option<JsValue>,
) -> Result<JsCashFlowSchedule, JsValue> {
    Ok(JsCashFlowSchedule::from_inner(
        finstack_quant_cashflows::schedule_from_classified_flows(
            js_wire::<Vec<CashFlow>>(&flows, "flows")?,
            js_wire::<DayCount>(&day_count, "dayCount")?,
            js_opt_wire::<ScheduleBuildOpts>(opts.as_ref(), "opts")?.unwrap_or_default(),
        ),
    ))
}

/// Parse an array of schedules given as canonical JSON or plain objects.
///
/// The facade turns `CashFlowSchedule` handles into JSON before calling: a
/// wasm-bindgen parameter cannot borrow an array of handles.
pub(crate) fn schedules_arg(
    value: &JsValue,
    label: &str,
) -> Result<Vec<CashFlowSchedule>, JsValue> {
    if !js_sys::Array::is_array(value) {
        return Err(crate::utils::input::invalid_type(
            label,
            "expected an array of CashFlowSchedule handles or schedule JSON",
        ));
    }
    js_sys::Array::from(value)
        .iter()
        .map(|item| crate::utils::input::from_js_json(&item, label))
        .collect()
}

/// Merge several schedules into one under a new notional and day count.
///
/// Flows are concatenated and put into canonical order; calendar ids and
/// projected fixings are merged.
///
/// @param schedules - Schedules to merge: `CashFlowSchedule` handles (the facade passes them as JSON), canonical JSON strings or plain objects.
/// @param notional - `Notional` wire object of the merged schedule.
/// @param day_count - `DayCount` wire string of the merged schedule.
/// @returns The merged `CashFlowSchedule` handle.
/// @throws If `schedules` is not an array (kind `invalid_type`) or an argument does not match its wire type (kind `validation`).
#[wasm_bindgen(js_name = mergeCashflowSchedules)]
pub fn merge_cashflow_schedules_js(
    schedules: JsValue,
    notional: JsValue,
    day_count: JsValue,
) -> Result<JsCashFlowSchedule, JsValue> {
    Ok(JsCashFlowSchedule::from_inner(merge_cashflow_schedules(
        schedules_arg(&schedules, "schedules")?,
        js_wire::<Notional>(&notional, "notional")?,
        js_wire::<DayCount>(&day_count, "dayCount")?,
    )))
}
