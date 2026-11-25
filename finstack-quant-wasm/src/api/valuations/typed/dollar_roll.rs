//! A paired purchase and sale of agency TBA contracts on distinct value dates.

use crate::api::core::money::JsMoney;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::fixed_income::dollar_roll::{
    DollarRoll, DollarRollBuilder,
};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// A paired purchase and sale of agency TBA contracts on distinct value dates.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.DollarRoll.example();
    /// console.log(future.id);
    /// future.free();
    /// ```
    JsDollarRoll, "DollarRoll", DollarRoll
);
instrument_surface!(
    JsDollarRoll,
    "DollarRoll",
    JsDollarRollBuilder,
    "generated.valuations.DollarRoll"
);
factories!(JsDollarRoll, "DollarRoll", DollarRoll, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsDollarRoll, "DollarRoll", |i| {
    /// Effective expiry of the contract.
    /// @returns The ISO expiry, or null if the instrument defines none.
    expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's agency.
    /// @returns Agency program.
    agency as agency: "generated.valuations.DollarRoll['agency']" => json(i.agency),
    /// Read the contract's coupon.
    /// @returns Pass-through coupon rate.
    coupon as coupon: "number" => json(i.coupon),
    /// Read the contract's term.
    /// @returns Original loan term.
    term as term: "generated.valuations.DollarRoll['term']" => json(i.term),
    /// Read the contract's notional.
    /// @returns Trade notional (par amount).
    notional as notional: "Money" => money(i.notional),
    /// Read the contract's front settlement year.
    /// @returns Front-month settlement year.
    front_settlement_year as frontSettlementYear: "number" => json(i.front_settlement_year),
    /// Read the contract's front settlement month.
    /// @returns Front-month settlement month (1-12).
    front_settlement_month as frontSettlementMonth: "number" => json(i.front_settlement_month),
    /// Read the contract's back settlement year.
    /// @returns Back-month settlement year.
    back_settlement_year as backSettlementYear: "number" => json(i.back_settlement_year),
    /// Read the contract's back settlement month.
    /// @returns Back-month settlement month (1-12).
    back_settlement_month as backSettlementMonth: "number" => json(i.back_settlement_month),
    /// Read the contract's settlement class.
    /// @returns SIFMA settlement class of the deliverable pools (A/B/C/D), used for both legs. Required: never inferred from agency or term.
    settlement_class as settlementClass: "generated.valuations.DollarRoll['settlement_class']" => json(i.settlement_class),
    /// Read the contract's front settlement date.
    /// @returns Explicit front-month settlement date override. When set, bypasses the SIFMA calendar lookup for the front leg.
    front_settlement_date as frontSettlementDate: "string | null" => opt_date(i.front_settlement_date),
    /// Read the contract's back settlement date.
    /// @returns Explicit back-month settlement date override. When set, bypasses the SIFMA calendar lookup for the back leg.
    back_settlement_date as backSettlementDate: "string | null" => opt_date(i.back_settlement_date),
    /// Read the contract's front price.
    /// @returns Front-month price (sell price).
    front_price as frontPrice: "number" => json(i.front_price),
    /// Read the contract's back price.
    /// @returns Back-month price (buy price).
    back_price as backPrice: "number" => json(i.back_price),
    /// Read the contract's trade date.
    /// @returns Trade date.
    trade_date as tradeDate: "string | null" => opt_date(i.trade_date),
    /// Read the contract's prepayment spec.
    /// @returns Prepayment model of the generic pool both legs deliver. None uses the embedded generic PSA assumption of the TBA legs.
    prepayment_spec as prepaymentSpec: "NonNullable<generated.valuations.DollarRoll['prepayment_spec']> | null" => json(i.prepayment_spec),
    /// Read the contract's discount curve id.
    /// @returns Discount curve identifier.
    discount_curve_id as discountCurveId: "string" => json(i.discount_curve_id),
    /// Read the contract's repo curve id.
    /// @returns Optional repo/financing curve identifier (carry-only). Discount curve used by roll specialness to calculate the simple financing rate (DF(front)/DF(back) − 1)/τ over the front/back settlement interval, with τ on ACT/360. The implied financing rate itself is determined by prices and carry. See the carry calculations (see crate::dollar_roll::carry module). Does **not** affect the mark-to-market PV, which always discounts both legs at discount_curve_id. When None, the discount curve rate is used as the reference financing rate for carry analytics.
    repo_curve_id as repoCurveId: "string | null" => json(i.repo_curve_id),
});
common_getters!(JsDollarRoll, "DollarRoll");

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.DollarRoll.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsDollarRollBuilder, "DollarRollBuilder", DollarRollBuilder, JsDollarRoll, |b| b.build()
);

#[wasm_bindgen(js_class = DollarRoll)]
impl JsDollarRoll {
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    #[wasm_bindgen(js_name = toBuilder)]
    pub fn to_builder(&self) -> JsDollarRollBuilder {
        JsDollarRollBuilder {
            inner: super::Staged::new(self.inner.to_builder()),
        }
    }
}

setters!(JsDollarRollBuilder, "DollarRollBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's agency.
    /// @param value - Agency program.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    agency as agency: "generated.valuations.DollarRoll['agency']" => js_wire,
    /// Set the contract's coupon.
    /// @param value - Pass-through coupon rate.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    coupon as coupon: "number" => js_f64,
    /// Set the contract's term.
    /// @param value - Original loan term.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    term as term: "generated.valuations.DollarRoll['term']" => js_wire,
    /// Set the contract's front settlement year.
    /// @param value - Front-month settlement year.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    front_settlement_year as frontSettlementYear: "number" => js_int,
    /// Set the contract's front settlement month.
    /// @param value - Front-month settlement month (1-12).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    front_settlement_month as frontSettlementMonth: "number" => js_uint,
    /// Set the contract's back settlement year.
    /// @param value - Back-month settlement year.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    back_settlement_year as backSettlementYear: "number" => js_int,
    /// Set the contract's back settlement month.
    /// @param value - Back-month settlement month (1-12).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    back_settlement_month as backSettlementMonth: "number" => js_uint,
    /// Set the contract's settlement class.
    /// @param value - SIFMA settlement class of the deliverable pools (A/B/C/D), used for both legs. Required: never inferred from agency or term.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    settlement_class as settlementClass: "generated.valuations.DollarRoll['settlement_class']" => js_wire,
    /// Set the contract's front settlement date.
    /// @param value - Explicit front-month settlement date override. When set, bypasses the SIFMA calendar lookup for the front leg.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    front_settlement_date as frontSettlementDate: "string | null" => opt_date via front_settlement_date_opt,
    /// Set the contract's back settlement date.
    /// @param value - Explicit back-month settlement date override. When set, bypasses the SIFMA calendar lookup for the back leg.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    back_settlement_date as backSettlementDate: "string | null" => opt_date via back_settlement_date_opt,
    /// Set the contract's front price.
    /// @param value - Front-month price (sell price).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    front_price as frontPrice: "number" => js_f64,
    /// Set the contract's back price.
    /// @param value - Back-month price (buy price).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    back_price as backPrice: "number" => js_f64,
    /// Set the contract's trade date.
    /// @param value - Trade date.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    trade_date as tradeDate: "string | null" => opt_date via trade_date_opt,
    /// Set the contract's prepayment spec.
    /// @param value - Prepayment model of the generic pool both legs deliver. None uses the embedded generic PSA assumption of the TBA legs.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    prepayment_spec as prepaymentSpec: "NonNullable<generated.valuations.DollarRoll['prepayment_spec']> | null" => opt_wire via prepayment_spec_opt,
    /// Set the contract's discount curve id.
    /// @param value - Discount curve identifier.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    discount_curve_id as discountCurveId: "string" => id,
    /// Set the contract's repo curve id.
    /// @param value - Optional repo/financing curve identifier (carry-only). Discount curve used by roll specialness to calculate the simple financing rate (DF(front)/DF(back) − 1)/τ over the front/back settlement interval, with τ on ACT/360. The implied financing rate itself is determined by prices and carry. See the carry calculations (see crate::dollar_roll::carry module). Does **not** affect the mark-to-market PV, which always discounts both legs at discount_curve_id. When None, the discount curve rate is used as the reference financing rate for carry analytics.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    repo_curve_id as repoCurveId: "string | null" => optional_id via repo_curve_id_opt,
});
common_setters!(JsDollarRollBuilder, "DollarRollBuilder");

handle_setters!(JsDollarRollBuilder, "DollarRollBuilder", {
    /// Set the contract's notional.
    /// @param value - Trade notional (par amount).
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    notional as notional => JsMoney,
});
