//! A spot FX receipt with optional domestic discounting.

use crate::api::core::money::JsMoney;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::fx::fx_spot::{FxSpot, FxSpotBuilder};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// Typed WASM wrapper for the Rust FX instrument FxSpot.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const envelope = valuations.instruments.instrumentEnvelopeFromSpec("fx_spot", {
    ///   id: "EURUSD-SPOT",
    ///   base_currency: "EUR",
    ///   quote_currency: "USD",
    ///   settlement_date: "2025-01-17",
    ///   quoted_spot: 1.1,
    ///   notional: { amount: "1000000", currency: "EUR" },
    ///   attributes: {},
    /// });
    /// const spot = valuations.instruments.FxSpot.fromJson(envelope);
    /// console.log(JSON.parse(spot.toJson()).instrument.type);
    /// spot.free();
    /// ```
    JsFxSpot, "FxSpot", FxSpot
);
instrument_surface!(
    JsFxSpot,
    "FxSpot",
    JsFxSpotBuilder,
    "generated.valuations.FxSpot"
);
factories!(JsFxSpot, "FxSpot", FxSpot, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsFxSpot, "FxSpot", |i| {
    /// Effective expiry of the contract.
    /// @returns The ISO expiry, or null if the instrument defines none.
    expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's base currency.
    /// @returns Base currency (the currency being priced)
    base_currency as baseCurrency: "string" => text(i.base_currency),
    /// Read the contract's quote currency.
    /// @returns Quote currency (the currency used for pricing)
    quote_currency as quoteCurrency: "string" => text(i.quote_currency),
    /// Read the contract's settlement date.
    /// @returns Actual settlement (value) date, used as given; when set, settlement_days is ignored.
    settlement_date as settlementDate: "string | null" => opt_date(i.settlement_date),
    /// Read the contract's settlement days.
    /// @returns Contractual spot lag in business days on the joint calendar, required when settlement_date is not provided.
    settlement_days as settlementDays: "number | null" => json(i.settlement_days),
    /// Read the contract's quoted spot.
    /// @returns Optional quoted FX spot rate (quote per base); None reads the FxMatrix.
    quoted_spot as quotedSpot: "number | null" => json(i.quoted_spot),
    /// Read the contract's domestic discount curve id.
    /// @returns Optional quote-currency discount curve for PV-ing the settlement cashflow. When absent the settlement amount is reported undiscounted (a 1–2 day effect for standard spot lags).
    domestic_discount_curve_id as domesticDiscountCurveId: "string | null" => json(i.domestic_discount_curve_id),
    /// Read the contract's notional.
    /// @returns Notional amount in base currency.
    notional as notional: "Money" => money(i.notional),
    /// Read the contract's base calendar id.
    /// @returns Optional base currency calendar for joint calendar settlement adjustment. Per market convention, FX settlement uses the joint calendar of both currencies. A date is a good business day only if it's valid in both calendars.
    base_calendar_id as baseCalendarId: "string | null" => json(i.base_calendar_id),
    /// Read the contract's quote calendar id.
    /// @returns Optional quote currency calendar for joint calendar settlement adjustment. Per market convention, FX settlement uses the joint calendar of both currencies. A date is a good business day only if it's valid in both calendars.
    quote_calendar_id as quoteCalendarId: "string | null" => json(i.quote_calendar_id),
});
common_getters!(JsFxSpot, "FxSpot");

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.FxSpot.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsFxSpotBuilder, "FxSpotBuilder", FxSpotBuilder, JsFxSpot, |b| b.build()
);

to_builder_method!(
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    JsFxSpot, "FxSpot", JsFxSpotBuilder
);

setters!(JsFxSpotBuilder, "FxSpotBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's base currency.
    /// @param value - Base currency (the currency being priced)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    base_currency as baseCurrency: "string" => js_currency,
    /// Set the contract's quote currency.
    /// @param value - Quote currency (the currency used for pricing)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    quote_currency as quoteCurrency: "string" => js_currency,
    /// Set the contract's settlement date.
    /// @param value - Actual settlement (value) date, used as given; when set, settlement_days is ignored.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    settlement_date as settlementDate: "string | null" => opt_date via settlement_date_opt,
    /// Set the contract's settlement days.
    /// @param value - Contractual spot lag in business days on the joint calendar, required when settlement_date is not provided.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    settlement_days as settlementDays: "number | null" => opt_uint via settlement_days_opt,
    /// Set the contract's quoted spot.
    /// @param value - Optional quoted FX spot rate (quote per base); None reads the FxMatrix.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    quoted_spot as quotedSpot: "number | null" => opt_f64 via quoted_spot_opt,
    /// Set the contract's domestic discount curve id.
    /// @param value - Optional quote-currency discount curve for PV-ing the settlement cashflow. When absent the settlement amount is reported undiscounted (a 1–2 day effect for standard spot lags).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    domestic_discount_curve_id as domesticDiscountCurveId: "string | null" => optional_id via domestic_discount_curve_id_opt,
    /// Set the contract's base calendar id.
    /// @param value - Optional base currency calendar for joint calendar settlement adjustment. Per market convention, FX settlement uses the joint calendar of both currencies. A date is a good business day only if it's valid in both calendars.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    base_calendar_id as baseCalendarId: "string | null" => optional_id via base_calendar_id_opt,
    /// Set the contract's quote calendar id.
    /// @param value - Optional quote currency calendar for joint calendar settlement adjustment. Per market convention, FX settlement uses the joint calendar of both currencies. A date is a good business day only if it's valid in both calendars.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    quote_calendar_id as quoteCalendarId: "string | null" => optional_id via quote_calendar_id_opt,
});
common_setters!(JsFxSpotBuilder, "FxSpotBuilder");

handle_setters!(JsFxSpotBuilder, "FxSpotBuilder", {
    /// Set the contract's notional.
    /// @param value - Notional amount in base currency.
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    notional as notional => JsMoney,
});
