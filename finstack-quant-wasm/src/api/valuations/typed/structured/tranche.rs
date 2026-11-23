//! Typed `TrancheBuilder` and `StructuredCreditTranche` WASM bindings.

use super::deal::JsStructuredCredit;

use crate::api::core::dates::{JsDayCount, JsTenor};
use crate::api::core::money::JsMoney;
use crate::utils::input::js_f64;
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_cashflows::builder::RateSpec;
use wasm_bindgen::prelude::*;

builder_class!(
    @staged
    /// Fluent builder for a `Tranche`; one setter per Rust `TrancheBuilder` setter.
    ///
    /// Every setter returns the builder, so calls chain; `build()` validates and
    /// consumes it. Create one with `valuations.instruments.trancheBuilder()`.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const builder = valuations.instruments.trancheBuilder();
    /// console.log(typeof builder.build);
    /// builder.free();
    /// ```
    JsTrancheBuilder,
    "TrancheBuilder",
    finstack_quant_valuations::instruments::fixed_income::structured_credit::TrancheBuilder,
    finstack_quant_valuations::instruments::fixed_income::structured_credit::Tranche::builder()
);

#[wasm_bindgen(js_class = TrancheBuilder)]
impl JsTrancheBuilder {
    /// Set a fixed coupon.
    /// @param rate - Annual coupon as a decimal (`0.05` = 5%).
    /// @returns The builder, for chaining.
    /// @throws Error - Throws with kind `invalid_type` if `rate` is not a number, and kind `validation` if the builder was already consumed by `build()`.
    #[wasm_bindgen(js_name = couponFixed)]
    pub fn coupon_fixed(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] rate: JsValue,
    ) -> Result<JsTrancheBuilder, JsValue> {
        let rate = js_f64(&rate, "rate")?;
        let inner = self.inner.apply(|b| b.coupon(RateSpec::Fixed { rate }))?;
        Ok(Self { inner })
    }

    /// Validate the staged fields and build the tranche.
    ///
    /// Runs the Rust `TrancheBuilder::build` validation and consumes the
    /// builder. Attachment and detachment points may both be omitted, in
    /// which case `TrancheStructure` derives them from the balances.
    /// @returns The validated `Tranche` plain object.
    /// @throws Error - Throws with kind `validation` if the builder was already consumed, a required field is missing, only one of the attachment and detachment points is set, or validation fails.
    #[wasm_bindgen(unchecked_return_type = "generated.valuations.Tranche")]
    pub fn build(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.take()?.build().map_err(to_js_err)?)
    }
}

use finstack_quant_valuations::instruments::fixed_income::structured_credit::StructuredCreditTranche;

setters!(JsTrancheBuilder, "TrancheBuilder", {
        /// Set the tranche identifier.
        /// @param value - Unique identifier for the tranche.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        id as id: "string" => js_string,
        /// Set the attachment point.
        /// @param value - Attachment point quoted in percent on a 0-100 scale (e.g. `0.0` for equity, `10.0` for a tranche attaching at 10%).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        attach_pct as attachPct: "number" => js_f64,
        /// Set the detachment point.
        /// @param value - Detachment point quoted in percent on a 0-100 scale (e.g. `100.0` for the most senior tranche).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        detach_pct as detachPct: "number" => js_f64,
        /// Set the tranche seniority.
        /// @param value - Structural seniority of the tranche.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        seniority as seniority: "generated.valuations.Tranche['seniority']" => js_wire,
        /// Set a floating-rate coupon from a JSON `RateSpec::Floating` payload.
        /// @param value - JSON-encoded, externally-tagged `RateSpec` value, e.g. `{"floating": {...FloatingRateSpec fields...}}`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        coupon_floating as couponFloating: "generated.valuations.Tranche['coupon']" => from_js_json via coupon,
        /// Set the legal final maturity date.
        /// @param value - Legal final maturity date.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        maturity as maturity: "string" => js_date,
        /// Enable payment-in-kind accretion of interest shortfalls.
        /// @param value - `true` capitalizes unpaid interest into the balance; `false` (the default) defers it as a claim.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        pik_enabled as pikEnabled: "boolean" => js_bool,
        /// Mark the coupon non-deferrable or deferrable, overriding the seniority convention.
        /// @param value - `true` for a coupon the template pays from principal proceeds when interest falls short (the senior default); `false` for one that defers (the default for every other class).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        non_deferrable as nonDeferrable: "boolean" => js_bool,
        /// Set the credit rating.
        /// @param value - Rating string (`"AAA"`, `"BBB"`, `"NR"` ...).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        rating as rating: "NonNullable<generated.valuations.Tranche['rating']>" => js_wire,
        /// Attach a per-tranche overcollateralization trigger.
        /// @param value - `CoverageTrigger` serde object: `trigger_level` (ratio), optional `cure_level`, `consequence` and breach memory fields.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        oc_trigger as ocTrigger: "NonNullable<generated.valuations.Tranche['oc_trigger']>" => from_js_json,
        /// Attach a per-tranche interest-coverage trigger.
        /// @param value - `CoverageTrigger` serde object (see `oc_trigger`).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        ic_trigger as icTrigger: "NonNullable<generated.valuations.Tranche['ic_trigger']>" => from_js_json,
        /// Set free-form attributes (tags and metadata).
        /// @param value - Attribute bag; a plain object populates `meta` (an optional `"tags"` list populates `tags`).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        attributes as attributes: "generated.valuations.Tranche['attributes']" => from_js_json,
});

handle_setters!(JsTrancheBuilder, "TrancheBuilder", {
        /// Set the original tranche balance.
        /// @param value - Original tranche balance, as a `Money` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        original_balance as originalBalance => JsMoney via balance,
        /// Set the payment frequency.
        /// @param value - Payment frequency, as a `Tenor` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        frequency as frequency => JsTenor,
        /// Set the day count convention for interest accrual.
        /// @param value - Day count convention for interest accrual, as a `DayCount` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        day_count as dayCount => JsDayCount,
        /// Set the current (factored) balance.
        /// @param value - Current (factored) balance, as a `Money` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        current_balance as currentBalance => JsMoney,
        /// Set interest already deferred (unpaid, still owed) at closing.
        /// @param value - Interest already deferred (unpaid, still owed) at closing, as a `Money` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        deferred_interest as deferredInterest => JsMoney,
});

instrument_class!(
    /// A standalone note priced against its original deal waterfall.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const note = valuations.instruments.StructuredCreditTranche.example();
    /// console.log(note.trancheId);
    /// note.free();
    /// ```
    JsStructuredCreditTranche, "StructuredCreditTranche", StructuredCreditTranche
);
factories!(JsStructuredCreditTranche, "StructuredCreditTranche", StructuredCreditTranche, {
    /// Construct a note from the canonical structured-credit example.
    /// @returns Standalone note with its originating waterfall and collateral.
    /// @throws FinstackError if the sample deal or its note is invalid.
    example as example,
});
instrument_pricing!(JsStructuredCreditTranche, "StructuredCreditTranche");
getters!(JsStructuredCreditTranche, "StructuredCreditTranche", |note| {
    /// Read the selected note identifier within the original deal.
    /// @returns Contract identifier stored on the deal's tranche structure.
    tranche_id as trancheId: "string" => text(note.tranche_id()),
    /// Registered model used by the note when no model is selected.
    /// @returns The deterministic discounting model.
    default_model as defaultModel: "string" => text(finstack_quant_valuations::instruments::Instrument::default_model(note)),
    /// Read the inherited deal attributes used for scenario selection.
    /// @returns The attribute bag of the originating deal.
    attributes as attributes: "generated.valuations.Attributes" => json(finstack_quant_valuations::instruments::Instrument::attributes(note)),
});

#[wasm_bindgen(js_class = StructuredCreditTranche)]
impl JsStructuredCreditTranche {
    /// Output the bare specification of the selected note.
    /// @returns Standalone instrument ID, selected tranche ID and complete deal specification.
    /// @throws FinstackError if the specification cannot be serialized.
    #[wasm_bindgen(js_name = toDict, unchecked_return_type = "generated.valuations.StructuredCreditTranche")]
    pub fn to_dict(&self) -> Result<JsValue, JsValue> {
        crate::utils::to_js_value(&self.inner)
    }

    /// Report the market data used by the selected note's waterfall and discounting.
    /// @returns Active pricing curves, fixings and other market identifiers.
    /// @throws FinstackError if the originating deal cannot resolve dependencies.
    #[wasm_bindgen(js_name = marketDependencies, unchecked_return_type = "generated.valuations.MarketDependencies")]
    pub fn market_dependencies(&self) -> Result<JsValue, JsValue> {
        super::super::market_dependencies(&self.inner)
    }

    /// Select one existing note as a priceable instrument.
    /// @param deal - Originating deal, with its full collateral and waterfall structure.
    /// @param trancheId - Exact identifier of a note within the deal.
    /// @returns A standalone note with stable deal and note identity.
    /// @throws FinstackError if the deal or note is invalid.
    #[wasm_bindgen(js_name = fromDeal)]
    pub fn from_deal(
        deal: &JsStructuredCredit,
        #[wasm_bindgen(js_name = "trancheId", unchecked_param_type = "string")] tranche_id: JsValue,
    ) -> Result<JsStructuredCreditTranche, JsValue> {
        let tranche_id = crate::utils::input::js_string(&tranche_id, "trancheId")?;
        StructuredCreditTranche::from_deal(deal.inner.clone(), &tranche_id)
            .map(|inner| Self { inner })
            .map_err(crate::utils::to_js_err)
    }
}
