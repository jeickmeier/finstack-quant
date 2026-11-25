//! An option to enter a commodity swap.

use crate::api::core::dates::JsDayCount;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::commodity::commodity_swaption::{
    CommoditySwaption, CommoditySwaptionBuilder,
};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// An option to enter a commodity swap.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.CommoditySwaption.example();
    /// console.log(future.id);
    /// future.free();
    /// ```
    JsCommoditySwaption, "CommoditySwaption", CommoditySwaption
);
instrument_surface!(
    JsCommoditySwaption,
    "CommoditySwaption",
    JsCommoditySwaptionBuilder,
    "generated.valuations.CommoditySwaption"
);
factories!(JsCommoditySwaption, "CommoditySwaption", CommoditySwaption, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsCommoditySwaption, "CommoditySwaption", |i| {
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's underlying.
    /// @returns Commodity underlying parameters (commodity_type, underlying_ticker, unit, currency).
    underlying as underlying: "Pick<generated.valuations.CommodityForward, 'commodity_type' | 'underlying_ticker' | 'unit' | 'currency'>" => json(i.underlying),
    /// Read the contract's option type.
    /// @returns Option type (call = right to enter pay-fixed swap, put = right to enter receive-fixed swap).
    option_type as optionType: "generated.valuations.CommoditySwaption['option_type']" => json(i.option_type),
    /// Read the contract's expiry.
    /// @returns Option expiry date.
    expiry as expiry: "string" => date(i.expiry),
    /// Read the contract's underlying start date.
    /// @returns Underlying swap start date.
    underlying_start_date as underlyingStartDate: "string" => date(i.underlying_start_date),
    /// Read the contract's underlying maturity.
    /// @returns Underlying swap end date.
    underlying_maturity as underlyingMaturity: "string" => date(i.underlying_maturity),
    /// Read the contract's swap frequency.
    /// @returns Underlying swap payment frequency.
    swap_frequency as swapFrequency: "generated.valuations.CommoditySwaption['swap_frequency']" => json(i.swap_frequency),
    /// Read the contract's fixed price.
    /// @returns Fixed price (strike) of the underlying swap.
    fixed_price as fixedPrice: "number" => json(i.fixed_price),
    /// Read the contract's quantity.
    /// @returns Notional quantity per period.
    quantity as quantity: "number" => json(i.quantity),
    /// Read the contract's forward curve id.
    /// @returns Forward/futures curve ID for commodity price interpolation.
    forward_curve_id as forwardCurveId: "string" => text(i.forward_curve_id),
    /// Read the contract's discount curve id.
    /// @returns Discount curve ID for present value.
    discount_curve_id as discountCurveId: "string" => text(i.discount_curve_id),
    /// Read the contract's vol surface id.
    /// @returns Volatility surface ID for implied vol.
    vol_surface_id as volSurfaceId: "string" => text(i.vol_surface_id),
    /// Read the contract's calendar id.
    /// @returns Optional calendar ID for date adjustments.
    calendar_id as calendarId: "string | null" => opt_text(i.calendar_id),
    /// Read the contract's business day convention.
    /// @returns Business day convention for date adjustments.
    business_day_convention as businessDayConvention: "generated.valuations.CommoditySwaption['business_day_convention']" => json(i.business_day_convention),
    /// Read the contract's day count.
    /// @returns Day count convention for time to expiry.
    day_count as dayCount: "DayCount" => day_count(i.day_count),
});
common_getters!(JsCommoditySwaption, "CommoditySwaption");

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.CommoditySwaption.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsCommoditySwaptionBuilder, "CommoditySwaptionBuilder", CommoditySwaptionBuilder, JsCommoditySwaption, |b| b.build()
);

to_builder_method!(
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    JsCommoditySwaption, "CommoditySwaption", JsCommoditySwaptionBuilder
);

setters!(JsCommoditySwaptionBuilder, "CommoditySwaptionBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's underlying.
    /// @param value - Commodity underlying parameters (commodity_type, underlying_ticker, unit, currency).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    underlying as underlying: "Pick<generated.valuations.CommodityForward, 'commodity_type' | 'underlying_ticker' | 'unit' | 'currency'>" => js_wire,
    /// Set the contract's option type.
    /// @param value - Option type (call = right to enter pay-fixed swap, put = right to enter receive-fixed swap).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    option_type as optionType: "generated.valuations.CommoditySwaption['option_type']" => js_wire,
    /// Set the contract's expiry.
    /// @param value - Option expiry date.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    expiry as expiry: "string" => js_date,
    /// Set the contract's underlying start date.
    /// @param value - Underlying swap start date.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    underlying_start_date as underlyingStartDate: "string" => js_date,
    /// Set the contract's underlying maturity.
    /// @param value - Underlying swap end date.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    underlying_maturity as underlyingMaturity: "string" => js_date,
    /// Set the contract's swap frequency.
    /// @param value - Underlying swap payment frequency.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    swap_frequency as swapFrequency: "generated.valuations.CommoditySwaption['swap_frequency']" => js_wire,
    /// Set the contract's fixed price.
    /// @param value - Fixed price (strike) of the underlying swap.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    fixed_price as fixedPrice: "number" => js_f64,
    /// Set the contract's quantity.
    /// @param value - Notional quantity per period.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    quantity as quantity: "number" => js_f64,
    /// Set the contract's forward curve id.
    /// @param value - Forward/futures curve ID for commodity price interpolation.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    forward_curve_id as forwardCurveId: "string" => id,
    /// Set the contract's discount curve id.
    /// @param value - Discount curve ID for present value.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    discount_curve_id as discountCurveId: "string" => id,
    /// Set the contract's vol surface id.
    /// @param value - Volatility surface ID for implied vol.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    vol_surface_id as volSurfaceId: "string" => id,
    /// Set the contract's calendar id.
    /// @param value - Optional calendar ID for date adjustments.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    calendar_id as calendarId: "string | null" => optional_id via calendar_id_opt,
    /// Set the contract's business day convention.
    /// @param value - Business day convention for date adjustments.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    business_day_convention as businessDayConvention: "generated.valuations.CommoditySwaption['business_day_convention']" => js_wire,
});
common_setters!(JsCommoditySwaptionBuilder, "CommoditySwaptionBuilder");

handle_setters!(JsCommoditySwaptionBuilder, "CommoditySwaptionBuilder", {
    /// Set the contract's day count.
    /// @param value - Day count convention for time to expiry.
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    day_count as dayCount => JsDayCount,
});
