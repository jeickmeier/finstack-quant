//! Typed contracts and fluent builders for inflation-linked bonds.

use crate::api::core::dates::JsTenor;
use crate::api::core::money::JsMoney;
use crate::utils::{input::js_string, to_js_err};
use finstack_quant_core::market_data::scalars::InflationLag;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::fixed_income::inflation_linked_bond::{
    InflationLinkedBond, InflationLinkedBondBuilder,
};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// Bond whose nominal cashflows follow the contractual CPI indexation rules.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const bond = valuations.instruments.InflationLinkedBond.example();
    /// console.log(bond.realCoupon);
    /// bond.free();
    /// ```
    JsInflationLinkedBond, "InflationLinkedBond", InflationLinkedBond
);
instrument_surface!(
    JsInflationLinkedBond,
    "InflationLinkedBond",
    JsInflationLinkedBondBuilder,
    "generated.valuations.InflationLinkedBond"
);
factories!(JsInflationLinkedBond, "InflationLinkedBond", InflationLinkedBond, {
    /// Construct the canonical inflation-linked bond example.
    /// @returns A validated bond with a contractual real coupon and CPI base.
    /// @throws FinstackError if the example's terms are invalid.
    example as example,
});
getters!(JsInflationLinkedBond, "InflationLinkedBond", |i| {
    /// Effective maturity after contractual calendar adjustment.
    /// @returns The ISO maturity date, or null when date adjustment fails.
    expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
    /// Native default pricing model.
    /// @returns The registered model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the bond's notional.
    /// @returns Positive face amount in real terms, with its nominal payment currency.
    notional as notional: "Money" => money(i.notional),
    /// Read the bond's issue date.
    /// @returns Contractual issue date before payment-date adjustments.
    issue_date as issueDate: "string" => date(i.issue_date),
    /// Read the bond's maturity.
    /// @returns Contractual maturity before payment-date adjustments.
    maturity as maturity: "string" => date(i.maturity),
    /// Read the bond's real coupon.
    /// @returns Exact annual real coupon as a decimal rate, such as 0.025 for 2.5 percent.
    real_coupon as realCoupon: "string" => text(i.real_coupon),
    /// Read the bond's frequency.
    /// @returns Positive coupon interval, such as 6M for semiannual payments.
    frequency as frequency: "Tenor" => tenor(i.frequency),
    /// Read the bond's day count.
    /// @returns Accrual year-fraction convention, such as 30_360.
    day_count as dayCount: "DayCount" => day_count(i.day_count),
    /// Read the bond's base date.
    /// @returns Reference CPI date from the issue terms, which may differ from the issue date.
    base_date as baseDate: "string" => date(i.base_date),
    /// Read the bond's base cpi.
    /// @returns Positive contractual base index level in CPI points.
    base_cpi as baseCpi: "number" => json(i.base_cpi),
    /// Read the bond's indexation method.
    /// @returns Indexation convention: tips, canadian, uk, french or japanese.
    indexation_method as indexationMethod: "NonNullable<generated.valuations.InflationLinkedBond['indexation_method']>" => json(i.indexation_method),
    /// Read the bond's lag.
    /// @returns Contractual observation shift, using 3M for three calendar months, 90D for 90 days or none.
    lag as lag: "string" => text(i.lag),
    /// Read the bond's deflation protection.
    /// @returns Protected payments: none, maturity_only or all_payments.
    deflation_protection as deflationProtection: "NonNullable<generated.valuations.InflationLinkedBond['deflation_protection']>" => json(i.deflation_protection),
    /// Read the bond's floor vol surface id.
    /// @returns Black strike surface ID for unpublished protected CPI ratios.
    floor_vol_surface_id as floorVolSurfaceId: "string | null" => opt_text(i.floor_vol_surface_id),
    /// Read the bond's business day convention.
    /// @returns Payment-date adjustment. Omit the setter to use modified_following.
    business_day_convention as businessDayConvention: "NonNullable<generated.valuations.InflationLinkedBond['business_day_convention']>" => json(i.business_day_convention),
    /// Read the bond's stub.
    /// @returns Coupon stub convention. Omit the setter to use short_front.
    stub as stub: "NonNullable<generated.valuations.InflationLinkedBond['stub']>" => json(i.stub),
    /// Read the bond's calendar id.
    /// @returns Holiday calendar driving contractual payment adjustment.
    calendar_id as calendarId: "string | null" => opt_text(i.calendar_id),
    /// Read the bond's discount curve id.
    /// @returns Nominal discount curve ID used for projected nominal cash.
    discount_curve_id as discountCurveId: "string" => text(i.discount_curve_id),
    /// Read the bond's inflation index id.
    /// @returns ID shared by the realized CPI series and projected CPI curve.
    inflation_index_id as inflationIndexId: "string" => text(i.inflation_index_id),
});
common_getters!(JsInflationLinkedBond, "InflationLinkedBond");

#[wasm_bindgen(js_class = InflationLinkedBond)]
impl JsInflationLinkedBond {
    /// Copy the native terms into an editable builder.
    /// @returns A builder retaining the exact coupon, terms, and overrides.
    #[wasm_bindgen(js_name = toBuilder)]
    pub fn to_builder(&self) -> JsInflationLinkedBondBuilder {
        JsInflationLinkedBondBuilder {
            inner: super::Staged::new(self.inner.to_builder()),
        }
    }
}

builder_class!(
    /// Fluent authoring of inflation-linked bond terms.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const builder = valuations.instruments.InflationLinkedBond.builder().id("TIPS");
    /// builder.free();
    /// ```
    JsInflationLinkedBondBuilder, "InflationLinkedBondBuilder", InflationLinkedBondBuilder,
    JsInflationLinkedBond, |b| b.build()
);
setters!(JsInflationLinkedBondBuilder, "InflationLinkedBondBuilder", {
    /// Set the bond's id.
    /// @param value - Stable contract identifier used for pricing and metric keys.
    /// @returns The same builder for chaining.
    /// @throws TypeError for a mistyped value or FinstackError for invalid input or a consumed builder.
    id as id: "string" => id,
    /// Set the bond's issue date.
    /// @param value - Contractual issue date before payment-date adjustments.
    /// @returns The same builder for chaining.
    /// @throws TypeError for a mistyped value or FinstackError for invalid input or a consumed builder.
    issue_date as issueDate: "string" => js_date,
    /// Set the bond's maturity.
    /// @param value - Contractual maturity before payment-date adjustments.
    /// @returns The same builder for chaining.
    /// @throws TypeError for a mistyped value or FinstackError for invalid input or a consumed builder.
    maturity as maturity: "string" => js_date,
    /// Set the bond's day count.
    /// @param value - Accrual year-fraction convention, such as 30_360.
    /// @returns The same builder for chaining.
    /// @throws TypeError for a mistyped value or FinstackError for invalid input or a consumed builder.
    day_count as dayCount: "generated.valuations.InflationLinkedBond['day_count']" => js_parse,
    /// Set the bond's base date.
    /// @param value - Reference CPI date from the issue terms, which may differ from the issue date.
    /// @returns The same builder for chaining.
    /// @throws TypeError for a mistyped value or FinstackError for invalid input or a consumed builder.
    base_date as baseDate: "string" => js_date,
    /// Set the bond's base cpi.
    /// @param value - Positive contractual base index level in CPI points.
    /// @returns The same builder for chaining.
    /// @throws TypeError for a mistyped value or FinstackError for invalid input or a consumed builder.
    base_cpi as baseCpi: "number" => js_f64,
    /// Set the bond's indexation method.
    /// @param value - Indexation convention: tips, canadian, uk, french or japanese.
    /// @returns The same builder for chaining.
    /// @throws TypeError for a mistyped value or FinstackError for invalid input or a consumed builder.
    indexation_method as indexationMethod: "NonNullable<generated.valuations.InflationLinkedBond['indexation_method']>" => js_wire,
    /// Set the bond's deflation protection.
    /// @param value - Protected payments: none, maturity_only or all_payments.
    /// @returns The same builder for chaining.
    /// @throws TypeError for a mistyped value or FinstackError for invalid input or a consumed builder.
    deflation_protection as deflationProtection: "NonNullable<generated.valuations.InflationLinkedBond['deflation_protection']>" => js_wire,
    /// Set the bond's floor vol surface id.
    /// @param value - Black strike surface ID for unpublished protected CPI ratios.
    /// @returns The same builder for chaining.
    /// @throws TypeError for a mistyped value or FinstackError for invalid input or a consumed builder.
    floor_vol_surface_id as floorVolSurfaceId: "string" => id,
    /// Set the bond's business day convention.
    /// @param value - Payment-date adjustment. Omit the setter to use modified_following.
    /// @returns The same builder for chaining.
    /// @throws TypeError for a mistyped value or FinstackError for invalid input or a consumed builder.
    business_day_convention as businessDayConvention: "NonNullable<generated.valuations.InflationLinkedBond['business_day_convention']>" => js_wire,
    /// Set the bond's stub.
    /// @param value - Coupon stub convention. Omit the setter to use short_front.
    /// @returns The same builder for chaining.
    /// @throws TypeError for a mistyped value or FinstackError for invalid input or a consumed builder.
    stub as stub: "NonNullable<generated.valuations.InflationLinkedBond['stub']>" => js_wire,
    /// Set the bond's calendar id.
    /// @param value - Holiday calendar driving contractual payment adjustment.
    /// @returns The same builder for chaining.
    /// @throws TypeError for a mistyped value or FinstackError for invalid input or a consumed builder.
    calendar_id as calendarId: "string" => id,
    /// Set the bond's discount curve id.
    /// @param value - Nominal discount curve ID used for projected nominal cash.
    /// @returns The same builder for chaining.
    /// @throws TypeError for a mistyped value or FinstackError for invalid input or a consumed builder.
    discount_curve_id as discountCurveId: "string" => id,
    /// Set the bond's inflation index id.
    /// @param value - ID shared by the realized CPI series and projected CPI curve.
    /// @returns The same builder for chaining.
    /// @throws TypeError for a mistyped value or FinstackError for invalid input or a consumed builder.
    inflation_index_id as inflationIndexId: "string" => id,
});
common_setters!(JsInflationLinkedBondBuilder, "InflationLinkedBondBuilder");
handle_setters!(JsInflationLinkedBondBuilder, "InflationLinkedBondBuilder", {
    /// Set the contractual face amount.
    /// @param value - Positive face amount in real terms, with its nominal payment currency.
    /// @returns The builder for chaining.
    /// @throws FinstackError for a consumed builder.
    notional as notional => JsMoney,
    /// Set the coupon interval.
    /// @param value - Positive tenor, such as six months for semiannual coupon payments.
    /// @returns The builder for chaining.
    /// @throws FinstackError for a consumed builder.
    frequency as frequency => JsTenor,
});

#[wasm_bindgen(js_class = InflationLinkedBondBuilder)]
impl JsInflationLinkedBondBuilder {
    /// Set the exact annual real coupon.
    /// @param value - Decimal string in rate units. Use "0.025" for 2.5 percent.
    /// @returns The builder for chaining.
    /// @throws TypeError for a mistyped value or FinstackError for invalid decimal text or a consumed builder.
    #[wasm_bindgen(js_name = realCoupon)]
    pub fn real_coupon(
        &self,
        #[wasm_bindgen(unchecked_param_type = "string")] value: JsValue,
    ) -> Result<JsInflationLinkedBondBuilder, JsValue> {
        let rate = crate::utils::wire::js_decimal(&value, "value")?.0;
        let inner = self.inner.apply(|builder| builder.real_coupon(rate))?;
        Ok(Self { inner })
    }

    /// Set the contractual reference CPI observation shift.
    /// @param value - Observation lag label, such as "3M", "90D" or "none".
    /// @returns The builder for chaining.
    /// @throws TypeError for a mistyped value or FinstackError for invalid lag text or a consumed builder.
    #[wasm_bindgen(js_name = lag)]
    pub fn lag(
        &self,
        #[wasm_bindgen(unchecked_param_type = "string")] value: JsValue,
    ) -> Result<JsInflationLinkedBondBuilder, JsValue> {
        let lag = js_string(&value, "value")?
            .parse::<InflationLag>()
            .map_err(to_js_err)?;
        let inner = self.inner.apply(|builder| builder.lag(lag))?;
        Ok(Self { inner })
    }
}
