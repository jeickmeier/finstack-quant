//! A range accrual with explicit issuer call rights.

use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::exotics::callable_range_accrual::{
    CallableRangeAccrual, CallableRangeAccrualBuilder,
};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// A range accrual with explicit issuer call rights.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.CallableRangeAccrual.example();
    /// console.log(future.id);
    /// future.free();
    /// ```
    JsCallableRangeAccrual, "CallableRangeAccrual", CallableRangeAccrual
);
instrument_surface!(
    JsCallableRangeAccrual,
    "CallableRangeAccrual",
    JsCallableRangeAccrualBuilder,
    "generated.valuations.CallableRangeAccrual"
);
factories!(JsCallableRangeAccrual, "CallableRangeAccrual", CallableRangeAccrual, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsCallableRangeAccrual, "CallableRangeAccrual", |i| {
    /// Effective expiry of the contract.
    /// @returns The ISO expiry, or null if the instrument defines none.
    expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's range accrual.
    /// @returns Range accrual contract terms (no identity, attributes or overrides of their own).
    range_accrual as rangeAccrual: "generated.valuations.CallableRangeAccrual['range_accrual']" => json(i.range_accrual),
    /// Read the contract's call provision.
    /// @returns Bermudan call provision.
    call_provision as callProvision: "generated.valuations.CallableRangeAccrual['call_provision']" => json(i.call_provision),
});
common_getters!(JsCallableRangeAccrual, "CallableRangeAccrual");

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.CallableRangeAccrual.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsCallableRangeAccrualBuilder, "CallableRangeAccrualBuilder", CallableRangeAccrualBuilder, JsCallableRangeAccrual, |b| b.build()
);

to_builder_method!(
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    JsCallableRangeAccrual, "CallableRangeAccrual", JsCallableRangeAccrualBuilder
);

setters!(JsCallableRangeAccrualBuilder, "CallableRangeAccrualBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's range accrual.
    /// @param value - Range accrual contract terms (no identity, attributes or overrides of their own).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    range_accrual as rangeAccrual: "generated.valuations.CallableRangeAccrual['range_accrual']" => js_wire,
    /// Set the contract's call provision.
    /// @param value - Bermudan call provision.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    call_provision as callProvision: "generated.valuations.CallableRangeAccrual['call_provision']" => js_wire,
});
common_setters!(JsCallableRangeAccrualBuilder, "CallableRangeAccrualBuilder");
