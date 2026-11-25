//! An option on the elected constant-maturity swap rate.

use crate::api::core::dates::JsDayCount;
use crate::api::core::money::JsMoney;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::rates::cms_option::{CmsOption, CmsOptionBuilder};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// An option on the elected constant-maturity swap rate.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.CmsOption.example();
    /// console.log(future.id);
    /// future.free();
    /// ```
    JsCmsOption, "CmsOption", CmsOption
);
instrument_surface!(
    JsCmsOption,
    "CmsOption",
    JsCmsOptionBuilder,
    "generated.valuations.CmsOption"
);
factories!(JsCmsOption, "CmsOption", CmsOption, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsCmsOption, "CmsOption", |i| {
    /// Effective expiry of the contract.
    /// @returns The ISO expiry, or null if the instrument defines none.
    expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's strike.
    /// @returns Strike (fixed rate for CMS option)
    strike as strike: "string" => text(i.strike),
    /// Read the contract's cms tenor.
    /// @returns Tenor of the CMS reference swap (e.g. 10Y); must be month- or year-based.
    cms_tenor as cmsTenor: "generated.valuations.CmsOption['cms_tenor']" => json(i.cms_tenor),
    /// Read the contract's fixing dates.
    /// @returns Observation/fixing dates for CMS rate
    fixing_dates as fixingDates: "generated.valuations.CmsOption['fixing_dates']" => dates(i.fixing_dates),
    /// Read the contract's payment dates.
    /// @returns Payment dates for each period (usually fixing date + lag or period end)
    payment_dates as paymentDates: "generated.valuations.CmsOption['payment_dates']" => dates(i.payment_dates),
    /// Read the contract's accrual fractions.
    /// @returns Finite nonnegative accrual fractions for each period; zero gives a zero coupon.
    accrual_fractions as accrualFractions: "generated.valuations.CmsOption['accrual_fractions']" => json(i.accrual_fractions),
    /// Read the contract's option type.
    /// @returns Option type (call or put on CMS rate)
    option_type as optionType: "generated.valuations.CmsOption['option_type']" => json(i.option_type),
    /// Read the contract's notional.
    /// @returns Notional amount
    notional as notional: "Money" => money(i.notional),
    /// Read the contract's day count.
    /// @returns Day count convention for the option accrual
    day_count as dayCount: "DayCount" => day_count(i.day_count),
    /// Read the contract's index id.
    /// @returns Rate-index convention-registry key of the underlying swap (e.g. USD-SOFR-OIS). When set, provides default values for swap_fixed_frequency, swap_float_frequency, swap_fixed_day_count, and swap_float_day_count. Individual fields still override the convention when explicitly set. Required for USD CMS.
    index_id as indexId: "NonNullable<generated.valuations.CmsOption['index_id']> | null" => json(i.index_id),
    /// Read the contract's cms fixing index id.
    /// @returns Explicit CMS benchmark fixing identifier used for historical fixings (FIXING:<cms_fixing_index_id>), distinct from forward_curve_id and index_id.
    cms_fixing_index_id as cmsFixingIndexId: "NonNullable<generated.valuations.CmsOption['cms_fixing_index_id']> | null" => json(i.cms_fixing_index_id),
    /// Read the contract's swap fixed frequency.
    /// @returns Fixed leg frequency of the underlying swap (overrides convention if set)
    swap_fixed_frequency as swapFixedFrequency: "NonNullable<generated.valuations.CmsOption['swap_fixed_frequency']> | null" => json(i.swap_fixed_frequency),
    /// Read the contract's swap float frequency.
    /// @returns Floating leg frequency of the underlying swap (overrides convention if set)
    swap_float_frequency as swapFloatFrequency: "NonNullable<generated.valuations.CmsOption['swap_float_frequency']> | null" => json(i.swap_float_frequency),
    /// Read the contract's swap fixed day count.
    /// @returns Day count convention of the underlying swap fixed leg (overrides convention if set)
    swap_fixed_day_count as swapFixedDayCount: "NonNullable<generated.valuations.CmsOption['swap_fixed_day_count']> | null" => json(i.swap_fixed_day_count),
    /// Read the contract's swap float day count.
    /// @returns Optional day count convention of the underlying swap floating leg
    swap_float_day_count as swapFloatDayCount: "NonNullable<generated.valuations.CmsOption['swap_float_day_count']> | null" => json(i.swap_float_day_count),
    /// Read the contract's discount curve id.
    /// @returns Discount curve ID for present value calculations
    discount_curve_id as discountCurveId: "string" => json(i.discount_curve_id),
    /// Read the contract's forward curve id.
    /// @returns Forward/projection curve ID for CMS rate projection
    forward_curve_id as forwardCurveId: "string" => json(i.forward_curve_id),
    /// Read the contract's vol surface id.
    /// @returns Volatility surface ID for CMS rates
    vol_surface_id as volSurfaceId: "string" => json(i.vol_surface_id),
});
common_getters!(JsCmsOption, "CmsOption");

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.CmsOption.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsCmsOptionBuilder, "CmsOptionBuilder", CmsOptionBuilder, JsCmsOption, |b| b.build()
);

#[wasm_bindgen(js_class = CmsOption)]
impl JsCmsOption {
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    #[wasm_bindgen(js_name = toBuilder)]
    pub fn to_builder(&self) -> JsCmsOptionBuilder {
        JsCmsOptionBuilder {
            inner: super::Staged::new(self.inner.to_builder()),
        }
    }
}

setters!(JsCmsOptionBuilder, "CmsOptionBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's strike.
    /// @param value - Strike (fixed rate for CMS option)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    strike as strike: "string" => exact_decimal,
    /// Set the contract's cms tenor.
    /// @param value - Tenor of the CMS reference swap (e.g. 10Y); must be month- or year-based.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    cms_tenor as cmsTenor: "generated.valuations.CmsOption['cms_tenor']" => js_wire,
    /// Set the contract's fixing dates.
    /// @param value - Observation/fixing dates for CMS rate
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    fixing_dates as fixingDates: "generated.valuations.CmsOption['fixing_dates']" => js_wire,
    /// Set the contract's payment dates.
    /// @param value - Payment dates for each period (usually fixing date + lag or period end)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    payment_dates as paymentDates: "generated.valuations.CmsOption['payment_dates']" => js_wire,
    /// Set the contract's accrual fractions.
    /// @param value - Finite nonnegative accrual fractions for each period; zero gives a zero coupon.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    accrual_fractions as accrualFractions: "generated.valuations.CmsOption['accrual_fractions']" => js_wire,
    /// Set the contract's option type.
    /// @param value - Option type (call or put on CMS rate)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    option_type as optionType: "generated.valuations.CmsOption['option_type']" => js_wire,
    /// Set the contract's index id.
    /// @param value - Rate-index convention-registry key of the underlying swap (e.g. USD-SOFR-OIS). When set, provides default values for swap_fixed_frequency, swap_float_frequency, swap_fixed_day_count, and swap_float_day_count. Individual fields still override the convention when explicitly set. Required for USD CMS.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    index_id as indexId: "NonNullable<generated.valuations.CmsOption['index_id']> | null" => optional_id via index_id_opt,
    /// Set the contract's cms fixing index id.
    /// @param value - Explicit CMS benchmark fixing identifier used for historical fixings (FIXING:<cms_fixing_index_id>), distinct from forward_curve_id and index_id.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    cms_fixing_index_id as cmsFixingIndexId: "NonNullable<generated.valuations.CmsOption['cms_fixing_index_id']> | null" => optional_id via cms_fixing_index_id_opt,
    /// Set the contract's swap fixed frequency.
    /// @param value - Fixed leg frequency of the underlying swap (overrides convention if set)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    swap_fixed_frequency as swapFixedFrequency: "NonNullable<generated.valuations.CmsOption['swap_fixed_frequency']> | null" => opt_wire via swap_fixed_frequency_opt,
    /// Set the contract's swap float frequency.
    /// @param value - Floating leg frequency of the underlying swap (overrides convention if set)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    swap_float_frequency as swapFloatFrequency: "NonNullable<generated.valuations.CmsOption['swap_float_frequency']> | null" => opt_wire via swap_float_frequency_opt,
    /// Set the contract's swap fixed day count.
    /// @param value - Day count convention of the underlying swap fixed leg (overrides convention if set)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    swap_fixed_day_count as swapFixedDayCount: "NonNullable<generated.valuations.CmsOption['swap_fixed_day_count']> | null" => opt_wire via swap_fixed_day_count_opt,
    /// Set the contract's swap float day count.
    /// @param value - Optional day count convention of the underlying swap floating leg
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    swap_float_day_count as swapFloatDayCount: "NonNullable<generated.valuations.CmsOption['swap_float_day_count']> | null" => opt_wire via swap_float_day_count_opt,
    /// Set the contract's discount curve id.
    /// @param value - Discount curve ID for present value calculations
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    discount_curve_id as discountCurveId: "string" => id,
    /// Set the contract's forward curve id.
    /// @param value - Forward/projection curve ID for CMS rate projection
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    forward_curve_id as forwardCurveId: "string" => id,
    /// Set the contract's vol surface id.
    /// @param value - Volatility surface ID for CMS rates
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    vol_surface_id as volSurfaceId: "string" => id,
});
common_setters!(JsCmsOptionBuilder, "CmsOptionBuilder");

handle_setters!(JsCmsOptionBuilder, "CmsOptionBuilder", {
    /// Set the contract's notional.
    /// @param value - Notional amount
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    notional as notional => JsMoney,
    /// Set the contract's day count.
    /// @param value - Day count convention for the option accrual
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    day_count as dayCount => JsDayCount,
});
