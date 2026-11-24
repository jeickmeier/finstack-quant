//! A listed currency future with quote-currency variation margin.

use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::fx::fx_future::{FxFuture, FxFutureBuilder};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// A listed currency future with quote-currency variation margin.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.FxFuture.example();
    /// console.log(future.terms.contracts);
    /// future.free();
    /// ```
    JsFxFuture, "FxFuture", FxFuture
);
instrument_surface!(
    JsFxFuture,
    "FxFuture",
    JsFxFutureBuilder,
    "generated.valuations.FxFuture"
);
factories!(JsFxFuture, "FxFuture", FxFuture, {
    /// Construct the native example for this exchange-listed future.
    /// @returns Validated position and exchange lifecycle terms.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsFxFuture, "FxFuture", |i| {
    /// Effective expiry of the listed contract.
    /// @returns The ISO expiry, or null if the instrument defines none.
    expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Return the contract's position and lifecycle terms.
    /// @returns Independent copy of position, lifecycle dates and exchange marks.
    terms as terms: "generated.valuations.ListedFutureTerms" => json(i.terms),
    /// Read the contract's base currency.
    /// @returns ISO currency in the denominator of quote currency per unit of base currency.
    base_currency as baseCurrency: "string" => text(i.base_currency),
    /// Read the contract's domestic discount curve id.
    /// @returns Market identifier of the quote-currency discount curve.
    domestic_discount_curve_id as domesticDiscountCurveId: "string" => text(i.domestic_discount_curve_id),
    /// Read the contract's foreign discount curve id.
    /// @returns Market identifier of the base-currency discount curve.
    foreign_discount_curve_id as foreignDiscountCurveId: "string" => text(i.foreign_discount_curve_id),
    /// Read the contract's quoted spot.
    /// @returns Optional positive spot override in quote currency per unit of base currency, or null.
    quoted_spot as quotedSpot: "number | null" => json(i.quoted_spot),
});
common_getters!(JsFxFuture, "FxFuture");

builder_class!(
    /// Builder for an exchange-listed future.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.FxFuture.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("FUTURE-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsFxFutureBuilder, "FxFutureBuilder", FxFutureBuilder, JsFxFuture, |b| b.build()
);

to_builder_method!(
    /// Seed an independent builder from the complete listed contract.
    /// @returns Builder carrying settlement, exchange marks and market identifiers.
    JsFxFuture, "FxFuture", JsFxFutureBuilder
);

setters!(JsFxFutureBuilder, "FxFutureBuilder", {
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
    /// Set the contract's base currency.
    /// @param value - ISO currency in the denominator of quote currency per unit of base currency.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    base_currency as baseCurrency: "string" => js_wire,
    /// Set the contract's domestic discount curve id.
    /// @param value - Market identifier of the quote-currency discount curve.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    domestic_discount_curve_id as domesticDiscountCurveId: "string" => js_wire,
    /// Set the contract's foreign discount curve id.
    /// @param value - Market identifier of the base-currency discount curve.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    foreign_discount_curve_id as foreignDiscountCurveId: "string" => js_wire,
    /// Set the contract's quoted spot.
    /// @param value - Optional positive spot override in quote currency per unit of base currency. Pass null to clear it.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    quoted_spot as quotedSpot: "number | null" => js_wire via quoted_spot_opt,
});
common_setters!(JsFxFutureBuilder, "FxFutureBuilder");
