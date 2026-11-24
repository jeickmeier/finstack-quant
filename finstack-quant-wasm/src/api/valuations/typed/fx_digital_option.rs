//! A cash or asset payout contingent on the FX strike.

use crate::api::core::currency::JsCurrency;
use crate::api::core::dates::JsDayCount;
use crate::api::core::money::JsMoney;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::fx::fx_digital_option::{
    FxDigitalOption, FxDigitalOptionBuilder,
};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// Typed WASM wrapper for the Rust FX instrument FxDigitalOption.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations, type JsonInput } from "finstack-quant-wasm";
    /// await init();
    /// // The canonical `fx_digital_option` envelope published with the instrument schema.
    /// const [envelope] = valuations.schema.instrumentSchema("fx_digital_option").examples as JsonInput[];
    /// const option = valuations.instruments.FxDigitalOption.fromJson(envelope);
    /// console.log(option.id);
    /// option.free();
    /// ```
    JsFxDigitalOption,
    "FxDigitalOption",
    FxDigitalOption
);
instrument_surface!(
    JsFxDigitalOption,
    "FxDigitalOption",
    JsFxDigitalOptionBuilder,
    "generated.valuations.FxDigitalOption"
);
impl JsFxDigitalOption {
    #[cfg(test)]
    pub(crate) const GREEK_METHODS: &'static [&'static str] =
        &["delta", "gamma", "vega", "theta", "rho", "foreign_rho"];
    #[cfg(test)]
    pub(crate) const INSTRUMENT_TYPE: finstack_quant_valuations::pricer::InstrumentType =
        finstack_quant_valuations::pricer::InstrumentType::FxDigitalOption;
}
greek_methods!(JsFxDigitalOption, "FxDigitalOption", {
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
    /// @returns Model theta per day on the `metric_pricing_overrides.theta_day_basis` day count (default 365 calendar days): analytic for cash-or-nothing payouts, a one-day finite-difference decay for asset-or-nothing; not annualized.
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
option_greeks_bundle!(JsFxDigitalOption, "FxDigitalOption");
factories!(JsFxDigitalOption, "FxDigitalOption", FxDigitalOption, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsFxDigitalOption, "FxDigitalOption", |i| {
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's base currency.
    /// @returns Base currency (foreign currency)
    base_currency as baseCurrency: "Currency" => currency(i.base_currency),
    /// Read the contract's quote currency.
    /// @returns Quote currency (domestic currency)
    quote_currency as quoteCurrency: "Currency" => currency(i.quote_currency),
    /// Read the contract's strike.
    /// @returns Strike exchange rate (quote per base)
    strike as strike: "number" => json(i.strike),
    /// Read the contract's option type.
    /// @returns Option type (call or put on base currency)
    option_type as optionType: "generated.valuations.FxDigitalOption['option_type']" => json(i.option_type),
    /// Read the contract's payout type.
    /// @returns Payout type (cash-or-nothing or asset-or-nothing)
    payout_type as payoutType: "generated.valuations.FxDigitalOption['payout_type']" => json(i.payout_type),
    /// Read the contract's payout amount.
    /// @returns Fixed payout amount (used for cash-or-nothing; for asset-or-nothing this is the notional of foreign currency delivered)
    payout_amount as payoutAmount: "Money" => money(i.payout_amount),
    /// Read the contract's expiry.
    /// @returns Option expiry date
    expiry as expiry: "string" => date(i.expiry),
    /// Read the contract's day count.
    /// @returns Day count convention (defaults to ACT/365F).
    day_count as dayCount: "DayCount" => day_count(i.day_count),
    /// Read the contract's notional.
    /// @returns Notional amount in base currency
    notional as notional: "Money" => money(i.notional),
    /// Read the contract's domestic discount curve id.
    /// @returns Domestic currency discount curve ID
    domestic_discount_curve_id as domesticDiscountCurveId: "string" => json(i.domestic_discount_curve_id),
    /// Read the contract's foreign discount curve id.
    /// @returns Foreign currency discount curve ID
    foreign_discount_curve_id as foreignDiscountCurveId: "string" => json(i.foreign_discount_curve_id),
    /// Read the contract's vol surface id.
    /// @returns FX volatility surface ID
    vol_surface_id as volSurfaceId: "string" => json(i.vol_surface_id),
});
common_getters!(JsFxDigitalOption, "FxDigitalOption");

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.FxDigitalOption.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsFxDigitalOptionBuilder, "FxDigitalOptionBuilder", FxDigitalOptionBuilder, JsFxDigitalOption, |b| b.build()
);

to_builder_method!(
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    JsFxDigitalOption, "FxDigitalOption", JsFxDigitalOptionBuilder
);

setters!(JsFxDigitalOptionBuilder, "FxDigitalOptionBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's strike.
    /// @param value - Strike exchange rate (quote per base)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    strike as strike: "number" => js_wire,
    /// Set the contract's option type.
    /// @param value - Option type (call or put on base currency)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    option_type as optionType: "generated.valuations.FxDigitalOption['option_type']" => js_wire,
    /// Set the contract's payout type.
    /// @param value - Payout type (cash-or-nothing or asset-or-nothing)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    payout_type as payoutType: "generated.valuations.FxDigitalOption['payout_type']" => js_wire,
    /// Set the contract's expiry.
    /// @param value - Option expiry date
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    expiry as expiry: "string" => js_date,
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
});
common_setters!(JsFxDigitalOptionBuilder, "FxDigitalOptionBuilder");

handle_setters!(JsFxDigitalOptionBuilder, "FxDigitalOptionBuilder", {
    /// Set the contract's payout amount.
    /// @param value - Fixed payout amount (used for cash-or-nothing; for asset-or-nothing this is the notional of foreign currency delivered)
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    payout_amount as payoutAmount => JsMoney,
    /// Set the contract's notional.
    /// @param value - Notional amount in base currency
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    notional as notional => JsMoney,
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
