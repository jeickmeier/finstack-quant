//! A constant-maturity swap on a specified swap-rate index.

use crate::api::core::dates::JsDayCount;
use crate::api::core::money::JsMoney;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::rates::cms_swap::{CmsSwap, CmsSwapBuilder};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// A constant-maturity swap on a specified swap-rate index.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.CmsSwap.example();
    /// console.log(future.id);
    /// future.free();
    /// ```
    JsCmsSwap, "CmsSwap", CmsSwap
);
instrument_surface!(
    JsCmsSwap,
    "CmsSwap",
    JsCmsSwapBuilder,
    "generated.valuations.CmsSwap"
);
factories!(JsCmsSwap, "CmsSwap", CmsSwap, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsCmsSwap, "CmsSwap", |i| {
    /// Effective expiry of the contract.
    /// @returns The ISO expiry, or null if the instrument defines none.
    expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's notional.
    /// @returns Notional amount.
    notional as notional: "Money" => money(i.notional),
    /// Read the contract's side.
    /// @returns Pay direction: Pay means pay CMS leg, receive funding leg.
    side as side: "generated.valuations.CmsSwap['side']" => json(i.side),
    /// Read the contract's cms tenor.
    /// @returns Tenor of the CMS reference swap (e.g. 10Y); must be month- or year-based.
    cms_tenor as cmsTenor: "generated.valuations.CmsSwap['cms_tenor']" => json(i.cms_tenor),
    /// Read the contract's fixing dates.
    /// @returns Fixing dates for CMS rate observations.
    fixing_dates as fixingDates: "generated.valuations.CmsSwap['fixing_dates']" => dates(i.fixing_dates),
    /// Read the contract's payment dates.
    /// @returns Payment dates for the CMS leg.
    payment_dates as paymentDates: "generated.valuations.CmsSwap['payment_dates']" => dates(i.payment_dates),
    /// Read the contract's accrual fractions.
    /// @returns Accrual fractions for each CMS period.
    accrual_fractions as accrualFractions: "generated.valuations.CmsSwap['accrual_fractions']" => json(i.accrual_fractions),
    /// Read the contract's day count.
    /// @returns Day count convention for CMS leg accrual.
    day_count as dayCount: "DayCount" => day_count(i.day_count),
    /// Read the contract's cms spread bp.
    /// @returns Additive spread over the CMS rate in basis points (10 = 10bp).
    cms_spread_bp as cmsSpreadBp: "string" => text(i.cms_spread_bp),
    /// Read the contract's cms cap.
    /// @returns Optional cap on the CMS rate (decimal).
    cms_cap as cmsCap: "number | null" => json(i.cms_cap),
    /// Read the contract's cms floor.
    /// @returns Optional floor on the CMS rate (decimal).
    cms_floor as cmsFloor: "number | null" => json(i.cms_floor),
    /// Read the contract's index id.
    /// @returns Rate-index convention-registry key of the underlying swap (e.g. USD-SOFR-OIS). Required for USD CMS; legacy contracts must name their legacy index explicitly.
    index_id as indexId: "NonNullable<generated.valuations.CmsSwap['index_id']> | null" => json(i.index_id),
    /// Read the contract's cms fixing index id.
    /// @returns Explicit CMS benchmark fixing identifier used for historical fixings (FIXING:<cms_fixing_index_id>), distinct from forward_curve_id and index_id.
    cms_fixing_index_id as cmsFixingIndexId: "NonNullable<generated.valuations.CmsSwap['cms_fixing_index_id']> | null" => json(i.cms_fixing_index_id),
    /// Read the contract's swap fixed frequency.
    /// @returns Fixed leg frequency of the underlying swap (overrides convention).
    swap_fixed_frequency as swapFixedFrequency: "NonNullable<generated.valuations.CmsSwap['swap_fixed_frequency']> | null" => json(i.swap_fixed_frequency),
    /// Read the contract's swap float frequency.
    /// @returns Floating leg frequency of the underlying swap (overrides convention).
    swap_float_frequency as swapFloatFrequency: "NonNullable<generated.valuations.CmsSwap['swap_float_frequency']> | null" => json(i.swap_float_frequency),
    /// Read the contract's swap fixed day count.
    /// @returns Day count of the underlying swap fixed leg (overrides convention).
    swap_fixed_day_count as swapFixedDayCount: "NonNullable<generated.valuations.CmsSwap['swap_fixed_day_count']> | null" => json(i.swap_fixed_day_count),
    /// Read the contract's swap float day count.
    /// @returns Day count of the underlying swap floating leg (overrides convention).
    swap_float_day_count as swapFloatDayCount: "NonNullable<generated.valuations.CmsSwap['swap_float_day_count']> | null" => json(i.swap_float_day_count),
    /// Read the contract's funding leg.
    /// @returns Funding leg definition (fixed or floating).
    funding_leg as fundingLeg: "generated.valuations.CmsSwap['funding_leg']" => json(i.funding_leg),
    /// Read the contract's discount curve id.
    /// @returns Discount curve ID for present value calculations.
    discount_curve_id as discountCurveId: "string" => json(i.discount_curve_id),
    /// Read the contract's forward curve id.
    /// @returns Forward/projection curve ID for CMS rate projection.
    forward_curve_id as forwardCurveId: "string" => json(i.forward_curve_id),
    /// Read the contract's vol surface id.
    /// @returns Volatility surface ID for CMS convexity adjustment.
    vol_surface_id as volSurfaceId: "string" => json(i.vol_surface_id),
});
common_getters!(JsCmsSwap, "CmsSwap");

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.CmsSwap.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsCmsSwapBuilder, "CmsSwapBuilder", CmsSwapBuilder, JsCmsSwap, |b| b.build()
);

#[wasm_bindgen(js_class = CmsSwap)]
impl JsCmsSwap {
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    #[wasm_bindgen(js_name = toBuilder)]
    pub fn to_builder(&self) -> JsCmsSwapBuilder {
        JsCmsSwapBuilder {
            inner: super::Staged::new(self.inner.to_builder()),
        }
    }
}

setters!(JsCmsSwapBuilder, "CmsSwapBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's side.
    /// @param value - Pay direction: Pay means pay CMS leg, receive funding leg.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    side as side: "generated.valuations.CmsSwap['side']" => js_wire,
    /// Set the contract's cms tenor.
    /// @param value - Tenor of the CMS reference swap (e.g. 10Y); must be month- or year-based.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    cms_tenor as cmsTenor: "generated.valuations.CmsSwap['cms_tenor']" => js_wire,
    /// Set the contract's fixing dates.
    /// @param value - Fixing dates for CMS rate observations.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    fixing_dates as fixingDates: "generated.valuations.CmsSwap['fixing_dates']" => js_wire,
    /// Set the contract's payment dates.
    /// @param value - Payment dates for the CMS leg.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    payment_dates as paymentDates: "generated.valuations.CmsSwap['payment_dates']" => js_wire,
    /// Set the contract's accrual fractions.
    /// @param value - Accrual fractions for each CMS period.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    accrual_fractions as accrualFractions: "generated.valuations.CmsSwap['accrual_fractions']" => js_wire,
    /// Set the contract's cms spread bp.
    /// @param value - Additive spread over the CMS rate in basis points (10 = 10bp).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    cms_spread_bp as cmsSpreadBp: "string" => exact_decimal,
    /// Set the contract's cms cap.
    /// @param value - Optional cap on the CMS rate (decimal).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    cms_cap as cmsCap: "number | null" => opt_f64 via cms_cap_opt,
    /// Set the contract's cms floor.
    /// @param value - Optional floor on the CMS rate (decimal).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    cms_floor as cmsFloor: "number | null" => opt_f64 via cms_floor_opt,
    /// Set the contract's index id.
    /// @param value - Rate-index convention-registry key of the underlying swap (e.g. USD-SOFR-OIS). Required for USD CMS; legacy contracts must name their legacy index explicitly.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    index_id as indexId: "NonNullable<generated.valuations.CmsSwap['index_id']> | null" => optional_id via index_id_opt,
    /// Set the contract's cms fixing index id.
    /// @param value - Explicit CMS benchmark fixing identifier used for historical fixings (FIXING:<cms_fixing_index_id>), distinct from forward_curve_id and index_id.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    cms_fixing_index_id as cmsFixingIndexId: "NonNullable<generated.valuations.CmsSwap['cms_fixing_index_id']> | null" => optional_id via cms_fixing_index_id_opt,
    /// Set the contract's swap fixed frequency.
    /// @param value - Fixed leg frequency of the underlying swap (overrides convention).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    swap_fixed_frequency as swapFixedFrequency: "NonNullable<generated.valuations.CmsSwap['swap_fixed_frequency']> | null" => opt_wire via swap_fixed_frequency_opt,
    /// Set the contract's swap float frequency.
    /// @param value - Floating leg frequency of the underlying swap (overrides convention).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    swap_float_frequency as swapFloatFrequency: "NonNullable<generated.valuations.CmsSwap['swap_float_frequency']> | null" => opt_wire via swap_float_frequency_opt,
    /// Set the contract's swap fixed day count.
    /// @param value - Day count of the underlying swap fixed leg (overrides convention).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    swap_fixed_day_count as swapFixedDayCount: "NonNullable<generated.valuations.CmsSwap['swap_fixed_day_count']> | null" => opt_wire via swap_fixed_day_count_opt,
    /// Set the contract's swap float day count.
    /// @param value - Day count of the underlying swap floating leg (overrides convention).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    swap_float_day_count as swapFloatDayCount: "NonNullable<generated.valuations.CmsSwap['swap_float_day_count']> | null" => opt_wire via swap_float_day_count_opt,
    /// Set the contract's funding leg.
    /// @param value - Funding leg definition (fixed or floating).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    funding_leg as fundingLeg: "generated.valuations.CmsSwap['funding_leg']" => js_wire,
    /// Set the contract's discount curve id.
    /// @param value - Discount curve ID for present value calculations.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    discount_curve_id as discountCurveId: "string" => id,
    /// Set the contract's forward curve id.
    /// @param value - Forward/projection curve ID for CMS rate projection.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    forward_curve_id as forwardCurveId: "string" => id,
    /// Set the contract's vol surface id.
    /// @param value - Volatility surface ID for CMS convexity adjustment.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    vol_surface_id as volSurfaceId: "string" => id,
});
common_setters!(JsCmsSwapBuilder, "CmsSwapBuilder");

handle_setters!(JsCmsSwapBuilder, "CmsSwapBuilder", {
    /// Set the contract's notional.
    /// @param value - Notional amount.
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    notional as notional => JsMoney,
    /// Set the contract's day count.
    /// @param value - Day count convention for CMS leg accrual.
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    day_count as dayCount => JsDayCount,
});
