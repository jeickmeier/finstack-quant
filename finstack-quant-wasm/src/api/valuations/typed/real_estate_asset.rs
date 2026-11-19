//! A real-estate asset with explicit leases and cash-flow valuation assumptions.

use crate::api::core::dates::JsDayCount;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::equity::real_estate::{
    RealEstateAsset, RealEstateAssetBuilder,
};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// A real-estate asset with explicit leases and cash-flow valuation assumptions.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.RealEstateAsset.example();
    /// console.log(future.id);
    /// future.free();
    /// ```
    JsRealEstateAsset, "RealEstateAsset", RealEstateAsset
);
instrument_surface!(
    JsRealEstateAsset,
    "RealEstateAsset",
    JsRealEstateAssetBuilder,
    "generated.valuations.RealEstateAsset"
);
factories!(JsRealEstateAsset, "RealEstateAsset", RealEstateAsset, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsRealEstateAsset, "RealEstateAsset", |i| {
    /// Effective expiry of the contract.
    /// @returns The ISO expiry, or null if the instrument defines none.
    expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's currency.
    /// @returns Currency for valuation.
    currency as currency: "string" => text(i.currency),
    /// Read the contract's valuation method.
    /// @returns Valuation method (DCF or DirectCap).
    valuation_method as valuationMethod: "generated.valuations.RealEstateAsset['valuation_method']" => json(i.valuation_method),
    /// Read the contract's property type.
    /// @returns Optional property type classification (for reporting).
    property_type as propertyType: "NonNullable<generated.valuations.RealEstateAsset['property_type']> | null" => json(i.property_type),
    /// Read the contract's noi schedule.
    /// @returns Net operating income schedule (date, amount). Entries must be **annual** NOI amounts: cap-rate formulas (direct cap, exit-cap terminal value, going-in cap rate) apply a quoted annual cap rate to a single schedule entry, so a monthly or quarterly schedule would understate those values by 12x/4x. Flows dated exactly on the valuation as_of are included in PV undiscounted (t = 0).
    noi_schedule as noiSchedule: "generated.valuations.RealEstateAsset['noi_schedule']" => dated_nums(i.noi_schedule),
    /// Read the contract's capex schedule.
    /// @returns Capital expenditure schedule (date, amount). Values are treated as **positive outflows**. When present, cashflows are valued as NOI - CapEx (unlevered net cash flow).
    capex_schedule as capexSchedule: "NonNullable<generated.valuations.RealEstateAsset['capex_schedule']> | null" => json(i.capex_schedule),
    /// Read the contract's discount rate.
    /// @returns Discount rate for DCF (annualized).
    discount_rate as discountRate: "number | null" => json(i.discount_rate),
    /// Read the contract's cap rate.
    /// @returns Capitalization rate for direct cap (annualized).
    cap_rate as capRate: "number | null" => json(i.cap_rate),
    /// Read the contract's stabilized noi.
    /// @returns Optional stabilized NOI override for direct cap.
    stabilized_noi as stabilizedNoi: "number | null" => json(i.stabilized_noi),
    /// Read the contract's terminal cap rate.
    /// @returns Optional terminal cap rate for DCF (uses last NOI).
    terminal_cap_rate as terminalCapRate: "number | null" => json(i.terminal_cap_rate),
    /// Read the contract's terminal growth rate.
    /// @returns Optional terminal growth rate used to project NOI_{N+1} for exit valuation. Market convention for exit-cap terminal value is (TV = NOI_{N+1} / cap_rate_exit). When not provided, defaults to 0 (uses last NOI as-is, i.e. a trailing-NOI terminal value). Exactly **one** period of growth is applied to the last NOI on/before the exit date — it is not compounded over any gap between the last scheduled NOI and a later sale_date. Validation range is ([-100%, 20%]) to guard against configuration errors.
    terminal_growth_rate as terminalGrowthRate: "number | null" => json(i.terminal_growth_rate),
    /// Read the contract's sale date.
    /// @returns Optional sale/exit date that truncates the DCF horizon. When set, DCF only values unlevered flows up to and including sale_date. Terminal proceeds (if configured) are realized on sale_date.
    sale_date as saleDate: "string | null" => opt_date(i.sale_date),
    /// Read the contract's sale price.
    /// @returns Optional explicit gross sale price (terminal proceeds), before disposition costs. When set, this takes precedence over terminal_cap_rate for terminal proceeds.
    sale_price as salePrice: "NonNullable<generated.valuations.RealEstateAsset['sale_price']> | null" => json(i.sale_price),
    /// Read the contract's purchase price.
    /// @returns Optional purchase price (useful for IRR / cap rate metrics).
    purchase_price as purchasePrice: "NonNullable<generated.valuations.RealEstateAsset['purchase_price']> | null" => json(i.purchase_price),
    /// Read the contract's acquisition costs.
    /// @returns Acquisition (closing) cost line items in instrument currency, as positive outflow magnitudes deducted at as_of in DCF valuation.
    acquisition_costs as acquisitionCosts: "generated.valuations.RealEstateAsset['acquisition_costs']" => json(i.acquisition_costs),
    /// Read the contract's disposition cost decimal.
    /// @returns Optional disposition cost percentage applied to terminal value. A value of 0.02 represents 2% selling costs. Must be in ([0, 1)).
    disposition_cost_decimal as dispositionCostDecimal: "number | null" => json(i.disposition_cost_decimal),
    /// Read the contract's disposition costs.
    /// @returns Optional detailed disposition cost line items (positive outflows) deducted from terminal proceeds.
    disposition_costs as dispositionCosts: "generated.valuations.RealEstateAsset['disposition_costs']" => json(i.disposition_costs),
    /// Read the contract's appraisal value.
    /// @returns Optional appraisal override value.
    appraisal_value as appraisalValue: "NonNullable<generated.valuations.RealEstateAsset['appraisal_value']> | null" => json(i.appraisal_value),
    /// Read the contract's day count.
    /// @returns Day count convention for year fractions.
    day_count as dayCount: "DayCount" => day_count(i.day_count),
});
common_getters!(JsRealEstateAsset, "RealEstateAsset");

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.RealEstateAsset.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsRealEstateAssetBuilder, "RealEstateAssetBuilder", RealEstateAssetBuilder, JsRealEstateAsset, |b| b.build()
);

#[wasm_bindgen(js_class = RealEstateAsset)]
impl JsRealEstateAsset {
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    #[wasm_bindgen(js_name = toBuilder)]
    pub fn to_builder(&self) -> JsRealEstateAssetBuilder {
        JsRealEstateAssetBuilder {
            inner: super::Staged::new(self.inner.to_builder()),
        }
    }
}

setters!(JsRealEstateAssetBuilder, "RealEstateAssetBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's currency.
    /// @param value - Currency for valuation.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    currency as currency: "string" => js_wire,
    /// Set the contract's valuation method.
    /// @param value - Valuation method (DCF or DirectCap).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    valuation_method as valuationMethod: "generated.valuations.RealEstateAsset['valuation_method']" => js_wire,
    /// Set the contract's property type.
    /// @param value - Optional property type classification (for reporting).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    property_type as propertyType: "NonNullable<generated.valuations.RealEstateAsset['property_type']> | null" => js_wire via property_type_opt,
    /// Set the contract's noi schedule.
    /// @param value - Net operating income schedule (date, amount). Entries must be **annual** NOI amounts: cap-rate formulas (direct cap, exit-cap terminal value, going-in cap rate) apply a quoted annual cap rate to a single schedule entry, so a monthly or quarterly schedule would understate those values by 12x/4x. Flows dated exactly on the valuation as_of are included in PV undiscounted (t = 0).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    noi_schedule as noiSchedule: "generated.valuations.RealEstateAsset['noi_schedule']" => js_wire,
    /// Set the contract's capex schedule.
    /// @param value - Capital expenditure schedule (date, amount). Values are treated as **positive outflows**. When present, cashflows are valued as NOI - CapEx (unlevered net cash flow).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    capex_schedule as capexSchedule: "NonNullable<generated.valuations.RealEstateAsset['capex_schedule']> | null" => js_wire via capex_schedule_opt,
    /// Set the contract's discount rate.
    /// @param value - Discount rate for DCF (annualized).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    discount_rate as discountRate: "number | null" => js_wire via discount_rate_opt,
    /// Set the contract's cap rate.
    /// @param value - Capitalization rate for direct cap (annualized).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    cap_rate as capRate: "number | null" => js_wire via cap_rate_opt,
    /// Set the contract's stabilized noi.
    /// @param value - Optional stabilized NOI override for direct cap.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    stabilized_noi as stabilizedNoi: "number | null" => js_wire via stabilized_noi_opt,
    /// Set the contract's terminal cap rate.
    /// @param value - Optional terminal cap rate for DCF (uses last NOI).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    terminal_cap_rate as terminalCapRate: "number | null" => js_wire via terminal_cap_rate_opt,
    /// Set the contract's terminal growth rate.
    /// @param value - Optional terminal growth rate used to project NOI_{N+1} for exit valuation. Market convention for exit-cap terminal value is (TV = NOI_{N+1} / cap_rate_exit). When not provided, defaults to 0 (uses last NOI as-is, i.e. a trailing-NOI terminal value). Exactly **one** period of growth is applied to the last NOI on/before the exit date — it is not compounded over any gap between the last scheduled NOI and a later sale_date. Validation range is ([-100%, 20%]) to guard against configuration errors.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    terminal_growth_rate as terminalGrowthRate: "number | null" => js_wire via terminal_growth_rate_opt,
    /// Set the contract's sale date.
    /// @param value - Optional sale/exit date that truncates the DCF horizon. When set, DCF only values unlevered flows up to and including sale_date. Terminal proceeds (if configured) are realized on sale_date.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    sale_date as saleDate: "string | null" => js_wire via sale_date_opt,
    /// Set the contract's sale price.
    /// @param value - Optional explicit gross sale price (terminal proceeds), before disposition costs. When set, this takes precedence over terminal_cap_rate for terminal proceeds.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    sale_price as salePrice: "NonNullable<generated.valuations.RealEstateAsset['sale_price']> | null" => js_wire via sale_price_opt,
    /// Set the contract's purchase price.
    /// @param value - Optional purchase price (useful for IRR / cap rate metrics).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    purchase_price as purchasePrice: "NonNullable<generated.valuations.RealEstateAsset['purchase_price']> | null" => js_wire via purchase_price_opt,
    /// Set the contract's acquisition costs.
    /// @param value - Acquisition (closing) cost line items in instrument currency, as positive outflow magnitudes deducted at as_of in DCF valuation.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    acquisition_costs as acquisitionCosts: "generated.valuations.RealEstateAsset['acquisition_costs']" => js_wire,
    /// Set the contract's disposition cost decimal.
    /// @param value - Optional disposition cost percentage applied to terminal value. A value of 0.02 represents 2% selling costs. Must be in ([0, 1)).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    disposition_cost_decimal as dispositionCostDecimal: "number | null" => js_wire via disposition_cost_decimal_opt,
    /// Set the contract's disposition costs.
    /// @param value - Optional detailed disposition cost line items (positive outflows) deducted from terminal proceeds.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    disposition_costs as dispositionCosts: "generated.valuations.RealEstateAsset['disposition_costs']" => js_wire,
    /// Set the contract's appraisal value.
    /// @param value - Optional appraisal override value.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    appraisal_value as appraisalValue: "NonNullable<generated.valuations.RealEstateAsset['appraisal_value']> | null" => js_wire via appraisal_value_opt,
});
common_setters!(JsRealEstateAssetBuilder, "RealEstateAssetBuilder");

handle_setters!(JsRealEstateAssetBuilder, "RealEstateAssetBuilder", {
    /// Set the contract's day count.
    /// @param value - Day count convention for year fractions.
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    day_count as dayCount => JsDayCount,
});
