//! An autocallable equity-linked claim with explicit trigger and settlement terms.

use crate::api::core::dates::JsDayCount;
use crate::api::core::money::JsMoney;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::equity::autocallable::{
    Autocallable, AutocallableBuilder,
};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// An autocallable equity-linked claim with explicit trigger and settlement terms.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.Autocallable.example();
    /// console.log(future.id);
    /// future.free();
    /// ```
    JsAutocallable, "Autocallable", Autocallable
);
instrument_surface!(
    JsAutocallable,
    "Autocallable",
    JsAutocallableBuilder,
    "generated.valuations.Autocallable"
);
factories!(JsAutocallable, "Autocallable", Autocallable, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsAutocallable, "Autocallable", |i| {
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's underlying ticker.
    /// @returns Underlying asset ticker symbol
    underlying_ticker as underlyingTicker: "string" => json(i.underlying_ticker),
    /// Read the contract's observation dates.
    /// @returns Observation dates for autocall and coupon checks. Barriers are monitored **discretely** at these exact dates only. The Monte Carlo time grid is constructed to include these dates precisely.
    observation_dates as observationDates: "generated.valuations.Autocallable['observation_dates']" => dates(i.observation_dates),
    /// Read the contract's payment dates.
    /// @returns Contractual payment dates for coupons and early redemption. Each entry corresponds to observation_dates at the same index.
    payment_dates as paymentDates: "generated.valuations.Autocallable['payment_dates']" => dates(i.payment_dates),
    /// Read the contract's expiry.
    /// @returns Explicit terminal expiry date for the structure.
    expiry as expiry: "string" => date(i.expiry),
    /// Read the contract's autocall barriers.
    /// @returns Autocall barrier levels (as ratios of initial spot, e.g., 1.0 = 100%). Each barrier corresponds to the observation date at the same index. If spot ≥ barrier × initial_spot on the observation date, the product autocalls.
    autocall_barriers as autocallBarriers: "generated.valuations.Autocallable['autocall_barriers']" => json(i.autocall_barriers),
    /// Read the contract's coupon barriers.
    /// @returns Coupon barriers as ratios of initial spot. Coupon eligibility is independent of the autocall barrier. Each entry corresponds to the observation date at the same index.
    coupon_barriers as couponBarriers: "generated.valuations.Autocallable['coupon_barriers']" => json(i.coupon_barriers),
    /// Read the contract's coupons.
    /// @returns Coupon return amounts for each observation date.
    coupons as coupons: "generated.valuations.Autocallable['coupons']" => json(i.coupons),
    /// Read the contract's memory coupons.
    /// @returns Memory ("Phoenix") coupon feature. When enabled, a coupon whose coupon barrier is missed accrues until a later observation meets its coupon barrier. The current and all accrued coupons are then paid on that observation's payment date. Coupon eligibility remains independent of autocall eligibility.
    memory_coupons as memoryCoupons: "boolean" => json(i.memory_coupons),
    /// Read the contract's final barrier.
    /// @returns Final barrier level for final payoff determination
    final_barrier as finalBarrier: "number" => json(i.final_barrier),
    /// Read the contract's final payoff type.
    /// @returns Type of final payoff (capital protection, participation, knock-in put)
    final_payoff_type as finalPayoffType: "generated.valuations.Autocallable['final_payoff_type']" => json(i.final_payoff_type),
    /// Read the contract's cap level.
    /// @returns Cap level for final payoff (maximum return)
    cap_level as capLevel: "number" => json(i.cap_level),
    /// Read the contract's notional.
    /// @returns Notional amount
    notional as notional: "Money" => money(i.notional),
    /// Read the contract's day count.
    /// @returns Day count convention for interest calculations
    day_count as dayCount: "DayCount" => day_count(i.day_count),
    /// Read the contract's discount curve id.
    /// @returns Discount curve ID for present value calculations
    discount_curve_id as discountCurveId: "string" => json(i.discount_curve_id),
    /// Read the contract's spot id.
    /// @returns Spot price identifier for underlying asset
    spot_id as spotId: "string" => json(i.spot_id),
    /// Read the contract's vol surface id.
    /// @returns Volatility surface ID for option pricing
    vol_surface_id as volSurfaceId: "string" => json(i.vol_surface_id),
    /// Read the contract's path model.
    /// @returns Explicit path model selection. AtmTermGbm is an ATM-term-structure approximation and does not model equity strike skew; callers must opt into that limitation.
    path_model as pathModel: "generated.valuations.Autocallable['path_model']" => json(i.path_model),
    /// Read the contract's div yield id.
    /// @returns Optional dividend-yield scalar ID. Some(id): lookup MUST succeed (a missing or non-unitless scalar returns an error). None: no implicit default; treated as zero continuous dividend yield. Set explicitly for index underlyings.
    div_yield_id as divYieldId: "string | null" => json(i.div_yield_id),
    /// Read the contract's initial level.
    /// @returns Initial (strike-set) underlying level S_0 used as the reference for barrier and payoff ratios. None (the default) uses the spot at the valuation date, which is only correct for a new trade priced on its strike-set date. **Required for seasoned trades** (any observation date on or before as_of): pricing errors if past observation dates exist without it.
    initial_level as initialLevel: "number | null" => json(i.initial_level),
    /// Read the contract's past fixings.
    /// @returns Observed underlying fixings for seasoned trades (date, level pairs). For a mid-life autocallable, every observation date on or before the valuation date must have a matching fixing here; pricing errors otherwise. Past fixings are evaluated deterministically (autocall, missed memory coupons, discrete knock-in monitoring) and only the remaining future observation dates are simulated.
    past_fixings as pastFixings: "generated.valuations.Autocallable['past_fixings']" => dated_nums(i.past_fixings),
});
common_getters!(JsAutocallable, "Autocallable");

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.Autocallable.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsAutocallableBuilder, "AutocallableBuilder", AutocallableBuilder, JsAutocallable, |b| b.build()
);

#[wasm_bindgen(js_class = Autocallable)]
impl JsAutocallable {
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    #[wasm_bindgen(js_name = toBuilder)]
    pub fn to_builder(&self) -> JsAutocallableBuilder {
        JsAutocallableBuilder {
            inner: super::Staged::new(self.inner.to_builder()),
        }
    }
}

setters!(JsAutocallableBuilder, "AutocallableBuilder", {
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
    /// Set the contract's observation dates.
    /// @param value - Observation dates for autocall and coupon checks. Barriers are monitored **discretely** at these exact dates only. The Monte Carlo time grid is constructed to include these dates precisely.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    observation_dates as observationDates: "generated.valuations.Autocallable['observation_dates']" => js_wire,
    /// Set the contract's payment dates.
    /// @param value - Contractual payment dates for coupons and early redemption. Each entry corresponds to observation_dates at the same index.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    payment_dates as paymentDates: "generated.valuations.Autocallable['payment_dates']" => js_wire,
    /// Set the contract's expiry.
    /// @param value - Explicit terminal expiry date for the structure.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    expiry as expiry: "string" => js_date,
    /// Set the contract's autocall barriers.
    /// @param value - Autocall barrier levels (as ratios of initial spot, e.g., 1.0 = 100%). Each barrier corresponds to the observation date at the same index. If spot ≥ barrier × initial_spot on the observation date, the product autocalls.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    autocall_barriers as autocallBarriers: "generated.valuations.Autocallable['autocall_barriers']" => js_wire,
    /// Set the contract's coupon barriers.
    /// @param value - Coupon barriers as ratios of initial spot. Coupon eligibility is independent of the autocall barrier. Each entry corresponds to the observation date at the same index.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    coupon_barriers as couponBarriers: "generated.valuations.Autocallable['coupon_barriers']" => js_wire,
    /// Set the contract's coupons.
    /// @param value - Coupon return amounts for each observation date.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    coupons as coupons: "generated.valuations.Autocallable['coupons']" => js_wire,
    /// Set the contract's memory coupons.
    /// @param value - Memory ("Phoenix") coupon feature. When enabled, a coupon whose coupon barrier is missed accrues until a later observation meets its coupon barrier. The current and all accrued coupons are then paid on that observation's payment date. Coupon eligibility remains independent of autocall eligibility.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    memory_coupons as memoryCoupons: "boolean" => js_wire,
    /// Set the contract's final barrier.
    /// @param value - Final barrier level for final payoff determination
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    final_barrier as finalBarrier: "number" => js_wire,
    /// Set the contract's final payoff type.
    /// @param value - Type of final payoff (capital protection, participation, knock-in put)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    final_payoff_type as finalPayoffType: "generated.valuations.Autocallable['final_payoff_type']" => js_wire,
    /// Set the contract's cap level.
    /// @param value - Cap level for final payoff (maximum return)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    cap_level as capLevel: "number" => js_wire,
    /// Set the contract's discount curve id.
    /// @param value - Discount curve ID for present value calculations
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    discount_curve_id as discountCurveId: "string" => id,
    /// Set the contract's spot id.
    /// @param value - Spot price identifier for underlying asset
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    spot_id as spotId: "string" => id,
    /// Set the contract's vol surface id.
    /// @param value - Volatility surface ID for option pricing
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    vol_surface_id as volSurfaceId: "string" => id,
    /// Set the contract's path model.
    /// @param value - Explicit path model selection. AtmTermGbm is an ATM-term-structure approximation and does not model equity strike skew; callers must opt into that limitation.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    path_model as pathModel: "generated.valuations.Autocallable['path_model']" => js_wire,
    /// Set the contract's div yield id.
    /// @param value - Optional dividend-yield scalar ID. Some(id): lookup MUST succeed (a missing or non-unitless scalar returns an error). None: no implicit default; treated as zero continuous dividend yield. Set explicitly for index underlyings.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    div_yield_id as divYieldId: "string | null" => optional_id via div_yield_id_opt,
    /// Set the contract's initial level.
    /// @param value - Initial (strike-set) underlying level S_0 used as the reference for barrier and payoff ratios. None (the default) uses the spot at the valuation date, which is only correct for a new trade priced on its strike-set date. **Required for seasoned trades** (any observation date on or before as_of): pricing errors if past observation dates exist without it.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    initial_level as initialLevel: "number | null" => js_wire via initial_level_opt,
    /// Set the contract's past fixings.
    /// @param value - Observed underlying fixings for seasoned trades (date, level pairs). For a mid-life autocallable, every observation date on or before the valuation date must have a matching fixing here; pricing errors otherwise. Past fixings are evaluated deterministically (autocall, missed memory coupons, discrete knock-in monitoring) and only the remaining future observation dates are simulated.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    past_fixings as pastFixings: "generated.valuations.Autocallable['past_fixings']" => js_wire,
});
common_setters!(JsAutocallableBuilder, "AutocallableBuilder");

handle_setters!(JsAutocallableBuilder, "AutocallableBuilder", {
    /// Set the contract's notional.
    /// @param value - Notional amount
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    notional as notional => JsMoney,
    /// Set the contract's day count.
    /// @param value - Day count convention for interest calculations
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    day_count as dayCount => JsDayCount,
});
