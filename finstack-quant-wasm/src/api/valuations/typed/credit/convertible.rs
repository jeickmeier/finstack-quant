//! Typed `ConvertibleBond`, `ConvertibleBondBuilder`, and spot-ladder WASM bindings.

use crate::api::core::money::JsMoney;
use crate::api::valuations::pricing::parse_market;
use crate::utils::input::{from_js_json, js_f64, js_opt_f64_seq, js_opt_string};
use crate::utils::wire::js_date;
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_valuations::instruments::Instrument;
use wasm_bindgen::prelude::*;

instrument_class!(
    /// Typed wrapper for the Rust `ConvertibleBond` instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const convertibleBond = valuations.instruments.ConvertibleBond.example();
    /// console.log(convertibleBond.id, convertibleBond.toDict());
    /// convertibleBond.free();
    /// ```
    JsConvertibleBond,
    "ConvertibleBond",
    finstack_quant_valuations::instruments::fixed_income::convertible::ConvertibleBond
);

instrument_surface!(
    JsConvertibleBond,
    "ConvertibleBond",
    JsConvertibleBondBuilder,
    "generated.valuations.ConvertibleBond"
);

getters!(JsConvertibleBond, "ConvertibleBond", |i| {
        /// Canonical model key used when `model="default"` is passed to `price`.
        /// @returns Registered model key such as `"hazard_rate"` or `"black76"`.
        default_model as defaultModel: "string" => text(Instrument::default_model(i)),
        /// Principal amount.
        /// @returns Currency-tagged principal.
        notional as notional: "Money" => money(i.notional),
        /// Dated date from which the bond starts accruing interest.
        /// @returns Calendar date, unadjusted for business days. It anchors the coupon schedule and the first accrual period.
        issue_date as issueDate: "string" => date(i.issue_date),
        /// Scheduled redemption date on which principal is repaid.
        /// @returns Unadjusted calendar maturity; payment dates derived from it are rolled by the instrument's business-day convention.
        maturity as maturity: "string" => date(i.maturity),
        /// Discount curve identifier for the debt component.
        /// @returns The curve id.
        discount_curve_id as discountCurveId: "string" => text(i.discount_curve_id),
        /// Issuer hazard curve identifier.
        /// @returns The hazard curve id, or `null` (the cash component discounts at the risk-free curve).
        credit_curve_id as creditCurveId: "string | null" => opt_text(i.credit_curve_id),
        /// Conversion terms.
        /// @returns The typed conversion spec.
        conversion as conversion: "generated.valuations.ConvertibleBond['conversion']" => json(i.conversion),
        /// Base conversion ratio (shares per bond), derived from ratio or price (mirrors Rust `conversion_ratio`).
        /// @returns The ratio, or `null` when neither ratio nor price is set.
        conversion_ratio as conversionRatio: "number | null" => json(i.conversion_ratio()),
        /// Conversion ratio after anti-dilution adjustments (mirrors Rust `effective_conversion_ratio`).
        /// @returns The adjusted ratio, or `null`.
        effective_conversion_ratio as effectiveConversionRatio: "number | null" => json(i.effective_conversion_ratio()),
        /// Market-scalar id of the underlying share price.
        /// @returns The `MarketContext` price id read for the equity spot.
        spot_id as spotId: "string" => text(i.spot_id),
        /// Equity volatility id: a volatility surface, or a unitless scalar holding a flat volatility.
        /// @returns The volatility id.
        vol_surface_id as volSurfaceId: "string" => text(i.vol_surface_id),
        /// Unitless continuous dividend-yield scalar id.
        /// @returns The id, or `null` for a zero dividend yield.
        div_yield_id as divYieldId: "string | null" => opt_text(i.div_yield_id),
        /// Call/put schedule.
        /// @returns The typed schedule, or `null`.
        call_put as callPut: "NonNullable<generated.valuations.ConvertibleBond['call_put']> | null" => json(i.call_put),
        /// Soft-call trigger (`threshold_pct`, `observation_days`, `required_days_above`).
        /// @returns The trigger plain object, or `null`.
        soft_call_trigger as softCallTrigger: "NonNullable< generated.valuations.ConvertibleBond['soft_call_trigger'] > | null" => json(i.soft_call_trigger),
        /// Settlement lag in business days.
        /// @returns The lag, or `null` for same-day.
        settlement_days as settlementDays: "NonNullable< generated.valuations.ConvertibleBond['settlement_days'] > | null" => json(i.settlement_days),
        /// Assumed recovery rate on default as a fraction.
        /// @returns The recovery, or `null`.
        recovery_rate as recoveryRate: "NonNullable<generated.valuations.ConvertibleBond['recovery_rate']> | null" => json(i.recovery_rate),
        /// Recorded exercise and delayed delivery replacing the debt and its coupons.
        /// @returns Canonical settlement record with ISO dates, shares and currency-tagged cash/recovery basis, or `null` while unresolved.
        resolved_settlement as resolvedSettlement: "NonNullable< generated.valuations.ConvertibleBond['resolved_settlement'] > | null" => json(i.resolved_settlement),
        /// Coupon/cashflow specification in serde form, the same shape as `Bond.cashflow_spec`.
        /// @returns `{"fixed": {...}}`, `{"floating": {...}}`, `{"step_up": {...}}` or `{"amortizing": {...}}`; a zero-coupon convertible is a fixed spec with rate `"0"`.
        cashflow_spec as cashflowSpec: "generated.valuations.ConvertibleBond['cashflow_spec']" => json(i.cashflow_spec),
        /// Maturity as seen by the pricer.
        /// @returns The maturity, or `null`.
        expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
});

common_getters!(JsConvertibleBond, "ConvertibleBond");

/// Price, delta, parity, conversion premium, regime and bond floor of a convertible at a ladder of share prices (mirrors Rust `convertible_spot_ladder`).
///
/// Each row reprices on the market with only the share price moved, on the lattice `model` selects; volatility, rates and credit stay as supplied.
/// @param bond - Borrowed `ConvertibleBond` handle; it is not consumed. Its `metric_pricing_overrides.bump_config` sets the spot bump behind delta (default 1% of each row's share price).
/// @param marketJson - Canonical market-context JSON holding the bond's discount curve, share price, volatility and any dividend yield, hazard curve and floating index.
/// @param asOf - ISO-8601 valuation date and origin of the tree's ACT/365F clock.
/// @param spots - Share prices in the bond's currency, each finite and positive, sorted ascending with duplicates dropped; omit for 0.25, 0.5, 0.75, 1, 1.25, 1.5, 2 and 3 times the market's share price.
/// @param model - Credit lattice every price, delta and the bond floor use: `"tree"` (also the default when omitted) or `"tsiveriotis_fernandes"`; the two differ only when the bond has a credit curve.
/// @returns `ConvertibleSpotLadder` plain object: `instrument_id`, `as_of`, `currency`, `market_spot` and `rows` in ascending share price, each with `spot`, `is_market_spot`, `price`, `conversion_value` and `bond_floor` in `currency`, `parity` over notional, and `delta` per unit of share price, `hedge_ratio` as a decimal share of the conversion ratio, `regime` (`busted` below 0.40, `balanced` to 0.80, `equity_sensitive` above) and `conversion_premium` as a decimal, each `null` where undefined: a mandatory-variable bond has no hedge ratio or regime.
/// @throws Error - Throws with kind `validation` if `spots` is empty or holds a value that is not finite and positive, `model` is not a convertible lattice, or the bond or market JSON is invalid, and kind `not_found` if market data the bond needs is missing.
#[wasm_bindgen(js_name = convertibleSpotLadder, unchecked_return_type = "generated.valuations.ConvertibleSpotLadder")]
pub fn convertible_spot_ladder(
    bond: &JsConvertibleBond,
    #[wasm_bindgen(js_name = "marketJson", unchecked_param_type = "string")] market_json: JsValue,
    #[wasm_bindgen(js_name = "asOf", unchecked_param_type = "string")] as_of: JsValue,
    #[wasm_bindgen(unchecked_optional_param_type = "readonly number[] | null")] spots: Option<
        JsValue,
    >,
    #[wasm_bindgen(unchecked_optional_param_type = "string | null")] model: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let market = parse_market(&market_json)?;
    let as_of = js_date(&as_of, "asOf")?;
    let spots = js_opt_f64_seq(spots.as_ref(), "spots")?;
    let model = js_opt_string(model.as_ref(), "model")?
        .as_deref()
        .map(finstack_quant_valuations::pricer::parse_model_key)
        .transpose()
        .map_err(to_js_err)?;
    to_js_value(
        &finstack_quant_valuations::instruments::fixed_income::convertible::convertible_spot_ladder(
            &bond.inner,
            &market,
            as_of,
            spots.as_deref(),
            model,
        )
        .map_err(to_js_err)?,
    )
}

/// One row per share price of a convertible spot ladder (mirrors Rust `ConvertibleSpotLadder::to_table`).
///
/// Python reaches the same table through `ConvertibleSpotLadder.to_dataframe`.
/// @param ladder - `ConvertibleSpotLadder` plain object or JSON string, as `convertibleSpotLadder` returns it.
/// @returns `TableEnvelope` in ascending share price with columns `spot` (float), `is_market_spot` (boolean), `price` (float), `delta` and `hedge_ratio` (nullable float), `regime` (nullable string: `busted`, `balanced` or `equity_sensitive`), `conversion_value` and `parity` (float), `conversion_premium` (nullable float) and `bond_floor` (float); money columns are in the ladder's `currency`, which the table does not state. The columns remain when there are no rows.
/// @throws Error - Throws with kind `validation` if `ladder` does not match the `ConvertibleSpotLadder` schema.
#[wasm_bindgen(js_name = convertibleSpotLadderTable, unchecked_return_type = "TableEnvelope")]
pub fn convertible_spot_ladder_table(
    #[wasm_bindgen(unchecked_param_type = "generated.valuations.ConvertibleSpotLadder | string")]
    ladder: JsValue,
) -> Result<JsValue, JsValue> {
    let ladder: finstack_quant_valuations::instruments::fixed_income::convertible::ConvertibleSpotLadder =
        from_js_json(&ladder, "ladder")?;
    crate::utils::to_js_table_numeric(&ladder.to_table())
}

factories!(JsConvertibleBond, "ConvertibleBond", finstack_quant_valuations::instruments::fixed_income::convertible::ConvertibleBond, {
        /// Canonical 5-year USD 1,000,000 2% semi-annual convertible (mirrors Rust `ConvertibleBond::example`): ratio 25 shares per bond, voluntary conversion, underlying `"TECH"`, curves `USD-IG` / `USD-CREDIT-BBB`, issue 2024-01-15, maturity 2029-01-15.
        /// @returns The example bond.
        /// @throws Error - Throws if the canonical example fails validation (does not occur for a released build).
        example as example,
        /// Mandatory (PERCS/DECS-style) convertible example (mirrors Rust `ConvertibleBond::example_mandatory`): 3-year 5% semi-annual, mandatory-variable conversion at maturity (upper conversion price 60, lower 40), 130% soft call, call at 101% after year 2 and put at 100% after year 1.
        /// @returns The example bond.
        /// @throws Error - Throws if the canonical example fails validation (does not occur for a released build).
        example_mandatory as exampleMandatory,
});

builder_class!(
    /// Fluent builder for `ConvertibleBond`; one setter per Rust `FinancialBuilder` setter.
    ///
    /// Every setter returns the builder, so calls chain; `build()` validates
    /// and consumes it.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const builder = valuations.instruments.ConvertibleBond.builder();
    /// console.log(typeof builder.build);
    /// builder.free();
    /// ```
    JsConvertibleBondBuilder,
    "ConvertibleBondBuilder",
    finstack_quant_valuations::instruments::fixed_income::convertible::ConvertibleBondBuilder,
    JsConvertibleBond,
    |b| b.build()
);

setters!(JsConvertibleBondBuilder, "ConvertibleBondBuilder", {
        /// Set the instrument identifier.
        /// @param value - Unique identifier for the convertible bond.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        id as id: "string" => id,
        /// Set the issue date.
        /// @param value - Issue date as an ISO-8601 string.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        issue_date as issueDate: "string" => js_date,
        /// Set the maturity date.
        /// @param value - Maturity date as an ISO-8601 string.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        maturity as maturity: "string" => js_date,
        /// Set the discount curve identifier for the debt component.
        /// @param value - Discount curve identifier (risk-free or funding).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        discount_curve_id as discountCurveId: "string" => id,
        /// Set the issuer hazard curve identifier.
        /// @param value - `HazardCurve` identifier; the zero-recovery risky discount factor is the risk-free discount factor times its survival probability. If never set, the cash component discounts at the risk-free curve (no credit spread). Requires an explicit `recovery_rate`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        credit_curve_id as creditCurveId: "string" => id,
        /// Set the conversion terms.
        /// @param value - Typed `ConversionSpec`, a plain object, or a JSON object string with `ratio`, `price`, `policy`, `anti_dilution`, `dividend_adjustment` and `dilution_events`; at least one of `ratio` / `price` must be set.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        conversion as conversion: "generated.valuations.ConvertibleBond['conversion']" => from_js_json,
        /// Set the market-scalar id of the underlying share price (required).
        /// @param value - `MarketContext` price id holding the share price in the bond's currency, or a unitless level.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        spot_id as spotId: "string" => id,
        /// Set the equity volatility id (required).
        /// @param value - Volatility surface id read at the conversion strike and maturity, or the id of a unitless scalar holding a flat volatility.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        vol_surface_id as volSurfaceId: "string" => id,
        /// Set the continuous dividend-yield scalar id.
        /// @param value - Id of a unitless decimal dividend yield (`0.02` = 2%). When never set the dividend yield is zero; a set id must exist in the market.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        div_yield_id as divYieldId: "string" => id,
        /// Set the call/put schedule.
        /// @param value - Typed `CallPutSchedule`, a plain object, or a JSON object string with `calls` and `puts` arrays of windows.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        call_put as callPut: "NonNullable<generated.valuations.ConvertibleBond['call_put']>" => from_js_json,
        /// Set the soft-call trigger condition.
        /// @param value - `PriceTrigger` as a plain object or JSON object string with `threshold_pct` (percent of conversion price, e.g. `130.0`), `observation_days` and `required_days_above`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        soft_call_trigger as softCallTrigger: "NonNullable<generated.valuations.ConvertibleBond['soft_call_trigger']>" => from_js_json,
        /// Set the settlement lag.
        /// @param value - Business days from trade date to settlement (e.g. `2` for US corporate convertibles); same-day when never set.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        settlement_days as settlementDays: "number" => js_uint,
        /// Set the assumed recovery rate on default.
        /// @param value - Recovery rate as a fraction (`0.40` = 40%); only relevant when `credit_curve_id` is set.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        recovery_rate as recoveryRate: "number" => js_f64,
        /// Record the claim remaining after exercise extinguishes the debt.
        /// @param value - Canonical `ConvertibleSettlement` object or JSON string with ISO `exercise_date` and `delivery_date`, nonnegative `shares`, and currency-tagged `cash` and `recovery_basis`. Cash includes retained coupons; recovery basis excludes uncapitalized accrual. Omission leaves debt unresolved.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws for malformed input or a consumed builder; `build()` validates dates, currencies and amounts.
        resolved_settlement as resolvedSettlement: "NonNullable<generated.valuations.ConvertibleBond['resolved_settlement']> | string" => from_js_json,
        /// Set the coupon/cashflow specification.
        /// @param value - Rust `CashflowSpec` as a plain object or JSON object string (the same shape as `Bond.cashflow_spec`, e.g. `{"fixed": {"coupon_type": "cash", "rate": "0.05", ...schedule}}`); a zero-coupon convertible uses a fixed spec with rate `"0"`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        cashflow_spec as cashflowSpec: "generated.valuations.ConvertibleBond['cashflow_spec']" => from_js_json,
});

common_setters!(JsConvertibleBondBuilder, "ConvertibleBondBuilder");

handle_setters!(JsConvertibleBondBuilder, "ConvertibleBondBuilder", {
        /// Set the principal amount.
        /// @param value - Principal amount, as a `Money` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        notional as notional => JsMoney,
});

#[wasm_bindgen(js_class = ConvertibleBond)]
impl JsConvertibleBond {
    /// Conversion parity (mirrors Rust `ConvertibleBond::parity`).
    /// @param marketJson - Canonical market-context JSON (string or plain object) carrying the underlying spot under `spot_id`.
    /// @returns Value of the shares received on conversion, in the bond's currency.
    /// @throws Error - Throws with kind `not_found` if the spot is missing, and kind `validation` if the market JSON is malformed or the bond fails validation.
    pub fn parity(
        &self,
        #[wasm_bindgen(js_name = "marketJson", unchecked_param_type = "JsonInput")]
        market_json: JsValue,
    ) -> Result<f64, JsValue> {
        let market = parse_market(&market_json)?;
        self.inner.parity(&market).map_err(to_js_err)
    }

    /// Conversion premium over parity (mirrors Rust `ConvertibleBond::conversion_premium`).
    /// @param marketJson - Canonical market-context JSON (string or plain object) carrying the underlying spot under `spot_id`.
    /// @param bondPrice - Bond price in currency units for the whole notional, on the same scale as parity.
    /// @returns Premium as a decimal fraction of parity (`0.25` = 25% above parity).
    /// @throws Error - Throws with kind `not_found` if the spot is missing, kind `validation` if the market JSON is malformed or the bond fails validation, and kind `invalid_type` if `bondPrice` is not a number.
    #[wasm_bindgen(js_name = conversionPremium)]
    pub fn conversion_premium(
        &self,
        #[wasm_bindgen(js_name = "marketJson", unchecked_param_type = "JsonInput")]
        market_json: JsValue,
        #[wasm_bindgen(js_name = "bondPrice", unchecked_param_type = "number")] bond_price: JsValue,
    ) -> Result<f64, JsValue> {
        let market = parse_market(&market_json)?;
        let bond_price = js_f64(&bond_price, "bondPrice")?;
        self.inner
            .conversion_premium(&market, bond_price)
            .map_err(to_js_err)
    }

    /// Tree Greeks of the convertible (mirrors Rust `ConvertibleBond::greeks`).
    ///
    /// Uses the default binomial tree; its step count is
    /// `instrument_pricing_overrides.model_config.tree_steps`.
    /// @param marketJson - Canonical market-context JSON (string or plain object) supplying the curves, spot and volatility.
    /// @param asOf - ISO-8601 valuation date.
    /// @returns `ConvertibleGreeks` plain object with the price and its delta, gamma, vega, rho and theta.
    /// @throws Error - Throws with kind `not_found` if required market data is missing, kind `validation` if the market JSON or `asOf` is malformed, and kind `computation` if the tree fails.
    #[wasm_bindgen(unchecked_return_type = "generated.valuations.ConvertibleGreeks")]
    pub fn greeks(
        &self,
        #[wasm_bindgen(js_name = "marketJson", unchecked_param_type = "JsonInput")]
        market_json: JsValue,
        #[wasm_bindgen(js_name = "asOf", unchecked_param_type = "string")] as_of: JsValue,
    ) -> Result<JsValue, JsValue> {
        let market = parse_market(&market_json)?;
        let as_of = js_date(&as_of, "asOf")?;
        to_js_value(&self.inner.greeks(&market, as_of).map_err(to_js_err)?)
    }
}
