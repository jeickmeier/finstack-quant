//! Typed `CdsTranche` and `CdsTrancheBuilder` WASM bindings.

use crate::api::core::dates::{JsDayCount, JsTenor};
use crate::api::core::money::JsMoney;
use crate::utils::input::{from_js_json, js_string};
use crate::utils::to_js_err;
use crate::utils::wire::js_wire;
use finstack_quant_valuations::instruments::Instrument;
use wasm_bindgen::prelude::*;

instrument_class!(
    /// Typed wrapper for the Rust `CdsTranche` instrument (synthetic index tranche).
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const cdsTranche = valuations.instruments.CdsTranche.example();
    /// console.log(cdsTranche.id, cdsTranche.toDict());
    /// cdsTranche.free();
    /// ```
    JsCdsTranche,
    "CdsTranche",
    finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CdsTranche
);

instrument_surface!(
    JsCdsTranche,
    "CdsTranche",
    JsCdsTrancheBuilder,
    "generated.valuations.CdsTranche"
);

getters!(JsCdsTranche, "CdsTranche", |i| {
        /// Canonical model key used when `model="default"` is passed to `price`.
        /// @returns Registered model key such as `"hazard_rate"` or `"black76"`.
        default_model as defaultModel: "string" => text(Instrument::default_model(i)),
        /// Underlying index name.
        /// @returns The index name.
        index_name as indexName: "generated.valuations.CdsTranche['index_name']" => json(i.index_name),
        /// Index series number.
        /// @returns The series number.
        series as series: "generated.valuations.CdsTranche['series']" => json(i.series),
        /// Attachment point in percent.
        /// @returns Attachment (`3.0` = 3%).
        attach_pct as attachPct: "generated.valuations.CdsTranche['attach_pct']" => json(i.attach_pct),
        /// Detachment point in percent.
        /// @returns Detachment (`7.0` = 7%).
        detach_pct as detachPct: "generated.valuations.CdsTranche['detach_pct']" => json(i.detach_pct),
        /// Tranche notional.
        /// @returns Currency-tagged notional.
        notional as notional: "Money" => money(i.notional),
        /// Scheduled maturity.
        /// @returns The maturity date.
        maturity as maturity: "string" => date(i.maturity),
        /// Contractual running coupon (bp) paid on the tranche premium leg.
        /// @returns Coupon quoted in basis points per annum on the outstanding tranche notional (for example `100.0` for a 100 bp coupon), not as a decimal rate. Accrues on the premium-leg day count.
        coupon_bp as couponBp: "generated.valuations.CdsTranche['coupon_bp']" => json(i.coupon_bp),
        /// Payment frequency.
        /// @returns The coupon tenor (typically quarterly).
        frequency as frequency: "Tenor" => tenor(i.frequency),
        /// Day count convention.
        /// @returns Act/360 for standard tranches.
        day_count as dayCount: "DayCount" => day_count(i.day_count),
        /// Business day convention (serde name).
        /// @returns `"modified_following"` unless set otherwise.
        business_day_convention as businessDayConvention: "NonNullable< generated.valuations.CdsTranche['business_day_convention'] >" => json(i.business_day_convention),
        /// Holiday calendar identifier.
        /// @returns The calendar id, or `null`.
        calendar_id as calendarId: "string | null" => opt_text(i.calendar_id),
        /// Discount curve identifier.
        /// @returns The curve id.
        discount_curve_id as discountCurveId: "string" => text(i.discount_curve_id),
        /// Credit index identifier for the loss distribution.
        /// @returns The credit index id.
        credit_index_id as creditIndexId: "string" => text(i.credit_index_id),
        /// Direction of the tranche position from the holder's perspective.
        /// @returns Serde string, either `"pay"` (buys protection: pays the running coupon and receives tranche loss payments) or `"receive"` (sells protection: receives the coupon and pays losses), matching the CDS and CDS index `side`.
        side as side: "generated.valuations.CdsTranche['side']" => json(i.side),
        /// Explicit contract start (effective) date for schedule anchoring.
        /// @returns The date, or `null`.
        start_date as startDate: "string | null" => opt_date(i.start_date),
        /// Realized portfolio loss so far.
        /// @returns Fraction of the original portfolio notional.
        realized_loss as realizedLoss: "NonNullable<generated.valuations.CdsTranche['realized_loss']>" => json(i.realized_loss),
        /// Contractual date regime of the coupon schedule.
        /// @returns `"standard"` for the CDS roll dates (20th of Mar/Jun/Sep/Dec); `"bespoke"` for a `frequency`/`stub` schedule.
        date_regime as dateRegime: "generated.valuations.CdsTranche['date_regime']" => json(i.date_regime),
        /// Stub convention for a bespoke coupon schedule.
        /// @returns The stub rule (`short_front` by default).
        stub as stub: "NonNullable<generated.valuations.CdsTranche['stub']>" => json(i.stub),
        /// Upfront payment as `(payment_date, amount)`.
        /// @returns The pair, or `null`.
        upfront as upfront: "[string, Money] | null" => dated_money(i.upfront),
        /// Maturity as seen by the pricer.
        /// @returns The maturity, or `null`.
        expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
});

common_getters!(JsCdsTranche, "CdsTranche");

factories!(JsCdsTranche, "CdsTranche", finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CdsTranche, {
        /// Canonical CDX.NA.IG 42 equity (0–3%) tranche, USD 10,000,000 (mirrors Rust `CdsTranche::example`): buy protection, 100bp running, maturity 2029-12-20, curves `USD-OIS` / `CDX.NA.IG.HAZARD`.
        /// @returns The example tranche.
        /// @throws Error - Throws if the canonical example fails validation (does not occur for a released build).
        example as example,
});

builder_class!(
    /// Fluent builder for `CdsTranche`; one setter per Rust `FinancialBuilder` setter.
    ///
    /// Every setter returns the builder, so calls chain; `build()` validates
    /// and consumes it.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const builder = valuations.instruments.CdsTranche.builder();
    /// console.log(typeof builder.build);
    /// builder.free();
    /// ```
    JsCdsTrancheBuilder,
    "CdsTrancheBuilder",
    finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CdsTrancheBuilder,
    JsCdsTranche,
    |b| b.build()
);

setters!(JsCdsTrancheBuilder, "CdsTrancheBuilder", {
        /// Set the instrument identifier.
        /// @param value - Unique identifier for the tranche trade.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        id as id: "string" => id,
        /// Set the underlying index name.
        /// @param value - Index name, e.g. `"CDX.NA.IG"`, `"CDX.NA.HY"`, `"iTraxx EUR"`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        index_name as indexName: "string" => id,
        /// Set the series number.
        /// @param value - Series number, e.g. `42`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        series as series: "number" => js_uint,
        /// Set the attachment point.
        /// @param value - Attachment point quoted in percent (`0.0` for equity; `3.0` for a tranche attaching at 3%).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        attach_pct as attachPct: "number" => js_f64,
        /// Set the detachment point.
        /// @param value - Detachment point quoted in percent (`3.0` for a 0-3% tranche).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        detach_pct as detachPct: "number" => js_f64,
        /// Set the maturity date of the tranche.
        /// @param value - Maturity date as an ISO-8601 string.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        maturity as maturity: "string" => js_date,
        /// Set the running coupon.
        /// @param value - Running coupon in basis points (`100.0` = 1.00%).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        coupon_bp as couponBp: "number" => js_f64,
        /// Set the business day convention for coupon dates.
        /// @param value - Roll rule (`"modified_following"` when never set).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        business_day_convention as businessDayConvention: "NonNullable<generated.valuations.CdsTranche['business_day_convention']>" => js_wire,
        /// Set the holiday calendar identifier.
        /// @param value - Holiday calendar identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        calendar_id as calendarId: "string" => id,
        /// Set the discount curve identifier (by quote currency).
        /// @param value - Discount curve identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        discount_curve_id as discountCurveId: "string" => id,
        /// Set the credit index identifier for survival/loss modeling.
        /// @param value - Credit index identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        credit_index_id as creditIndexId: "string" => id,
        /// Set the tranche side (`"pay"` buys protection, `"receive"` sells it).
        /// @param value - Tranche side.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        side as side: "generated.valuations.CdsTranche['side']" => js_wire,
        /// Set the contract start (effective) date for schedule anchoring.
        /// @param value - Effective date. Required for the bespoke regime; under the standard regime it defaults to the CDS roll date on or before the valuation date and must be a roll date.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        start_date as startDate: "string" => js_date,
        /// Set the realized (settled) loss.
        /// @param value - Realized loss as a fraction of the original portfolio notional; `0.0` when never set.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        realized_loss as realizedLoss: "number" => js_f64,
        /// Set the contractual date regime of the coupon schedule (required).
        /// @param value - `"standard"` for the CDS roll dates (20th of Mar, Jun, Sep, Dec), which pay quarterly, or `"bespoke"` for a schedule from `frequency` and `stub`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        date_regime as dateRegime: "generated.valuations.CdsTranche['date_regime']" => js_wire,
        /// Set the stub convention for a bespoke coupon schedule.
        /// @param value - Stub rule of a bespoke schedule; `"short_front"` when never set.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        stub as stub: "NonNullable<generated.valuations.CdsTranche['stub']>" => js_wire,
});

common_setters!(JsCdsTrancheBuilder, "CdsTrancheBuilder");

handle_setters!(JsCdsTrancheBuilder, "CdsTrancheBuilder", {
        /// Set the notional amount of the tranche.
        /// @param value - Notional amount of the tranche, as a `Money` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        notional as notional => JsMoney,
        /// Set the payment frequency.
        /// @param value - Payment frequency, as a `Tenor` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        frequency as frequency => JsTenor,
        /// Set the day count convention.
        /// @param value - Day count convention, as a `DayCount` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        day_count as dayCount => JsDayCount,
});

#[wasm_bindgen(js_class = CdsTranche)]
impl JsCdsTranche {
    /// Create a standard index tranche (mirrors Rust `CdsTranche::standard`).
    ///
    /// Uses the standard quarterly, Act/360, following, CDS-IMM schedule.
    /// @param id - Unique instrument identifier.
    /// @param params - `CdsTrancheParams`, e.g. from `valuations.instruments.cdsTrancheParamsEquityTranche`.
    /// @param discountCurveId - Discount curve identifier.
    /// @param creditIndexId - Credit index data identifier (index hazard curve and base correlation).
    /// @param side - Protection direction: `pay` (buy protection) or `receive`.
    /// @returns The validated tranche.
    /// @throws Error - Throws with kind `validation` if `params` or `side` is malformed or the tranche fails validation, and kind `invalid_type` for a wrong argument type.
    pub fn standard(
        #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
        #[wasm_bindgen(unchecked_param_type = "generated.valuations.CdsTrancheParams | string")]
        params: JsValue,
        #[wasm_bindgen(js_name = "discountCurveId", unchecked_param_type = "string")]
        discount_curve_id: JsValue,
        #[wasm_bindgen(js_name = "creditIndexId", unchecked_param_type = "string")] credit_index_id: JsValue,
        #[wasm_bindgen(unchecked_param_type = "'pay' | 'receive'")] side: JsValue,
    ) -> Result<JsCdsTranche, JsValue> {
        let params: finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CdsTrancheParams =
            from_js_json(&params, "params")?;
        finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CdsTranche::standard(
            js_string(&id, "id")?,
            &params,
            js_string(&discount_curve_id, "discountCurveId")?,
            js_string(&credit_index_id, "creditIndexId")?,
            js_wire(&side, "side")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }
}

upfront_setter!(JsCdsTrancheBuilder, "CdsTrancheBuilder");
