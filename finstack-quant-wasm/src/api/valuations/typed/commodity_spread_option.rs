//! An option on the spread between two commodity prices.

use crate::api::core::dates::JsDayCount;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::commodity::commodity_spread_option::{
    CommoditySpreadOption, CommoditySpreadOptionBuilder,
};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// An option on the spread between two commodity prices.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.CommoditySpreadOption.example();
    /// console.log(future.id);
    /// future.free();
    /// ```
    JsCommoditySpreadOption, "CommoditySpreadOption", CommoditySpreadOption
);
instrument_surface!(
    JsCommoditySpreadOption,
    "CommoditySpreadOption",
    JsCommoditySpreadOptionBuilder,
    "generated.valuations.CommoditySpreadOption"
);
factories!(JsCommoditySpreadOption, "CommoditySpreadOption", CommoditySpreadOption, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsCommoditySpreadOption, "CommoditySpreadOption", |i| {
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's currency.
    /// @returns Settlement currency.
    currency as currency: "string" => text(i.currency),
    /// Read the contract's option type.
    /// @returns Option type (call or put on the spread S1 - S2).
    option_type as optionType: "generated.valuations.CommoditySpreadOption['option_type']" => json(i.option_type),
    /// Read the contract's expiry.
    /// @returns Option expiry date.
    expiry as expiry: "string" => date(i.expiry),
    /// Read the contract's strike.
    /// @returns Spread strike price K in the payoff max(S1 - S2 - K, 0).
    strike as strike: "number" => json(i.strike),
    /// Read the contract's quantity.
    /// @returns Notional quantity (number of units).
    quantity as quantity: "number" => json(i.quantity),
    /// Read the contract's leg1 forward curve id.
    /// @returns Forward/price curve ID for leg 1 (the "long" commodity).
    leg1_forward_curve_id as leg1ForwardCurveId: "string" => json(i.leg1_forward_curve_id),
    /// Read the contract's leg2 forward curve id.
    /// @returns Forward/price curve ID for leg 2 (the "short" commodity).
    leg2_forward_curve_id as leg2ForwardCurveId: "string" => json(i.leg2_forward_curve_id),
    /// Read the contract's leg1 vol surface id.
    /// @returns Volatility surface ID for leg 1.
    leg1_vol_surface_id as leg1VolSurfaceId: "string" => json(i.leg1_vol_surface_id),
    /// Read the contract's leg2 vol surface id.
    /// @returns Volatility surface ID for leg 2.
    leg2_vol_surface_id as leg2VolSurfaceId: "string" => json(i.leg2_vol_surface_id),
    /// Read the contract's discount curve id.
    /// @returns Discount curve ID for present value.
    discount_curve_id as discountCurveId: "string" => json(i.discount_curve_id),
    /// Read the contract's correlation.
    /// @returns Correlation between the two commodity prices, in [-1, 1].
    correlation as correlation: "number" => json(i.correlation),
    /// Read the contract's day count.
    /// @returns Day count convention for time to expiry.
    day_count as dayCount: "DayCount" => day_count(i.day_count),
});
common_getters!(JsCommoditySpreadOption, "CommoditySpreadOption");

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.CommoditySpreadOption.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsCommoditySpreadOptionBuilder, "CommoditySpreadOptionBuilder", CommoditySpreadOptionBuilder, JsCommoditySpreadOption, |b| b.build()
);

#[wasm_bindgen(js_class = CommoditySpreadOption)]
impl JsCommoditySpreadOption {
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    #[wasm_bindgen(js_name = toBuilder)]
    pub fn to_builder(&self) -> JsCommoditySpreadOptionBuilder {
        JsCommoditySpreadOptionBuilder {
            inner: super::Staged::new(self.inner.to_builder()),
        }
    }
}

setters!(JsCommoditySpreadOptionBuilder, "CommoditySpreadOptionBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's currency.
    /// @param value - Settlement currency.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    currency as currency: "string" => js_wire,
    /// Set the contract's option type.
    /// @param value - Option type (call or put on the spread S1 - S2).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    option_type as optionType: "generated.valuations.CommoditySpreadOption['option_type']" => js_wire,
    /// Set the contract's expiry.
    /// @param value - Option expiry date.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    expiry as expiry: "string" => js_date,
    /// Set the contract's strike.
    /// @param value - Spread strike price K in the payoff max(S1 - S2 - K, 0).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    strike as strike: "number" => js_wire,
    /// Set the contract's quantity.
    /// @param value - Notional quantity (number of units).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    quantity as quantity: "number" => js_wire,
    /// Set the contract's leg1 forward curve id.
    /// @param value - Forward/price curve ID for leg 1 (the "long" commodity).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    leg1_forward_curve_id as leg1ForwardCurveId: "string" => id,
    /// Set the contract's leg2 forward curve id.
    /// @param value - Forward/price curve ID for leg 2 (the "short" commodity).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    leg2_forward_curve_id as leg2ForwardCurveId: "string" => id,
    /// Set the contract's leg1 vol surface id.
    /// @param value - Volatility surface ID for leg 1.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    leg1_vol_surface_id as leg1VolSurfaceId: "string" => id,
    /// Set the contract's leg2 vol surface id.
    /// @param value - Volatility surface ID for leg 2.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    leg2_vol_surface_id as leg2VolSurfaceId: "string" => id,
    /// Set the contract's discount curve id.
    /// @param value - Discount curve ID for present value.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    discount_curve_id as discountCurveId: "string" => id,
    /// Set the contract's correlation.
    /// @param value - Correlation between the two commodity prices, in [-1, 1].
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    correlation as correlation: "number" => js_wire,
});
common_setters!(
    JsCommoditySpreadOptionBuilder,
    "CommoditySpreadOptionBuilder"
);

handle_setters!(JsCommoditySpreadOptionBuilder, "CommoditySpreadOptionBuilder", {
    /// Set the contract's day count.
    /// @param value - Day count convention for time to expiry.
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    day_count as dayCount => JsDayCount,
});
