//! An agency collateralized mortgage obligation priced through its waterfall.

use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::fixed_income::cmo::{AgencyCmo, AgencyCmoBuilder};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// An agency collateralized mortgage obligation priced through its waterfall.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const cmo = valuations.instruments.AgencyCmo.example();
    /// console.log(cmo.id);
    /// cmo.free();
    /// ```
    JsAgencyCmo, "AgencyCmo", AgencyCmo
);
instrument_surface!(
    JsAgencyCmo,
    "AgencyCmo",
    JsAgencyCmoBuilder,
    "generated.valuations.AgencyCmo"
);
factories!(JsAgencyCmo, "AgencyCmo", AgencyCmo, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsAgencyCmo, "AgencyCmo", |i| {
    /// Effective expiry of the contract.
    /// @returns The ISO expiry, or null if the instrument defines none.
    expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's deal name.
    /// @returns Deal name (e.g., "FNR 2024-1")
    deal_name as dealName: "string" => text(i.deal_name),
    /// Read the contract's agency.
    /// @returns Agency program
    agency as agency: "generated.valuations.AgencyCmo['agency']" => json(i.agency),
    /// Read the contract's issue date.
    /// @returns Issue date
    issue_date as issueDate: "string" => date(i.issue_date),
    /// Read the contract's waterfall.
    /// @returns Waterfall configuration with tranches
    waterfall as waterfall: "generated.valuations.AgencyCmo['waterfall']" => json(i.waterfall),
    /// Read the contract's reference tranche id.
    /// @returns Reference tranche ID for pricing (which tranche to value)
    reference_tranche_id as referenceTrancheId: "string" => text(i.reference_tranche_id),
    /// Read the contract's collateral.
    /// @returns Collateral pool (optional - for detailed cashflow projection)
    collateral as collateral: "NonNullable<generated.valuations.AgencyCmo['collateral']> | null" => json(i.collateral),
    /// Read the contract's collateral wac.
    /// @returns Collateral WAC (if no explicit collateral)
    collateral_wac as collateralWac: "number | null" => json(i.collateral_wac),
    /// Read the contract's collateral wam months.
    /// @returns Remaining collateral WAM in months (if no explicit collateral).
    collateral_wam_months as collateralWamMonths: "number | null" => json(i.collateral_wam_months),
    /// Read the contract's discount curve id.
    /// @returns Discount curve identifier.
    discount_curve_id as discountCurveId: "string" => text(i.discount_curve_id),
});
common_getters!(JsAgencyCmo, "AgencyCmo");
to_builder_method!(
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    JsAgencyCmo,
    "AgencyCmo",
    JsAgencyCmoBuilder
);

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.AgencyCmo.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsAgencyCmoBuilder, "AgencyCmoBuilder", AgencyCmoBuilder, JsAgencyCmo, |b| b.build()
);

setters!(JsAgencyCmoBuilder, "AgencyCmoBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's deal name.
    /// @param value - Deal name (e.g., "FNR 2024-1")
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    deal_name as dealName: "string" => id,
    /// Set the contract's agency.
    /// @param value - Agency program
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    agency as agency: "generated.valuations.AgencyCmo['agency']" => js_wire,
    /// Set the contract's issue date.
    /// @param value - Issue date
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    issue_date as issueDate: "string" => js_date,
    /// Set the contract's waterfall.
    /// @param value - Waterfall configuration with tranches
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    waterfall as waterfall: "generated.valuations.AgencyCmo['waterfall']" => js_wire,
    /// Set the contract's reference tranche id.
    /// @param value - Reference tranche ID for pricing (which tranche to value)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    reference_tranche_id as referenceTrancheId: "string" => id,
    /// Set the contract's collateral.
    /// @param value - Collateral pool (optional - for detailed cashflow projection)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    collateral as collateral: "NonNullable<generated.valuations.AgencyCmo['collateral']> | null" => opt_wire via collateral_opt,
    /// Set the contract's collateral wac.
    /// @param value - Collateral WAC (if no explicit collateral)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    collateral_wac as collateralWac: "number | null" => opt_f64 via collateral_wac_opt,
    /// Set the contract's collateral wam months.
    /// @param value - Remaining collateral WAM in months (if no explicit collateral).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    collateral_wam_months as collateralWamMonths: "number | null" => opt_uint via collateral_wam_months_opt,
    /// Set the contract's discount curve id.
    /// @param value - Discount curve identifier.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    discount_curve_id as discountCurveId: "string" => id,
});
common_setters!(JsAgencyCmoBuilder, "AgencyCmoBuilder");
