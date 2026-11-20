//! A cross-currency swap exchanging flows in two currencies.

use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::rates::xccy_swap::{XccySwap, XccySwapBuilder};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// A cross-currency swap exchanging flows in two currencies.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.XccySwap.example();
    /// console.log(future.id);
    /// future.free();
    /// ```
    JsXccySwap, "XccySwap", XccySwap
);
instrument_surface!(
    JsXccySwap,
    "XccySwap",
    JsXccySwapBuilder,
    "generated.valuations.XccySwap"
);
factories!(JsXccySwap, "XccySwap", XccySwap, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsXccySwap, "XccySwap", |i| {
    /// Effective expiry of the contract.
    /// @returns The ISO expiry, or null if the instrument defines none.
    expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's leg1.
    /// @returns First leg.
    leg1 as leg1: "generated.valuations.XccySwap['leg1']" => json(i.leg1),
    /// Read the contract's leg2.
    /// @returns Second leg.
    leg2 as leg2: "generated.valuations.XccySwap['leg2']" => json(i.leg2),
    /// Read the contract's notional exchange.
    /// @returns Whether and when principal is exchanged.
    notional_exchange as notionalExchange: "generated.valuations.XccySwap['notional_exchange']" => json(i.notional_exchange),
    /// Read the contract's reporting currency.
    /// @returns PV reporting currency (output currency of value/npv).
    reporting_currency as reportingCurrency: "string" => text(i.reporting_currency),
    /// Read the contract's allow calendar fallback.
    /// @returns Allow a weekends-only calendar when either leg's leg.calendar_id is missing or cannot be resolved. When false (default), missing calendars are treated as input errors.
    allow_calendar_fallback as allowCalendarFallback: "boolean" => json(i.allow_calendar_fallback),
});
common_getters!(JsXccySwap, "XccySwap");

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.XccySwap.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsXccySwapBuilder, "XccySwapBuilder", XccySwapBuilder, JsXccySwap, |b| b.build()
);

#[wasm_bindgen(js_class = XccySwap)]
impl JsXccySwap {
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    #[wasm_bindgen(js_name = toBuilder)]
    pub fn to_builder(&self) -> JsXccySwapBuilder {
        JsXccySwapBuilder {
            inner: super::Staged::new(self.inner.to_builder()),
        }
    }
}

setters!(JsXccySwapBuilder, "XccySwapBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's leg1.
    /// @param value - First leg.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    leg1 as leg1: "generated.valuations.XccySwap['leg1']" => js_wire,
    /// Set the contract's leg2.
    /// @param value - Second leg.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    leg2 as leg2: "generated.valuations.XccySwap['leg2']" => js_wire,
    /// Set the contract's notional exchange.
    /// @param value - Whether and when principal is exchanged.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    notional_exchange as notionalExchange: "generated.valuations.XccySwap['notional_exchange']" => js_wire,
    /// Set the contract's reporting currency.
    /// @param value - PV reporting currency (output currency of value/npv).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    reporting_currency as reportingCurrency: "string" => js_wire,
    /// Set the contract's allow calendar fallback.
    /// @param value - Allow a weekends-only calendar when either leg's leg.calendar_id is missing or cannot be resolved. When false (default), missing calendars are treated as input errors.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    allow_calendar_fallback as allowCalendarFallback: "boolean" => js_wire,
});
common_setters!(JsXccySwapBuilder, "XccySwapBuilder");
