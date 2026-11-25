//! An option on the spread between two constant-maturity swap rates.

use crate::api::core::dates::JsDayCount;
use crate::api::core::money::JsMoney;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::rates::cms_spread_option::{
    CmsSpreadOption, CmsSpreadOptionBuilder,
};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// An option on the spread between two constant-maturity swap rates.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.CmsSpreadOption.example();
    /// console.log(future.id);
    /// future.free();
    /// ```
    JsCmsSpreadOption, "CmsSpreadOption", CmsSpreadOption
);
instrument_surface!(
    JsCmsSpreadOption,
    "CmsSpreadOption",
    JsCmsSpreadOptionBuilder,
    "generated.valuations.CmsSpreadOption"
);
factories!(JsCmsSpreadOption, "CmsSpreadOption", CmsSpreadOption, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsCmsSpreadOption, "CmsSpreadOption", |i| {
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's long cms tenor.
    /// @returns Long CMS tenor (e.g., 10Y).
    long_cms_tenor as longCmsTenor: "generated.valuations.CmsSpreadOption['long_cms_tenor']" => json(i.long_cms_tenor),
    /// Read the contract's short cms tenor.
    /// @returns Short CMS tenor (e.g., 2Y).
    short_cms_tenor as shortCmsTenor: "generated.valuations.CmsSpreadOption['short_cms_tenor']" => json(i.short_cms_tenor),
    /// Read the contract's strike.
    /// @returns Strike on the CMS spread as a decimal rate (0.005 = 50bp).
    strike as strike: "string" => text(i.strike),
    /// Read the contract's option type.
    /// @returns Call or put on the spread CMS_long - CMS_short: a call pays max(spread - strike, 0), a put pays max(strike - spread, 0).
    option_type as optionType: "generated.valuations.CmsSpreadOption['option_type']" => json(i.option_type),
    /// Read the contract's notional.
    /// @returns Notional amount.
    notional as notional: "Money" => money(i.notional),
    /// Read the contract's expiry.
    /// @returns Option expiry date.
    expiry as expiry: "string" => date(i.expiry),
    /// Read the contract's payment date.
    /// @returns Payment date (may differ from expiry).
    payment_date as paymentDate: "string" => date(i.payment_date),
    /// Read the contract's long vol surface id.
    /// @returns Black swaption volatility surface for the long tenor. The selected expiry slice must be flat across strikes; tenor-axis ATM surfaces are valid.
    long_vol_surface_id as longVolSurfaceId: "string" => json(i.long_vol_surface_id),
    /// Read the contract's short vol surface id.
    /// @returns Black swaption volatility surface for the short tenor, subject to the same flat-strike restriction as the long-tenor surface.
    short_vol_surface_id as shortVolSurfaceId: "string" => json(i.short_vol_surface_id),
    /// Read the contract's discount curve id.
    /// @returns Discount curve ID.
    discount_curve_id as discountCurveId: "string" => json(i.discount_curve_id),
    /// Read the contract's forward curve id.
    /// @returns Forward curve ID (for swap rate projection).
    forward_curve_id as forwardCurveId: "string" => json(i.forward_curve_id),
    /// Read the contract's correlation.
    /// @returns Gaussian-copula correlation between the two CMS rates, a decimal in [-1, 1].
    correlation as correlation: "number" => json(i.correlation),
    /// Read the contract's day count.
    /// @returns Day count convention.
    day_count as dayCount: "DayCount" => day_count(i.day_count),
    /// Read the contract's index id.
    /// @returns Rate-index convention-registry key of the underlying CMS swaps (e.g. EUR-ESTR-OIS). Required for USD CMS; other supported currencies use their registered overnight-index default when omitted. Supplies calendar, settlement lag and default leg conventions. Individual leg fields override the index.
    index_id as indexId: "NonNullable<generated.valuations.CmsSpreadOption['index_id']> | null" => json(i.index_id),
    /// Read the contract's long cms fixing index id.
    /// @returns Explicit CMS benchmark fixing identifier for the long-tenor leg (FIXING:<long_cms_fixing_index_id>), distinct from forward_curve_id, index_id, and short_cms_fixing_index_id.
    long_cms_fixing_index_id as longCmsFixingIndexId: "NonNullable<generated.valuations.CmsSpreadOption['long_cms_fixing_index_id']> | null" => json(i.long_cms_fixing_index_id),
    /// Read the contract's short cms fixing index id.
    /// @returns Explicit CMS benchmark fixing identifier for the short-tenor leg (FIXING:<short_cms_fixing_index_id>), distinct from forward_curve_id, index_id, and long_cms_fixing_index_id.
    short_cms_fixing_index_id as shortCmsFixingIndexId: "NonNullable<generated.valuations.CmsSpreadOption['short_cms_fixing_index_id']> | null" => json(i.short_cms_fixing_index_id),
    /// Read the contract's swap fixed frequency.
    /// @returns Fixed leg frequency of the underlying CMS swaps (overrides convention).
    swap_fixed_frequency as swapFixedFrequency: "NonNullable<generated.valuations.CmsSpreadOption['swap_fixed_frequency']> | null" => json(i.swap_fixed_frequency),
    /// Read the contract's swap float frequency.
    /// @returns Floating leg frequency of the underlying CMS swaps (overrides convention).
    swap_float_frequency as swapFloatFrequency: "NonNullable<generated.valuations.CmsSpreadOption['swap_float_frequency']> | null" => json(i.swap_float_frequency),
    /// Read the contract's swap fixed day count.
    /// @returns Fixed leg day count of the underlying CMS swaps (overrides convention).
    swap_fixed_day_count as swapFixedDayCount: "NonNullable<generated.valuations.CmsSpreadOption['swap_fixed_day_count']> | null" => json(i.swap_fixed_day_count),
    /// Read the contract's swap float day count.
    /// @returns Floating leg day count of the underlying CMS swaps (overrides convention).
    swap_float_day_count as swapFloatDayCount: "NonNullable<generated.valuations.CmsSpreadOption['swap_float_day_count']> | null" => json(i.swap_float_day_count),
});
common_getters!(JsCmsSpreadOption, "CmsSpreadOption");

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.CmsSpreadOption.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsCmsSpreadOptionBuilder, "CmsSpreadOptionBuilder", CmsSpreadOptionBuilder, JsCmsSpreadOption, |b| b.build()
);

#[wasm_bindgen(js_class = CmsSpreadOption)]
impl JsCmsSpreadOption {
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    #[wasm_bindgen(js_name = toBuilder)]
    pub fn to_builder(&self) -> JsCmsSpreadOptionBuilder {
        JsCmsSpreadOptionBuilder {
            inner: super::Staged::new(self.inner.to_builder()),
        }
    }
}

setters!(JsCmsSpreadOptionBuilder, "CmsSpreadOptionBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's long cms tenor.
    /// @param value - Long CMS tenor (e.g., 10Y).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    long_cms_tenor as longCmsTenor: "generated.valuations.CmsSpreadOption['long_cms_tenor']" => js_wire,
    /// Set the contract's short cms tenor.
    /// @param value - Short CMS tenor (e.g., 2Y).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    short_cms_tenor as shortCmsTenor: "generated.valuations.CmsSpreadOption['short_cms_tenor']" => js_wire,
    /// Set the contract's strike.
    /// @param value - Strike on the CMS spread as a decimal rate (0.005 = 50bp).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    strike as strike: "string" => exact_decimal,
    /// Set the contract's option type.
    /// @param value - Call or put on the spread CMS_long - CMS_short: a call pays max(spread - strike, 0), a put pays max(strike - spread, 0).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    option_type as optionType: "generated.valuations.CmsSpreadOption['option_type']" => js_wire,
    /// Set the contract's expiry.
    /// @param value - Option expiry date.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    expiry as expiry: "string" => js_date,
    /// Set the contract's payment date.
    /// @param value - Payment date (may differ from expiry).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    payment_date as paymentDate: "string" => js_date,
    /// Set the contract's long vol surface id.
    /// @param value - Black swaption volatility surface for the long tenor. The selected expiry slice must be flat across strikes; tenor-axis ATM surfaces are valid.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    long_vol_surface_id as longVolSurfaceId: "string" => id,
    /// Set the contract's short vol surface id.
    /// @param value - Black swaption volatility surface for the short tenor, subject to the same flat-strike restriction as the long-tenor surface.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    short_vol_surface_id as shortVolSurfaceId: "string" => id,
    /// Set the contract's discount curve id.
    /// @param value - Discount curve ID.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    discount_curve_id as discountCurveId: "string" => id,
    /// Set the contract's forward curve id.
    /// @param value - Forward curve ID (for swap rate projection).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    forward_curve_id as forwardCurveId: "string" => id,
    /// Set the contract's correlation.
    /// @param value - Gaussian-copula correlation between the two CMS rates, a decimal in [-1, 1].
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    correlation as correlation: "number" => js_f64,
    /// Set the contract's index id.
    /// @param value - Rate-index convention-registry key of the underlying CMS swaps (e.g. EUR-ESTR-OIS). Required for USD CMS; other supported currencies use their registered overnight-index default when omitted. Supplies calendar, settlement lag and default leg conventions. Individual leg fields override the index.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    index_id as indexId: "NonNullable<generated.valuations.CmsSpreadOption['index_id']> | null" => optional_id via index_id_opt,
    /// Set the contract's long cms fixing index id.
    /// @param value - Explicit CMS benchmark fixing identifier for the long-tenor leg (FIXING:<long_cms_fixing_index_id>), distinct from forward_curve_id, index_id, and short_cms_fixing_index_id.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    long_cms_fixing_index_id as longCmsFixingIndexId: "NonNullable<generated.valuations.CmsSpreadOption['long_cms_fixing_index_id']> | null" => optional_id via long_cms_fixing_index_id_opt,
    /// Set the contract's short cms fixing index id.
    /// @param value - Explicit CMS benchmark fixing identifier for the short-tenor leg (FIXING:<short_cms_fixing_index_id>), distinct from forward_curve_id, index_id, and long_cms_fixing_index_id.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    short_cms_fixing_index_id as shortCmsFixingIndexId: "NonNullable<generated.valuations.CmsSpreadOption['short_cms_fixing_index_id']> | null" => optional_id via short_cms_fixing_index_id_opt,
    /// Set the contract's swap fixed frequency.
    /// @param value - Fixed leg frequency of the underlying CMS swaps (overrides convention).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    swap_fixed_frequency as swapFixedFrequency: "NonNullable<generated.valuations.CmsSpreadOption['swap_fixed_frequency']> | null" => opt_wire via swap_fixed_frequency_opt,
    /// Set the contract's swap float frequency.
    /// @param value - Floating leg frequency of the underlying CMS swaps (overrides convention).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    swap_float_frequency as swapFloatFrequency: "NonNullable<generated.valuations.CmsSpreadOption['swap_float_frequency']> | null" => opt_wire via swap_float_frequency_opt,
    /// Set the contract's swap fixed day count.
    /// @param value - Fixed leg day count of the underlying CMS swaps (overrides convention).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    swap_fixed_day_count as swapFixedDayCount: "NonNullable<generated.valuations.CmsSpreadOption['swap_fixed_day_count']> | null" => opt_wire via swap_fixed_day_count_opt,
    /// Set the contract's swap float day count.
    /// @param value - Floating leg day count of the underlying CMS swaps (overrides convention).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    swap_float_day_count as swapFloatDayCount: "NonNullable<generated.valuations.CmsSpreadOption['swap_float_day_count']> | null" => opt_wire via swap_float_day_count_opt,
});
common_setters!(JsCmsSpreadOptionBuilder, "CmsSpreadOptionBuilder");

handle_setters!(JsCmsSpreadOptionBuilder, "CmsSpreadOptionBuilder", {
    /// Set the contract's notional.
    /// @param value - Notional amount.
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    notional as notional => JsMoney,
    /// Set the contract's day count.
    /// @param value - Day count convention.
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    day_count as dayCount => JsDayCount,
});
