//! A range accrual paying coupons weighted by eligible observations.

use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::exotics::range_accrual::{
    RangeAccrual, RangeAccrualBuilder,
};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// A range accrual paying coupons weighted by eligible observations.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.RangeAccrual.example();
    /// console.log(future.id);
    /// future.free();
    /// ```
    JsRangeAccrual, "RangeAccrual", RangeAccrual
);
instrument_surface!(
    JsRangeAccrual,
    "RangeAccrual",
    JsRangeAccrualBuilder,
    "generated.valuations.RangeAccrual"
);
factories!(JsRangeAccrual, "RangeAccrual", RangeAccrual, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsRangeAccrual, "RangeAccrual", |i| {
    /// Effective expiry of the contract.
    /// @returns The ISO expiry, or null if the instrument defines none.
    expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's terms.
    /// @returns Contract terms of the range accrual, serialized flat at this level.
    terms as terms: "generated.valuations.RangeAccrualTerms" => json(i.terms),
});
common_getters!(JsRangeAccrual, "RangeAccrual");

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.RangeAccrual.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsRangeAccrualBuilder, "RangeAccrualBuilder", RangeAccrualBuilder, JsRangeAccrual, |b| b.build()
);

#[wasm_bindgen(js_class = RangeAccrual)]
impl JsRangeAccrual {
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    #[wasm_bindgen(js_name = toBuilder)]
    pub fn to_builder(&self) -> JsRangeAccrualBuilder {
        JsRangeAccrualBuilder {
            inner: super::Staged::new(self.inner.to_builder()),
        }
    }
}

setters!(JsRangeAccrualBuilder, "RangeAccrualBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's terms.
    /// @param value - Contract terms of the range accrual, serialized flat at this level.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    terms as terms: "generated.valuations.RangeAccrualTerms" => js_wire,
});
common_setters!(JsRangeAccrualBuilder, "RangeAccrualBuilder");
