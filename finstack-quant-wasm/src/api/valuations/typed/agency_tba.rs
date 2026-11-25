//! A forward purchase or sale of agency mortgage collateral under TBA terms.

use crate::api::core::money::JsMoney;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::fixed_income::tba::{AgencyTba, AgencyTbaBuilder};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// A forward purchase or sale of agency mortgage collateral under TBA terms.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.AgencyTba.example();
    /// console.log(future.id);
    /// future.free();
    /// ```
    JsAgencyTba, "AgencyTba", AgencyTba
);
instrument_surface!(
    JsAgencyTba,
    "AgencyTba",
    JsAgencyTbaBuilder,
    "generated.valuations.AgencyTba"
);
factories!(JsAgencyTba, "AgencyTba", AgencyTba, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsAgencyTba, "AgencyTba", |i| {
    /// Effective expiry of the contract.
    /// @returns The ISO expiry, or null if the instrument defines none.
    expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's agency.
    /// @returns Agency program (FNMA, FHLMC, GNMA).
    agency as agency: "generated.valuations.AgencyTba['agency']" => json(i.agency),
    /// Read the contract's coupon.
    /// @returns Pass-through coupon rate (e.g., 0.04 for 4%).
    coupon as coupon: "number" => json(i.coupon),
    /// Read the contract's term.
    /// @returns Original loan term.
    term as term: "generated.valuations.AgencyTba['term']" => json(i.term),
    /// Read the contract's settlement year.
    /// @returns Settlement year.
    settlement_year as settlementYear: "number" => json(i.settlement_year),
    /// Read the contract's settlement month.
    /// @returns Settlement month (1-12).
    settlement_month as settlementMonth: "number" => json(i.settlement_month),
    /// Read the contract's settlement class.
    /// @returns SIFMA settlement class of the deliverable pools (A/B/C/D). Required: it is never inferred from agency or term, because those labels do not establish security eligibility for a class.
    settlement_class as settlementClass: "generated.valuations.AgencyTba['settlement_class']" => json(i.settlement_class),
    /// Read the contract's settlement date.
    /// @returns Explicit settlement date override. When set, bypasses the SIFMA calendar lookup for settlement_year/settlement_month in AgencyTba::effective_settlement_date. Dollar rolls use this to keep leg pricing consistent with explicit roll settlement dates.
    settlement_date as settlementDate: "string | null" => opt_date(i.settlement_date),
    /// Read the contract's notional.
    /// @returns Current face amount purchased at settlement, in the trade currency.
    notional as notional: "Money" => money(i.notional),
    /// Read the contract's trade price.
    /// @returns Trade price (percentage of par, e.g., 98.5).
    trade_price as tradePrice: "number" => json(i.trade_price),
    /// Read the contract's trade date.
    /// @returns Trade date.
    trade_date as tradeDate: "string | null" => opt_date(i.trade_date),
    /// Read the contract's pool factor.
    /// @returns Current/original face ratio in (0, 1] for the assumed delivered pool. Defaults to 1.0 for a generic pool. Original face is notional / factor; the purchased current face remains notional. When an explicit pool is supplied, its factor is used and any specified factor must agree.
    pool_factor as poolFactor: "number | null" => json(i.pool_factor),
    /// Read the contract's assumed pool.
    /// @returns Optional assumed pool for valuation. Its current and original faces are scaled together to the trade's purchased current face. If absent, generic pool characteristics apply.
    assumed_pool as assumedPool: "NonNullable<generated.valuations.AgencyTba['assumed_pool']> | null" => json(i.assumed_pool),
    /// Read the contract's prepayment spec.
    /// @returns Prepayment model of the generic assumed pool. Applies only when assumed_pool is absent (an explicit pool carries its own model); None uses the embedded generic PSA assumption. Setting it together with assumed_pool is rejected.
    prepayment_spec as prepaymentSpec: "NonNullable<generated.valuations.AgencyTba['prepayment_spec']> | null" => json(i.prepayment_spec),
    /// Read the contract's discount curve id.
    /// @returns Discount curve identifier.
    discount_curve_id as discountCurveId: "string" => json(i.discount_curve_id),
});
common_getters!(JsAgencyTba, "AgencyTba");

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.AgencyTba.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsAgencyTbaBuilder, "AgencyTbaBuilder", AgencyTbaBuilder, JsAgencyTba, |b| b.build()
);

#[wasm_bindgen(js_class = AgencyTba)]
impl JsAgencyTba {
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    #[wasm_bindgen(js_name = toBuilder)]
    pub fn to_builder(&self) -> JsAgencyTbaBuilder {
        JsAgencyTbaBuilder {
            inner: super::Staged::new(self.inner.to_builder()),
        }
    }
}

setters!(JsAgencyTbaBuilder, "AgencyTbaBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's agency.
    /// @param value - Agency program (FNMA, FHLMC, GNMA).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    agency as agency: "generated.valuations.AgencyTba['agency']" => js_wire,
    /// Set the contract's coupon.
    /// @param value - Pass-through coupon rate (e.g., 0.04 for 4%).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    coupon as coupon: "number" => js_f64,
    /// Set the contract's term.
    /// @param value - Original loan term.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    term as term: "generated.valuations.AgencyTba['term']" => js_wire,
    /// Set the contract's settlement year.
    /// @param value - Settlement year.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    settlement_year as settlementYear: "number" => js_int,
    /// Set the contract's settlement month.
    /// @param value - Settlement month (1-12).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    settlement_month as settlementMonth: "number" => js_uint,
    /// Set the contract's settlement class.
    /// @param value - SIFMA settlement class of the deliverable pools (A/B/C/D). Required: it is never inferred from agency or term, because those labels do not establish security eligibility for a class.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    settlement_class as settlementClass: "generated.valuations.AgencyTba['settlement_class']" => js_wire,
    /// Set the contract's settlement date.
    /// @param value - Explicit settlement date override. When set, bypasses the SIFMA calendar lookup for settlement_year/settlement_month in AgencyTba::effective_settlement_date. Dollar rolls use this to keep leg pricing consistent with explicit roll settlement dates.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    settlement_date as settlementDate: "string | null" => opt_date via settlement_date_opt,
    /// Set the contract's trade price.
    /// @param value - Trade price (percentage of par, e.g., 98.5).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    trade_price as tradePrice: "number" => js_f64,
    /// Set the contract's trade date.
    /// @param value - Trade date.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    trade_date as tradeDate: "string | null" => opt_date via trade_date_opt,
    /// Set the contract's pool factor.
    /// @param value - Current/original face ratio in (0, 1] for the assumed delivered pool. Defaults to 1.0 for a generic pool. Original face is notional / factor; the purchased current face remains notional. When an explicit pool is supplied, its factor is used and any specified factor must agree.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    pool_factor as poolFactor: "number | null" => opt_f64 via pool_factor_opt,
    /// Set the contract's assumed pool.
    /// @param value - Optional assumed pool for valuation. Its current and original faces are scaled together to the trade's purchased current face. If absent, generic pool characteristics apply.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    assumed_pool as assumedPool: "NonNullable<generated.valuations.AgencyTba['assumed_pool']> | null" => opt_wire via assumed_pool_opt,
    /// Set the contract's prepayment spec.
    /// @param value - Prepayment model of the generic assumed pool. Applies only when assumed_pool is absent (an explicit pool carries its own model); None uses the embedded generic PSA assumption. Setting it together with assumed_pool is rejected.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    prepayment_spec as prepaymentSpec: "NonNullable<generated.valuations.AgencyTba['prepayment_spec']> | null" => opt_wire via prepayment_spec_opt,
    /// Set the contract's discount curve id.
    /// @param value - Discount curve identifier.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    discount_curve_id as discountCurveId: "string" => id,
});
common_setters!(JsAgencyTbaBuilder, "AgencyTbaBuilder");

handle_setters!(JsAgencyTbaBuilder, "AgencyTbaBuilder", {
    /// Set the contract's notional.
    /// @param value - Current face amount purchased at settlement, in the trade currency.
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    notional as notional => JsMoney,
});
