//! Typed `AssetBackedFacility` and `AssetBackedFacilityBuilder` WASM bindings.

use crate::api::core::dates::{JsDayCount, JsTenor};
use crate::api::core::money::JsMoney;
use crate::api::valuations::pricing::parse_market;
use crate::utils::wire::js_date;
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_valuations::instruments::Instrument;
use wasm_bindgen::prelude::*;

instrument_class!(
    /// Typed wrapper for the canonical Rust AssetBackedFacility instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { core, valuations } from "finstack-quant-wasm";
    /// await init();
    /// const facility = valuations.instruments.AssetBackedFacility.example();
    /// const base = facility.borrowingBase();
    /// const market = new core.MarketContext();
    /// market.insert(core.DiscountCurve.flat("USD-OIS", "2024-01-15", 0.03, "act_365f"));
    /// const result = valuations.instruments.priceInstrument(facility.toJson(), market.toJson(), "2024-01-15", "default");
    /// console.log(base.borrowing_base.amount, result.value.amount);
    /// ```
    JsAssetBackedFacility, "AssetBackedFacility", finstack_quant_valuations::instruments::fixed_income::asset_backed_facility::AssetBackedFacility
);

#[wasm_bindgen(js_class = AssetBackedFacility)]
impl JsAssetBackedFacility {
    /// Undrawn commitment at closing: `commitment - drawn`.
    /// @returns Undrawn amount in the commitment currency.
    /// @throws Error - Throws with kind `validation` if commitment and drawn amounts differ in currency.
    #[wasm_bindgen(getter)]
    pub fn undrawn(&self) -> Result<JsMoney, JsValue> {
        self.inner
            .undrawn()
            .map(|inner| JsMoney { inner })
            .map_err(to_js_err)
    }

    /// Effective end of revolving: the scheduled `revolving_end` or the
    /// earliest scheduled date amortization event, whichever is first.
    /// @returns ISO-8601 date string (`"YYYY-MM-DD"`).
    #[wasm_bindgen(getter, js_name = effectiveRevolvingEnd)]
    pub fn effective_revolving_end(&self) -> String {
        crate::utils::date_to_iso(self.inner.effective_revolving_end())
    }

    /// Project the facility and residual cashflows through the synthetic
    /// two-class structured-credit deal (mirrors Rust `AssetBackedFacility::project`).
    /// @param marketJson - Serialized `MarketContext` with the curves and fixings the collateral and facility coupon project from.
    /// @param asOf - Valuation date as an ISO-8601 string; the projection starts here.
    /// @returns Plain `FacilityProjection` object: `facility` and `residual` tranche cashflows, `commitment_fees` and `draws` as `[date, Money]` pairs, and the per-period `diagnostics`.
    /// @throws Error - Throws with kind `validation` if the facility fails validation or an input is malformed, and kind `not_found` if market data is missing.
    #[wasm_bindgen(unchecked_return_type = "FacilityProjection")]
    pub fn project(
        &self,
        #[wasm_bindgen(js_name = "marketJson", unchecked_param_type = "JsonInput")]
        market_json: JsValue,
        #[wasm_bindgen(js_name = "asOf", unchecked_param_type = "string")] as_of: JsValue,
    ) -> Result<JsValue, JsValue> {
        let market = parse_market(&market_json)?;
        let as_of = js_date(&as_of, "asOf")?;
        to_js_value(&self.inner.project(&market, as_of).map_err(to_js_err)?)
    }

    /// The two-class structured-credit deal the engine runs for this facility
    /// (mirrors Rust `AssetBackedFacility::synthesized_deal`): facility note
    /// plus residual, borrowing-base test, reinvestment window,
    /// early-amortization rules and the term-out call.
    /// @returns Canonical `finstack_quant.instrument/1` envelope JSON of type `structured_credit`, accepted by `priceInstrument`.
    /// @throws Error - Throws with kind `validation` if the facility or the synthetic deal fails validation.
    #[wasm_bindgen(js_name = synthesizedDealJson)]
    pub fn synthesized_deal_json(&self) -> Result<String, JsValue> {
        super::super::envelope_json(self.inner.synthesized_deal().map_err(to_js_err)?)
    }

    /// Borrowing base on the closing collateral: eligible collateral,
    /// concentration excess and the advance-rate-weighted base.
    /// @returns Plain `BorrowingBaseReport` object with `eligible_collateral`, `concentration_excess` and `borrowing_base` Money values.
    /// @throws If the borrowing-base rules are malformed.
    #[wasm_bindgen(js_name = borrowingBase, unchecked_return_type = "BorrowingBaseReport")]
    pub fn borrowing_base(&self) -> Result<JsValue, JsValue> {
        let report = self.inner.borrowing_base().map_err(to_js_err)?;
        to_js_value(&report)
    }
}

factories!(JsAssetBackedFacility, "AssetBackedFacility", finstack_quant_valuations::instruments::fixed_income::asset_backed_facility::AssetBackedFacility, {
        /// The canonical example facility: the example CLO pool financed by a
        /// USD 80M commitment drawn USD 70M.
        /// @returns The example facility.
        /// @throws If construction fails (should not occur).
        example as example,
});

market_metrics!(JsAssetBackedFacility, "AssetBackedFacility", {
        /// Lender IRR: XIRR of `-drawn` on `asOf` against every projected interest,
        /// principal and fee receipt (mirrors Rust `AssetBackedFacility::facility_irr`).
        /// @param marketJson - Serialized `MarketContext` with the curves and fixings for the projection.
        /// @param asOf - Valuation date as an ISO-8601 string; the investment is dated here.
        /// @returns Annual IRR as a decimal (`0.08` = 8%).
        /// @throws Error - Throws with kind `validation` if an input is malformed, kind `not_found` if market data is missing, and kind `computation` if the XIRR does not converge.
        facility_irr as facilityIrr,
});

instrument_surface!(
    JsAssetBackedFacility,
    "AssetBackedFacility",
    JsAssetBackedFacilityBuilder,
    "generated.valuations.AssetBackedFacility"
);

getters!(JsAssetBackedFacility, "AssetBackedFacility", |i| {
        /// Collateral pool the facility lends against.
        /// @returns The pool (asset rows, rep lines or instrument collateral).
        collateral as collateral: "generated.valuations.AssetBackedFacility['collateral']" => json(i.collateral),
        /// Advance rates and concentration limits as their serde plain object.
        /// @returns `BorrowingBaseRules` with `advance_rates` and `concentration_limits`.
        borrowing_base_rules as borrowingBaseRules: "generated.valuations.AssetBackedFacility['borrowing_base_rules']" => json(i.borrowing_base_rules),
        /// Total commitment.
        /// @returns Currency-tagged commitment.
        commitment as commitment: "Money" => money(i.commitment),
        /// Amount drawn at closing.
        /// @returns Currency-tagged drawn balance.
        drawn as drawn: "Money" => money(i.drawn),
        /// Facility coupon as its `RateSpec` serde plain object.
        /// @returns `{"fixed": {"rate": r}}` with a decimal all-in rate, or `{"floating": {...}}` with a `FloatingRateSpec`.
        rate as rate: "generated.valuations.AssetBackedFacility['rate']" => json(i.rate),
        /// Commitment fee on the undrawn commitment in basis points per annum.
        /// @returns Basis points per annum.
        commitment_fee_bp as commitmentFeeBp: "number" => dec(i.commitment_fee_bp),
        /// Closing date; the first payment date is one frequency later.
        /// @returns The closing date.
        closing_date as closingDate: "string" => date(i.closing_date),
        /// Scheduled end of the revolving period.
        /// @returns The scheduled revolving end.
        revolving_end as revolvingEnd: "string" => date(i.revolving_end),
        /// Final repayment date: the revolving end plus the term-out window, capped at maturity.
        /// @returns The repayment date.
        /// @throws `FinstackError` (kind `validation`) if the term-out window
        /// exceeds the supported date range.
        repayment_date as repaymentDate: "string" => date(i.repayment_date().map_err(to_js_err)?),
        /// Legal final maturity.
        /// @returns The maturity date.
        maturity as maturity: "string" => date(i.maturity),
        /// Payment frequency of interest, fees and the borrowing-base test.
        /// @returns The payment frequency.
        frequency as frequency: "Tenor" => tenor(i.frequency),
        /// Quote compounding of the facility's spread (z-spread CS01), stated independently of the payment frequency.
        /// @returns The `Compounding` wire value.
        quote_compounding as quoteCompounding: "generated.valuations.AssetBackedFacility['quote_compounding']" => json(i.quote_compounding),
        /// Accrual day count of the facility interest and the commitment fee.
        /// @returns The accrual convention.
        day_count as dayCount: "DayCount" => day_count(i.day_count),
        /// Payment calendar identifier, or `null`.
        /// @returns Holiday calendar id (e.g. `"nyse"`).
        calendar_id as calendarId: "string | null" => opt_text(i.calendar_id),
        /// Events that end revolving early, as `AmortizationEvent` serde plain objects.
        /// @returns Each `{"kind": "date", "date": ...}`, `{"kind": "cumulative_loss", "max_cumulative_loss": ...}` or `{"kind": "excess_spread", "min_excess_spread_3m": ...}`.
        amortization_events as amortizationEvents: "NonNullable< generated.valuations.AssetBackedFacility['amortization_events'] >" => json(i.amortization_events),
        /// Term-out window as its serde plain object (`{"months": ...}`), or `null`.
        /// @returns Months after the revolving end at which the collateral is liquidated.
        term_out as termOut: "NonNullable<generated.valuations.AssetBackedFacility['term_out']> | null" => json(i.term_out),
        /// Collateral liquidation price at the term-out end in percent of par, or `null` for par.
        /// @returns Percent of par.
        liquidation_price_pct as liquidationPricePct: "NonNullable< generated.valuations.AssetBackedFacility['liquidation_price_pct'] > | null" => json(i.liquidation_price_pct),
        /// Transaction fees paid ahead of the facility's interest as the `DealFees` serde plain object.
        /// @returns `null` when the facility carries no fees.
        fees as fees: "NonNullable<generated.valuations.AssetBackedFacility['fees']> | null" => json(i.fees),
        /// Scheduled draws after closing as `DrawEvent` serde plain objects (`date`, `amount`).
        /// @returns Empty when the line is fully funded at closing.
        draws as draws: "NonNullable<generated.valuations.AssetBackedFacility['draws']>" => json(i.draws),
        /// Whether the line is re-advanced up to the borrowing base each revolving period.
        /// @returns `true` when re-advances apply.
        readvance_to_borrowing_base as readvanceToBorrowingBase: "NonNullable< generated.valuations.AssetBackedFacility['readvance_to_borrowing_base'] >" => json(i.readvance_to_borrowing_base),
        /// Discount curve identifier.
        /// @returns The curve id.
        discount_curve_id as discountCurveId: "string" => text(i.discount_curve_id),
        /// Collateral behavior as its `CreditModelConfig` serde plain object.
        /// @returns Prepayment, default, recovery and delinquency assumptions.
        credit_model as creditModel: "CreditModelConfig" => json(i.credit_model),
        /// Deterministic prepayment model of the collateral.
        /// @returns The typed spec.
        prepayment_spec as prepaymentSpec: "NonNullable<generated.valuations.AssetBackedFacility['prepayment_spec']>" => json(i.credit_model.prepayment_spec),
        /// Deterministic default model of the collateral.
        /// @returns The typed spec.
        default_spec as defaultSpec: "NonNullable<generated.valuations.AssetBackedFacility['default_spec']>" => json(i.credit_model.default_spec),
        /// Recovery model of the collateral.
        /// @returns The typed spec.
        recovery_spec as recoverySpec: "NonNullable<generated.valuations.AssetBackedFacility['recovery_spec']>" => json(i.credit_model.recovery_spec),
        /// Scenario-selection attributes.
        /// @returns The attribute map.
        attributes as attributes: "NonNullable<generated.valuations.AssetBackedFacility['attributes']>" => json(i.attributes),
        /// Default pricing model key from the `Instrument` trait (`"discounting"`).
        /// @returns The model key.
        default_model as defaultModel: "string" => text(Instrument::default_model(i)),
});

common_getters!(JsAssetBackedFacility, "AssetBackedFacility", overrides);

builder_class!(
    /// Fluent builder for `AssetBackedFacility`; one setter per Rust `FinancialBuilder` setter.
    ///
    /// Every setter returns the builder, so calls chain; `build()` validates
    /// and consumes it.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const builder = valuations.instruments.AssetBackedFacility.builder();
    /// console.log(typeof builder.build);
    /// builder.free();
    /// ```
    JsAssetBackedFacilityBuilder,
    "AssetBackedFacilityBuilder",
    finstack_quant_valuations::instruments::fixed_income::asset_backed_facility::AssetBackedFacilityBuilder,
    JsAssetBackedFacility,
    |b| b.build()
);

setters!(JsAssetBackedFacilityBuilder, "AssetBackedFacilityBuilder", {
        /// Set the facility identifier.
        /// @param value - Stable facility identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        id as id: "string" => id,
        /// Set the collateral pool.
        /// @param value - Collateral pool (asset rows, rep lines or instrument collateral).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        collateral as collateral: "generated.valuations.AssetBackedFacility['collateral']" => from_js_json,
        /// Set the advance rates, eligibility and concentration limits.
        /// @param value - `BorrowingBaseRules` serde object: `advance_rates` (each with an `asset_class` wire name or `"*"`, a decimal `rate` and an optional `eligibility`) and `concentration_limits` (each with `scope` `"obligor"` / `"industry"` / `"asset_class"` and a percent `max_pct`).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        borrowing_base_rules as borrowingBaseRules: "generated.valuations.AssetBackedFacility['borrowing_base_rules']" => from_js_json,
        /// Set the facility coupon.
        /// @param value - `RateSpec` plain object or JSON string: `{"fixed": {"rate": 0.06}}` for a fixed all-in rate (`0.06` = 6%) or `{"floating": {...}}` with a `FloatingRateSpec`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        rate as rate: "generated.valuations.AssetBackedFacility['rate']" => from_js_json,
        /// Set the commitment fee on the undrawn commitment.
        /// @param value - Basis points per annum (`50.0` = 0.50%).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        commitment_fee_bp as commitmentFeeBp: "number" => dec,
        /// Set the closing date.
        /// @param value - Closing date; the first payment date is one frequency later.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        closing_date as closingDate: "string" => js_date,
        /// Set the scheduled end of the revolving period.
        /// @param value - Revolving end; after it, collateral principal repays the facility.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        revolving_end as revolvingEnd: "string" => js_date,
        /// Set the legal final maturity.
        /// @param value - Legal final maturity.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        maturity as maturity: "string" => js_date,
        /// Set the payment calendar.
        /// @param value - Holiday calendar identifier (e.g. `"nyse"`); required for pricing.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        calendar_id as calendarId: "string" => id,
        /// Set the events that end revolving early.
        /// @param value - `AmortizationEvent` objects: `{"kind": "date", "date": ...}`, `{"kind": "cumulative_loss", "max_cumulative_loss": ...}` or `{"kind": "excess_spread", "min_excess_spread_3m": ...}`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        amortization_events as amortizationEvents: "NonNullable<generated.valuations.AssetBackedFacility['amortization_events']>" => from_js_json,
        /// Set the term-out window after revolving.
        /// @param value - Month count (e.g. `24`) or `TermOutSpec` plain object (`{"months": 24}`): the amortization term after the revolving period ends.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        term_out as termOut: "number | NonNullable<generated.valuations.AssetBackedFacility['term_out']>" => term_out,
        /// Set the transaction fees paid through the waterfall ahead of the facility's interest.
        /// @param value - `DealFees` serde object (`trustee_fee` Money per annum, `senior_mgmt_fee_bp`, `subordinated_mgmt_fee_bp`, `servicing_fee_bp`, optional `master_servicer_fee_bp` / `workout_fee_pct` / `liquidation_fee_pct` / `special_servicer_fee_bp` / `incentive_fee`).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        fees as fees: "NonNullable<generated.valuations.AssetBackedFacility['fees']>" => from_js_json,
        /// Set the scheduled draws after closing.
        /// @param value - `DrawEvent` serde objects `{"date": "2025-01-01", "amount": {"amount": "10000000", "currency": "USD"}}`, ascending by date; each is applied on the first payment date at or after its date.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        draws as draws: "NonNullable<generated.valuations.AssetBackedFacility['draws']>" => from_js_json,
        /// Set the quote compounding of the facility's spread (required; never derived from the payment frequency).
        /// @param value - `Compounding` wire value: `"continuous"`, `"simple"` or `{ periodic: n }`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        quote_compounding as quoteCompounding: "generated.valuations.AssetBackedFacility['quote_compounding']" => from_js_json,
        /// Set whether the line is re-advanced up to the borrowing base each revolving period.
        /// @param value - `true` draws `min(commitment, borrowing base) − balance` every revolving period.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        readvance_to_borrowing_base as readvanceToBorrowingBase: "boolean" => js_bool,
        /// Set the collateral liquidation price at the term-out end.
        /// @param value - Percent of par (`100.0` = par).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        liquidation_price_pct as liquidationPricePct: "number" => js_f64,
        /// Set the discount curve.
        /// @param value - Discount curve identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        discount_curve_id as discountCurveId: "string" => id,
        /// Replace the whole collateral behavior model.
        /// @param value - `CreditModelConfig` serde object; the per-field setters (`prepayment_spec` ...) then modify it.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        credit_model as creditModel: "CreditModelConfig" => from_js_json,
        /// Set the deterministic prepayment model of the collateral.
        /// @param value - Typed spec or its serde form.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        prepayment_spec as prepaymentSpec: "NonNullable<generated.valuations.AssetBackedFacility['prepayment_spec']>" => from_js_json,
        /// Set the deterministic default model of the collateral.
        /// @param value - Typed spec or its serde form.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        default_spec as defaultSpec: "NonNullable<generated.valuations.AssetBackedFacility['default_spec']>" => from_js_json,
        /// Set the recovery model of the collateral.
        /// @param value - Typed spec or its serde form.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        recovery_spec as recoverySpec: "NonNullable<generated.valuations.AssetBackedFacility['recovery_spec']>" => from_js_json,
        /// Set scenario-selection attributes.
        /// @param value - Attribute map; `null` clears it.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        attributes as attributes: "NonNullable<generated.valuations.AssetBackedFacility['attributes']>" => from_js_json,
});

common_setters!(
    JsAssetBackedFacilityBuilder,
    "AssetBackedFacilityBuilder",
    overrides
);

handle_setters!(JsAssetBackedFacilityBuilder, "AssetBackedFacilityBuilder", {
        /// Set the total commitment.
        /// @param value - Total commitment, as a `Money` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        commitment as commitment => JsMoney,
        /// Set the amount drawn at closing.
        /// @param value - Amount drawn at closing, as a `Money` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        drawn as drawn => JsMoney,
        /// Set the payment frequency.
        /// @param value - Payment frequency, as a `Tenor` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        frequency as frequency => JsTenor,
        /// Set the accrual day count.
        /// @param value - Accrual day count, as a `DayCount` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        day_count as dayCount => JsDayCount,
});
