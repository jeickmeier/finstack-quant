//! A basis swap exchanging flows on the specified floating-rate indices.

use crate::api::core::money::JsMoney;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::rates::basis_swap::{BasisSwap, BasisSwapBuilder};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// A basis swap exchanging flows on the specified floating-rate indices.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.BasisSwap.example();
    /// console.log(future.id);
    /// future.free();
    /// ```
    JsBasisSwap, "BasisSwap", BasisSwap
);
instrument_surface!(
    JsBasisSwap,
    "BasisSwap",
    JsBasisSwapBuilder,
    "generated.valuations.BasisSwap"
);
factories!(JsBasisSwap, "BasisSwap", BasisSwap, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsBasisSwap, "BasisSwap", |i| {
    /// Effective expiry of the contract.
    /// @returns The ISO expiry, or null if the instrument defines none.
    expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's notional.
    /// @returns Notional amount for both legs.
    notional as notional: "Money" => money(i.notional),
    /// Read the contract's primary leg.
    /// @returns Primary leg that typically receives the spread.
    primary_leg as primaryLeg: "generated.valuations.BasisSwap['primary_leg']" => json(i.primary_leg),
    /// Read the contract's reference leg.
    /// @returns Reference leg that typically pays flat.
    reference_leg as referenceLeg: "generated.valuations.BasisSwap['reference_leg']" => json(i.reference_leg),
    /// Read the contract's allow calendar fallback.
    /// @returns Allow calendar-day fallback when the calendar cannot be resolved. When false (default), missing calendars are treated as an input error to avoid silently misaligning schedule and payment-lag conventions.
    allow_calendar_fallback as allowCalendarFallback: "boolean" => json(i.allow_calendar_fallback),
    /// Read the contract's allow same curve.
    /// @returns Allow both legs to reference the same forward curve. When false (default), having identical forward curves on both legs produces a validation error, as this is almost always a configuration mistake (NPV would equal spread × annuity by construction). Set to true only for testing or deliberate same-index spread trades.
    allow_same_curve as allowSameCurve: "boolean" => json(i.allow_same_curve),
});
common_getters!(JsBasisSwap, "BasisSwap");

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.BasisSwap.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsBasisSwapBuilder, "BasisSwapBuilder", BasisSwapBuilder, JsBasisSwap, |b| b.build()
);

#[wasm_bindgen(js_class = BasisSwap)]
impl JsBasisSwap {
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    #[wasm_bindgen(js_name = toBuilder)]
    pub fn to_builder(&self) -> JsBasisSwapBuilder {
        JsBasisSwapBuilder {
            inner: super::Staged::new(self.inner.to_builder()),
        }
    }
}

setters!(JsBasisSwapBuilder, "BasisSwapBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's primary leg.
    /// @param value - Primary leg that typically receives the spread.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    primary_leg as primaryLeg: "generated.valuations.BasisSwap['primary_leg']" => js_wire,
    /// Set the contract's reference leg.
    /// @param value - Reference leg that typically pays flat.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    reference_leg as referenceLeg: "generated.valuations.BasisSwap['reference_leg']" => js_wire,
    /// Set the contract's allow calendar fallback.
    /// @param value - Allow calendar-day fallback when the calendar cannot be resolved. When false (default), missing calendars are treated as an input error to avoid silently misaligning schedule and payment-lag conventions.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    allow_calendar_fallback as allowCalendarFallback: "boolean" => js_bool,
    /// Set the contract's allow same curve.
    /// @param value - Allow both legs to reference the same forward curve. When false (default), having identical forward curves on both legs produces a validation error, as this is almost always a configuration mistake (NPV would equal spread × annuity by construction). Set to true only for testing or deliberate same-index spread trades.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    allow_same_curve as allowSameCurve: "boolean" => js_bool,
});
common_setters!(JsBasisSwapBuilder, "BasisSwapBuilder");

handle_setters!(JsBasisSwapBuilder, "BasisSwapBuilder", {
    /// Set the contract's notional.
    /// @param value - Notional amount for both legs.
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    notional as notional => JsMoney,
});
