//! An FX touch or no-touch claim under a specified monitoring interval.

use crate::api::core::currency::JsCurrency;
use crate::api::core::dates::JsDayCount;
use crate::api::core::money::JsMoney;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::fx::fx_touch_option::{
    FxTouchOption, FxTouchOptionBuilder,
};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// Typed WASM wrapper for the Rust FX instrument FxTouchOption.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations, type JsonInput } from "finstack-quant-wasm";
    /// await init();
    /// // The canonical `fx_touch_option` envelope published with the instrument schema.
    /// const [envelope] = valuations.schema.instrumentSchema("fx_touch_option").examples as JsonInput[];
    /// const option = valuations.instruments.FxTouchOption.fromJson(envelope);
    /// console.log(option.id);
    /// option.free();
    /// ```
    JsFxTouchOption,
    "FxTouchOption",
    FxTouchOption
);
instrument_surface!(
    JsFxTouchOption,
    "FxTouchOption",
    JsFxTouchOptionBuilder,
    "generated.valuations.FxTouchOption"
);
impl JsFxTouchOption {
    #[cfg(test)]
    pub(crate) const GREEK_METHODS: &'static [&'static str] =
        &["delta", "gamma", "vega", "theta", "rho", "foreign_rho"];
    #[cfg(test)]
    pub(crate) const INSTRUMENT_TYPE: finstack_quant_valuations::pricer::InstrumentType =
        finstack_quant_valuations::pricer::InstrumentType::FxTouchOption;
}
greek_methods!(JsFxTouchOption, "FxTouchOption", {
    /// Spot delta of the option.
    /// @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
    /// @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
    /// @param model - Optional pricing-model identifier; omit to use the instrument's default model.
    /// @returns Cash delta `dPV/dS`: PV change per unit move in the spot rate, including notional scaling.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if the instrument or market JSON,
    /// `asOf`, or `model` is invalid; required market data is missing;
    /// pricing fails; or the Greek is not produced by the selected model.
    delta as delta => "delta",
    /// Spot gamma of the option.
    /// @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
    /// @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
    /// @param model - Optional pricing-model identifier; omit to use the instrument's default model.
    /// @returns Cash gamma `d²PV/dS²`: PV per unit-spot squared, including notional scaling.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if the instrument or market JSON,
    /// `asOf`, or `model` is invalid; required market data is missing;
    /// pricing fails; or the Greek is not produced by the selected model.
    gamma as gamma => "gamma",
    /// Vega of the option.
    /// @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
    /// @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
    /// @param model - Optional pricing-model identifier; omit to use the instrument's default model.
    /// @returns Cash vega: PV change for a 1 vol-point (0.01 absolute) move in implied volatility.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if the instrument or market JSON,
    /// `asOf`, or `model` is invalid; required market data is missing;
    /// pricing fails; or the Greek is not produced by the selected model.
    vega as vega => "vega",
    /// Theta of the option.
    /// @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
    /// @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
    /// @param model - Optional pricing-model identifier; omit to use the instrument's default model.
    /// @returns Theta: P&L over the theta horizon (default one day, capped at expiry) with spot held at its `asOf` level, so a roll into the monitoring window observes that spot; not annualized.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if the instrument or market JSON,
    /// `asOf`, or `model` is invalid; required market data is missing;
    /// pricing fails; or the Greek is not produced by the selected model.
    theta as theta => "theta",
    /// Domestic rate rho of the option.
    /// @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
    /// @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
    /// @param model - Optional pricing-model identifier; omit to use the instrument's default model.
    /// @returns Domestic rho: PV change for a 1bp (0.0001) move in the domestic discount rate.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if the instrument or market JSON,
    /// `asOf`, or `model` is invalid; required market data is missing;
    /// pricing fails; or the Greek is not produced by the selected model.
    rho as rho => "rho",
    /// Foreign rate rho of the option.
    /// @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
    /// @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
    /// @param model - Optional pricing-model identifier; omit to use the instrument's default model.
    /// @returns Foreign rho: PV change for a 1bp (0.0001) move in the foreign discount rate.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if the instrument or market JSON,
    /// `asOf`, or `model` is invalid; required market data is missing;
    /// pricing fails; or the Greek is not produced by the selected model.
    foreign_rho as foreignRho => "foreign_rho",
});
option_greeks_bundle!(JsFxTouchOption, "FxTouchOption");
factories!(JsFxTouchOption, "FxTouchOption", FxTouchOption, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsFxTouchOption, "FxTouchOption", |i| {
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's base currency.
    /// @returns Base currency (foreign currency)
    base_currency as baseCurrency: "Currency" => currency(i.base_currency),
    /// Read the contract's quote currency.
    /// @returns Quote currency (domestic currency)
    quote_currency as quoteCurrency: "Currency" => currency(i.quote_currency),
    /// Read the contract's barrier.
    /// @returns Barrier level (exchange rate that triggers the touch)
    barrier as barrier: "number" => json(i.barrier),
    /// Read the contract's touch type.
    /// @returns Touch type (one-touch or no-touch)
    touch_type as touchType: "generated.valuations.FxTouchOption['touch_type']" => json(i.touch_type),
    /// Read the contract's barrier direction.
    /// @returns Barrier direction (up or down)
    barrier_direction as barrierDirection: "generated.valuations.FxTouchOption['barrier_direction']" => json(i.barrier_direction),
    /// Read the contract's payout amount.
    /// @returns Fixed payout amount
    payout_amount as payoutAmount: "Money" => money(i.payout_amount),
    /// Read the contract's payout timing.
    /// @returns Payout timing (at hit or at expiry)
    payout_timing as payoutTiming: "generated.valuations.FxTouchOption['payout_timing']" => json(i.payout_timing),
    /// Read the contract's expiry.
    /// @returns Option expiry date
    expiry as expiry: "string" => date(i.expiry),
    /// Read the contract's monitoring start date.
    /// @returns First date on which barrier monitoring is active. When set, a live valuation after this date requires observed_barrier_breached.
    monitoring_start_date as monitoringStartDate: "string | null" => opt_date(i.monitoring_start_date),
    /// Read the contract's day count.
    /// @returns Day count convention (defaults to ACT/365F).
    day_count as dayCount: "DayCount" => day_count(i.day_count),
    /// Read the contract's domestic discount curve id.
    /// @returns Domestic currency discount curve ID
    domestic_discount_curve_id as domesticDiscountCurveId: "string" => json(i.domestic_discount_curve_id),
    /// Read the contract's foreign discount curve id.
    /// @returns Foreign currency discount curve ID
    foreign_discount_curve_id as foreignDiscountCurveId: "string" => json(i.foreign_discount_curve_id),
    /// Read the contract's vol surface id.
    /// @returns FX volatility surface ID
    vol_surface_id as volSurfaceId: "string" => json(i.vol_surface_id),
    /// Read the contract's observed barrier breached.
    /// @returns Observed barrier event state for expired valuations. Some(true) means the barrier was touched during the option life, Some(false) means it was observed not to have touched, and None means the historical touch state is unavailable. This is only required once the option has expired. Without it, a touched-and-reverted path cannot be distinguished from an untouched path using the terminal spot alone.
    observed_barrier_breached as observedBarrierBreached: "boolean | null" => json(i.observed_barrier_breached),
});
common_getters!(JsFxTouchOption, "FxTouchOption");

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.FxTouchOption.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsFxTouchOptionBuilder, "FxTouchOptionBuilder", FxTouchOptionBuilder, JsFxTouchOption, |b| b.build()
);

to_builder_method!(
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    JsFxTouchOption, "FxTouchOption", JsFxTouchOptionBuilder
);

setters!(JsFxTouchOptionBuilder, "FxTouchOptionBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's barrier.
    /// @param value - Barrier level (exchange rate that triggers the touch)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    barrier as barrier: "number" => js_wire,
    /// Set the contract's touch type.
    /// @param value - Touch type (one-touch or no-touch)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    touch_type as touchType: "generated.valuations.FxTouchOption['touch_type']" => js_wire,
    /// Set the contract's barrier direction.
    /// @param value - Barrier direction (up or down)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    barrier_direction as barrierDirection: "generated.valuations.FxTouchOption['barrier_direction']" => js_wire,
    /// Set the contract's payout timing.
    /// @param value - Payout timing (at hit or at expiry)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    payout_timing as payoutTiming: "generated.valuations.FxTouchOption['payout_timing']" => js_wire,
    /// Set the contract's expiry.
    /// @param value - Option expiry date
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    expiry as expiry: "string" => js_date,
    /// Set the contract's monitoring start date.
    /// @param value - First date on which barrier monitoring is active. When set, a live valuation after this date requires observed_barrier_breached.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    monitoring_start_date as monitoringStartDate: "string | null" => js_wire via monitoring_start_date_opt,
    /// Set the contract's domestic discount curve id.
    /// @param value - Domestic currency discount curve ID
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    domestic_discount_curve_id as domesticDiscountCurveId: "string" => id,
    /// Set the contract's foreign discount curve id.
    /// @param value - Foreign currency discount curve ID
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    foreign_discount_curve_id as foreignDiscountCurveId: "string" => id,
    /// Set the contract's vol surface id.
    /// @param value - FX volatility surface ID
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    vol_surface_id as volSurfaceId: "string" => id,
    /// Set the contract's observed barrier breached.
    /// @param value - Observed barrier event state for expired valuations. Some(true) means the barrier was touched during the option life, Some(false) means it was observed not to have touched, and None means the historical touch state is unavailable. This is only required once the option has expired. Without it, a touched-and-reverted path cannot be distinguished from an untouched path using the terminal spot alone.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    observed_barrier_breached as observedBarrierBreached: "boolean | null" => js_wire via observed_barrier_breached_opt,
});
common_setters!(JsFxTouchOptionBuilder, "FxTouchOptionBuilder");

handle_setters!(JsFxTouchOptionBuilder, "FxTouchOptionBuilder", {
    /// Set the contract's payout amount.
    /// @param value - Fixed payout amount
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    payout_amount as payoutAmount => JsMoney,
    /// Set the contract's base currency.
    /// @param value - Base currency (foreign currency)
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    base_currency as baseCurrency => JsCurrency,
    /// Set the contract's quote currency.
    /// @param value - Quote currency (domestic currency)
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    quote_currency as quoteCurrency => JsCurrency,
    /// Set the contract's day count.
    /// @param value - Day count convention (defaults to ACT/365F).
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    day_count as dayCount => JsDayCount,
});
