//! A matched exchange of two currencies on distinct near and far settlement dates.

use crate::api::core::currency::JsCurrency;
use crate::api::core::money::JsMoney;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::fx::fx_swap::{FxSwap, FxSwapBuilder};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// Typed WASM wrapper for the Rust FX instrument FxSwap.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations, type JsonInput } from "finstack-quant-wasm";
    /// await init();
    /// // The canonical `fx_swap` envelope published with the instrument schema.
    /// const [envelope] = valuations.schema.instrumentSchema("fx_swap").examples as JsonInput[];
    /// const swap = valuations.instruments.FxSwap.fromJson(envelope);
    /// console.log(swap.id);
    /// swap.free();
    /// ```
    JsFxSwap, "FxSwap", FxSwap
);
instrument_surface!(
    JsFxSwap,
    "FxSwap",
    JsFxSwapBuilder,
    "generated.valuations.FxSwap"
);
factories!(JsFxSwap, "FxSwap", FxSwap, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsFxSwap, "FxSwap", |i| {
    /// Effective expiry of the contract.
    /// @returns The ISO expiry, or null if the instrument defines none.
    expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's base currency.
    /// @returns Base currency (foreign)
    base_currency as baseCurrency: "Currency" => currency(i.base_currency),
    /// Read the contract's quote currency.
    /// @returns Quote currency (domestic)
    quote_currency as quoteCurrency: "Currency" => currency(i.quote_currency),
    /// Read the contract's near date.
    /// @returns Near leg settlement date (spot leg)
    near_date as nearDate: "string" => date(i.near_date),
    /// Read the contract's far date.
    /// @returns Far leg settlement date (forward leg)
    far_date as farDate: "string" => date(i.far_date),
    /// Read the contract's notional.
    /// @returns Notional amount in base currency (exchanged on near, reversed on far)
    notional as notional: "Money" => money(i.notional),
    /// Read the contract's domestic discount curve id.
    /// @returns Domestic discount curve id (quote currency)
    domestic_discount_curve_id as domesticDiscountCurveId: "string" => json(i.domestic_discount_curve_id),
    /// Read the contract's foreign discount curve id.
    /// @returns Foreign discount curve id (base currency)
    foreign_discount_curve_id as foreignDiscountCurveId: "string" => json(i.foreign_discount_curve_id),
    /// Read the contract's near rate.
    /// @returns Optional near leg FX rate (quote per base). If absent, use the market outright forward from the valuation date to near settlement.
    near_rate as nearRate: "number | null" => json(i.near_rate),
    /// Read the contract's far rate.
    /// @returns Optional far leg FX rate (quote per base). If absent, use the market outright forward from the valuation date to far settlement.
    far_rate as farRate: "number | null" => json(i.far_rate),
    /// Read the contract's base calendar id.
    /// @returns Optional base currency calendar for spot/settlement adjustment metadata.
    base_calendar_id as baseCalendarId: "string | null" => json(i.base_calendar_id),
    /// Read the contract's quote calendar id.
    /// @returns Optional quote currency calendar for spot/settlement adjustment metadata.
    quote_calendar_id as quoteCalendarId: "string | null" => json(i.quote_calendar_id),
});
common_getters!(JsFxSwap, "FxSwap");

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.FxSwap.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsFxSwapBuilder, "FxSwapBuilder", FxSwapBuilder, JsFxSwap, |b| b.build()
);

to_builder_method!(
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    JsFxSwap, "FxSwap", JsFxSwapBuilder
);

setters!(JsFxSwapBuilder, "FxSwapBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's near date.
    /// @param value - Near leg settlement date (spot leg)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    near_date as nearDate: "string" => js_date,
    /// Set the contract's far date.
    /// @param value - Far leg settlement date (forward leg)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    far_date as farDate: "string" => js_date,
    /// Set the contract's domestic discount curve id.
    /// @param value - Domestic discount curve id (quote currency)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    domestic_discount_curve_id as domesticDiscountCurveId: "string" => id,
    /// Set the contract's foreign discount curve id.
    /// @param value - Foreign discount curve id (base currency)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    foreign_discount_curve_id as foreignDiscountCurveId: "string" => id,
    /// Set the contract's near rate.
    /// @param value - Optional near leg FX rate (quote per base). If absent, use the market outright forward from the valuation date to near settlement.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    near_rate as nearRate: "number | null" => js_wire via near_rate_opt,
    /// Set the contract's far rate.
    /// @param value - Optional far leg FX rate (quote per base). If absent, use the market outright forward from the valuation date to far settlement.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    far_rate as farRate: "number | null" => js_wire via far_rate_opt,
    /// Set the contract's base calendar id.
    /// @param value - Optional base currency calendar for spot/settlement adjustment metadata.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    base_calendar_id as baseCalendarId: "string | null" => optional_id via base_calendar_id_opt,
    /// Set the contract's quote calendar id.
    /// @param value - Optional quote currency calendar for spot/settlement adjustment metadata.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    quote_calendar_id as quoteCalendarId: "string | null" => optional_id via quote_calendar_id_opt,
});
common_setters!(JsFxSwapBuilder, "FxSwapBuilder");

handle_setters!(JsFxSwapBuilder, "FxSwapBuilder", {
    /// Set the contract's notional.
    /// @param value - Notional amount in base currency (exchanged on near, reversed on far)
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    notional as notional => JsMoney,
    /// Set the contract's base currency.
    /// @param value - Base currency (foreign)
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    base_currency as baseCurrency => JsCurrency,
    /// Set the contract's quote currency.
    /// @param value - Quote currency (domestic)
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    quote_currency as quoteCurrency => JsCurrency,
});
