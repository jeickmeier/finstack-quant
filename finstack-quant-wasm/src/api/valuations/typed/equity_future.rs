//! A listed future on an equity or index.

use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::equity::equity_future::{
    EquityFuture, EquityFutureBuilder,
};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// A listed future on an equity or index.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.EquityFuture.example();
    /// console.log(future.terms.contracts);
    /// future.free();
    /// ```
    JsEquityFuture, "EquityFuture", EquityFuture
);
instrument_surface!(
    JsEquityFuture,
    "EquityFuture",
    JsEquityFutureBuilder,
    "generated.valuations.EquityFuture"
);
factories!(JsEquityFuture, "EquityFuture", EquityFuture, {
    /// Construct the native example for this exchange-listed future.
    /// @returns Validated position and exchange lifecycle terms.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsEquityFuture, "EquityFuture", |i| {
    /// Effective expiry of the listed contract.
    /// @returns The ISO expiry, or null if the instrument defines none.
    expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Return the contract's position and lifecycle terms.
    /// @returns Independent copy of position, lifecycle dates and exchange marks.
    terms as terms: "generated.valuations.ListedFutureTerms" => json(i.terms),
});
common_getters!(JsEquityFuture, "EquityFuture");

builder_class!(
    /// Builder for an exchange-listed future.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.EquityFuture.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("FUTURE-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsEquityFutureBuilder, "EquityFutureBuilder", EquityFutureBuilder, JsEquityFuture, |b| b.build()
);

#[wasm_bindgen(js_class = EquityFuture)]
impl JsEquityFuture {
    /// Seed an independent builder from the complete listed contract.
    /// @returns Builder carrying settlement, exchange marks and market identifiers.
    #[wasm_bindgen(js_name = toBuilder)]
    pub fn to_builder(&self) -> JsEquityFutureBuilder {
        JsEquityFutureBuilder {
            inner: super::Staged::new(self.inner.to_builder()),
        }
    }
}

setters!(JsEquityFutureBuilder, "EquityFutureBuilder", {
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
    /// Set the contract's underlying ticker.
    /// @param value - Ticker of the underlying equity or index, independent of the spot quote key.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    underlying_ticker as underlyingTicker: "string" => js_wire,
    /// Set the contract's underlying currency.
    /// @param value - ISO currency of one underlying spot price unit.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    underlying_currency as underlyingCurrency: "string" => js_wire,
    /// Set the contract's discount curve id.
    /// @param value - Market identifier of the financing discount curve.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    discount_curve_id as discountCurveId: "string" => js_wire,
    /// Set the contract's spot id.
    /// @param value - Market quote identifier for the underlying spot price.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    spot_id as spotId: "string" => js_wire,
    /// Set the contract's div yield id.
    /// @param value - Optional market quote identifier for the continuous decimal dividend yield. Pass null to clear it.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    div_yield_id as divYieldId: "string | null" => js_wire via div_yield_id_opt,
    /// Set the contract's discrete dividends.
    /// @param value - Ex-date and cash amount per share in underlying currency for each discrete dividend.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    discrete_dividends as discreteDividends: "Array<[string, number]>" => js_wire,
    /// Set the contract's vol surface id.
    /// @param value - Optional equity volatility surface identifier used for quanto pricing. Pass null to clear it.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    vol_surface_id as volSurfaceId: "string | null" => js_wire via vol_surface_id_opt,
    /// Set the contract's quanto.
    /// @param value - Optional conversion policy, FX volatility and correlation inputs for quanto valuation. Pass null to clear it.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    quanto as quanto: "generated.valuations.QuantoSpec | null" => js_wire via quanto_opt,
});
common_setters!(JsEquityFutureBuilder, "EquityFutureBuilder");
