//! A non-deliverable FX forward with an explicit fixing regime.

use crate::api::core::currency::JsCurrency;
use crate::api::core::money::JsMoney;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::fx::ndf::{Ndf, NdfBuilder};
use wasm_bindgen::prelude::*;

use super::fx::JsNdf;

instrument_surface!(
    JsNdf,
    "Ndf",
    JsNdfBuilder,
    "generated.valuations.Ndf",
    unpriced
);
factories!(JsNdf, "Ndf", Ndf, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsNdf, "Ndf", |i| {
    /// Effective expiry of the contract.
    /// @returns The ISO expiry, or null if the instrument defines none.
    expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's base currency.
    /// @returns Base currency (restricted/non-deliverable currency, numerator).
    base_currency as baseCurrency: "Currency" => currency(i.base_currency),
    /// Read the contract's settlement currency.
    /// @returns Settlement currency (freely convertible, typically USD, denominator and PV currency).
    settlement_currency as settlementCurrency: "Currency" => currency(i.settlement_currency),
    /// Read the contract's fixing date.
    /// @returns Fixing date (rate observation date, typically T-2 before maturity).
    fixing_date as fixingDate: "string" => date(i.fixing_date),
    /// Read the contract's maturity.
    /// @returns Maturity/settlement date.
    maturity as maturity: "string" => date(i.maturity),
    /// Read the contract's notional.
    /// @returns Notional amount in base currency.
    notional as notional: "Money" => money(i.notional),
    /// Read the contract's contract rate.
    /// @returns Contract forward rate. Interpretation depends on quote_convention: - BasePerSettlement: base per settlement (e.g., 7.25 CNY per USD) - SettlementPerBase: settlement per base (e.g., 0.138 USD per CNY) None values the NDF at-market: the contract rate equals the market forward, so the settlement amount and PV are zero.
    contract_rate as contractRate: "number | null" => json(i.contract_rate),
    /// Read the contract's domestic discount curve id.
    /// @returns Settlement currency discount curve ID.
    domestic_discount_curve_id as domesticDiscountCurveId: "string" => json(i.domestic_discount_curve_id),
    /// Read the contract's quote convention.
    /// @returns Quote convention for contract_rate and observed_fixing.
    quote_convention as quoteConvention: "generated.valuations.Ndf['quote_convention']" => json(i.quote_convention),
    /// Read the contract's foreign discount curve id.
    /// @returns Optional foreign (base) currency discount curve ID. Required for pre-fixing forward estimation unless quoted_forward is supplied.
    foreign_discount_curve_id as foreignDiscountCurveId: "string | null" => json(i.foreign_discount_curve_id),
    /// Read the contract's observed fixing.
    /// @returns Observed fixing rate. Interpretation depends on quote_convention. If Some, NDF is post-fixing.
    observed_fixing as observedFixing: "number | null" => json(i.observed_fixing),
    /// Read the contract's fixing source.
    /// @returns Official fixing source/benchmark enum for type-safe specification. Use this field for validated fixing sources. See NdfFixingSource for supported benchmarks and their typical currencies.
    fixing_source as fixingSource: "NonNullable<generated.valuations.Ndf['fixing_source']> | null" => json(i.fixing_source),
    /// Read the contract's quoted spot.
    /// @returns Optional spot rate override for forward rate calculation. Interpretation depends on quote_convention.
    quoted_spot as quotedSpot: "number | null" => json(i.quoted_spot),
    /// Read the contract's quoted forward.
    /// @returns Explicit pre-fixing forward rate in quote_convention units. Use this for NDF market quotes or basis-adjusted forwards when a base currency discount curve is unavailable.
    quoted_forward as quotedForward: "number | null" => json(i.quoted_forward),
    /// Read the contract's base calendar id.
    /// @returns Optional base currency calendar.
    base_calendar_id as baseCalendarId: "string | null" => json(i.base_calendar_id),
    /// Read the contract's settlement calendar id.
    /// @returns Optional settlement currency calendar.
    settlement_calendar_id as settlementCalendarId: "string | null" => json(i.settlement_calendar_id),
});
common_getters!(JsNdf, "Ndf");

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.Ndf.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsNdfBuilder, "NdfBuilder", NdfBuilder, JsNdf, |b| b.build()
);

#[wasm_bindgen(js_class = Ndf)]
impl JsNdf {
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    #[wasm_bindgen(js_name = toBuilder)]
    pub fn to_builder(&self) -> JsNdfBuilder {
        JsNdfBuilder {
            inner: super::Staged::new(self.inner.to_builder()),
        }
    }
}

setters!(JsNdfBuilder, "NdfBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's fixing date.
    /// @param value - Fixing date (rate observation date, typically T-2 before maturity).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    fixing_date as fixingDate: "string" => js_date,
    /// Set the contract's maturity.
    /// @param value - Maturity/settlement date.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    maturity as maturity: "string" => js_date,
    /// Set the contract's contract rate.
    /// @param value - Contract forward rate. Interpretation depends on quote_convention: - BasePerSettlement: base per settlement (e.g., 7.25 CNY per USD) - SettlementPerBase: settlement per base (e.g., 0.138 USD per CNY) None values the NDF at-market: the contract rate equals the market forward, so the settlement amount and PV are zero.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    contract_rate as contractRate: "number | null" => js_wire via contract_rate_opt,
    /// Set the contract's domestic discount curve id.
    /// @param value - Settlement currency discount curve ID.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    domestic_discount_curve_id as domesticDiscountCurveId: "string" => id,
    /// Set the contract's quote convention.
    /// @param value - Quote convention for contract_rate and observed_fixing.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    quote_convention as quoteConvention: "generated.valuations.Ndf['quote_convention']" => js_wire,
    /// Set the contract's foreign discount curve id.
    /// @param value - Optional foreign (base) currency discount curve ID. Required for pre-fixing forward estimation unless quoted_forward is supplied.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    foreign_discount_curve_id as foreignDiscountCurveId: "string | null" => optional_id via foreign_discount_curve_id_opt,
    /// Set the contract's observed fixing.
    /// @param value - Observed fixing rate. Interpretation depends on quote_convention. If Some, NDF is post-fixing.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    observed_fixing as observedFixing: "number | null" => js_wire via observed_fixing_opt,
    /// Set the contract's fixing source.
    /// @param value - Official fixing source/benchmark enum for type-safe specification. Use this field for validated fixing sources. See NdfFixingSource for supported benchmarks and their typical currencies.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    fixing_source as fixingSource: "NonNullable<generated.valuations.Ndf['fixing_source']> | null" => js_wire via fixing_source_opt,
    /// Set the contract's quoted spot.
    /// @param value - Optional spot rate override for forward rate calculation. Interpretation depends on quote_convention.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    quoted_spot as quotedSpot: "number | null" => js_wire via quoted_spot_opt,
    /// Set the contract's quoted forward.
    /// @param value - Explicit pre-fixing forward rate in quote_convention units. Use this for NDF market quotes or basis-adjusted forwards when a base currency discount curve is unavailable.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    quoted_forward as quotedForward: "number | null" => js_wire via quoted_forward_opt,
    /// Set the contract's base calendar id.
    /// @param value - Optional base currency calendar.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    base_calendar_id as baseCalendarId: "string | null" => optional_id via base_calendar_id_opt,
    /// Set the contract's settlement calendar id.
    /// @param value - Optional settlement currency calendar.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    settlement_calendar_id as settlementCalendarId: "string | null" => optional_id via settlement_calendar_id_opt,
});
common_setters!(JsNdfBuilder, "NdfBuilder");

handle_setters!(JsNdfBuilder, "NdfBuilder", {
    /// Set the contract's notional.
    /// @param value - Notional amount in base currency.
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    notional as notional => JsMoney,
    /// Set the contract's base currency.
    /// @param value - Base currency (restricted/non-deliverable currency, numerator).
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    base_currency as baseCurrency => JsCurrency,
    /// Set the contract's settlement currency.
    /// @param value - Settlement currency (freely convertible, typically USD, denominator and PV currency).
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    settlement_currency as settlementCurrency => JsCurrency,
});
