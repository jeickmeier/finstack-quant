//! A listed future on a forward volatility index level.

use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::equity::vol_index_future::{
    VolatilityIndexFuture, VolatilityIndexFutureBuilder,
};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// A listed future on a forward volatility index level.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.VolatilityIndexFuture.example();
    /// console.log(future.terms.contracts);
    /// future.free();
    /// ```
    JsVolatilityIndexFuture, "VolatilityIndexFuture", VolatilityIndexFuture
);
instrument_surface!(
    JsVolatilityIndexFuture,
    "VolatilityIndexFuture",
    JsVolatilityIndexFutureBuilder,
    "generated.valuations.VolatilityIndexFuture"
);
factories!(JsVolatilityIndexFuture, "VolatilityIndexFuture", VolatilityIndexFuture, {
    /// Construct the native example for this exchange-listed future.
    /// @returns Validated position and exchange lifecycle terms.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsVolatilityIndexFuture, "VolatilityIndexFuture", |i| {
    /// Effective expiry of the listed contract.
    /// @returns The ISO expiry, or null if the instrument defines none.
    expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Return the contract's position and lifecycle terms.
    /// @returns Independent copy of position, lifecycle dates and exchange marks.
    terms as terms: "generated.valuations.ListedFutureTerms" => json(i.terms),
    /// Return the volatility index futures curve identifier.
    /// @returns Market price-curve identifier for futures index levels expressed in index points.
    vol_index_curve_id as volIndexCurveId: "string" => text(i.vol_index_curve_id),
});
common_getters!(JsVolatilityIndexFuture, "VolatilityIndexFuture");

builder_class!(
    /// Builder for an exchange-listed future.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.VolatilityIndexFuture.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("FUTURE-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsVolatilityIndexFutureBuilder, "VolatilityIndexFutureBuilder", VolatilityIndexFutureBuilder, JsVolatilityIndexFuture, |b| b.build()
);

to_builder_method!(
    /// Seed an independent builder from the complete listed contract.
    /// @returns Builder carrying settlement, exchange marks and market identifiers.
    JsVolatilityIndexFuture,
    "VolatilityIndexFuture",
    JsVolatilityIndexFutureBuilder
);

setters!(JsVolatilityIndexFutureBuilder, "VolatilityIndexFutureBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's terms.
    /// @param value - Position, contract multiplier, trade and official marks, and lifecycle dates.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    terms as terms: "generated.valuations.ListedFutureTerms" => js_wire,
    /// Set the contract's vol index curve id.
    /// @param value - Market price-curve identifier for futures index levels expressed in index points.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    vol_index_curve_id as volIndexCurveId: "string" => id,
});
common_setters!(
    JsVolatilityIndexFutureBuilder,
    "VolatilityIndexFutureBuilder"
);
