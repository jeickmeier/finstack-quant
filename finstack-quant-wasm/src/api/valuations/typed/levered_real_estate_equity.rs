//! An equity residual after the specified real-estate funding claims.

use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::equity::real_estate::{
    LeveredRealEstateEquity, LeveredRealEstateEquityBuilder,
};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// An equity residual after the specified real-estate funding claims.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.LeveredRealEstateEquity.example();
    /// console.log(future.id);
    /// future.free();
    /// ```
    JsLeveredRealEstateEquity, "LeveredRealEstateEquity", LeveredRealEstateEquity
);
instrument_surface!(
    JsLeveredRealEstateEquity,
    "LeveredRealEstateEquity",
    JsLeveredRealEstateEquityBuilder,
    "generated.valuations.LeveredRealEstateEquity"
);
factories!(JsLeveredRealEstateEquity, "LeveredRealEstateEquity", LeveredRealEstateEquity, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsLeveredRealEstateEquity, "LeveredRealEstateEquity", |i| {
    /// Effective expiry of the contract.
    /// @returns The ISO expiry, or null if the instrument defines none.
    expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's currency.
    /// @returns Currency (must match asset currency; financing PV is validated at valuation time).
    currency as currency: "string" => text(i.currency),
    /// Read the contract's asset.
    /// @returns Underlying unlevered asset.
    asset as asset: "generated.valuations.LeveredRealEstateEquity['asset']" => json(i.asset),
    /// Read the contract's financing.
    /// @returns Supported borrower liabilities, valued from the lender perspective and netted from asset PV.
    financing as financing: "generated.valuations.LeveredRealEstateEquity['financing']" => json(i.financing),
});
common_getters!(JsLeveredRealEstateEquity, "LeveredRealEstateEquity");

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.LeveredRealEstateEquity.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsLeveredRealEstateEquityBuilder, "LeveredRealEstateEquityBuilder", LeveredRealEstateEquityBuilder, JsLeveredRealEstateEquity, |b| b.build()
);

#[wasm_bindgen(js_class = LeveredRealEstateEquity)]
impl JsLeveredRealEstateEquity {
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    #[wasm_bindgen(js_name = toBuilder)]
    pub fn to_builder(&self) -> JsLeveredRealEstateEquityBuilder {
        JsLeveredRealEstateEquityBuilder {
            inner: super::Staged::new(self.inner.to_builder()),
        }
    }
}

setters!(JsLeveredRealEstateEquityBuilder, "LeveredRealEstateEquityBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's currency.
    /// @param value - Currency (must match asset currency; financing PV is validated at valuation time).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    currency as currency: "string" => js_wire,
    /// Set the contract's asset.
    /// @param value - Underlying unlevered asset.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    asset as asset: "generated.valuations.LeveredRealEstateEquity['asset']" => js_wire,
    /// Set the contract's financing.
    /// @param value - Supported borrower liabilities, valued from the lender perspective and netted from asset PV.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    financing as financing: "generated.valuations.LeveredRealEstateEquity['financing']" => js_wire,
});
common_setters!(
    JsLeveredRealEstateEquityBuilder,
    "LeveredRealEstateEquityBuilder"
);
