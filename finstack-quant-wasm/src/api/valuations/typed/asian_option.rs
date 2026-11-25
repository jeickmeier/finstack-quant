//! An option on the documented average of an underlying asset.

use crate::api::core::dates::JsDayCount;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::exotics::asian_option::{
    AsianOption, AsianOptionBuilder,
};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// An option on the documented average of an underlying asset.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.AsianOption.example();
    /// console.log(future.id);
    /// future.free();
    /// ```
    JsAsianOption, "AsianOption", AsianOption
);
instrument_surface!(
    JsAsianOption,
    "AsianOption",
    JsAsianOptionBuilder,
    "generated.valuations.AsianOption"
);
factories!(JsAsianOption, "AsianOption", AsianOption, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsAsianOption, "AsianOption", |i| {
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's underlying ticker.
    /// @returns Underlying asset ticker symbol
    underlying_ticker as underlyingTicker: "string" => text(i.underlying_ticker),
    /// Read the contract's strike.
    /// @returns Strike price
    strike as strike: "number" => json(i.strike),
    /// Read the contract's option type.
    /// @returns Option type (call or put)
    option_type as optionType: "generated.valuations.AsianOption['option_type']" => json(i.option_type),
    /// Read the contract's averaging method.
    /// @returns Averaging method (arithmetic or geometric)
    averaging_method as averagingMethod: "generated.valuations.AsianOption['averaging_method']" => json(i.averaging_method),
    /// Read the contract's expiry.
    /// @returns Option expiry date
    expiry as expiry: "string" => date(i.expiry),
    /// Read the contract's fixing dates.
    /// @returns Dates on which underlying is observed for averaging. **Note**: These dates should be pre-adjusted for business day conventions. The pricer uses these dates directly without further adjustment. See struct-level documentation for business day convention guidance.
    fixing_dates as fixingDates: "generated.valuations.AsianOption['fixing_dates']" => dates(i.fixing_dates),
    /// Read the contract's quantity.
    /// @returns Number of underlying units the option is written on; PV and Greeks scale linearly with it.
    quantity as quantity: "number" => json(i.quantity),
    /// Read the contract's currency.
    /// @returns Currency of the strike, premium and present value.
    currency as currency: "string" => text(i.currency),
    /// Read the contract's day count.
    /// @returns Day count convention
    day_count as dayCount: "DayCount" => day_count(i.day_count),
    /// Read the contract's discount curve id.
    /// @returns Discount curve ID for present value calculations
    discount_curve_id as discountCurveId: "string" => text(i.discount_curve_id),
    /// Read the contract's spot id.
    /// @returns Spot price identifier
    spot_id as spotId: "string" => text(i.spot_id),
    /// Read the contract's vol surface id.
    /// @returns Volatility surface ID
    vol_surface_id as volSurfaceId: "string" => text(i.vol_surface_id),
    /// Read the contract's div yield id.
    /// @returns Optional dividend-yield scalar ID
    div_yield_id as divYieldId: "string | null" => opt_text(i.div_yield_id),
    /// Read the contract's past fixings.
    /// @returns Past fixings for seasoned options (date, observed price pairs). For seasoned options where some averaging observations have already occurred, provide the historical fixings here. Only fixings that match dates in fixing_dates and are on or before the valuation date are considered.
    past_fixings as pastFixings: "generated.valuations.AsianOption['past_fixings']" => dated_nums(i.past_fixings),
});
common_getters!(JsAsianOption, "AsianOption");

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.AsianOption.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsAsianOptionBuilder, "AsianOptionBuilder", AsianOptionBuilder, JsAsianOption, |b| b.build()
);

to_builder_method!(
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    JsAsianOption, "AsianOption", JsAsianOptionBuilder
);

setters!(JsAsianOptionBuilder, "AsianOptionBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's underlying ticker.
    /// @param value - Underlying asset ticker symbol
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    underlying_ticker as underlyingTicker: "string" => id,
    /// Set the contract's strike.
    /// @param value - Strike price
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    strike as strike: "number" => js_f64,
    /// Set the contract's option type.
    /// @param value - Option type (call or put)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    option_type as optionType: "generated.valuations.AsianOption['option_type']" => js_wire,
    /// Set the contract's averaging method.
    /// @param value - Averaging method (arithmetic or geometric)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    averaging_method as averagingMethod: "generated.valuations.AsianOption['averaging_method']" => js_wire,
    /// Set the contract's expiry.
    /// @param value - Option expiry date
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    expiry as expiry: "string" => js_date,
    /// Set the contract's fixing dates.
    /// @param value - Dates on which underlying is observed for averaging. **Note**: These dates should be pre-adjusted for business day conventions. The pricer uses these dates directly without further adjustment. See struct-level documentation for business day convention guidance.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    fixing_dates as fixingDates: "generated.valuations.AsianOption['fixing_dates']" => js_wire,
    /// Set the contract's quantity.
    /// @param value - Number of underlying units the option is written on; PV and Greeks scale linearly with it.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    quantity as quantity: "number" => js_f64,
    /// Set the contract's currency.
    /// @param value - Currency of the strike, premium and present value.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    currency as currency: "string" => js_currency,
    /// Set the contract's discount curve id.
    /// @param value - Discount curve ID for present value calculations
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    discount_curve_id as discountCurveId: "string" => id,
    /// Set the contract's spot id.
    /// @param value - Spot price identifier
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    spot_id as spotId: "string" => id,
    /// Set the contract's vol surface id.
    /// @param value - Volatility surface ID
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    vol_surface_id as volSurfaceId: "string" => id,
    /// Set the contract's div yield id.
    /// @param value - Optional dividend-yield scalar ID
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    div_yield_id as divYieldId: "string | null" => optional_id via div_yield_id_opt,
    /// Set the contract's past fixings.
    /// @param value - Past fixings for seasoned options (date, observed price pairs). For seasoned options where some averaging observations have already occurred, provide the historical fixings here. Only fixings that match dates in fixing_dates and are on or before the valuation date are considered.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    past_fixings as pastFixings: "generated.valuations.AsianOption['past_fixings']" => js_wire,
});
common_setters!(JsAsianOptionBuilder, "AsianOptionBuilder");

handle_setters!(JsAsianOptionBuilder, "AsianOptionBuilder", {
    /// Set the contract's day count.
    /// @param value - Day count convention
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    day_count as dayCount => JsDayCount,
});
