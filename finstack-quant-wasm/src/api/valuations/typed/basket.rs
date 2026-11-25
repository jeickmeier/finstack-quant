//! A basket aggregating the prices of signed constituents.

use crate::api::core::money::JsMoney;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::exotics::basket::{Basket, BasketBuilder};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// A basket aggregating the prices of signed constituents.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.Basket.example();
    /// console.log(future.id);
    /// future.free();
    /// ```
    JsBasket, "Basket", Basket
);
instrument_surface!(
    JsBasket,
    "Basket",
    JsBasketBuilder,
    "generated.valuations.Basket"
);
factories!(JsBasket, "Basket", Basket, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsBasket, "Basket", |i| {
    /// Effective expiry of the contract.
    /// @returns The ISO expiry, or null if the instrument defines none.
    expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's constituents.
    /// @returns Basket constituents (the actual holdings)
    constituents as constituents: "generated.valuations.Basket['constituents']" => json(i.constituents),
    /// Read the contract's expense ratio.
    /// @returns Total expense ratio (as decimal, e.g., 0.0025 = 0.25%) The explicit expense election supplies accrued cash.
    expense_ratio as expenseRatio: "number" => json(i.expense_ratio),
    /// Read the accrued operating expense election.
    /// @returns The no-expense election or reporting-currency accrued cash per basket unit.
    expense as expense: "generated.valuations.Basket['expense']" => json(i.expense),
    /// Read the contract's reporting currency.
    /// @returns Reporting currency of the basket: NAV and PV are stated in it and every constituent in another currency is FX-converted into it.
    reporting_currency as reportingCurrency: "string" => text(i.reporting_currency),
    /// Read the contract's notional.
    /// @returns Position notional used to scale basket NAV to portfolio PV.
    notional as notional: "Money" => money(i.notional),
    /// Read the contract's discount curve id.
    /// @returns Discount curve identifier for present value calculations
    discount_curve_id as discountCurveId: "string" => text(i.discount_curve_id),
    /// Read the contract's pricing config.
    /// @returns Pricing configuration
    pricing_config as pricingConfig: "generated.valuations.Basket['pricing_config']" => json(i.pricing_config),
});
common_getters!(JsBasket, "Basket");

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.Basket.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsBasketBuilder, "BasketBuilder", BasketBuilder, JsBasket, |b| b.build()
);

to_builder_method!(
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    JsBasket, "Basket", JsBasketBuilder
);

setters!(JsBasketBuilder, "BasketBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's constituents.
    /// @param value - Basket constituents (the actual holdings)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    constituents as constituents: "generated.valuations.Basket['constituents']" => js_wire,
    /// Set the contract's expense ratio.
    /// @param value - Total expense ratio (as decimal, e.g., 0.0025 = 0.25%) The explicit expense election supplies accrued cash.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    expense_ratio as expenseRatio: "number" => js_f64,
    /// Set the basket's accrued expense election.
    /// @param value - No expense or nonnegative reporting-currency cash per basket unit.
    /// @returns This builder for chaining.
    /// @throws Error - Throws for malformed inputs or a consumed builder.
    expense as expense: "generated.valuations.Basket['expense']" => js_wire,

    /// Set the contract's reporting currency.
    /// @param value - Reporting currency of the basket: NAV and PV are stated in it and every constituent in another currency is FX-converted into it.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    reporting_currency as reportingCurrency: "string" => js_currency,
    /// Set the contract's discount curve id.
    /// @param value - Discount curve identifier for present value calculations
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    discount_curve_id as discountCurveId: "string" => id,
    /// Set the contract's pricing config.
    /// @param value - Pricing configuration
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    pricing_config as pricingConfig: "generated.valuations.Basket['pricing_config']" => js_wire,
});
common_setters!(JsBasketBuilder, "BasketBuilder");

handle_setters!(JsBasketBuilder, "BasketBuilder", {
    /// Set the contract's notional.
    /// @param value - Position notional used to scale basket NAV to portfolio PV.
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    notional as notional => JsMoney,
});
