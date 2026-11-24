//! An equity option with contractually specified foreign-exchange conversion.

use crate::api::core::currency::JsCurrency;
use crate::api::core::dates::JsDayCount;
use crate::api::core::money::JsMoney;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::fx::quanto_option::{
    QuantoOption, QuantoOptionBuilder,
};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// Typed WASM wrapper for the Rust FX instrument QuantoOption.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations, type JsonInput } from "finstack-quant-wasm";
    /// await init();
    /// // The canonical `quanto_option` envelope published with the instrument schema.
    /// const [envelope] = valuations.schema.instrumentSchema("quanto_option").examples as JsonInput[];
    /// const option = valuations.instruments.QuantoOption.fromJson(envelope);
    /// console.log(option.id);
    /// option.free();
    /// ```
    JsQuantoOption,
    "QuantoOption",
    QuantoOption
);
instrument_surface!(
    JsQuantoOption,
    "QuantoOption",
    JsQuantoOptionBuilder,
    "generated.valuations.QuantoOption"
);
impl JsQuantoOption {
    #[cfg(test)]
    pub(crate) const GREEK_METHODS: &'static [&'static str] = &[
        "delta",
        "gamma",
        "vega",
        "theta",
        "rho",
        "foreign_rho",
        "vanna",
        "volga",
    ];
    #[cfg(test)]
    pub(crate) const INSTRUMENT_TYPE: finstack_quant_valuations::pricer::InstrumentType =
        finstack_quant_valuations::pricer::InstrumentType::QuantoOption;
}
greek_methods!(JsQuantoOption, "QuantoOption", {
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
    /// Vanna of the option.
    /// @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
    /// @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
    /// @param model - Optional pricing-model identifier; omit to use the instrument's default model.
    /// @returns Vanna `d²PV/(dS dσ)`: PV per unit spot per vol point (0.01 absolute vol).
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if the instrument or market JSON,
    /// `asOf`, or `model` is invalid; required market data is missing;
    /// pricing fails; or the Greek is not produced by the selected model.
    vanna as vanna => "vanna",
    /// Volga of the option.
    /// @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
    /// @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
    /// @param model - Optional pricing-model identifier; omit to use the instrument's default model.
    /// @returns Volga `d²PV/dσ²`: PV per vol-point squared (0.01 absolute vol).
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if the instrument or market JSON,
    /// `asOf`, or `model` is invalid; required market data is missing;
    /// pricing fails; or the Greek is not produced by the selected model.
    volga as volga => "volga",
});
option_greeks_bundle!(JsQuantoOption, "QuantoOption");
factories!(JsQuantoOption, "QuantoOption", QuantoOption, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsQuantoOption, "QuantoOption", |i| {
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's underlying ticker.
    /// @returns Underlying equity ticker symbol
    underlying_ticker as underlyingTicker: "string" => json(i.underlying_ticker),
    /// Read the contract's strike.
    /// @returns Per-unit strike of the equity underlying, in base_currency price units.
    strike as strike: "number" => json(i.strike),
    /// Read the contract's option type.
    /// @returns Option type (call or put)
    option_type as optionType: "generated.valuations.QuantoOption['option_type']" => json(i.option_type),
    /// Read the contract's expiry.
    /// @returns Option expiry date
    expiry as expiry: "string" => date(i.expiry),
    /// Read the contract's notional.
    /// @returns Strike-equivalent domestic reference notional. When quantity and payoff_fx_rate are supplied, this must equal quantity * payoff_fx_rate * strike.
    notional as notional: "Money" => money(i.notional),
    /// Read the contract's quantity.
    /// @returns Number of underlying units covered by the option payoff. Quanto pricing is performed per unit of the underlying in the base currency. This quantity converts the per-unit price into contract-level exposure.
    quantity as quantity: "number | null" => json(i.quantity),
    /// Read the contract's payoff fx rate.
    /// @returns Fixed payoff FX conversion rate from base-currency payoff into quote currency. Example: for a JPY-underlying option settled in USD at a fixed 140 JPY/USD conversion, payoff_fx_rate is 1.0 / 140.0 USD per JPY.
    payoff_fx_rate as payoffFxRate: "number | null" => json(i.payoff_fx_rate),
    /// Read the contract's base currency.
    /// @returns Base currency (equity denomination)
    base_currency as baseCurrency: "Currency" => currency(i.base_currency),
    /// Read the contract's quote currency.
    /// @returns Quote currency (payment/settlement currency)
    quote_currency as quoteCurrency: "Currency" => currency(i.quote_currency),
    /// Read the contract's correlation.
    /// @returns Correlation between equity price and FX rate
    correlation as correlation: "generated.valuations.QuantoOption['correlation']" => json(i.correlation),
    /// Read the contract's day count.
    /// @returns Day count convention (defaults to ACT/365F).
    day_count as dayCount: "DayCount" => day_count(i.day_count),
    /// Read the contract's domestic discount curve id.
    /// @returns Discount curve ID (domestic currency)
    domestic_discount_curve_id as domesticDiscountCurveId: "string" => json(i.domestic_discount_curve_id),
    /// Read the contract's foreign discount curve id.
    /// @returns Discount curve ID (foreign currency)
    foreign_discount_curve_id as foreignDiscountCurveId: "string" => json(i.foreign_discount_curve_id),
    /// Read the contract's spot id.
    /// @returns Equity spot price identifier
    spot_id as spotId: "string" => json(i.spot_id),
    /// Read the contract's vol surface id.
    /// @returns Equity volatility surface ID
    vol_surface_id as volSurfaceId: "string" => json(i.vol_surface_id),
    /// Read the contract's div yield id.
    /// @returns Optional dividend-yield scalar ID
    div_yield_id as divYieldId: "string | null" => json(i.div_yield_id),
    /// Read the contract's fx spot id.
    /// @returns Optional FX rate identifier. The referenced scalar must be quoted as **quote currency per unit of base currency** (e.g. for base JPY / quote USD, USD-per-JPY ≈ 0.0071, conventionally named JPYUSD). The pricer compounds it with exp((r_quote − r_base)·t) to obtain the ATM forward for the FX vol lookup, so an inverted quote (JPY-per-USD) silently mislocates the smile. validate() rejects ids whose embedded pair name contradicts this direction.
    fx_spot_id as fxSpotId: "string | null" => json(i.fx_spot_id),
    /// Read the contract's fx vol surface id.
    /// @returns Optional FX volatility surface ID. Must be calibrated in the same **quote-per-base** direction as fx_spot_id so absolute-strike lookups at the CIRP forward land on the correct smile. validate() rejects ids whose embedded pair name contradicts this direction.
    fx_vol_surface_id as fxVolSurfaceId: "string | null" => json(i.fx_vol_surface_id),
});
common_getters!(JsQuantoOption, "QuantoOption");

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.QuantoOption.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsQuantoOptionBuilder, "QuantoOptionBuilder", QuantoOptionBuilder, JsQuantoOption, |b| b.build()
);

to_builder_method!(
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    JsQuantoOption, "QuantoOption", JsQuantoOptionBuilder
);

setters!(JsQuantoOptionBuilder, "QuantoOptionBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's underlying ticker.
    /// @param value - Underlying equity ticker symbol
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    underlying_ticker as underlyingTicker: "string" => js_wire,
    /// Set the contract's strike.
    /// @param value - Per-unit strike of the equity underlying, in base_currency price units.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    strike as strike: "number" => js_wire,
    /// Set the contract's option type.
    /// @param value - Option type (call or put)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    option_type as optionType: "generated.valuations.QuantoOption['option_type']" => js_wire,
    /// Set the contract's expiry.
    /// @param value - Option expiry date
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    expiry as expiry: "string" => js_date,
    /// Set the contract's quantity.
    /// @param value - Number of underlying units covered by the option payoff. Quanto pricing is performed per unit of the underlying in the base currency. This quantity converts the per-unit price into contract-level exposure.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    quantity as quantity: "number | null" => js_wire via quantity_opt,
    /// Set the contract's payoff fx rate.
    /// @param value - Fixed payoff FX conversion rate from base-currency payoff into quote currency. Example: for a JPY-underlying option settled in USD at a fixed 140 JPY/USD conversion, payoff_fx_rate is 1.0 / 140.0 USD per JPY.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    payoff_fx_rate as payoffFxRate: "number | null" => js_wire via payoff_fx_rate_opt,
    /// Set the contract's correlation.
    /// @param value - Correlation between equity price and FX rate
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    correlation as correlation: "generated.valuations.QuantoOption['correlation']" => js_wire,
    /// Set the contract's domestic discount curve id.
    /// @param value - Discount curve ID (domestic currency)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    domestic_discount_curve_id as domesticDiscountCurveId: "string" => id,
    /// Set the contract's foreign discount curve id.
    /// @param value - Discount curve ID (foreign currency)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    foreign_discount_curve_id as foreignDiscountCurveId: "string" => id,
    /// Set the contract's spot id.
    /// @param value - Equity spot price identifier
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    spot_id as spotId: "string" => id,
    /// Set the contract's vol surface id.
    /// @param value - Equity volatility surface ID
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    vol_surface_id as volSurfaceId: "string" => id,
    /// Set the contract's div yield id.
    /// @param value - Optional dividend-yield scalar ID
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    div_yield_id as divYieldId: "string | null" => optional_id via div_yield_id_opt,
    /// Set the contract's fx spot id.
    /// @param value - Optional FX rate identifier. The referenced scalar must be quoted as **quote currency per unit of base currency** (e.g. for base JPY / quote USD, USD-per-JPY ≈ 0.0071, conventionally named JPYUSD). The pricer compounds it with exp((r_quote − r_base)·t) to obtain the ATM forward for the FX vol lookup, so an inverted quote (JPY-per-USD) silently mislocates the smile. validate() rejects ids whose embedded pair name contradicts this direction.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    fx_spot_id as fxSpotId: "string | null" => optional_id via fx_spot_id_opt,
    /// Set the contract's fx vol surface id.
    /// @param value - Optional FX volatility surface ID. Must be calibrated in the same **quote-per-base** direction as fx_spot_id so absolute-strike lookups at the CIRP forward land on the correct smile. validate() rejects ids whose embedded pair name contradicts this direction.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    fx_vol_surface_id as fxVolSurfaceId: "string | null" => optional_id via fx_vol_surface_id_opt,
});
common_setters!(JsQuantoOptionBuilder, "QuantoOptionBuilder");

handle_setters!(JsQuantoOptionBuilder, "QuantoOptionBuilder", {
    /// Set the contract's notional.
    /// @param value - Strike-equivalent domestic reference notional. When quantity and payoff_fx_rate are supplied, this must equal quantity * payoff_fx_rate * strike.
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    notional as notional => JsMoney,
    /// Set the contract's base currency.
    /// @param value - Base currency (equity denomination)
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    base_currency as baseCurrency => JsCurrency,
    /// Set the contract's quote currency.
    /// @param value - Quote currency (payment/settlement currency)
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    quote_currency as quoteCurrency => JsCurrency,
    /// Set the contract's day count.
    /// @param value - Day count convention (defaults to ACT/365F).
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    day_count as dayCount => JsDayCount,
});
