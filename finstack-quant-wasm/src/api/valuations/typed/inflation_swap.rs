//! A zero-coupon swap exchanging fixed and cumulative inflation-linked payments.

use crate::api::core::dates::JsDayCount;
use crate::api::core::money::JsMoney;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::rates::inflation_swap::{
    InflationSwap, InflationSwapBuilder,
};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// A zero-coupon swap exchanging fixed and cumulative inflation-linked payments.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.InflationSwap.example();
    /// console.log(future.id);
    /// future.free();
    /// ```
    JsInflationSwap, "InflationSwap", InflationSwap
);
instrument_surface!(
    JsInflationSwap,
    "InflationSwap",
    JsInflationSwapBuilder,
    "generated.valuations.InflationSwap"
);
factories!(JsInflationSwap, "InflationSwap", InflationSwap, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsInflationSwap, "InflationSwap", |i| {
    /// Effective expiry of the contract.
    /// @returns The ISO expiry, or null if the instrument defines none.
    expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's notional.
    /// @returns Notional in quote currency
    notional as notional: "Money" => money(i.notional),
    /// Read the contract's start date.
    /// @returns Start date of indexation
    start_date as startDate: "string" => date(i.start_date),
    /// Read the contract's maturity.
    /// @returns Maturity date
    maturity as maturity: "string" => date(i.maturity),
    /// Read the contract's fixed rate.
    /// @returns Fixed real rate (as decimal)
    fixed_rate as fixedRate: "string" => text(i.fixed_rate),
    /// Read the contract's inflation index id.
    /// @returns Inflation index identifier (e.g., US-CPI-U)
    inflation_index_id as inflationIndexId: "string" => text(i.inflation_index_id),
    /// Read the contract's discount curve id.
    /// @returns Discount curve identifier (quote currency)
    discount_curve_id as discountCurveId: "string" => text(i.discount_curve_id),
    /// Read the contract's day count.
    /// @returns Day count for accrual calculation (fixed leg compounding)
    day_count as dayCount: "DayCount" => day_count(i.day_count),
    /// Read the contract's side.
    /// @returns Trade side
    side as side: "generated.valuations.InflationSwap['side']" => json(i.side),
    /// Read the contract's lag.
    /// @returns Explicit contractual CPI observation lag; missing terms fail reference resolution.
    lag as lag: "NonNullable<generated.valuations.InflationSwap['lag']> | null" => json(i.lag),
    /// Read the contract's interpolation.
    /// @returns Explicit contractual monthly reference-index interpolation; no source default is used.
    interpolation as interpolation: "NonNullable<generated.valuations.InflationSwap['interpolation']> | null" => json(i.interpolation),
    /// Read the contract's base cpi.
    /// @returns Explicit Base CPI (reference index level at start with lag applied). If not provided, it will be looked up/calculated from start date.
    base_cpi as baseCpi: "number | null" => json(i.base_cpi),
    /// Read the contract's business day convention.
    /// @returns Business day convention for payment date adjustment. Defaults to Following if not specified.
    business_day_convention as businessDayConvention: "generated.valuations.InflationSwap['business_day_convention']" => json(i.business_day_convention),
    /// Read the contract's calendar id.
    /// @returns Holiday calendar identifier for payment date adjustment. If not specified, payment dates are used unadjusted.
    calendar_id as calendarId: "string | null" => opt_text(i.calendar_id),
});
common_getters!(JsInflationSwap, "InflationSwap");
to_builder_method!(
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    JsInflationSwap,
    "InflationSwap",
    JsInflationSwapBuilder
);

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.InflationSwap.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsInflationSwapBuilder, "InflationSwapBuilder", InflationSwapBuilder, JsInflationSwap, |b| b.build()
);

setters!(JsInflationSwapBuilder, "InflationSwapBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's start date.
    /// @param value - Start date of indexation
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    start_date as startDate: "string" => js_date,
    /// Set the contract's maturity.
    /// @param value - Maturity date
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    maturity as maturity: "string" => js_date,
    /// Set the contract's fixed rate.
    /// @param value - Fixed real rate (as decimal)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    fixed_rate as fixedRate: "string" => exact_decimal,
    /// Set the contract's inflation index id.
    /// @param value - Inflation index identifier (e.g., US-CPI-U)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    inflation_index_id as inflationIndexId: "string" => id,
    /// Set the contract's discount curve id.
    /// @param value - Discount curve identifier (quote currency)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    discount_curve_id as discountCurveId: "string" => id,
    /// Set the contract's side.
    /// @param value - Trade side
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    side as side: "generated.valuations.InflationSwap['side']" => js_wire,
    /// Set the contract's lag.
    /// @param value - Explicit contractual CPI observation lag; missing terms fail reference resolution.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    lag as lag: "NonNullable<generated.valuations.InflationSwap['lag']> | null" => opt_wire via lag_opt,
    /// Set the contract's interpolation.
    /// @param value - Explicit contractual monthly reference-index interpolation; no source default is used.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    interpolation as interpolation: "NonNullable<generated.valuations.InflationSwap['interpolation']> | null" => opt_wire via interpolation_opt,
    /// Set the contract's base cpi.
    /// @param value - Explicit Base CPI (reference index level at start with lag applied). If not provided, it will be looked up/calculated from start date.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    base_cpi as baseCpi: "number | null" => opt_f64 via base_cpi_opt,
    /// Set the contract's business day convention.
    /// @param value - Business day convention for payment date adjustment. Defaults to Following if not specified.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    business_day_convention as businessDayConvention: "generated.valuations.InflationSwap['business_day_convention']" => js_wire,
    /// Set the contract's calendar id.
    /// @param value - Holiday calendar identifier for payment date adjustment. If not specified, payment dates are used unadjusted.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    calendar_id as calendarId: "string | null" => optional_id via calendar_id_opt,
});
common_setters!(JsInflationSwapBuilder, "InflationSwapBuilder");

handle_setters!(JsInflationSwapBuilder, "InflationSwapBuilder", {
    /// Set the contract's notional.
    /// @param value - Notional in quote currency
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    notional as notional => JsMoney,
    /// Set the contract's day count.
    /// @param value - Day count for accrual calculation (fixed leg compounding)
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    day_count as dayCount => JsDayCount,
});
