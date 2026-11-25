//! An agency mortgage pass-through with projected scheduled and prepaid principal.

use crate::api::core::dates::JsDayCount;
use crate::api::core::money::JsMoney;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::fixed_income::mbs_passthrough::{
    AgencyMbsPassthrough, AgencyMbsPassthroughBuilder,
};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// An agency mortgage pass-through with projected scheduled and prepaid principal.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.AgencyMbsPassthrough.example();
    /// console.log(future.id);
    /// future.free();
    /// ```
    JsAgencyMbsPassthrough, "AgencyMbsPassthrough", AgencyMbsPassthrough
);
instrument_surface!(
    JsAgencyMbsPassthrough,
    "AgencyMbsPassthrough",
    JsAgencyMbsPassthroughBuilder,
    "generated.valuations.AgencyMbsPassthrough"
);
factories!(JsAgencyMbsPassthrough, "AgencyMbsPassthrough", AgencyMbsPassthrough, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsAgencyMbsPassthrough, "AgencyMbsPassthrough", |i| {
    /// Effective expiry of the contract.
    /// @returns The ISO expiry, or null if the instrument defines none.
    expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's pool id.
    /// @returns Pool identifier (CUSIP or internal pool ID).
    pool_id as poolId: "string" => text(i.pool_id),
    /// Read the contract's agency.
    /// @returns Agency program (FNMA, FHLMC, GNMA).
    agency as agency: "generated.valuations.AgencyMbsPassthrough['agency']" => json(i.agency),
    /// Read the contract's pool type.
    /// @returns Pool type (generic or specified).
    pool_type as poolType: "generated.valuations.AgencyMbsPassthrough['pool_type']" => json(i.pool_type),
    /// Read the contract's original face.
    /// @returns Original face amount (initial principal balance).
    original_face as originalFace: "Money" => money(i.original_face),
    /// Read the contract's current face.
    /// @returns Current face amount (remaining principal balance).
    current_face as currentFace: "Money" => money(i.current_face),
    /// Read the contract's wac.
    /// @returns Weighted average coupon (gross rate on underlying mortgages).
    wac as wac: "number" => json(i.wac),
    /// Read the contract's coupon.
    /// @returns Net pass-through coupon paid to the investor, as an annual decimal (0.04 = 4%): wac less the servicing and guarantee fees.
    coupon as coupon: "number" => json(i.coupon),
    /// Read the contract's servicing fee bp.
    /// @returns Annual servicing fee in basis points (25.0 = 0.25%). Defaults to 0.0 when omitted.
    servicing_fee_bp as servicingFeeBp: "number" => json(i.servicing_fee_bp),
    /// Read the contract's guarantee fee bp.
    /// @returns Annual agency guarantee fee (g-fee) in basis points (25.0 = 0.25%). Defaults to 0.0 when omitted.
    guarantee_fee_bp as guaranteeFeeBp: "number" => json(i.guarantee_fee_bp),
    /// Read the contract's wam months.
    /// @returns Remaining weighted average maturity in months as of the valuation date (current WAM, not the original term). Pool age (WALA) for seasoning ramps is derived separately from issue_date.
    wam_months as wamMonths: "number" => json(i.wam_months),
    /// Read the contract's issue date.
    /// @returns Issue date of the pool.
    issue_date as issueDate: "string" => date(i.issue_date),
    /// Read the contract's last paid accrual end.
    /// @returns End date of the latest accrual period whose delayed P&I payment has settled and is already reflected in current_face. When omitted, pricing infers the latest paid period from the agency payment-delay rule and as_of.
    last_paid_accrual_end as lastPaidAccrualEnd: "string | null" => opt_date(i.last_paid_accrual_end),
    /// Read the contract's maturity.
    /// @returns Legal maturity date.
    maturity as maturity: "string" => date(i.maturity),
    /// Read the contract's stated delay days.
    /// @returns Optional custom stated payment delay in days (overrides the agency rule). A delay D pays on day D − 30k of the month k = (D − 1)/30 months after the accrual month, rolled Following on the usny calendar (55 → 25th of the next month, 75 → 15th two months later). Must be at least 1.
    stated_delay_days as statedDelayDays: "number | null" => json(i.stated_delay_days),
    /// Read the contract's prepayment spec.
    /// @returns Prepayment model specification.
    prepayment_spec as prepaymentSpec: "generated.valuations.AgencyMbsPassthrough['prepayment_spec']" => json(i.prepayment_spec),
    /// Read the contract's discount curve id.
    /// @returns Discount curve identifier for pricing.
    discount_curve_id as discountCurveId: "string" => text(i.discount_curve_id),
    /// Read the contract's day count.
    /// @returns Day count convention for accrual.
    day_count as dayCount: "DayCount" => day_count(i.day_count),
});
common_getters!(JsAgencyMbsPassthrough, "AgencyMbsPassthrough");
to_builder_method!(
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    JsAgencyMbsPassthrough,
    "AgencyMbsPassthrough",
    JsAgencyMbsPassthroughBuilder
);

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.AgencyMbsPassthrough.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsAgencyMbsPassthroughBuilder, "AgencyMbsPassthroughBuilder", AgencyMbsPassthroughBuilder, JsAgencyMbsPassthrough, |b| b.build()
);

setters!(JsAgencyMbsPassthroughBuilder, "AgencyMbsPassthroughBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's pool id.
    /// @param value - Pool identifier (CUSIP or internal pool ID).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    pool_id as poolId: "string" => id,
    /// Set the contract's agency.
    /// @param value - Agency program (FNMA, FHLMC, GNMA).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    agency as agency: "generated.valuations.AgencyMbsPassthrough['agency']" => js_wire,
    /// Set the contract's pool type.
    /// @param value - Pool type (generic or specified).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    pool_type as poolType: "generated.valuations.AgencyMbsPassthrough['pool_type']" => js_wire,
    /// Set the contract's wac.
    /// @param value - Weighted average coupon (gross rate on underlying mortgages).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    wac as wac: "number" => js_f64,
    /// Set the contract's coupon.
    /// @param value - Net pass-through coupon paid to the investor, as an annual decimal (0.04 = 4%): wac less the servicing and guarantee fees.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    coupon as coupon: "number" => js_f64,
    /// Set the contract's servicing fee bp.
    /// @param value - Annual servicing fee in basis points (25.0 = 0.25%). Defaults to 0.0 when omitted.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    servicing_fee_bp as servicingFeeBp: "number" => js_f64,
    /// Set the contract's guarantee fee bp.
    /// @param value - Annual agency guarantee fee (g-fee) in basis points (25.0 = 0.25%). Defaults to 0.0 when omitted.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    guarantee_fee_bp as guaranteeFeeBp: "number" => js_f64,
    /// Set the contract's wam months.
    /// @param value - Remaining weighted average maturity in months as of the valuation date (current WAM, not the original term). Pool age (WALA) for seasoning ramps is derived separately from issue_date.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    wam_months as wamMonths: "number" => js_uint,
    /// Set the contract's issue date.
    /// @param value - Issue date of the pool.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    issue_date as issueDate: "string" => js_date,
    /// Set the contract's last paid accrual end.
    /// @param value - End date of the latest accrual period whose delayed P&I payment has settled and is already reflected in current_face. When omitted, pricing infers the latest paid period from the agency payment-delay rule and as_of.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    last_paid_accrual_end as lastPaidAccrualEnd: "string | null" => opt_date via last_paid_accrual_end_opt,
    /// Set the contract's maturity.
    /// @param value - Legal maturity date.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    maturity as maturity: "string" => js_date,
    /// Set the contract's stated delay days.
    /// @param value - Optional custom stated payment delay in days (overrides the agency rule). A delay D pays on day D − 30k of the month k = (D − 1)/30 months after the accrual month, rolled Following on the usny calendar (55 → 25th of the next month, 75 → 15th two months later). Must be at least 1.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    stated_delay_days as statedDelayDays: "number | null" => opt_uint via stated_delay_days_opt,
    /// Set the contract's prepayment spec.
    /// @param value - Prepayment model specification.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    prepayment_spec as prepaymentSpec: "generated.valuations.AgencyMbsPassthrough['prepayment_spec']" => js_wire,
    /// Set the contract's discount curve id.
    /// @param value - Discount curve identifier for pricing.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    discount_curve_id as discountCurveId: "string" => id,
});
common_setters!(JsAgencyMbsPassthroughBuilder, "AgencyMbsPassthroughBuilder");

handle_setters!(JsAgencyMbsPassthroughBuilder, "AgencyMbsPassthroughBuilder", {
    /// Set the contract's original face.
    /// @param value - Original face amount (initial principal balance).
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    original_face as originalFace => JsMoney,
    /// Set the contract's current face.
    /// @param value - Current face amount (remaining principal balance).
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    current_face as currentFace => JsMoney,
    /// Set the contract's day count.
    /// @param value - Day count convention for accrual.
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    day_count as dayCount => JsDayCount,
});
