//! A swap exchanging fixed-income index total return for contractual financing.

use crate::api::core::money::JsMoney;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::fixed_income::fi_trs::{
    FiIndexTotalReturnSwap, FiIndexTotalReturnSwapBuilder,
};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// A swap exchanging fixed-income index total return for contractual financing.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.FiIndexTotalReturnSwap.example();
    /// console.log(future.id);
    /// future.free();
    /// ```
    JsFiIndexTotalReturnSwap, "FiIndexTotalReturnSwap", FiIndexTotalReturnSwap
);
instrument_surface!(
    JsFiIndexTotalReturnSwap,
    "FiIndexTotalReturnSwap",
    JsFiIndexTotalReturnSwapBuilder,
    "generated.valuations.FiIndexTotalReturnSwap"
);
factories!(JsFiIndexTotalReturnSwap, "FiIndexTotalReturnSwap", FiIndexTotalReturnSwap, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsFiIndexTotalReturnSwap, "FiIndexTotalReturnSwap", |i| {
    /// Effective expiry of the contract.
    /// @returns The ISO expiry, or null if the instrument defines none.
    expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's notional.
    /// @returns Notional amount for the swap.
    notional as notional: "Money" => money(i.notional),
    /// Read the contract's underlying.
    /// @returns Underlying index parameters (index ID, yield, duration, base currency).
    underlying as underlying: "generated.valuations.FiIndexTotalReturnSwap['underlying']" => json(i.underlying),
    /// Read the contract's financing leg.
    /// @returns Financing leg specification (curves, spread, day count).
    financing_leg as financingLeg: "generated.valuations.FiIndexTotalReturnSwap['financing_leg']" => json(i.financing_leg),
    /// Read the contract's schedule.
    /// @returns Schedule specification (payment dates and frequency).
    schedule as schedule: "generated.valuations.FiIndexTotalReturnSwap['schedule']" => json(i.schedule),
    /// Read the contract's side.
    /// @returns Trade side: receive receives the total-return leg and pays financing; pay pays the total return and receives financing.
    side as side: "generated.valuations.FiIndexTotalReturnSwap['side']" => json(i.side),
    /// Read the contract's initial level.
    /// @returns Index level at the first contractual reset, in the index's own units. Used only at inception when no fixing series is supplied; the realized return to date is index_level / initial_level. Subsequent resets require fixing_series_id. Must be positive and finite when set.
    initial_level as initialLevel: "number | null" => json(i.initial_level),
    /// Read the contract's fixing series id.
    /// @returns Market time-series identifier containing positive total-return index levels on contractual reset dates, including the final accrual boundary. Historical endpoints require exact observations; no yield-based history is inferred.
    fixing_series_id as fixingSeriesId: "string | null" => opt_text(i.fixing_series_id),
    /// Read the contract's margin spec.
    /// @returns Optional OTC margin specification for VM/IM. Fixed income index TRS use duration-based margin calculations.
    margin_spec as marginSpec: "NonNullable<generated.valuations.FiIndexTotalReturnSwap['margin_spec']> | null" => json(i.margin_spec),
});
common_getters!(JsFiIndexTotalReturnSwap, "FiIndexTotalReturnSwap");
to_builder_method!(
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    JsFiIndexTotalReturnSwap,
    "FiIndexTotalReturnSwap",
    JsFiIndexTotalReturnSwapBuilder
);

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.FiIndexTotalReturnSwap.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsFiIndexTotalReturnSwapBuilder, "FiIndexTotalReturnSwapBuilder", FiIndexTotalReturnSwapBuilder, JsFiIndexTotalReturnSwap, |b| b.build()
);

setters!(JsFiIndexTotalReturnSwapBuilder, "FiIndexTotalReturnSwapBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's underlying.
    /// @param value - Underlying index parameters (index ID, yield, duration, base currency).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    underlying as underlying: "generated.valuations.FiIndexTotalReturnSwap['underlying']" => js_wire,
    /// Set the contract's financing leg.
    /// @param value - Financing leg specification (curves, spread, day count).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    financing_leg as financingLeg: "generated.valuations.FiIndexTotalReturnSwap['financing_leg']" => js_wire,
    /// Set the contract's schedule.
    /// @param value - Schedule specification (payment dates and frequency).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    schedule as schedule: "generated.valuations.FiIndexTotalReturnSwap['schedule']" => js_wire,
    /// Set the contract's side.
    /// @param value - Trade side: receive receives the total-return leg and pays financing; pay pays the total return and receives financing.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    side as side: "generated.valuations.FiIndexTotalReturnSwap['side']" => js_wire,
    /// Set the contract's initial level.
    /// @param value - Index level at the first contractual reset, in the index's own units. Used only at inception when no fixing series is supplied; the realized return to date is index_level / initial_level. Subsequent resets require fixing_series_id. Must be positive and finite when set.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    initial_level as initialLevel: "number | null" => opt_f64 via initial_level_opt,
    /// Set the contract's fixing series id.
    /// @param value - Market time-series identifier containing positive total-return index levels on contractual reset dates, including the final accrual boundary. Historical endpoints require exact observations; no yield-based history is inferred.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    fixing_series_id as fixingSeriesId: "string | null" => optional_id via fixing_series_id_opt,
    /// Set the contract's margin spec.
    /// @param value - Optional OTC margin specification for VM/IM. Fixed income index TRS use duration-based margin calculations.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    margin_spec as marginSpec: "NonNullable<generated.valuations.FiIndexTotalReturnSwap['margin_spec']> | null" => opt_wire via margin_spec_opt,
});
common_setters!(
    JsFiIndexTotalReturnSwapBuilder,
    "FiIndexTotalReturnSwapBuilder"
);

handle_setters!(JsFiIndexTotalReturnSwapBuilder, "FiIndexTotalReturnSwapBuilder", {
    /// Set the contract's notional.
    /// @param value - Notional amount for the swap.
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    notional as notional => JsMoney,
});
