//! Typed options on listed futures.

use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::rates::ir_future_option::{
    InterestRateFutureOption, InterestRateFutureOptionBuilder,
};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// Exchange-listed option on a futures price.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.InterestRateFutureOption.example();
    /// console.log(future.terms.strike);
    /// future.free();
    /// ```
    JsInterestRateFutureOption, "InterestRateFutureOption", InterestRateFutureOption
);
instrument_surface!(
    JsInterestRateFutureOption,
    "InterestRateFutureOption",
    JsInterestRateFutureOptionBuilder,
    "generated.valuations.InterestRateFutureOption"
);
factories!(JsInterestRateFutureOption, "InterestRateFutureOption", InterestRateFutureOption, {
    /// Construct the native option-on-future example.
    /// @returns Example with an explicit pricing model and reference price units.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsInterestRateFutureOption, "InterestRateFutureOption", |i| {
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
common_getters!(JsInterestRateFutureOption, "InterestRateFutureOption");

builder_class!(
    /// Builder for a listed option on a futures contract.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.InterestRateFutureOption.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("OPTION-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsInterestRateFutureOptionBuilder, "InterestRateFutureOptionBuilder", InterestRateFutureOptionBuilder, JsInterestRateFutureOption, |b| b.build()
);

#[wasm_bindgen(js_class = InterestRateFutureOption)]
impl JsInterestRateFutureOption {
    /// Seed an independent builder from the complete option contract.
    /// @returns Builder carrying exercise, settlement, mark and pricing inputs.
    #[wasm_bindgen(js_name = toBuilder)]
    pub fn to_builder(&self) -> JsInterestRateFutureOptionBuilder {
        JsInterestRateFutureOptionBuilder {
            inner: super::Staged::new(self.inner.to_builder()),
        }
    }
}

setters!(JsInterestRateFutureOptionBuilder, "InterestRateFutureOptionBuilder", {
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
common_setters!(
    JsInterestRateFutureOptionBuilder,
    "InterestRateFutureOptionBuilder"
);
