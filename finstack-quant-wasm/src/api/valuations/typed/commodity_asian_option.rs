//! A commodity option on the documented average across a fixing schedule.

use crate::api::core::dates::JsDayCount;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::commodity::commodity_asian_option::{
    CommodityAsianOption, CommodityAsianOptionBuilder,
};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// A commodity option on the documented average across a fixing schedule.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.CommodityAsianOption.example();
    /// console.log(future.id);
    /// future.free();
    /// ```
    JsCommodityAsianOption, "CommodityAsianOption", CommodityAsianOption
);
instrument_surface!(
    JsCommodityAsianOption,
    "CommodityAsianOption",
    JsCommodityAsianOptionBuilder,
    "generated.valuations.CommodityAsianOption"
);
factories!(JsCommodityAsianOption, "CommodityAsianOption", CommodityAsianOption, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsCommodityAsianOption, "CommodityAsianOption", |i| {
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's underlying.
    /// @returns Commodity underlying parameters (type, ticker, unit, currency).
    underlying as underlying: "Pick<generated.valuations.CommodityForward, 'commodity_type' | 'underlying_ticker' | 'unit' | 'currency'>" => json(i.underlying),
    /// Read the contract's strike.
    /// @returns Strike price per unit.
    strike as strike: "number" => json(i.strike),
    /// Read the contract's option type.
    /// @returns Option type (call or put).
    option_type as optionType: "generated.valuations.CommodityAsianOption['option_type']" => json(i.option_type),
    /// Read the contract's averaging method.
    /// @returns Averaging method (arithmetic or geometric).
    averaging_method as averagingMethod: "generated.valuations.CommodityAsianOption['averaging_method']" => json(i.averaging_method),
    /// Read the contract's fixing dates.
    /// @returns Dates on which the commodity price is observed for averaging. **Note**: These dates should be pre-adjusted for business day conventions.
    fixing_dates as fixingDates: "generated.valuations.CommodityAsianOption['fixing_dates']" => dates(i.fixing_dates),
    /// Read the contract's past fixings.
    /// @returns Already observed fixings for seasoned options (ex-date, price pairs).
    past_fixings as pastFixings: "generated.valuations.CommodityAsianOption['past_fixings']" => dated_nums(i.past_fixings),
    /// Read the contract's quantity.
    /// @returns Contract quantity in commodity units.
    quantity as quantity: "number" => json(i.quantity),
    /// Read the contract's expiry.
    /// @returns Option expiry/settlement date for the payoff.
    expiry as expiry: "string" => date(i.expiry),
    /// Read the contract's forward curve id.
    /// @returns Forward/futures price curve ID.
    forward_curve_id as forwardCurveId: "string" => text(i.forward_curve_id),
    /// Read the contract's discount curve id.
    /// @returns Discount curve ID for present value calculations.
    discount_curve_id as discountCurveId: "string" => text(i.discount_curve_id),
    /// Read the contract's vol surface id.
    /// @returns Volatility surface ID for implied vol.
    vol_surface_id as volSurfaceId: "string" => text(i.vol_surface_id),
    /// Read the contract's day count.
    /// @returns Day count convention.
    day_count as dayCount: "DayCount" => day_count(i.day_count),
});
common_getters!(JsCommodityAsianOption, "CommodityAsianOption");

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.CommodityAsianOption.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsCommodityAsianOptionBuilder, "CommodityAsianOptionBuilder", CommodityAsianOptionBuilder, JsCommodityAsianOption, |b| b.build()
);

to_builder_method!(
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    JsCommodityAsianOption, "CommodityAsianOption", JsCommodityAsianOptionBuilder
);

setters!(JsCommodityAsianOptionBuilder, "CommodityAsianOptionBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's underlying.
    /// @param value - Commodity underlying parameters (type, ticker, unit, currency).
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
    option_type as optionType: "generated.valuations.CommodityAsianOption['option_type']" => js_wire,
    /// Set the contract's averaging method.
    /// @param value - Averaging method (arithmetic or geometric).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    averaging_method as averagingMethod: "generated.valuations.CommodityAsianOption['averaging_method']" => js_wire,
    /// Set the contract's fixing dates.
    /// @param value - Dates on which the commodity price is observed for averaging. **Note**: These dates should be pre-adjusted for business day conventions.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    fixing_dates as fixingDates: "generated.valuations.CommodityAsianOption['fixing_dates']" => js_wire,
    /// Set the contract's past fixings.
    /// @param value - Already observed fixings for seasoned options (ex-date, price pairs).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    past_fixings as pastFixings: "generated.valuations.CommodityAsianOption['past_fixings']" => js_wire,
    /// Set the contract's quantity.
    /// @param value - Contract quantity in commodity units.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    quantity as quantity: "number" => js_f64,
    /// Set the contract's expiry.
    /// @param value - Option expiry/settlement date for the payoff.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    expiry as expiry: "string" => js_date,
    /// Set the contract's forward curve id.
    /// @param value - Forward/futures price curve ID.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    forward_curve_id as forwardCurveId: "string" => id,
    /// Set the contract's discount curve id.
    /// @param value - Discount curve ID for present value calculations.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    discount_curve_id as discountCurveId: "string" => id,
    /// Set the contract's vol surface id.
    /// @param value - Volatility surface ID for implied vol.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    vol_surface_id as volSurfaceId: "string" => id,
});
common_setters!(JsCommodityAsianOptionBuilder, "CommodityAsianOptionBuilder");

handle_setters!(JsCommodityAsianOptionBuilder, "CommodityAsianOptionBuilder", {
    /// Set the contract's day count.
    /// @param value - Day count convention.
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    day_count as dayCount => JsDayCount,
});
