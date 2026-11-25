//! An option to enter a swap on any admissible exercise date.

use crate::api::core::money::JsMoney;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::rates::swaption::{
    BermudanSwaption, BermudanSwaptionBuilder,
};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// An option to enter a swap on any admissible exercise date.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.BermudanSwaption.example();
    /// console.log(future.id);
    /// future.free();
    /// ```
    JsBermudanSwaption, "BermudanSwaption", BermudanSwaption
);
instrument_surface!(
    JsBermudanSwaption,
    "BermudanSwaption",
    JsBermudanSwaptionBuilder,
    "generated.valuations.BermudanSwaption"
);
factories!(JsBermudanSwaption, "BermudanSwaption", BermudanSwaption, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsBermudanSwaption, "BermudanSwaption", |i| {
    /// Effective expiry of the contract.
    /// @returns The ISO expiry, or null if the instrument defines none.
    expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's option type.
    /// @returns Option type (payer = Call, receiver = Put)
    option_type as optionType: "generated.valuations.BermudanSwaption['option_type']" => json(i.option_type),
    /// Read the contract's notional.
    /// @returns Notional amount of underlying swap
    notional as notional: "Money" => money(i.notional),
    /// Read the contract's settlement.
    /// @returns Settlement method (physical or cash)
    settlement as settlement: "generated.valuations.BermudanSwaption['settlement']" => json(i.settlement),
    /// Read the contract's vol surface id.
    /// @returns Volatility surface ID for calibration
    vol_surface_id as volSurfaceId: "string" => text(i.vol_surface_id),
    /// Read the contract's exercise schedule.
    /// @returns Bermudan exercise schedule
    exercise_schedule as exerciseSchedule: "generated.valuations.BermudanSwaption['exercise_schedule']" => json(i.exercise_schedule),
    /// Read the contract's bermudan type.
    /// @returns Co-terminal or non-co-terminal exercise
    bermudan_type as bermudanType: "generated.valuations.BermudanSwaption['bermudan_type']" => json(i.bermudan_type),
    /// Read the contract's underlying fixed leg.
    /// @returns Complete fixed leg of the underlying swap.
    underlying_fixed_leg as underlyingFixedLeg: "generated.valuations.BermudanSwaption['underlying_fixed_leg']" => json(i.underlying_fixed_leg),
    /// Read the contract's underlying float leg.
    /// @returns Complete floating leg of the underlying swap.
    underlying_float_leg as underlyingFloatLeg: "generated.valuations.BermudanSwaption['underlying_float_leg']" => json(i.underlying_float_leg),
});
common_getters!(JsBermudanSwaption, "BermudanSwaption");
to_builder_method!(
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    JsBermudanSwaption,
    "BermudanSwaption",
    JsBermudanSwaptionBuilder
);

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.BermudanSwaption.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsBermudanSwaptionBuilder, "BermudanSwaptionBuilder", BermudanSwaptionBuilder, JsBermudanSwaption, |b| b.build()
);

setters!(JsBermudanSwaptionBuilder, "BermudanSwaptionBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's option type.
    /// @param value - Option type (payer = Call, receiver = Put)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    option_type as optionType: "generated.valuations.BermudanSwaption['option_type']" => js_wire,
    /// Set the contract's settlement.
    /// @param value - Settlement method (physical or cash)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    settlement as settlement: "generated.valuations.BermudanSwaption['settlement']" => js_wire,
    /// Set the contract's vol surface id.
    /// @param value - Volatility surface ID for calibration
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    vol_surface_id as volSurfaceId: "string" => id,
    /// Set the contract's exercise schedule.
    /// @param value - Bermudan exercise schedule
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    exercise_schedule as exerciseSchedule: "generated.valuations.BermudanSwaption['exercise_schedule']" => js_wire,
    /// Set the contract's bermudan type.
    /// @param value - Co-terminal or non-co-terminal exercise
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    bermudan_type as bermudanType: "generated.valuations.BermudanSwaption['bermudan_type']" => js_wire,
    /// Set the contract's underlying fixed leg.
    /// @param value - Complete fixed leg of the underlying swap.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    underlying_fixed_leg as underlyingFixedLeg: "generated.valuations.BermudanSwaption['underlying_fixed_leg']" => js_wire,
    /// Set the contract's underlying float leg.
    /// @param value - Complete floating leg of the underlying swap.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    underlying_float_leg as underlyingFloatLeg: "generated.valuations.BermudanSwaption['underlying_float_leg']" => js_wire,
});
common_setters!(JsBermudanSwaptionBuilder, "BermudanSwaptionBuilder");

handle_setters!(JsBermudanSwaptionBuilder, "BermudanSwaptionBuilder", {
    /// Set the contract's notional.
    /// @param value - Notional amount of underlying swap
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    notional as notional => JsMoney,
});
