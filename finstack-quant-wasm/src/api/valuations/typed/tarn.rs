//! A note redeeming when its accumulated coupon reaches the contractual target.

use crate::api::core::dates::JsDayCount;
use crate::api::core::money::JsMoney;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::exotics::tarn::{Tarn, TarnBuilder};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// A note redeeming when its accumulated coupon reaches the contractual target.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.Tarn.example();
    /// console.log(future.id);
    /// future.free();
    /// ```
    JsTarn, "Tarn", Tarn
);
instrument_surface!(JsTarn, "Tarn", JsTarnBuilder, "generated.valuations.Tarn");
factories!(JsTarn, "Tarn", Tarn, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsTarn, "Tarn", |i| {
    /// Effective expiry of the contract.
    /// @returns The ISO expiry, or null if the instrument defines none.
    expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's fixed rate.
    /// @returns Fixed coupon rate (the "strike" rate) as a decimal annual rate (0.05 = 5%).
    fixed_rate as fixedRate: "string" => text(i.fixed_rate),
    /// Read the contract's coupon floor.
    /// @returns Floor on each period's coupon (typically 0.0).
    coupon_floor as couponFloor: "number" => json(i.coupon_floor),
    /// Read the contract's target coupon.
    /// @returns Target cumulative coupon level (triggers early redemption). A zero target is allowed and represents redemption at the first future coupon date without coupon accrual.
    target_coupon as targetCoupon: "number" => json(i.target_coupon),
    /// Read the contract's notional.
    /// @returns Notional amount.
    notional as notional: "Money" => money(i.notional),
    /// Read the contract's start date.
    /// @returns Accrual start of the first coupon period; also the first in-advance fixing date.
    start_date as startDate: "string" => date(i.start_date),
    /// Read the contract's payment dates.
    /// @returns Coupon payment dates, one per period, strictly ascending and after start_date. Period i accrues from the previous payment date (or start_date) to payment_dates[i] and fixes in advance at its start.
    payment_dates as paymentDates: "generated.valuations.Tarn['payment_dates']" => dates(i.payment_dates),
    /// Read the contract's index tenor.
    /// @returns Contractual tenor of the observed floating index (e.g. 3M, 6M); must match the forward curve tenor.
    index_tenor as indexTenor: "generated.valuations.Tarn['index_tenor']" => json(i.index_tenor),
    /// Read the contract's forward curve id.
    /// @returns Rates forward curve that projects the floating index (also the fixing-series key).
    forward_curve_id as forwardCurveId: "string" => json(i.forward_curve_id),
    /// Read the contract's discount curve id.
    /// @returns Discount curve ID for PV calculations.
    discount_curve_id as discountCurveId: "string" => json(i.discount_curve_id),
    /// Read the contract's vol surface id.
    /// @returns Optional normal-vol surface used to infer HW1F short-rate σ for stress scenarios.
    vol_surface_id as volSurfaceId: "string | null" => json(i.vol_surface_id),
    /// Read the contract's day count.
    /// @returns Day count convention for coupon accrual.
    day_count as dayCount: "DayCount" => day_count(i.day_count),
});
common_getters!(JsTarn, "Tarn");

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.Tarn.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsTarnBuilder, "TarnBuilder", TarnBuilder, JsTarn, |b| b.build()
);

#[wasm_bindgen(js_class = Tarn)]
impl JsTarn {
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    #[wasm_bindgen(js_name = toBuilder)]
    pub fn to_builder(&self) -> JsTarnBuilder {
        JsTarnBuilder {
            inner: super::Staged::new(self.inner.to_builder()),
        }
    }
}

setters!(JsTarnBuilder, "TarnBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's fixed rate.
    /// @param value - Fixed coupon rate (the "strike" rate) as a decimal annual rate (0.05 = 5%).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    fixed_rate as fixedRate: "string" => exact_decimal,
    /// Set the contract's coupon floor.
    /// @param value - Floor on each period's coupon (typically 0.0).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    coupon_floor as couponFloor: "number" => js_wire,
    /// Set the contract's target coupon.
    /// @param value - Target cumulative coupon level (triggers early redemption). A zero target is allowed and represents redemption at the first future coupon date without coupon accrual.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    target_coupon as targetCoupon: "number" => js_wire,
    /// Set the contract's start date.
    /// @param value - Accrual start of the first coupon period; also the first in-advance fixing date.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    start_date as startDate: "string" => js_date,
    /// Set the contract's payment dates.
    /// @param value - Coupon payment dates, one per period, strictly ascending and after start_date. Period i accrues from the previous payment date (or start_date) to payment_dates[i] and fixes in advance at its start.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    payment_dates as paymentDates: "generated.valuations.Tarn['payment_dates']" => js_wire,
    /// Set the contract's index tenor.
    /// @param value - Contractual tenor of the observed floating index (e.g. 3M, 6M); must match the forward curve tenor.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    index_tenor as indexTenor: "generated.valuations.Tarn['index_tenor']" => js_wire,
    /// Set the contract's forward curve id.
    /// @param value - Rates forward curve that projects the floating index (also the fixing-series key).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    forward_curve_id as forwardCurveId: "string" => id,
    /// Set the contract's discount curve id.
    /// @param value - Discount curve ID for PV calculations.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    discount_curve_id as discountCurveId: "string" => id,
    /// Set the contract's vol surface id.
    /// @param value - Optional normal-vol surface used to infer HW1F short-rate σ for stress scenarios.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    vol_surface_id as volSurfaceId: "string | null" => optional_id via vol_surface_id_opt,
});
common_setters!(JsTarnBuilder, "TarnBuilder");

handle_setters!(JsTarnBuilder, "TarnBuilder", {
    /// Set the contract's notional.
    /// @param value - Notional amount.
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    notional as notional => JsMoney,
    /// Set the contract's day count.
    /// @param value - Day count convention for coupon accrual.
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    day_count as dayCount => JsDayCount,
});
