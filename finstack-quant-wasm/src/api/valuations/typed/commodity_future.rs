//! A listed future on a commodity and an exchange settlement price.

use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::commodity::commodity_future::{
    CommodityFuture, CommodityFutureBuilder,
};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// A listed future on a commodity and an exchange settlement price.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.CommodityFuture.example();
    /// console.log(future.id);
    /// future.free();
    /// ```
    JsCommodityFuture, "CommodityFuture", CommodityFuture
);
instrument_surface!(
    JsCommodityFuture,
    "CommodityFuture",
    JsCommodityFutureBuilder,
    "generated.valuations.CommodityFuture"
);
factories!(JsCommodityFuture, "CommodityFuture", CommodityFuture, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsCommodityFuture, "CommodityFuture", |i| {
    /// Effective expiry of the contract.
    /// @returns The ISO expiry, or null if the instrument defines none.
    expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's underlying ticker.
    /// @returns Exchange symbol or underlying label.
    underlying_ticker as underlyingTicker: "string" => json(i.underlying_ticker),
    /// Read the contract's terms.
    /// @returns Standard listed position and lifecycle terms.
    terms as terms: "generated.valuations.CommodityFuture['terms']" => json(i.terms),
    /// Read the contract's forward curve id.
    /// @returns Commodity forward PriceCurve used for projected observations.
    forward_curve_id as forwardCurveId: "string" => json(i.forward_curve_id),
    /// Read the contract's fixing.
    /// @returns Final-settlement price fixing rule (single observation or average).
    fixing as fixing: "generated.valuations.CommodityFuture['fixing']" => json(i.fixing),
});
common_getters!(JsCommodityFuture, "CommodityFuture");

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.CommodityFuture.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsCommodityFutureBuilder, "CommodityFutureBuilder", CommodityFutureBuilder, JsCommodityFuture, |b| b.build()
);

#[wasm_bindgen(js_class = CommodityFuture)]
impl JsCommodityFuture {
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    #[wasm_bindgen(js_name = toBuilder)]
    pub fn to_builder(&self) -> JsCommodityFutureBuilder {
        JsCommodityFutureBuilder {
            inner: super::Staged::new(self.inner.to_builder()),
        }
    }
}

setters!(JsCommodityFutureBuilder, "CommodityFutureBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's underlying ticker.
    /// @param value - Exchange symbol or underlying label.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    underlying_ticker as underlyingTicker: "string" => id,
    /// Set the contract's terms.
    /// @param value - Standard listed position and lifecycle terms.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    terms as terms: "generated.valuations.CommodityFuture['terms']" => js_wire,
    /// Set the contract's forward curve id.
    /// @param value - Commodity forward PriceCurve used for projected observations.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    forward_curve_id as forwardCurveId: "string" => id,
    /// Set the contract's fixing.
    /// @param value - Final-settlement price fixing rule (single observation or average).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    fixing as fixing: "generated.valuations.CommodityFuture['fixing']" => js_wire,
});
common_setters!(JsCommodityFutureBuilder, "CommodityFutureBuilder");
