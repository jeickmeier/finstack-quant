//! A private-markets fund valued from its dated commitments and cash calls.

use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::equity::pe_fund::{
    PrivateMarketsFund, PrivateMarketsFundBuilder,
};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// A private-markets fund valued from its dated commitments and cash calls.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.PrivateMarketsFund.example();
    /// console.log(future.id);
    /// future.free();
    /// ```
    JsPrivateMarketsFund, "PrivateMarketsFund", PrivateMarketsFund
);
instrument_surface!(
    JsPrivateMarketsFund,
    "PrivateMarketsFund",
    JsPrivateMarketsFundBuilder,
    "generated.valuations.PrivateMarketsFund"
);
factories!(JsPrivateMarketsFund, "PrivateMarketsFund", PrivateMarketsFund, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsPrivateMarketsFund, "PrivateMarketsFund", |i| {
    /// Effective expiry of the contract.
    /// @returns The ISO expiry, or null if the instrument defines none.
    expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's currency.
    /// @returns Functional currency of the fund.
    currency as currency: "string" => text(i.currency),
    /// Read the contract's waterfall spec.
    /// @returns Waterfall specification defining LP/GP allocation tiers.
    waterfall_spec as waterfallSpec: "generated.valuations.PrivateMarketsFund['waterfall_spec']" => json(i.waterfall_spec),
    /// Read the contract's events.
    /// @returns Time-ordered list of fund events (contributions, proceeds, distributions).
    events as events: "generated.valuations.PrivateMarketsFund['events']" => json(i.events),
    /// Read the contract's discount curve id.
    /// @returns Optional discount curve for future LP cashflows. When None, future cashflows are included undiscounted relative to the caller's valuation date.
    discount_curve_id as discountCurveId: "string | null" => json(i.discount_curve_id),
    /// Read the contract's unrealized nav.
    /// @returns Unrealized net asset value attributable to the LP, stated as of the fund's valuation date. Holder-view residual value : the fund's present value is the PV of LP cashflows strictly after the valuation date plus this NAV. When None, the residual value is zero — a fully realized fund prices to ~0. **Do not combine a NAV mark with forecast distribution events after the valuation date.** The NAV mark already embeds the value of the fund's future realizations, so supplying both double counts them: model either realized history + NAV mark (marked fund), or realized history + projected future events with no NAV (cashflow projection). The NAV is added undiscounted, taken as stated at the valuation date.
    unrealized_nav as unrealizedNav: "NonNullable<generated.valuations.PrivateMarketsFund['unrealized_nav']> | null" => json(i.unrealized_nav),
});
common_getters!(JsPrivateMarketsFund, "PrivateMarketsFund");

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.PrivateMarketsFund.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsPrivateMarketsFundBuilder, "PrivateMarketsFundBuilder", PrivateMarketsFundBuilder, JsPrivateMarketsFund, |b| b.build()
);

#[wasm_bindgen(js_class = PrivateMarketsFund)]
impl JsPrivateMarketsFund {
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    #[wasm_bindgen(js_name = toBuilder)]
    pub fn to_builder(&self) -> JsPrivateMarketsFundBuilder {
        JsPrivateMarketsFundBuilder {
            inner: super::Staged::new(self.inner.to_builder()),
        }
    }
}

setters!(JsPrivateMarketsFundBuilder, "PrivateMarketsFundBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's currency.
    /// @param value - Functional currency of the fund.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    currency as currency: "string" => js_wire,
    /// Set the contract's waterfall spec.
    /// @param value - Waterfall specification defining LP/GP allocation tiers.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    waterfall_spec as waterfallSpec: "generated.valuations.PrivateMarketsFund['waterfall_spec']" => js_wire,
    /// Set the contract's events.
    /// @param value - Time-ordered list of fund events (contributions, proceeds, distributions).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    events as events: "generated.valuations.PrivateMarketsFund['events']" => js_wire,
    /// Set the contract's discount curve id.
    /// @param value - Optional discount curve for future LP cashflows. When None, future cashflows are included undiscounted relative to the caller's valuation date.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    discount_curve_id as discountCurveId: "string | null" => optional_id via discount_curve_id_opt,
    /// Set the contract's unrealized nav.
    /// @param value - Unrealized net asset value attributable to the LP, stated as of the fund's valuation date. Holder-view residual value : the fund's present value is the PV of LP cashflows strictly after the valuation date plus this NAV. When None, the residual value is zero — a fully realized fund prices to ~0. **Do not combine a NAV mark with forecast distribution events after the valuation date.** The NAV mark already embeds the value of the fund's future realizations, so supplying both double counts them: model either realized history + NAV mark (marked fund), or realized history + projected future events with no NAV (cashflow projection). The NAV is added undiscounted, taken as stated at the valuation date.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    unrealized_nav as unrealizedNav: "NonNullable<generated.valuations.PrivateMarketsFund['unrealized_nav']> | null" => js_wire via unrealized_nav_opt,
});
common_setters!(JsPrivateMarketsFundBuilder, "PrivateMarketsFundBuilder");
