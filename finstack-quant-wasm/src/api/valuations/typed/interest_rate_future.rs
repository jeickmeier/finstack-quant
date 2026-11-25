//! A listed future on a short-term rate.

use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::rates::ir_future::{
    InterestRateFuture, InterestRateFutureBuilder,
};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// A listed future on a short-term rate.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.InterestRateFuture.example();
    /// console.log(future.terms.contracts);
    /// future.free();
    /// ```
    JsInterestRateFuture, "InterestRateFuture", InterestRateFuture
);
instrument_surface!(
    JsInterestRateFuture,
    "InterestRateFuture",
    JsInterestRateFutureBuilder,
    "generated.valuations.InterestRateFuture"
);
factories!(JsInterestRateFuture, "InterestRateFuture", InterestRateFuture, {
    /// Construct the native example for this exchange-listed future.
    /// @returns Validated position and exchange lifecycle terms.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsInterestRateFuture, "InterestRateFuture", |i| {
    /// Effective expiry of the listed contract.
    /// @returns The ISO expiry, or null if the instrument defines none.
    expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Return the contract's position and lifecycle terms.
    /// @returns Independent copy of position, lifecycle dates and exchange marks.
    terms as terms: "generated.valuations.ListedFutureTerms" => json(i.terms),
});
common_getters!(JsInterestRateFuture, "InterestRateFuture");

builder_class!(
    /// Builder for an exchange-listed future.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.InterestRateFuture.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("FUTURE-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsInterestRateFutureBuilder, "InterestRateFutureBuilder", InterestRateFutureBuilder, JsInterestRateFuture, |b| b.build()
);

#[wasm_bindgen(js_class = InterestRateFuture)]
impl JsInterestRateFuture {
    /// Seed an independent builder from the complete listed contract.
    /// @returns Builder carrying settlement, exchange marks and market identifiers.
    #[wasm_bindgen(js_name = toBuilder)]
    pub fn to_builder(&self) -> JsInterestRateFutureBuilder {
        JsInterestRateFutureBuilder {
            inner: super::Staged::new(self.inner.to_builder()),
        }
    }
}

setters!(JsInterestRateFutureBuilder, "InterestRateFutureBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's terms.
    /// @param value - Position, contract multiplier, trade and official marks, and lifecycle dates.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    terms as terms: "generated.valuations.ListedFutureTerms" => js_wire,
    /// Set the contract's fixing date.
    /// @param value - Optional observation date for term-rate settlement, in ISO calendar days. Pass null to clear it.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    fixing_date as fixingDate: "string | null" => opt_date via fixing_date_opt,
    /// Set the contract's period start.
    /// @param value - Optional start date of the reference rate accrual period. Pass null to clear it.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    period_start as periodStart: "string | null" => opt_date via period_start_opt,
    /// Set the contract's period end.
    /// @param value - Optional end date of the reference rate accrual period. Pass null to clear it.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    period_end as periodEnd: "string | null" => opt_date via period_end_opt,
    /// Set the contract's day count.
    /// @param value - Accrual and convexity clock day-count convention.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    day_count as dayCount: "string" => js_parse,
    /// Set the contract's contract specs.
    /// @param value - Exchange face, price tick, cycle months and optional decimal convexity adjustment.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    contract_specs as contractSpecs: "generated.valuations.FutureContractSpecs" => js_wire,
    /// Set the contract's discount curve id.
    /// @param value - Market identifier of the futures settlement discount curve.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    discount_curve_id as discountCurveId: "string" => id,
    /// Set the contract's forward curve id.
    /// @param value - Market identifier of the quoted reference-rate projection curve.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    forward_curve_id as forwardCurveId: "string" => id,
    /// Set the contract's compounding.
    /// @param value - Calculation convention for the rate period, including overnight averaging or compounding.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    compounding as compounding: "string" => js_wire,
    /// Set the contract's index id.
    /// @param value - Optional fixing-series identifier for observed resets. Pass null to clear it.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    index_id as indexId: "string | null" => optional_id via index_id_opt,
    /// Set the contract's fixing calendar id.
    /// @param value - Optional business-day calendar for looking up historical fixings. Pass null to clear it.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    fixing_calendar_id as fixingCalendarId: "string | null" => optional_id via fixing_calendar_id_opt,
    /// Set the contract's vol surface id.
    /// @param value - Optional surface supplying absolute normal-rate convexity volatility. Pass null to clear it.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    vol_surface_id as volSurfaceId: "string | null" => optional_id via vol_surface_id_opt,
});
common_setters!(JsInterestRateFutureBuilder, "InterestRateFutureBuilder");
