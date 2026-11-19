//! Typed options on listed futures.

use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::equity::equity_future_option::{
    EquityFutureOption, EquityFutureOptionBuilder,
};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// Exchange-listed option on a futures price.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.EquityFutureOption.example();
    /// console.log(future.terms.strike);
    /// future.free();
    /// ```
    JsEquityFutureOption, "EquityFutureOption", EquityFutureOption
);
instrument_surface!(
    JsEquityFutureOption,
    "EquityFutureOption",
    JsEquityFutureOptionBuilder,
    "generated.valuations.EquityFutureOption"
);
factories!(JsEquityFutureOption, "EquityFutureOption", EquityFutureOption, {
    /// Construct the native option-on-future example.
    /// @returns Example with an explicit pricing model and reference price units.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsEquityFutureOption, "EquityFutureOption", |i| {
    /// Effective expiry of the listed contract.
    /// @returns The ISO expiry, or null if the instrument defines none.
    expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Return the contract's position and lifecycle terms.
    /// @returns Independent copy of option and underlying marks, payoff and lifecycle terms.
    terms as terms: "generated.valuations.FutureOptionTerms" => json(i.terms),
});
common_getters!(JsEquityFutureOption, "EquityFutureOption");

builder_class!(
    /// Builder for a listed option on a futures contract.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.EquityFutureOption.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("OPTION-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsEquityFutureOptionBuilder, "EquityFutureOptionBuilder", EquityFutureOptionBuilder, JsEquityFutureOption, |b| b.build()
);

#[wasm_bindgen(js_class = EquityFutureOption)]
impl JsEquityFutureOption {
    /// Seed an independent builder from the complete option contract.
    /// @returns Builder carrying exercise, settlement, mark and pricing inputs.
    #[wasm_bindgen(js_name = toBuilder)]
    pub fn to_builder(&self) -> JsEquityFutureOptionBuilder {
        JsEquityFutureOptionBuilder {
            inner: super::Staged::new(self.inner.to_builder()),
        }
    }
}

setters!(JsEquityFutureOptionBuilder, "EquityFutureOptionBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's terms.
    /// @param value - Option strike, futures mark, contract count, model and exercise and settlement terms.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    terms as terms: "generated.valuations.FutureOptionTerms" => js_wire,
});
common_setters!(JsEquityFutureOptionBuilder, "EquityFutureOptionBuilder");
