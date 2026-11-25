//! Typed bond future construction and pricing.

use super::fixed_income::JsBond;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::fixed_income::bond_future::{
    BondFuture, BondFutureBuilder,
};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// Exchange-listed future with a caller-selected eligible delivery candidate.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.BondFuture.example();
    /// console.log(future.terms.contracts);
    /// future.free();
    /// ```
    JsBondFuture, "BondFuture", BondFuture
);
instrument_surface!(
    JsBondFuture,
    "BondFuture",
    JsBondFutureBuilder,
    "generated.valuations.BondFuture"
);
factories!(JsBondFuture, "BondFuture", BondFuture, {
    /// Construct a representative US Treasury future.
    /// @returns Listed contract with the native example basket and selected deliverable ID.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsBondFuture, "BondFuture", |i| {
    /// Effective expiry of the listed contract.
    /// @returns The ISO expiry, or null if the instrument defines none.
    expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Return the contract's position and lifecycle terms.
    /// @returns Independent listed quantity, price units, marks and settlement terms.
    terms as terms: "generated.valuations.ListedFutureTerms" => json(i.terms),
    /// First eligible delivery date.
    /// @returns ISO-8601 delivery start date.
    delivery_start as deliveryStart: "string" => date(i.delivery_start),
    /// Conversion-factor and repo specifications for the contract.
    /// @returns Standard coupon, maturity and repo day-count convention.
    contract_specs as contractSpecs: "generated.valuations.BondFutureSpecs" => json(i.contract_specs),
    /// Basket of eligible deliverable bonds with published conversion factors.
    /// @returns Ordered deliverable basket entries.
    deliverable_basket as deliverableBasket: "generated.valuations.DeliverableBond[]" => json(i.deliverable_basket),
    /// Identifier of the selected cheapest-to-deliver (CTD) bond.
    /// @returns Selected CTD bond identifier, or `null` when resolved implicitly from a single-bond basket.
    ctd_bond_id as ctdBondId: "string | null" => opt_text(i.ctd_bond_id),
    /// Embedded cheapest-to-deliver (CTD) bond definition, when supplied inline.
    /// @returns Inline CTD bond spec plain object, or `null`.
    ctd_bond as ctdBond: "NonNullable<generated.valuations.BondFuture['ctd_bond']> | null" => json(i.ctd_bond),
    /// Financing and discount curve identifier.
    /// @returns Curve identifier used for carrying the deliverable when no repo curve is set.
    discount_curve_id as discountCurveId: "string" => text(i.discount_curve_id),
    /// Optional repo financing curve identifier for the chosen deliverable.
    /// @returns Repo curve identifier, or `null`.
    repo_curve_id as repoCurveId: "string | null" => opt_text(i.repo_curve_id),
});
common_getters!(JsBondFuture, "BondFuture");
to_builder_method!(
    /// Seed an independent builder from the complete listed contract.
    /// @returns Builder carrying the terms, financing inputs and selected deliverable.
    JsBondFuture,
    "BondFuture",
    JsBondFutureBuilder
);

builder_class!(
    /// Builder for a listed bond future.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.BondFuture.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("TY-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsBondFutureBuilder, "BondFutureBuilder", BondFutureBuilder, JsBondFuture, |b| b.build()
);

setters!(JsBondFutureBuilder, "BondFutureBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's terms.
    /// @param value - Listed quantity, multiplier, trade price, exchange marks and lifecycle dates.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    terms as terms: "generated.valuations.ListedFutureTerms" => js_wire,
    /// Set the contract's delivery start.
    /// @param value - First eligible delivery date as a date or ISO string.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    delivery_start as deliveryStart: "string" => js_date,
    /// Set the contract's contract specs.
    /// @param value - Standard coupon and maturity for the conversion convention and the implied repo day count.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    contract_specs as contractSpecs: "generated.valuations.BondFutureSpecs" => js_wire,
    /// Set the contract's deliverable basket.
    /// @param value - Nonempty list of eligible bond identifiers and positive published conversion factors.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    deliverable_basket as deliverableBasket: "readonly generated.valuations.DeliverableBond[]" => js_wire,
    /// Set the contract's ctd bond id.
    /// @param value - Identifier of the chosen bond, which must belong to the eligible basket.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    ctd_bond_id as ctdBondId: "string" => id,
    /// Set the contract's discount curve id.
    /// @param value - Funding curve identifier used for carrying the deliverable when no repo curve is supplied.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    discount_curve_id as discountCurveId: "string" => id,
    /// Set the contract's repo curve id.
    /// @param value - Optional financing curve for the chosen deliverable.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    repo_curve_id as repoCurveId: "string" => id,
});
#[wasm_bindgen(js_class = BondFutureBuilder)]
impl JsBondFutureBuilder {
    /// Embed the eligible bond used to model the delivery price.
    /// @param value - Typed bond with an ID matching the selected basket entry.
    /// @returns The active builder with the embedded bond.
    /// @throws FinstackError if the builder is inactive.
    #[wasm_bindgen(js_name = ctdBond)]
    pub fn ctd_bond(&self, value: &JsBond) -> Result<JsBondFutureBuilder, JsValue> {
        let inner = self
            .inner
            .apply(|builder| builder.ctd_bond(value.inner.clone()))?;
        Ok(Self { inner })
    }
}
common_setters!(JsBondFutureBuilder, "BondFutureBuilder");
