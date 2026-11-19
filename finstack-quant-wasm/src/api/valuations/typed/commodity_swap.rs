//! A fixed-for-floating commodity swap across delivery periods.

use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::commodity::commodity_swap::{
    CommoditySwap, CommoditySwapBuilder,
};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// A fixed-for-floating commodity swap across delivery periods.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.CommoditySwap.example();
    /// console.log(future.id);
    /// future.free();
    /// ```
    JsCommoditySwap, "CommoditySwap", CommoditySwap
);
instrument_surface!(
    JsCommoditySwap,
    "CommoditySwap",
    JsCommoditySwapBuilder,
    "generated.valuations.CommoditySwap"
);
factories!(JsCommoditySwap, "CommoditySwap", CommoditySwap, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsCommoditySwap, "CommoditySwap", |i| {
    /// Effective expiry of the contract.
    /// @returns The ISO expiry, or null if the instrument defines none.
    expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's underlying.
    /// @returns Underlying commodity parameters (type, ticker, unit, currency).
    underlying as underlying: "Pick<generated.valuations.CommodityForward, 'commodity_type' | 'underlying_ticker' | 'unit' | 'currency'>" => json(i.underlying),
    /// Read the contract's quantity.
    /// @returns Notional quantity per period.
    quantity as quantity: "number" => json(i.quantity),
    /// Read the contract's fixed price.
    /// @returns Fixed price per commodity unit, in the notional currency (finite).
    fixed_price as fixedPrice: "number" => json(i.fixed_price),
    /// Read the contract's forward curve id.
    /// @returns Commodity forward PriceCurve that projects the floating-leg price observations.
    forward_curve_id as forwardCurveId: "string" => json(i.forward_curve_id),
    /// Read the contract's side.
    /// @returns Direction of the swap: Pay means paying the fixed price leg, Receive means receiving the fixed price leg.
    side as side: "generated.valuations.CommoditySwap['side']" => json(i.side),
    /// Read the contract's start date.
    /// @returns Start date of the swap.
    start_date as startDate: "string" => date(i.start_date),
    /// Read the contract's maturity.
    /// @returns End date of the swap.
    maturity as maturity: "string" => date(i.maturity),
    /// Read the contract's frequency.
    /// @returns Payment frequency as a Tenor.
    frequency as frequency: "generated.valuations.CommoditySwap['frequency']" => json(i.frequency),
    /// Read the contract's calendar id.
    /// @returns Optional calendar ID for date adjustments.
    calendar_id as calendarId: "string | null" => json(i.calendar_id),
    /// Read the contract's business day convention.
    /// @returns Business day convention for payment-schedule date adjustments. Only applied when calendar_id is set and resolves to a registered holiday calendar; without a calendar the schedule dates are left unadjusted.
    business_day_convention as businessDayConvention: "generated.valuations.CommoditySwap['business_day_convention']" => json(i.business_day_convention),
    /// Read the contract's discount curve id.
    /// @returns Discount curve ID.
    discount_curve_id as discountCurveId: "string" => json(i.discount_curve_id),
    /// Read the contract's index lag days.
    /// @returns Optional index lag in **calendar days**: the floating-leg averaging window is shifted back by exactly this many calendar days (no business-day adjustment of the shifted window endpoints).
    index_lag_days as indexLagDays: "number | null" => json(i.index_lag_days),
    /// Read the contract's past fixings.
    /// @returns Realized floating-index fixings as (date, price) pairs. Floating-leg observations with date strictly before the valuation date read from this store; a missing past fixing is an error — no silent substitution of today's spot . Observations on or after the valuation date project from the price curve.
    past_fixings as pastFixings: "generated.valuations.CommoditySwap['past_fixings']" => dated_nums(i.past_fixings),
});
common_getters!(JsCommoditySwap, "CommoditySwap");

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.CommoditySwap.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsCommoditySwapBuilder, "CommoditySwapBuilder", CommoditySwapBuilder, JsCommoditySwap, |b| b.build()
);

#[wasm_bindgen(js_class = CommoditySwap)]
impl JsCommoditySwap {
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    #[wasm_bindgen(js_name = toBuilder)]
    pub fn to_builder(&self) -> JsCommoditySwapBuilder {
        JsCommoditySwapBuilder {
            inner: super::Staged::new(self.inner.to_builder()),
        }
    }
}

setters!(JsCommoditySwapBuilder, "CommoditySwapBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's underlying.
    /// @param value - Underlying commodity parameters (type, ticker, unit, currency).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    underlying as underlying: "Pick<generated.valuations.CommodityForward, 'commodity_type' | 'underlying_ticker' | 'unit' | 'currency'>" => js_wire,
    /// Set the contract's quantity.
    /// @param value - Notional quantity per period.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    quantity as quantity: "number" => js_wire,
    /// Set the contract's fixed price.
    /// @param value - Fixed price per commodity unit, in the notional currency (finite).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    fixed_price as fixedPrice: "number" => js_wire,
    /// Set the contract's forward curve id.
    /// @param value - Commodity forward PriceCurve that projects the floating-leg price observations.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    forward_curve_id as forwardCurveId: "string" => id,
    /// Set the contract's side.
    /// @param value - Direction of the swap: Pay means paying the fixed price leg, Receive means receiving the fixed price leg.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    side as side: "generated.valuations.CommoditySwap['side']" => js_wire,
    /// Set the contract's start date.
    /// @param value - Start date of the swap.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    start_date as startDate: "string" => js_date,
    /// Set the contract's maturity.
    /// @param value - End date of the swap.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    maturity as maturity: "string" => js_date,
    /// Set the contract's frequency.
    /// @param value - Payment frequency as a Tenor.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    frequency as frequency: "generated.valuations.CommoditySwap['frequency']" => js_wire,
    /// Set the contract's calendar id.
    /// @param value - Optional calendar ID for date adjustments.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    calendar_id as calendarId: "string | null" => optional_id via calendar_id_opt,
    /// Set the contract's business day convention.
    /// @param value - Business day convention for payment-schedule date adjustments. Only applied when calendar_id is set and resolves to a registered holiday calendar; without a calendar the schedule dates are left unadjusted.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    business_day_convention as businessDayConvention: "generated.valuations.CommoditySwap['business_day_convention']" => js_wire,
    /// Set the contract's discount curve id.
    /// @param value - Discount curve ID.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    discount_curve_id as discountCurveId: "string" => id,
    /// Set the contract's index lag days.
    /// @param value - Optional index lag in **calendar days**: the floating-leg averaging window is shifted back by exactly this many calendar days (no business-day adjustment of the shifted window endpoints).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    index_lag_days as indexLagDays: "number | null" => js_wire via index_lag_days_opt,
    /// Set the contract's past fixings.
    /// @param value - Realized floating-index fixings as (date, price) pairs. Floating-leg observations with date strictly before the valuation date read from this store; a missing past fixing is an error — no silent substitution of today's spot . Observations on or after the valuation date project from the price curve.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    past_fixings as pastFixings: "generated.valuations.CommoditySwap['past_fixings']" => js_wire,
});
common_setters!(JsCommoditySwapBuilder, "CommoditySwapBuilder");
