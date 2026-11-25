//! An option on capped and floored returns over a sequence of equity resets.

use crate::api::core::dates::JsDayCount;
use crate::api::core::money::JsMoney;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::equity::cliquet_option::{
    CliquetOption, CliquetOptionBuilder,
};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// An option on capped and floored returns over a sequence of equity resets.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.CliquetOption.example();
    /// console.log(future.id);
    /// future.free();
    /// ```
    JsCliquetOption, "CliquetOption", CliquetOption
);
instrument_surface!(
    JsCliquetOption,
    "CliquetOption",
    JsCliquetOptionBuilder,
    "generated.valuations.CliquetOption"
);
factories!(JsCliquetOption, "CliquetOption", CliquetOption, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsCliquetOption, "CliquetOption", |i| {
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's underlying ticker.
    /// @returns Underlying asset ticker symbol
    underlying_ticker as underlyingTicker: "string" => text(i.underlying_ticker),
    /// Read the contract's reset dates.
    /// @returns Reset dates for periodic return locking
    reset_dates as resetDates: "generated.valuations.CliquetOption['reset_dates']" => dates(i.reset_dates),
    /// Read the contract's expiry.
    /// @returns Explicit terminal expiry date for the structure.
    expiry as expiry: "string" => date(i.expiry),
    /// Read the contract's local cap.
    /// @returns Local cap on individual period returns
    local_cap as localCap: "number" => json(i.local_cap),
    /// Read the contract's local floor.
    /// @returns Local floor on individual period returns (default 0.0)
    local_floor as localFloor: "number" => json(i.local_floor),
    /// Read the contract's global cap.
    /// @returns Global cap on sum of all period returns
    global_cap as globalCap: "number" => json(i.global_cap),
    /// Read the contract's global floor.
    /// @returns Global floor on sum of all period returns (default 0.0)
    global_floor as globalFloor: "number" => json(i.global_floor),
    /// Read the contract's notional.
    /// @returns Notional amount
    notional as notional: "Money" => money(i.notional),
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
    /// Read the contract's path model.
    /// @returns Explicit path model selection. AtmTermGbm is an ATM-term-structure approximation and does not model equity strike skew; callers must opt into that limitation.
    path_model as pathModel: "generated.valuations.CliquetOption['path_model']" => json(i.path_model),
    /// Read the contract's div yield id.
    /// @returns Optional dividend-yield scalar ID. Some(id): lookup MUST succeed (a missing or non-unitless scalar returns an error). None: no implicit default; treated as zero continuous dividend yield. Set explicitly for index underlyings.
    div_yield_id as divYieldId: "string | null" => opt_text(i.div_yield_id),
    /// Read the contract's initial level.
    /// @returns Strike-set underlying level anchoring the first period's return. None (the default) uses the spot at the valuation date, which is only correct for a new trade priced on its strike-set date. **Required for seasoned trades** (any reset date strictly before as_of): pricing errors otherwise. Do not duplicate the strike-set date inside reset_dates when providing this — reset dates are period-end observations.
    initial_level as initialLevel: "number | null" => json(i.initial_level),
    /// Read the contract's past fixings.
    /// @returns Observed underlying fixings for seasoned trades (date, level pairs). Every reset date strictly before the valuation date must have a matching fixing here; pricing errors otherwise. Locked-in period returns are computed deterministically from these fixings (with local cap/floor applied) and only the remaining future periods are simulated.
    past_fixings as pastFixings: "generated.valuations.CliquetOption['past_fixings']" => dated_nums(i.past_fixings),
    /// Read the contract's payoff type.
    /// @returns Payoff aggregation type (default: Additive)
    payoff_type as payoffType: "generated.valuations.CliquetOption['payoff_type']" => json(i.payoff_type),
});
common_getters!(JsCliquetOption, "CliquetOption");

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.CliquetOption.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsCliquetOptionBuilder, "CliquetOptionBuilder", CliquetOptionBuilder, JsCliquetOption, |b| b.build()
);

to_builder_method!(
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    JsCliquetOption, "CliquetOption", JsCliquetOptionBuilder
);

setters!(JsCliquetOptionBuilder, "CliquetOptionBuilder", {
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
    /// Set the contract's reset dates.
    /// @param value - Reset dates for periodic return locking
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    reset_dates as resetDates: "generated.valuations.CliquetOption['reset_dates']" => js_wire,
    /// Set the contract's expiry.
    /// @param value - Explicit terminal expiry date for the structure.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    expiry as expiry: "string" => js_date,
    /// Set the contract's local cap.
    /// @param value - Local cap on individual period returns
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    local_cap as localCap: "number" => js_f64,
    /// Set the contract's local floor.
    /// @param value - Local floor on individual period returns (default 0.0)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    local_floor as localFloor: "number" => js_f64,
    /// Set the contract's global cap.
    /// @param value - Global cap on sum of all period returns
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    global_cap as globalCap: "number" => js_f64,
    /// Set the contract's global floor.
    /// @param value - Global floor on sum of all period returns (default 0.0)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    global_floor as globalFloor: "number" => js_f64,
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
    /// Set the contract's path model.
    /// @param value - Explicit path model selection. AtmTermGbm is an ATM-term-structure approximation and does not model equity strike skew; callers must opt into that limitation.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    path_model as pathModel: "generated.valuations.CliquetOption['path_model']" => js_wire,
    /// Set the contract's div yield id.
    /// @param value - Optional dividend-yield scalar ID. Some(id): lookup MUST succeed (a missing or non-unitless scalar returns an error). None: no implicit default; treated as zero continuous dividend yield. Set explicitly for index underlyings.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    div_yield_id as divYieldId: "string | null" => optional_id via div_yield_id_opt,
    /// Set the contract's initial level.
    /// @param value - Strike-set underlying level anchoring the first period's return. None (the default) uses the spot at the valuation date, which is only correct for a new trade priced on its strike-set date. **Required for seasoned trades** (any reset date strictly before as_of): pricing errors otherwise. Do not duplicate the strike-set date inside reset_dates when providing this — reset dates are period-end observations.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    initial_level as initialLevel: "number | null" => opt_f64 via initial_level_opt,
    /// Set the contract's past fixings.
    /// @param value - Observed underlying fixings for seasoned trades (date, level pairs). Every reset date strictly before the valuation date must have a matching fixing here; pricing errors otherwise. Locked-in period returns are computed deterministically from these fixings (with local cap/floor applied) and only the remaining future periods are simulated.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    past_fixings as pastFixings: "generated.valuations.CliquetOption['past_fixings']" => js_wire,
    /// Set the contract's payoff type.
    /// @param value - Payoff aggregation type (default: Additive)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    payoff_type as payoffType: "generated.valuations.CliquetOption['payoff_type']" => js_wire,
});
common_setters!(JsCliquetOptionBuilder, "CliquetOptionBuilder");

handle_setters!(JsCliquetOptionBuilder, "CliquetOptionBuilder", {
    /// Set the contract's notional.
    /// @param value - Notional amount
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    notional as notional => JsMoney,
    /// Set the contract's day count.
    /// @param value - Day count convention
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    day_count as dayCount => JsDayCount,
});
