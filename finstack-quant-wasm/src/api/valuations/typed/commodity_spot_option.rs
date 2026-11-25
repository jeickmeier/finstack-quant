//! A commodity option on a specified spot or forward pricing basis.

use crate::api::core::dates::JsDayCount;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::commodity::commodity_spot_option::{
    CommoditySpotOption, CommoditySpotOptionBuilder,
};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// A commodity option on a specified spot or forward pricing basis.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.CommoditySpotOption.example();
    /// console.log(future.id);
    /// future.free();
    /// ```
    JsCommoditySpotOption, "CommoditySpotOption", CommoditySpotOption
);
instrument_surface!(
    JsCommoditySpotOption,
    "CommoditySpotOption",
    JsCommoditySpotOptionBuilder,
    "generated.valuations.CommoditySpotOption"
);
factories!(JsCommoditySpotOption, "CommoditySpotOption", CommoditySpotOption, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsCommoditySpotOption, "CommoditySpotOption", |i| {
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's underlying.
    /// @returns Commodity underlying parameters (commodity_type, underlying_ticker, unit, currency).
    underlying as underlying: "Pick<generated.valuations.CommodityForward, 'commodity_type' | 'underlying_ticker' | 'unit' | 'currency'>" => json(i.underlying),
    /// Read the contract's strike.
    /// @returns Strike price per unit.
    strike as strike: "number" => json(i.strike),
    /// Read the contract's option type.
    /// @returns Option type (call or put).
    option_type as optionType: "generated.valuations.CommoditySpotOption['option_type']" => json(i.option_type),
    /// Read the contract's exercise style.
    /// @returns Exercise style (European or American).
    exercise_style as exerciseStyle: "generated.valuations.CommoditySpotOption['exercise_style']" => json(i.exercise_style),
    /// Read the contract's exercise dates.
    /// @returns Optional Bermudan exercise schedule. Required when exercise_style == ExerciseStyle::Bermudan.
    exercise_dates as exerciseDates: "NonNullable<generated.valuations.CommoditySpotOption['exercise_dates']> | null" => json(i.exercise_dates),
    /// Read the contract's expiry.
    /// @returns Option expiry date.
    expiry as expiry: "string" => date(i.expiry),
    /// Read the contract's quantity.
    /// @returns Contract quantity in units.
    quantity as quantity: "number" => json(i.quantity),
    /// Read the contract's multiplier.
    /// @returns Contract multiplier (typically 1.0 for OTC options).
    multiplier as multiplier: "number" => json(i.multiplier),
    /// Read the contract's settlement.
    /// @returns Settlement type (physical or cash). Cash fixes the payoff on exercise; physical retains the marked delivery claim until its recorded settlement date.
    settlement as settlement: "generated.valuations.CommoditySpotOption['settlement']" => json(i.settlement),
    /// Read the contract's exercise.
    /// @returns Recorded exercise or expiry; required after expiry while settlement remains unpaid.
    exercise as exercise: "NonNullable<generated.valuations.CommoditySpotOption['exercise']> | null" => json(i.exercise),
    /// Read the contract's forward curve id.
    /// @returns Forward/futures curve ID for price interpolation.
    forward_curve_id as forwardCurveId: "string" => text(i.forward_curve_id),
    /// Read the contract's discount curve id.
    /// @returns Discount curve ID for present value.
    discount_curve_id as discountCurveId: "string" => text(i.discount_curve_id),
    /// Read the contract's vol surface id.
    /// @returns Volatility surface ID for implied vol.
    vol_surface_id as volSurfaceId: "string" => text(i.vol_surface_id),
    /// Read the contract's spot id.
    /// @returns Optional spot price ID (for spot-based pricing and American options). Numeric prices use the underlying currency; Money prices must match it.
    spot_id as spotId: "string | null" => opt_text(i.spot_id),
    /// Read the contract's quoted forward.
    /// @returns Optional quoted forward price (overrides curve lookup).
    quoted_forward as quotedForward: "number | null" => json(i.quoted_forward),
    /// Read the contract's day count.
    /// @returns Day count convention for time to expiry.
    day_count as dayCount: "DayCount" => day_count(i.day_count),
});
common_getters!(JsCommoditySpotOption, "CommoditySpotOption");

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.CommoditySpotOption.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsCommoditySpotOptionBuilder, "CommoditySpotOptionBuilder", CommoditySpotOptionBuilder, JsCommoditySpotOption, |b| b.build()
);

to_builder_method!(
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    JsCommoditySpotOption, "CommoditySpotOption", JsCommoditySpotOptionBuilder
);

setters!(JsCommoditySpotOptionBuilder, "CommoditySpotOptionBuilder", {
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
    /// Set the contract's strike.
    /// @param value - Strike price per unit.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    strike as strike: "number" => js_f64,
    /// Set the contract's option type.
    /// @param value - Option type (call or put).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    option_type as optionType: "generated.valuations.CommoditySpotOption['option_type']" => js_wire,
    /// Set the contract's exercise style.
    /// @param value - Exercise style (European or American).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    exercise_style as exerciseStyle: "generated.valuations.CommoditySpotOption['exercise_style']" => js_wire,
    /// Set the contract's exercise dates.
    /// @param value - Optional Bermudan exercise schedule. Required when exercise_style == ExerciseStyle::Bermudan.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    exercise_dates as exerciseDates: "NonNullable<generated.valuations.CommoditySpotOption['exercise_dates']> | null" => opt_wire via exercise_dates_opt,
    /// Set the contract's expiry.
    /// @param value - Option expiry date.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    expiry as expiry: "string" => js_date,
    /// Set the contract's quantity.
    /// @param value - Contract quantity in units.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    quantity as quantity: "number" => js_f64,
    /// Set the contract's multiplier.
    /// @param value - Contract multiplier (typically 1.0 for OTC options).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    multiplier as multiplier: "number" => js_f64,
    /// Set the contract's settlement.
    /// @param value - Settlement type (physical or cash). Cash fixes the payoff on exercise; physical retains the marked delivery claim until its recorded settlement date.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    settlement as settlement: "generated.valuations.CommoditySpotOption['settlement']" => js_wire,
    /// Set the contract's exercise.
    /// @param value - Recorded exercise or expiry; required after expiry while settlement remains unpaid.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    exercise as exercise: "NonNullable<generated.valuations.CommoditySpotOption['exercise']> | null" => opt_wire via exercise_opt,
    /// Set the contract's forward curve id.
    /// @param value - Forward/futures curve ID for price interpolation.
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
    /// Set the contract's spot id.
    /// @param value - Optional spot price ID (for spot-based pricing and American options). Numeric prices use the underlying currency; Money prices must match it.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    spot_id as spotId: "string | null" => optional_id via spot_id_opt,
    /// Set the contract's quoted forward.
    /// @param value - Optional quoted forward price (overrides curve lookup).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    quoted_forward as quotedForward: "number | null" => opt_f64 via quoted_forward_opt,
});
common_setters!(JsCommoditySpotOptionBuilder, "CommoditySpotOptionBuilder");

handle_setters!(JsCommoditySpotOptionBuilder, "CommoditySpotOptionBuilder", {
    /// Set the contract's day count.
    /// @param value - Day count convention for time to expiry.
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    day_count as dayCount => JsDayCount,
});
