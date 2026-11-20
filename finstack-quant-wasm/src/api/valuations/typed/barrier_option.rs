//! An option whose payoff is conditional on an observed barrier.

use crate::api::core::dates::JsDayCount;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::exotics::barrier_option::{
    BarrierOption, BarrierOptionBuilder,
};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// An option whose payoff is conditional on an observed barrier.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.BarrierOption.example();
    /// console.log(future.id);
    /// future.free();
    /// ```
    JsBarrierOption, "BarrierOption", BarrierOption
);
instrument_surface!(
    JsBarrierOption,
    "BarrierOption",
    JsBarrierOptionBuilder,
    "generated.valuations.BarrierOption"
);
factories!(JsBarrierOption, "BarrierOption", BarrierOption, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsBarrierOption, "BarrierOption", |i| {
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's underlying ticker.
    /// @returns Underlying asset ticker symbol
    underlying_ticker as underlyingTicker: "string" => json(i.underlying_ticker),
    /// Read the contract's strike.
    /// @returns Strike price
    strike as strike: "number" => json(i.strike),
    /// Read the contract's barrier.
    /// @returns Barrier level: the absolute underlying price that triggers the knock-in/out, in the same quote units as strike.
    barrier as barrier: "number" => json(i.barrier),
    /// Read the contract's rebate.
    /// @returns Total contractual trade rebate in the payoff currency, independent of notional. Knock-outs pay on a hit according to rebate_timing; knock-ins pay at expiry only if no hit occurred.
    rebate as rebate: "NonNullable<generated.valuations.BarrierOption['rebate']> | null" => json(i.rebate),
    /// Read the contract's rebate timing.
    /// @returns Payment timing of the rebate. Required exactly when rebate is set. For a knock-out, at_hit pays the rebate at the first qualifying hit and at_expiry defers it to expiry. A knock-in (no-hit) rebate is only determined at expiry, so it must be at_expiry; at_hit is rejected. The analytical pricer values at-hit knock-out rebates via the discounted first-passage closed form; Monte Carlo applies at-hit timing via with_rebate_at_hit.
    rebate_timing as rebateTiming: "NonNullable<generated.valuations.BarrierOption['rebate_timing']> | null" => json(i.rebate_timing),
    /// Read the contract's option type.
    /// @returns Option type (call or put)
    option_type as optionType: "generated.valuations.BarrierOption['option_type']" => json(i.option_type),
    /// Read the contract's barrier type.
    /// @returns Barrier type (up/down, in/out)
    barrier_type as barrierType: "generated.valuations.BarrierOption['barrier_type']" => json(i.barrier_type),
    /// Read the contract's expiry.
    /// @returns Option expiry date
    expiry as expiry: "string" => date(i.expiry),
    /// Read the contract's expiry fixing.
    /// @returns Terminal underlying fixing observed at expiry, in the same quote units as strike. Required when valuing after expiry so the realized intrinsic value is invariant to later market spot updates. At expiry, the current market spot is used when this field is absent.
    expiry_fixing as expiryFixing: "number | null" => json(i.expiry_fixing),
    /// Read the contract's observed barrier breached.
    /// @returns Observed barrier state for expired options. Historical barrier monitoring must be supplied explicitly for expired options because terminal spot alone does not reveal whether the barrier was breached intralife and then reversed.
    observed_barrier_breached as observedBarrierBreached: "boolean | null" => json(i.observed_barrier_breached),
    /// Read the contract's quantity.
    /// @returns Number of underlying units the option is written on; PV and Greeks scale linearly with it.
    quantity as quantity: "number" => json(i.quantity),
    /// Read the contract's currency.
    /// @returns Currency of the strike, premium and present value.
    currency as currency: "string" => text(i.currency),
    /// Read the contract's day count.
    /// @returns Day count convention
    day_count as dayCount: "DayCount" => day_count(i.day_count),
    /// Read the contract's monitoring.
    /// @returns Contractual monitoring: continuous or an explicit, strictly increasing set of observation dates no later than expiry. Discrete pricing does not interpolate hits between those dates.
    monitoring as monitoring: "generated.valuations.BarrierOption['monitoring']" => json(i.monitoring),
    /// Read the contract's discount curve id.
    /// @returns Discount curve ID for present value calculations
    discount_curve_id as discountCurveId: "string" => json(i.discount_curve_id),
    /// Read the contract's spot id.
    /// @returns Spot price identifier
    spot_id as spotId: "string" => json(i.spot_id),
    /// Read the contract's vol surface id.
    /// @returns Volatility surface ID
    vol_surface_id as volSurfaceId: "string" => json(i.vol_surface_id),
    /// Read the contract's div yield id.
    /// @returns Optional dividend-yield scalar ID
    div_yield_id as divYieldId: "string | null" => json(i.div_yield_id),
});
common_getters!(JsBarrierOption, "BarrierOption");

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.BarrierOption.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsBarrierOptionBuilder, "BarrierOptionBuilder", BarrierOptionBuilder, JsBarrierOption, |b| b.build()
);

#[wasm_bindgen(js_class = BarrierOption)]
impl JsBarrierOption {
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    #[wasm_bindgen(js_name = toBuilder)]
    pub fn to_builder(&self) -> JsBarrierOptionBuilder {
        JsBarrierOptionBuilder {
            inner: super::Staged::new(self.inner.to_builder()),
        }
    }
}

setters!(JsBarrierOptionBuilder, "BarrierOptionBuilder", {
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
    strike as strike: "number" => js_wire,
    /// Set the contract's barrier.
    /// @param value - Barrier level: the absolute underlying price that triggers the knock-in/out, in the same quote units as strike.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    barrier as barrier: "number" => js_wire,
    /// Set the contract's rebate.
    /// @param value - Total contractual trade rebate in the payoff currency, independent of notional. Knock-outs pay on a hit according to rebate_timing; knock-ins pay at expiry only if no hit occurred.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    rebate as rebate: "NonNullable<generated.valuations.BarrierOption['rebate']> | null" => js_wire via rebate_opt,
    /// Set the contract's rebate timing.
    /// @param value - Payment timing of the rebate. Required exactly when rebate is set. For a knock-out, at_hit pays the rebate at the first qualifying hit and at_expiry defers it to expiry. A knock-in (no-hit) rebate is only determined at expiry, so it must be at_expiry; at_hit is rejected. The analytical pricer values at-hit knock-out rebates via the discounted first-passage closed form; Monte Carlo applies at-hit timing via with_rebate_at_hit.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    rebate_timing as rebateTiming: "NonNullable<generated.valuations.BarrierOption['rebate_timing']> | null" => js_wire via rebate_timing_opt,
    /// Set the contract's option type.
    /// @param value - Option type (call or put)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    option_type as optionType: "generated.valuations.BarrierOption['option_type']" => js_wire,
    /// Set the contract's barrier type.
    /// @param value - Barrier type (up/down, in/out)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    barrier_type as barrierType: "generated.valuations.BarrierOption['barrier_type']" => js_wire,
    /// Set the contract's expiry.
    /// @param value - Option expiry date
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    expiry as expiry: "string" => js_date,
    /// Set the contract's expiry fixing.
    /// @param value - Terminal underlying fixing observed at expiry, in the same quote units as strike. Required when valuing after expiry so the realized intrinsic value is invariant to later market spot updates. At expiry, the current market spot is used when this field is absent.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    expiry_fixing as expiryFixing: "number | null" => js_wire via expiry_fixing_opt,
    /// Set the contract's observed barrier breached.
    /// @param value - Observed barrier state for expired options. Historical barrier monitoring must be supplied explicitly for expired options because terminal spot alone does not reveal whether the barrier was breached intralife and then reversed.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    observed_barrier_breached as observedBarrierBreached: "boolean | null" => js_wire via observed_barrier_breached_opt,
    /// Set the contract's quantity.
    /// @param value - Number of underlying units the option is written on; PV and Greeks scale linearly with it.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    quantity as quantity: "number" => js_wire,
    /// Set the contract's currency.
    /// @param value - Currency of the strike, premium and present value.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    currency as currency: "string" => js_wire,
    /// Set the contract's monitoring.
    /// @param value - Contractual monitoring: continuous or an explicit, strictly increasing set of observation dates no later than expiry. Discrete pricing does not interpolate hits between those dates.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    monitoring as monitoring: "generated.valuations.BarrierOption['monitoring']" => js_wire,
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
});
common_setters!(JsBarrierOptionBuilder, "BarrierOptionBuilder");

handle_setters!(JsBarrierOptionBuilder, "BarrierOptionBuilder", {
    /// Set the contract's day count.
    /// @param value - Day count convention
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    day_count as dayCount => JsDayCount,
});
