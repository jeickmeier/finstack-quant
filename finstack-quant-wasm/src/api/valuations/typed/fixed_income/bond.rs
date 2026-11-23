//! Typed `Bond` and `BondBuilder` WASM bindings.

use super::super::data::JsMertonMcConfig;
use crate::api::core::money::JsMoney;
use crate::api::core::types::JsRate;
use crate::utils::input::{from_js_json, js_f64, js_string};
use crate::utils::wire::js_date;
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_core::dates::StubKind;
use finstack_quant_valuations::instruments::Instrument;
use wasm_bindgen::prelude::*;

instrument_class!(
    /// Typed wrapper for the canonical Rust Bond instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { core, valuations } from "finstack-quant-wasm";
    /// await init();
    /// const usd = new core.Currency("USD");
    /// const bond = valuations.instruments.Bond.fixed(
    ///   "BOND-1",
    ///   new core.Money(1_000_000, usd),
    ///   new core.Rate(0.05),
    ///   "2024-01-01",
    ///   "2034-01-01",
    ///   "none",
    ///   "USD-OIS"
    /// );
    /// const market = new core.MarketContext();
    /// market.insert(core.DiscountCurve.flat("USD-OIS", "2024-06-30", 0.03, "act_365f"));
    /// const result = valuations.instruments.priceInstrument(bond.toJson(), market.toJson(), "2024-06-30", "default");
    /// console.log(result.value.amount, result.value.currency);
    /// ```
    JsBond, "Bond", finstack_quant_valuations::instruments::fixed_income::bond::Bond
);

#[wasm_bindgen(js_class = Bond)]
impl JsBond {
    /// Construct a typed bond from an explicit cashflow schedule.
    /// @param id - Stable bond identifier used for pricing and metric keys.
    /// @param schedule - Validated principal and coupon schedule, copied into the bond.
    /// @param quoteCompounding - Spread quote compounding. Use "continuous", "simple" or an object with periodic payments per year.
    /// @param discountCurveId - Identifier of the market discount curve for the scheduled cashflows.
    /// @param quotedCleanPricePct - Optional clean price in percent of par (99.5 means 99.5%).
    /// @returns A bond ready to price against the discount curve.
    /// @throws FinstackWasmError if the schedule, quote compounding or bond terms are invalid.
    #[wasm_bindgen(js_name = fromCashflows)]
    pub fn from_cashflows(
        #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
        schedule: &crate::api::cashflows::schedule::JsCashFlowSchedule,
        #[wasm_bindgen(
            js_name = "quoteCompounding",
            unchecked_param_type = "generated.valuations.Bond['quote_compounding']"
        )]
        quote_compounding: JsValue,
        #[wasm_bindgen(js_name = "discountCurveId", unchecked_param_type = "string")]
        discount_curve_id: JsValue,
        #[wasm_bindgen(
            js_name = "quotedCleanPricePct",
            unchecked_optional_param_type = "number | null"
        )]
        quoted_clean_price_pct: Option<JsValue>,
    ) -> Result<JsBond, JsValue> {
        let id = js_string(&id, "id")?;
        let quote_compounding =
            crate::utils::input::from_js_json(&quote_compounding, "quoteCompounding")?;
        let discount_curve_id = js_string(&discount_curve_id, "discountCurveId")?;
        let quoted_clean_price_pct = crate::utils::input::js_opt_f64(
            quoted_clean_price_pct.as_ref(),
            "quotedCleanPricePct",
        )?;
        finstack_quant_valuations::instruments::fixed_income::bond::Bond::from_cashflows(
            id,
            schedule.inner.clone(),
            quote_compounding,
            discount_curve_id,
            quoted_clean_price_pct,
        )
        .map(|inner| JsBond { inner })
        .map_err(to_js_err)
    }

    /// Create a US corporate fixed-rate bond (semi-annual, 30/360, T+1).
    ///
    /// Mirrors Rust `Bond::fixed` and requires an explicit stub policy.
    /// @param id - Unique instrument identifier.
    /// @param notional - Principal amount of the bond.
    /// @param couponRate - Annual coupon rate.
    /// @param issueDate - Issue date as an ISO-8601 string (`"YYYY-MM-DD"`).
    /// @param maturity - Maturity date as an ISO-8601 string (`"YYYY-MM-DD"`).
    /// @param stub - Stub policy: `none`, `short_front`, `short_back`,
    /// `long_front`, or `long_back`.
    /// @param discountCurveId - Discount curve identifier used for pricing.
    /// @returns The validated fixed-rate bond.
    /// @throws If validation fails (e.g. maturity not after issue_date).
    pub fn fixed(
        #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
        notional: &JsMoney,
        #[wasm_bindgen(js_name = "couponRate")] coupon_rate: &JsRate,
        #[wasm_bindgen(js_name = "issueDate", unchecked_param_type = "string")] issue_date: JsValue,
        #[wasm_bindgen(unchecked_param_type = "string")] maturity: JsValue,
        #[wasm_bindgen(
            unchecked_param_type = "'none' | 'short_front' | 'short_back' | 'long_front' | 'long_back'"
        )]
        stub: JsValue,
        #[wasm_bindgen(js_name = "discountCurveId", unchecked_param_type = "string")]
        discount_curve_id: JsValue,
    ) -> Result<JsBond, JsValue> {
        let id: &str = &js_string(&id, "id")?;
        let issue_date = js_date(&issue_date, "issueDate")?;
        let maturity = js_date(&maturity, "maturity")?;
        let stub: &str = &js_string(&stub, "stub")?;
        let discount_curve_id: &str = &js_string(&discount_curve_id, "discountCurveId")?;
        let inner = finstack_quant_valuations::instruments::fixed_income::bond::Bond::fixed(
            id,
            notional.inner,
            coupon_rate.inner,
            issue_date,
            maturity,
            stub.parse::<StubKind>().map_err(to_js_err)?,
            discount_curve_id,
        )
        .map_err(to_js_err)?;
        Ok(JsBond { inner })
    }

    /// Create a fixed-rate bond from a named market convention preset.
    ///
    /// Mirrors Rust `Bond::with_convention`: frequency, day count, calendar,
    /// business-day convention, settlement lag and stub rule all come from the
    /// preset. Chain `withStub` to override the preset's stub rule.
    /// @param id - Unique instrument identifier.
    /// @param notional - Principal amount of the bond.
    /// @param couponRate - Annual coupon rate.
    /// @param issueDate - Issue date as an ISO-8601 string (`"YYYY-MM-DD"`).
    /// @param maturity - Maturity date as an ISO-8601 string (`"YYYY-MM-DD"`).
    /// @param convention - Bond convention preset: `us_treasury`, `us_agency`,
    /// `german_bund`, `uk_gilt`, `french_oat`, `jgb`, `us_corporate`, or
    /// `eur_corporate`.
    /// @param discountCurveId - Discount curve identifier used for pricing.
    /// @returns The validated fixed-rate bond.
    /// @throws Error - Throws with kind `validation` if `convention` is not a
    /// known preset, a date is malformed, or bond validation fails (e.g.
    /// maturity not after issue_date).
    #[wasm_bindgen(js_name = withConvention)]
    pub fn with_convention(
        #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
        notional: &JsMoney,
        #[wasm_bindgen(js_name = "couponRate")] coupon_rate: &JsRate,
        #[wasm_bindgen(js_name = "issueDate", unchecked_param_type = "string")] issue_date: JsValue,
        #[wasm_bindgen(unchecked_param_type = "string")] maturity: JsValue,
        #[wasm_bindgen(
            unchecked_param_type = "| 'us_treasury' | 'us_agency' | 'german_bund' | 'uk_gilt' | 'french_oat' | 'jgb' | 'us_corporate' | 'eur_corporate'"
        )]
        convention: JsValue,
        #[wasm_bindgen(js_name = "discountCurveId", unchecked_param_type = "string")]
        discount_curve_id: JsValue,
    ) -> Result<JsBond, JsValue> {
        let id: &str = &js_string(&id, "id")?;
        let issue_date = js_date(&issue_date, "issueDate")?;
        let maturity = js_date(&maturity, "maturity")?;
        let convention: &str = &js_string(&convention, "convention")?;
        let discount_curve_id: &str = &js_string(&discount_curve_id, "discountCurveId")?;
        let inner =
            finstack_quant_valuations::instruments::fixed_income::bond::Bond::with_convention(
                id,
                notional.inner,
                coupon_rate.inner,
                issue_date,
                maturity,
                convention
                    .parse::<finstack_quant_valuations::instruments::fixed_income::BondConvention>()
                    .map_err(to_js_err)?,
                discount_curve_id,
            )
            .map_err(to_js_err)?;
        Ok(JsBond { inner })
    }

    /// Return a copy of this bond with a different coupon-schedule stub rule.
    ///
    /// Mirrors Rust `Bond::with_stub`; the receiver is not modified.
    /// @param stub - Stub policy: `none`, `short_front`, `short_back`,
    /// `long_front`, or `long_back`.
    /// @returns A new bond whose coupon schedule uses `stub`.
    /// @throws Error - Throws with kind `validation` if `stub` is not a known stub policy.
    #[wasm_bindgen(js_name = withStub)]
    pub fn with_stub(
        &self,
        #[wasm_bindgen(
            unchecked_param_type = "'none' | 'short_front' | 'short_back' | 'long_front' | 'long_back'"
        )]
        stub: JsValue,
    ) -> Result<JsBond, JsValue> {
        let stub: &str = &js_string(&stub, "stub")?;
        Ok(JsBond {
            inner: self
                .inner
                .clone()
                .with_stub(stub.parse::<StubKind>().map_err(to_js_err)?),
        })
    }

    /// Create a floating-rate bond (FRN) linked to a forward index.
    ///
    /// Mirrors Rust `Bond::floating`. Settlement, calendar, and
    /// business-day convention come from the notional currency: USD
    /// UsCorporate (T+1, usny), EUR EurCorporate (T+2, target2), GBP
    /// UkGilt (T+1), JPY Jgb (T+2). Unmapped currencies throw.
    /// @param params - `FloatingBondParams` (plain object or JSON text), every key
    /// required: `id`; `notional` - Money wire object
    /// `{ amount: "1000000", currency: "USD" }` (decimal string amount), the
    /// coupon and redemption currency; `forward_curve_id` - index projection curve
    /// (e.g. `"USD-SOFR-3M"`); `spread_bp` - exact basis-point spread over
    /// the index, as a decimal string such as `"25.125"`;
    /// `issue_date` / `maturity` - ISO-8601 dates; `frequency` - coupon tenor
    /// object (e.g. `{ count: 3, unit: "months" }`); `day_count` - accrual day
    /// count label (e.g. `"act_360"`);
    /// `discount_curve_id` - pricing curve.
    /// @returns The validated floating-rate note.
    /// @throws Error - Throws with kind `validation` if `params` is malformed,
    /// the notional currency has no mapped settlement convention, or bond
    /// validation fails, and kind `invalid_type` if `params` is not an object or
    /// JSON text.
    pub fn floating(
        #[wasm_bindgen(
            unchecked_param_type = "generated.valuations.FloatingBondParams | JsonInput"
        )]
        params: JsValue,
    ) -> Result<JsBond, JsValue> {
        let params: finstack_quant_valuations::instruments::fixed_income::bond::FloatingBondParams =
            from_js_json(&params, "params")?;
        finstack_quant_valuations::instruments::fixed_income::bond::Bond::floating(params)
            .map(|inner| JsBond { inner })
            .map_err(to_js_err)
    }

    /// Create a floating-rate bond (FRN) from a named market convention preset.
    ///
    /// Mirrors Rust `Bond::floating_with_convention`: calendar, business-day
    /// convention and settlement lag come from the preset; the coupon is
    /// `forward index + spread_bp`.
    /// @param params - `FloatingBondParams` (plain object or JSON text) with the
    /// same keys as `Bond.floating`: `id`, `notional` (Money wire object),
    /// `forward_curve_id`, `spread_bp` (basis points as a decimal string), `issue_date`,
    /// `maturity`, `frequency`, `day_count` and `discount_curve_id`.
    /// @param convention - Bond convention preset: `us_treasury`, `us_agency`,
    /// `german_bund`, `uk_gilt`, `french_oat`, `jgb`, `us_corporate`, or
    /// `eur_corporate`.
    /// @returns The validated floating-rate note.
    /// @throws Error - Throws with kind `validation` if `params` is malformed,
    /// `convention` is not a known preset, or bond validation fails, and kind
    /// `invalid_type` for a wrong argument type.
    #[wasm_bindgen(js_name = floatingWithConvention)]
    pub fn floating_with_convention(
        #[wasm_bindgen(
            unchecked_param_type = "generated.valuations.FloatingBondParams | JsonInput"
        )]
        params: JsValue,
        #[wasm_bindgen(
            unchecked_param_type = "| 'us_treasury' | 'us_agency' | 'german_bund' | 'uk_gilt' | 'french_oat' | 'jgb' | 'us_corporate' | 'eur_corporate'"
        )]
        convention: JsValue,
    ) -> Result<JsBond, JsValue> {
        let params: finstack_quant_valuations::instruments::fixed_income::bond::FloatingBondParams =
            from_js_json(&params, "params")?;
        finstack_quant_valuations::instruments::fixed_income::bond::Bond::floating_with_convention(
            params,
            js_string(&convention, "convention")?
                .parse::<finstack_quant_valuations::instruments::fixed_income::BondConvention>()
                .map_err(to_js_err)?,
        )
        .map(|inner| JsBond { inner })
        .map_err(to_js_err)
    }

    /// Create a zero-coupon bond that pays `notional` at maturity.
    ///
    /// Mirrors Rust `Bond::zero_coupon`.
    /// @param id - Unique instrument identifier.
    /// @param notional - Principal repaid at maturity.
    /// @param issueDate - Issue date as an ISO-8601 string (`"YYYY-MM-DD"`).
    /// @param maturity - Maturity date as an ISO-8601 string (`"YYYY-MM-DD"`).
    /// @param quoteCompounding - Compounding of the bond's z-spread quotes, in `Compounding` wire form (`"continuous"`, `"simple"` or `{ periodic: n }`); required.
    /// @param discountCurveId - Discount curve identifier used for pricing.
    /// @returns The validated zero-coupon bond.
    /// @throws Error - Throws with kind `validation` if a date is malformed or
    /// bond validation fails (e.g. maturity not after issue_date).
    #[wasm_bindgen(js_name = zeroCoupon)]
    pub fn zero_coupon(
        #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
        notional: &JsMoney,
        #[wasm_bindgen(js_name = "issueDate", unchecked_param_type = "string")] issue_date: JsValue,
        #[wasm_bindgen(unchecked_param_type = "string")] maturity: JsValue,
        #[wasm_bindgen(
            js_name = "quoteCompounding",
            unchecked_param_type = "generated.valuations.Bond['quote_compounding']"
        )]
        quote_compounding: JsValue,
        #[wasm_bindgen(js_name = "discountCurveId", unchecked_param_type = "string")]
        discount_curve_id: JsValue,
    ) -> Result<JsBond, JsValue> {
        let inner = finstack_quant_valuations::instruments::fixed_income::bond::Bond::zero_coupon(
            js_string(&id, "id")?,
            notional.inner,
            js_date(&issue_date, "issueDate")?,
            js_date(&maturity, "maturity")?,
            crate::utils::input::from_js_json(&quote_compounding, "quoteCompounding")?,
            js_string(&discount_curve_id, "discountCurveId")?,
        )
        .map_err(to_js_err)?;
        Ok(JsBond { inner })
    }

    /// Return a copy of this bond with a minimum-MOIC return floor.
    ///
    /// Mirrors Rust `Bond::min_moic`; the receiver is not modified.
    /// @param multiple - Minimum multiple of invested capital the holder
    /// receives (e.g. `1.3` for 1.3x), validated by the bond on use.
    /// @returns A new bond carrying the return floor.
    /// @throws Error - Throws with kind `invalid_type` if `multiple` is not a number.
    #[wasm_bindgen(js_name = minMoic)]
    pub fn min_moic(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] multiple: JsValue,
    ) -> Result<JsBond, JsValue> {
        Ok(JsBond {
            inner: self.inner.clone().min_moic(js_f64(&multiple, "multiple")?),
        })
    }

    /// Return a copy of this bond with a minimum-IRR (XIRR) return floor.
    ///
    /// Mirrors Rust `Bond::min_xirr`; the receiver is not modified.
    /// @param rate - Target annualized IRR (e.g. `Rate.fromPercent(12)`).
    /// @returns A new bond carrying the return floor.
    #[wasm_bindgen(js_name = minXirr)]
    pub fn min_xirr(&self, rate: &JsRate) -> JsBond {
        JsBond {
            inner: self.inner.clone().min_xirr(rate.inner),
        }
    }
}

factories!(JsBond, "Bond", finstack_quant_valuations::instruments::fixed_income::bond::Bond, {
        /// Canonical example bond (mirrors Rust `Bond::example`): a 10-year US
        /// Treasury, USD 1,000,000 at 4.25%, issued 2024-01-15.
        /// @returns The example bond.
        /// @throws Error - Throws if the canonical example fails validation (does not occur for a released build).
        example as example,
        /// Canonical example floating-rate note (mirrors Rust `Bond::example_floating`):
        /// USD SOFR 3M + 150bp with a 0% index floor.
        /// @returns The example floating-rate note.
        /// @throws Error - Throws if the canonical example fails validation (does not occur for a released build).
        example_floating as exampleFloating,
        /// Canonical example callable bond (mirrors Rust `Bond::example_callable`):
        /// a 5% semi-annual bond with a declining call schedule.
        /// @returns The example callable bond.
        /// @throws Error - Throws if the canonical example fails validation (does not occur for a released build).
        example_callable as exampleCallable,
        /// Canonical example amortizing bond (mirrors Rust `Bond::example_amortizing`):
        /// a 4% semi-annual bond with a scheduled principal amortization.
        /// @returns The example amortizing bond.
        /// @throws Error - Throws if the canonical example fails validation (does not occur for a released build).
        example_amortizing as exampleAmortizing,
});

instrument_surface!(JsBond, "Bond", JsBondBuilder, "generated.valuations.Bond");

getters!(JsBond, "Bond", |i| {
        /// Principal amount of the bond.
        /// @returns Currency-tagged principal.
        notional as notional: "Money" => money(i.notional),
        /// Issue date of the bond.
        /// @returns The contractual issue date.
        issue_date as issueDate: "string" => date(i.issue_date),
        /// Maturity (final redemption) date.
        /// @returns The contractual maturity date.
        maturity as maturity: "string" => date(i.maturity),
        /// Coupon/cashflow specification in serde form.
        /// @returns One-key plain object: `{"fixed": {...}}`, `{"floating": {...}}`, `{"step_up": {...}}` or `{"amortizing": {...}}`.
        cashflow_spec as cashflowSpec: "generated.valuations.Bond['cashflow_spec']" => json(i.cashflow_spec),
        /// Quote compounding of the z-spread (spread pricing, quote inversion and CS01), stated independently of the coupon frequency.
        /// @returns The `Compounding` wire value.
        quote_compounding as quoteCompounding: "generated.valuations.Bond['quote_compounding']" => json(i.quote_compounding),
        /// Discount curve identifier.
        /// @returns Curve id used for discounting.
        discount_curve_id as discountCurveId: "string" => text(i.discount_curve_id),
        /// Hazard curve identifier for `hazard_rate` and `rates_credit` pricing.
        /// @returns Curve id, or `null` when no credit-consuming model is configured.
        credit_curve_id as creditCurveId: "string | null" => opt_text(i.credit_curve_id),
        /// Repo (financing) discount curve identifier.
        /// @returns Curve id, or `null`.
        repo_curve_id as repoCurveId: "string | null" => opt_text(i.repo_curve_id),
        /// Call/put schedule in serde form.
        /// @returns `{"calls": [...], "puts": [...]}` or `null` for a bullet bond.
        call_put as callPut: "NonNullable<generated.valuations.Bond['call_put']> | null" => json(i.call_put),
        /// Return-floor specification (minimum MOIC / XIRR) in serde form.
        /// @returns The spec plain object, or `null`.
        return_floor as returnFloor: "NonNullable<generated.valuations.Bond['return_floor']> | null" => json(i.return_floor),
        /// Explicit cashflow schedule overriding generated coupons, in serde form.
        /// @returns The schedule plain object, or `null`.
        custom_cashflows as customCashflows: "NonNullable<generated.valuations.Bond['custom_cashflows']> | null" => json(i.custom_cashflows),
        /// Accrual method (serde string).
        /// @returns `"linear"` unless overridden.
        accrual_method as accrualMethod: "NonNullable<generated.valuations.Bond['accrual_method']>" => json(i.accrual_method),
        /// Settlement convention (settlement lag, ex-coupon period) in serde form.
        /// @returns `{"settlement_days": ..., "ex_coupon_days": ..., "ex_coupon_calendar_id": ...}` or `null`.
        settlement_convention as settlementConvention: "Pick< generated.valuations.Bond, 'settlement_days' | 'ex_coupon_days' | 'ex_coupon_calendar_id' > | null" => json(i.settlement_convention),
        /// Settlement lag in business days.
        /// @returns The lag, or `null` when no settlement convention is set.
        settlement_days as settlementDays: "number | null" => json(i.settlement_days()),
        /// Ex-coupon period in business days.
        /// @returns The period, or `null` when no settlement convention is set.
        ex_coupon_days as exCouponDays: "number | null" => json(i.ex_coupon_days()),
        /// Whether coupons depend on forward-curve projection (FRNs).
        /// @returns `true` for floating (or amortizing-floating) cashflow specs.
        has_floating_coupons as hasFloatingCoupons: "boolean" => json(i.has_floating_coupons()),
        /// Canonical model key used when `model="default"` is passed to `price`.
        /// @returns Registered model key such as `"discounting"` or `"black76"`.
        default_model as defaultModel: "string" => text(Instrument::default_model(i)),
        /// Expiry date exposed by the Rust `Instrument` trait.
        /// @returns The expiry/maturity date, or `null` when the instrument type reports none.
        expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
});

common_getters!(JsBond, "Bond");

builder_class!(
    /// Fluent builder for `Bond`; one setter per Rust `FinancialBuilder` setter.
    ///
    /// Every setter returns the builder, so calls chain; `build()` validates
    /// and consumes it.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const builder = valuations.instruments.Bond.builder();
    /// console.log(typeof builder.build);
    /// builder.free();
    /// ```
    JsBondBuilder,
    "BondBuilder",
    finstack_quant_valuations::instruments::fixed_income::bond::BondBuilder,
    JsBond,
    |b| b.build()
);

setters!(JsBondBuilder, "BondBuilder", {
        /// Set the quote compounding of the z-spread (required; never derived from the coupon frequency).
        /// @param value - `Compounding` wire value: `"continuous"`, `"simple"` or `{ periodic: n }`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        quote_compounding as quoteCompounding: "generated.valuations.Bond['quote_compounding']" => from_js_json,
        /// Set the instrument identifier.
        /// @param value - Instrument identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        id as id: "string" => id,
        /// Set the required contractual issue date.
        /// @param value - Required contractual issue date as an ISO-8601 string. `build()` throws when it is not set.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        issue_date as issueDate: "string" => js_date,
        /// Set the maturity date.
        /// @param value - Maturity date as an ISO-8601 string.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        maturity as maturity: "string" => js_date,
        /// Set the coupon/cashflow specification.
        /// @param value - Rust `CashflowSpec` in serde form (plain object or JSON string), e.g. `{"fixed": {"coupon_type": "cash", "rate": "0.05", "schedule": {...}}}` (copy `Bond.example().cashflow_spec`).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        cashflow_spec as cashflowSpec: "generated.valuations.Bond['cashflow_spec']" => from_js_json,
        /// Set the discount curve identifier.
        /// @param value - Discount curve identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        discount_curve_id as discountCurveId: "string" => id,
        /// Set the hazard curve identifier for `hazard_rate` and `rates_credit` pricing.
        /// @param value - Hazard curve identifier for scalar or joint rates-credit pricing.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        credit_curve_id as creditCurveId: "string" => id,
        /// Set the repo (financing) discount curve identifier.
        /// @param value - Repo (financing) discount curve identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        repo_curve_id as repoCurveId: "string" => id,
        /// Set the call/put schedule.
        /// @param value - Rust `CallPutSchedule` in serde form (plain object or JSON string), e.g. `{"calls": [{"start": "2027-01-15", "end": "2029-01-15", "price_pct_of_par": 100.0}], "puts": []}`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        call_put as callPut: "NonNullable<generated.valuations.Bond['call_put']>" => from_js_json,
        /// Set the return-floor specification (minimum MOIC / XIRR on early redemption).
        /// @param value - Rust `ReturnFloorSpec` in serde form (plain object or JSON string), e.g. `Bond.example().min_moic(1.25).return_floor`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        return_floor as returnFloor: "NonNullable<generated.valuations.Bond['return_floor']>" => from_js_json,
        /// Set the explicit cashflow schedule that overrides generated coupons.
        /// @param value - Rust `CashFlowSchedule` in serde form (plain object or JSON string), e.g. the `custom_cashflows` value of a bond built from cashflows.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        custom_cashflows as customCashflows: "NonNullable<generated.valuations.Bond['custom_cashflows']>" => from_js_json,
        /// Set the accrual method.
        /// @param value - Accrual method (serde string). `"linear"` is the default.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        accrual_method as accrualMethod: "NonNullable<generated.valuations.Bond['accrual_method']>" => js_wire,
        /// Set the settlement convention (settlement lag and ex-coupon period).
        /// @param value - Rust `BondSettlementConvention` in serde form (plain object or JSON string), e.g. `{"settlement_days": 2, "ex_coupon_days": 0, "ex_coupon_calendar_id": `null`}`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        settlement_convention as settlementConvention: "Pick< generated.valuations.Bond, 'settlement_days' | 'ex_coupon_days' | 'ex_coupon_calendar_id' >" => from_js_json,
});

common_setters!(JsBondBuilder, "BondBuilder");

handle_setters!(JsBondBuilder, "BondBuilder", {
        /// Set the principal amount.
        /// @param value - Principal amount, as a `Money` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        notional as notional => JsMoney,
});

#[wasm_bindgen(js_class = Bond)]
impl JsBond {
    /// Price the bond with the Merton structural Monte Carlo engine.
    ///
    /// Mirrors Rust `Bond::price_merton_mc`: simulates the issuer's asset
    /// value, defaults on a barrier crossing, and prices cash, PIK and toggle
    /// coupons path by path. The estimator count, antithetic flag and seed
    /// label come from `instrument_pricing_overrides.model_config`
    /// (`mc_paths`, `mc_antithetic`, `mc_seed_scenario`), defaulting to 5,000
    /// antithetic estimators seeded from the bond id.
    /// @param config - Merton Monte Carlo configuration built with `MertonMcConfig`.
    /// @param discountRate - Flat continuously compounded risk-free rate as a decimal (`0.04` = 4%).
    /// @param asOf - ISO-8601 valuation date.
    /// @returns `MertonMcResult` with clean and dirty price in percent of par, loss statistics and path statistics.
    /// @throws Error - Throws with kind `validation` if `asOf` is malformed or the bond or configuration is invalid, kind `invalid_type` if `discountRate` is not a number, and kind `computation` if the simulation fails.
    #[wasm_bindgen(js_name = priceMertonMc, unchecked_return_type = "generated.valuations.MertonMcResult")]
    pub fn price_merton_mc(
        &self,
        config: &JsMertonMcConfig,
        #[wasm_bindgen(js_name = "discountRate", unchecked_param_type = "number")]
        discount_rate: JsValue,
        #[wasm_bindgen(js_name = "asOf", unchecked_param_type = "string")] as_of: JsValue,
    ) -> Result<JsValue, JsValue> {
        let discount_rate = js_f64(&discount_rate, "discountRate")?;
        let as_of = js_date(&as_of, "asOf")?;
        to_js_value(
            &self
                .inner
                .price_merton_mc(&config.inner, discount_rate, as_of)
                .map_err(to_js_err)?,
        )
    }
}
