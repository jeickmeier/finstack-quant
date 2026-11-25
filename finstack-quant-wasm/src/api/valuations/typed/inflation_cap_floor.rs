//! An inflation cap or floor with specified coupon and observation terms.

use crate::api::core::dates::JsDayCount;
use crate::api::core::money::JsMoney;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::rates::inflation_cap_floor::{
    InflationCapFloor, InflationCapFloorBuilder,
};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// An inflation cap or floor with specified coupon and observation terms.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.InflationCapFloor.example();
    /// console.log(future.id);
    /// future.free();
    /// ```
    JsInflationCapFloor, "InflationCapFloor", InflationCapFloor
);
instrument_surface!(
    JsInflationCapFloor,
    "InflationCapFloor",
    JsInflationCapFloorBuilder,
    "generated.valuations.InflationCapFloor"
);
factories!(JsInflationCapFloor, "InflationCapFloor", InflationCapFloor, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsInflationCapFloor, "InflationCapFloor", |i| {
    /// Effective expiry of the contract.
    /// @returns The ISO expiry, or null if the instrument defines none.
    expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's rate option type.
    /// @returns Cap/floor type (cap, floor, caplet, floorlet). Caplet and floorlet price a single period.
    rate_option_type as rateOptionType: "generated.valuations.InflationCapFloor['rate_option_type']" => json(i.rate_option_type),
    /// Read the contract's notional.
    /// @returns Notional amount in quote currency.
    notional as notional: "Money" => money(i.notional),
    /// Read the contract's strike.
    /// @returns Strike (annualized, decimal).
    strike as strike: "string" => text(i.strike),
    /// Read the contract's start date.
    /// @returns Start date of the first inflation period.
    start_date as startDate: "string" => date(i.start_date),
    /// Read the contract's maturity.
    /// @returns End date of the final inflation period.
    maturity as maturity: "string" => date(i.maturity),
    /// Read the contract's frequency.
    /// @returns Payment frequency (ignored for caplet/floorlet).
    frequency as frequency: "generated.valuations.InflationCapFloor['frequency']" => json(i.frequency),
    /// Read the contract's day count.
    /// @returns Day count convention for accrual. Option quote time always uses ACT/365F.
    day_count as dayCount: "DayCount" => day_count(i.day_count),
    /// Read the contract's stub.
    /// @returns Schedule stub convention.
    stub as stub: "generated.valuations.InflationCapFloor['stub']" => json(i.stub),
    /// Read the contract's business day convention.
    /// @returns Business day convention for schedule and payments.
    business_day_convention as businessDayConvention: "generated.valuations.InflationCapFloor['business_day_convention']" => json(i.business_day_convention),
    /// Read the contract's calendar id.
    /// @returns Optional holiday calendar identifier.
    calendar_id as calendarId: "string | null" => json(i.calendar_id),
    /// Read the contract's inflation index id.
    /// @returns Inflation index/curve identifier (e.g., US-CPI-U).
    inflation_index_id as inflationIndexId: "string" => json(i.inflation_index_id),
    /// Read the contract's discount curve id.
    /// @returns Discount curve identifier.
    discount_curve_id as discountCurveId: "string" => json(i.discount_curve_id),
    /// Read the contract's vol surface id.
    /// @returns Volatility surface identifier.
    vol_surface_id as volSurfaceId: "string" => json(i.vol_surface_id),
    /// Read the contract's lag.
    /// @returns Explicit contractual CPI observation lag; missing terms fail reference resolution.
    lag as lag: "NonNullable<generated.valuations.InflationCapFloor['lag']> | null" => json(i.lag),
    /// Read the contract's interpolation.
    /// @returns Explicit contractual monthly CPI interpolation; no source default is used. Defaults to monthly step interpolation when neither source supplies it.
    interpolation as interpolation: "NonNullable<generated.valuations.InflationCapFloor['interpolation']> | null" => json(i.interpolation),
    /// Read the contract's volatility expiry.
    /// @returns Clock used by the volatility surface's annualized quotes. This is independent of CPI publication policy; changing it requires matching quotes, not merely relabelling the old surface.
    volatility_expiry as volatilityExpiry: "generated.valuations.InflationCapFloor['volatility_expiry']" => json(i.volatility_expiry),
});
common_getters!(JsInflationCapFloor, "InflationCapFloor");

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.InflationCapFloor.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsInflationCapFloorBuilder, "InflationCapFloorBuilder", InflationCapFloorBuilder, JsInflationCapFloor, |b| b.build()
);

#[wasm_bindgen(js_class = InflationCapFloor)]
impl JsInflationCapFloor {
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    #[wasm_bindgen(js_name = toBuilder)]
    pub fn to_builder(&self) -> JsInflationCapFloorBuilder {
        JsInflationCapFloorBuilder {
            inner: super::Staged::new(self.inner.to_builder()),
        }
    }
}

setters!(JsInflationCapFloorBuilder, "InflationCapFloorBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's rate option type.
    /// @param value - Cap/floor type (cap, floor, caplet, floorlet). Caplet and floorlet price a single period.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    rate_option_type as rateOptionType: "generated.valuations.InflationCapFloor['rate_option_type']" => js_wire,
    /// Set the contract's strike.
    /// @param value - Strike (annualized, decimal).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    strike as strike: "string" => exact_decimal,
    /// Set the contract's start date.
    /// @param value - Start date of the first inflation period.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    start_date as startDate: "string" => js_date,
    /// Set the contract's maturity.
    /// @param value - End date of the final inflation period.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    maturity as maturity: "string" => js_date,
    /// Set the contract's frequency.
    /// @param value - Payment frequency (ignored for caplet/floorlet).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    frequency as frequency: "generated.valuations.InflationCapFloor['frequency']" => js_wire,
    /// Set the contract's stub.
    /// @param value - Schedule stub convention.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    stub as stub: "generated.valuations.InflationCapFloor['stub']" => js_wire,
    /// Set the contract's business day convention.
    /// @param value - Business day convention for schedule and payments.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    business_day_convention as businessDayConvention: "generated.valuations.InflationCapFloor['business_day_convention']" => js_wire,
    /// Set the contract's calendar id.
    /// @param value - Optional holiday calendar identifier.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    calendar_id as calendarId: "string | null" => optional_id via calendar_id_opt,
    /// Set the contract's inflation index id.
    /// @param value - Inflation index/curve identifier (e.g., US-CPI-U).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    inflation_index_id as inflationIndexId: "string" => id,
    /// Set the contract's discount curve id.
    /// @param value - Discount curve identifier.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    discount_curve_id as discountCurveId: "string" => id,
    /// Set the contract's vol surface id.
    /// @param value - Volatility surface identifier.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    vol_surface_id as volSurfaceId: "string" => id,
    /// Set the contract's lag.
    /// @param value - Explicit contractual CPI observation lag; missing terms fail reference resolution.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    lag as lag: "NonNullable<generated.valuations.InflationCapFloor['lag']> | null" => opt_wire via lag_opt,
    /// Set the contract's interpolation.
    /// @param value - Explicit contractual monthly CPI interpolation; no source default is used. Defaults to monthly step interpolation when neither source supplies it.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    interpolation as interpolation: "NonNullable<generated.valuations.InflationCapFloor['interpolation']> | null" => opt_wire via interpolation_opt,
    /// Set the contract's volatility expiry.
    /// @param value - Clock used by the volatility surface's annualized quotes. This is independent of CPI publication policy; changing it requires matching quotes, not merely relabelling the old surface.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    volatility_expiry as volatilityExpiry: "generated.valuations.InflationCapFloor['volatility_expiry']" => js_wire,
});
common_setters!(JsInflationCapFloorBuilder, "InflationCapFloorBuilder");

handle_setters!(JsInflationCapFloorBuilder, "InflationCapFloorBuilder", {
    /// Set the contract's notional.
    /// @param value - Notional amount in quote currency.
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    notional as notional => JsMoney,
    /// Set the contract's day count.
    /// @param value - Day count convention for accrual. Option quote time always uses ACT/365F.
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    day_count as dayCount => JsDayCount,
});
