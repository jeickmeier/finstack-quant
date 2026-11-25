//! A structured note whose future coupons depend on prior coupon states.

use crate::api::core::dates::JsDayCount;
use crate::api::core::money::JsMoney;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::exotics::snowball::{Snowball, SnowballBuilder};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// A structured note whose future coupons depend on prior coupon states.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.Snowball.example();
    /// console.log(future.id);
    /// future.free();
    /// ```
    JsSnowball, "Snowball", Snowball
);
instrument_surface!(
    JsSnowball,
    "Snowball",
    JsSnowballBuilder,
    "generated.valuations.Snowball"
);
factories!(JsSnowball, "Snowball", Snowball, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsSnowball, "Snowball", |i| {
    /// Effective expiry of the contract.
    /// @returns The ISO expiry, or null if the instrument defines none.
    expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's variant.
    /// @returns Snowball or inverse floater variant.
    variant as variant: "generated.valuations.Snowball['variant']" => json(i.variant),
    /// Read the contract's initial coupon.
    /// @returns Initial coupon for snowball (c_0); ignored for inverse floater.
    initial_coupon as initialCoupon: "number" => json(i.initial_coupon),
    /// Read the contract's fixed rate.
    /// @returns Fixed rate component as a decimal annual rate (0.05 = 5%).
    fixed_rate as fixedRate: "string" => text(i.fixed_rate),
    /// Read the contract's gearing.
    /// @returns Multiplier on the floating fixing (1.0 for snowball, variable for inverse floater).
    gearing as gearing: "number" => json(i.gearing),
    /// Read the contract's coupon floor.
    /// @returns Floor on each period coupon (typically 0.0).
    coupon_floor as couponFloor: "number" => json(i.coupon_floor),
    /// Read the contract's coupon cap.
    /// @returns Optional cap on each period coupon.
    coupon_cap as couponCap: "number | null" => json(i.coupon_cap),
    /// Read the contract's notional.
    /// @returns Notional amount.
    notional as notional: "Money" => money(i.notional),
    /// Read the contract's start date.
    /// @returns Accrual start of the first coupon period; also the first in-advance fixing date.
    start_date as startDate: "string" => date(i.start_date),
    /// Read the contract's payment dates.
    /// @returns Coupon payment dates, one per period, strictly ascending and after start_date. Period i accrues from the previous payment date (or start_date) to payment_dates[i] and fixes in advance at its start.
    payment_dates as paymentDates: "generated.valuations.Snowball['payment_dates']" => dates(i.payment_dates),
    /// Read the contract's forward curve id.
    /// @returns Rates forward curve that projects the floating index (also the fixing-series key).
    forward_curve_id as forwardCurveId: "string" => text(i.forward_curve_id),
    /// Read the contract's index tenor.
    /// @returns Contractual tenor of the observed floating index (must match the forward curve tenor).
    index_tenor as indexTenor: "generated.valuations.Snowball['index_tenor']" => json(i.index_tenor),
    /// Read the contract's discount curve id.
    /// @returns Discount curve ID.
    discount_curve_id as discountCurveId: "string" => text(i.discount_curve_id),
    /// Read the contract's vol surface id.
    /// @returns Optional normal-vol surface used to infer HW1F short-rate σ for stress scenarios.
    vol_surface_id as volSurfaceId: "string | null" => opt_text(i.vol_surface_id),
    /// Read the contract's call provision.
    /// @returns Optional Bermudan call provision.
    call_provision as callProvision: "NonNullable<generated.valuations.Snowball['call_provision']> | null" => json(i.call_provision),
    /// Read the contract's day count.
    /// @returns Day count convention.
    day_count as dayCount: "DayCount" => day_count(i.day_count),
});
common_getters!(JsSnowball, "Snowball");

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.Snowball.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsSnowballBuilder, "SnowballBuilder", SnowballBuilder, JsSnowball, |b| b.build()
);

to_builder_method!(
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    JsSnowball, "Snowball", JsSnowballBuilder
);

setters!(JsSnowballBuilder, "SnowballBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's variant.
    /// @param value - Snowball or inverse floater variant.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    variant as variant: "generated.valuations.Snowball['variant']" => js_wire,
    /// Set the contract's initial coupon.
    /// @param value - Initial coupon for snowball (c_0); ignored for inverse floater.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    initial_coupon as initialCoupon: "number" => js_f64,
    /// Set the contract's fixed rate.
    /// @param value - Fixed rate component as a decimal annual rate (0.05 = 5%).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    fixed_rate as fixedRate: "string" => exact_decimal,
    /// Set the contract's gearing.
    /// @param value - Multiplier on the floating fixing (1.0 for snowball, variable for inverse floater).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    gearing as gearing: "number" => js_f64,
    /// Set the contract's coupon floor.
    /// @param value - Floor on each period coupon (typically 0.0).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    coupon_floor as couponFloor: "number" => js_f64,
    /// Set the contract's coupon cap.
    /// @param value - Optional cap on each period coupon.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    coupon_cap as couponCap: "number | null" => opt_f64 via coupon_cap_opt,
    /// Set the contract's start date.
    /// @param value - Accrual start of the first coupon period; also the first in-advance fixing date.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    start_date as startDate: "string" => js_date,
    /// Set the contract's payment dates.
    /// @param value - Coupon payment dates, one per period, strictly ascending and after start_date. Period i accrues from the previous payment date (or start_date) to payment_dates[i] and fixes in advance at its start.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    payment_dates as paymentDates: "generated.valuations.Snowball['payment_dates']" => js_wire,
    /// Set the contract's forward curve id.
    /// @param value - Rates forward curve that projects the floating index (also the fixing-series key).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    forward_curve_id as forwardCurveId: "string" => id,
    /// Set the contract's index tenor.
    /// @param value - Contractual tenor of the observed floating index (must match the forward curve tenor).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    index_tenor as indexTenor: "generated.valuations.Snowball['index_tenor']" => js_wire,
    /// Set the contract's discount curve id.
    /// @param value - Discount curve ID.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    discount_curve_id as discountCurveId: "string" => id,
    /// Set the contract's vol surface id.
    /// @param value - Optional normal-vol surface used to infer HW1F short-rate σ for stress scenarios.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    vol_surface_id as volSurfaceId: "string | null" => optional_id via vol_surface_id_opt,
    /// Set the contract's call provision.
    /// @param value - Optional Bermudan call provision.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    call_provision as callProvision: "NonNullable<generated.valuations.Snowball['call_provision']> | null" => opt_wire via call_provision_opt,
});
common_setters!(JsSnowballBuilder, "SnowballBuilder");

handle_setters!(JsSnowballBuilder, "SnowballBuilder", {
    /// Set the contract's notional.
    /// @param value - Notional amount.
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    notional as notional => JsMoney,
    /// Set the contract's day count.
    /// @param value - Day count convention.
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    day_count as dayCount => JsDayCount,
});
