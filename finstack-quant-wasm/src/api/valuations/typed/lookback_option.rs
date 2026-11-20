//! An option referencing the observed extreme of the underlying asset.

use crate::api::core::dates::JsDayCount;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::exotics::lookback_option::{
    LookbackOption, LookbackOptionBuilder,
};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// An option referencing the observed extreme of the underlying asset.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.LookbackOption.example();
    /// console.log(future.id);
    /// future.free();
    /// ```
    JsLookbackOption, "LookbackOption", LookbackOption
);
instrument_surface!(
    JsLookbackOption,
    "LookbackOption",
    JsLookbackOptionBuilder,
    "generated.valuations.LookbackOption"
);
factories!(JsLookbackOption, "LookbackOption", LookbackOption, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsLookbackOption, "LookbackOption", |i| {
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's underlying ticker.
    /// @returns Underlying asset ticker symbol
    underlying_ticker as underlyingTicker: "string" => json(i.underlying_ticker),
    /// Read the contract's strike.
    /// @returns Strike price (None for floating strike lookbacks)
    strike as strike: "NonNullable<generated.valuations.LookbackOption['strike']> | null" => json(i.strike),
    /// Read the contract's option type.
    /// @returns Option type (call or put)
    option_type as optionType: "generated.valuations.LookbackOption['option_type']" => json(i.option_type),
    /// Read the contract's lookback type.
    /// @returns Lookback type (fixed or floating strike)
    lookback_type as lookbackType: "generated.valuations.LookbackOption['lookback_type']" => json(i.lookback_type),
    /// Read the contract's expiry.
    /// @returns Option expiry date
    expiry as expiry: "string" => date(i.expiry),
    /// Read the contract's expiry fixing.
    /// @returns Terminal underlying fixing observed at expiry, in the same quote units as strike. Required when valuing after expiry so the realized payoff cannot move with a later market spot snapshot. At expiry itself, the current market spot is treated as the terminal fixing when this field is absent.
    expiry_fixing as expiryFixing: "number | null" => json(i.expiry_fixing),
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
    /// Read the contract's monitoring.
    /// @returns Contractual monitoring of the path extremum. continuous (default) prices with the Goldman-Sosin-Gatto closed form. discrete observes the extremum only on the strictly increasing observation_dates (no later than expiry) and prices by Monte Carlo.
    monitoring as monitoring: "generated.valuations.LookbackOption['monitoring']" => json(i.monitoring),
    /// Read the contract's observed min.
    /// @returns Observed minimum underlying level since inception, in the same quote units as strike (required for Floating Call / Fixed Put once seasoned).
    observed_min as observedMin: "number | null" => json(i.observed_min),
    /// Read the contract's observed max.
    /// @returns Observed maximum underlying level since inception, in the same quote units as strike (required for Floating Put / Fixed Call once seasoned).
    observed_max as observedMax: "number | null" => json(i.observed_max),
});
common_getters!(JsLookbackOption, "LookbackOption");

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.LookbackOption.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsLookbackOptionBuilder, "LookbackOptionBuilder", LookbackOptionBuilder, JsLookbackOption, |b| b.build()
);

#[wasm_bindgen(js_class = LookbackOption)]
impl JsLookbackOption {
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    #[wasm_bindgen(js_name = toBuilder)]
    pub fn to_builder(&self) -> JsLookbackOptionBuilder {
        JsLookbackOptionBuilder {
            inner: super::Staged::new(self.inner.to_builder()),
        }
    }
}

setters!(JsLookbackOptionBuilder, "LookbackOptionBuilder", {
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
    /// @param value - Strike price (None for floating strike lookbacks)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    strike as strike: "NonNullable<generated.valuations.LookbackOption['strike']> | null" => js_wire via strike_opt,
    /// Set the contract's option type.
    /// @param value - Option type (call or put)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    option_type as optionType: "generated.valuations.LookbackOption['option_type']" => js_wire,
    /// Set the contract's lookback type.
    /// @param value - Lookback type (fixed or floating strike)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    lookback_type as lookbackType: "generated.valuations.LookbackOption['lookback_type']" => js_wire,
    /// Set the contract's expiry.
    /// @param value - Option expiry date
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    expiry as expiry: "string" => js_date,
    /// Set the contract's expiry fixing.
    /// @param value - Terminal underlying fixing observed at expiry, in the same quote units as strike. Required when valuing after expiry so the realized payoff cannot move with a later market spot snapshot. At expiry itself, the current market spot is treated as the terminal fixing when this field is absent.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    expiry_fixing as expiryFixing: "number | null" => js_wire via expiry_fixing_opt,
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
    /// Set the contract's monitoring.
    /// @param value - Contractual monitoring of the path extremum. continuous (default) prices with the Goldman-Sosin-Gatto closed form. discrete observes the extremum only on the strictly increasing observation_dates (no later than expiry) and prices by Monte Carlo.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    monitoring as monitoring: "generated.valuations.LookbackOption['monitoring']" => js_wire,
    /// Set the contract's observed min.
    /// @param value - Observed minimum underlying level since inception, in the same quote units as strike (required for Floating Call / Fixed Put once seasoned).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    observed_min as observedMin: "number | null" => js_wire via observed_min_opt,
    /// Set the contract's observed max.
    /// @param value - Observed maximum underlying level since inception, in the same quote units as strike (required for Floating Put / Fixed Call once seasoned).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    observed_max as observedMax: "number | null" => js_wire via observed_max_opt,
});
common_setters!(JsLookbackOptionBuilder, "LookbackOptionBuilder");

handle_setters!(JsLookbackOptionBuilder, "LookbackOptionBuilder", {
    /// Set the contract's day count.
    /// @param value - Day count convention
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    day_count as dayCount => JsDayCount,
});
