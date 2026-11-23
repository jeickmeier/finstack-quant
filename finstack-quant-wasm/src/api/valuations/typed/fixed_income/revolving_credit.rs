//! Typed `RevolvingCredit` and `RevolvingCreditBuilder` WASM bindings.

use crate::api::core::dates::{JsDayCount, JsTenor};
use crate::api::core::money::JsMoney;
use crate::api::valuations::pricing::parse_market;
use crate::utils::wire::js_date;
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_valuations::instruments::Instrument;
use wasm_bindgen::prelude::*;

instrument_class!(
    /// Typed wrapper for the canonical Rust RevolvingCredit instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { core, valuations } from "finstack-quant-wasm";
    /// await init();
    /// const facility = valuations.instruments.RevolvingCredit.example();
    /// // The example facility commits on 2024-01-01 and draws on dated events after
    /// // that, so it is valued before the commitment date: a later date needs the
    /// // `FIXING:USD-SOFR-3M` history, and one past a draw event is rejected.
    /// const market = new core.MarketContext();
    /// market.insert(core.DiscountCurve.flat("USD-OIS", "2023-12-15", 0.03, "act_365f"));
    /// market.insert(core.ForwardCurve.flat("USD-SOFR-3M", "2023-12-15", 0.03, "act_360"));
    /// const result = valuations.instruments.priceInstrument(facility.toJson(), market.toJson(), "2023-12-15", "default");
    /// console.log(result.value.amount, result.value.currency);
    /// ```
    JsRevolvingCredit, "RevolvingCredit", finstack_quant_valuations::instruments::fixed_income::revolving_credit::RevolvingCredit
);

#[wasm_bindgen(js_class = RevolvingCredit)]
impl JsRevolvingCredit {
    /// Whether the draw/repay schedule is stochastic (Monte Carlo) rather than
    /// deterministic (mirrors Rust `RevolvingCredit::is_stochastic`).
    #[wasm_bindgen(getter, js_name = isStochastic)]
    pub fn is_stochastic(&self) -> bool {
        self.inner.is_stochastic()
    }

    /// Cashflow schedule of the facility: the contractual schedule for a
    /// deterministic draw/repay spec, the path-averaged schedule for a
    /// stochastic one (Rust `CashflowScheduleSource::raw_cashflow_schedule`,
    /// the same entry Python `expected_cashflows` calls).
    /// @param marketJson - Serialized `MarketContext` with the curves the schedule projects from.
    /// @param asOf - Valuation date as an ISO-8601 string; flows are projected from here.
    /// @returns Plain `CashFlowSchedule` object with dated interest, fee and principal flows from the lender's perspective (draws negative, repayments positive).
    /// @throws Error - Throws with kind `validation` if the facility fails validation or an input is malformed, and kind `not_found` if a required curve is missing.
    #[wasm_bindgen(js_name = expectedCashflows, unchecked_return_type = "generated.valuations.CashFlowSchedule")]
    pub fn expected_cashflows(
        &self,
        #[wasm_bindgen(js_name = "marketJson", unchecked_param_type = "JsonInput")]
        market_json: JsValue,
        #[wasm_bindgen(js_name = "asOf", unchecked_param_type = "string")] as_of: JsValue,
    ) -> Result<JsValue, JsValue> {
        use finstack_quant_cashflows::CashflowScheduleSource;
        let market = parse_market(&market_json)?;
        let as_of = js_date(&as_of, "asOf")?;
        to_js_value(
            &self
                .inner
                .raw_cashflow_schedule(&market, as_of)
                .map_err(to_js_err)?,
        )
    }

    /// Price a stochastic facility with the path-retaining Monte Carlo engine.
    ///
    /// Mirrors Rust `RevolvingCreditPricer::price_with_paths` and Python
    /// `RevolvingCredit.price_with_paths`: every simulated path is kept with
    /// its present value, utilization and credit-spread samples, cashflows and
    /// draw option cost, next to the antithetic-aware estimates. The draw
    /// option cost is negative when draws at the fixed margin are worth less
    /// than at the path's fair spread.
    /// @param marketJson - Serialized `MarketContext` holding the facility's discount curve, its floating index forward curve and fixings, and the credit curve of a market-anchored spread process.
    /// @param asOf - Valuation date as an ISO 8601 `YYYY-MM-DD` string.
    /// @returns Plain `EnhancedMonteCarloResult` object with `mc_result`, one `path_results` entry per simulated path and the `draw_option_cost` estimate; path counts are plain numbers and `mc_result.run` is `null`.
    /// @throws If the market JSON or date is malformed, a required curve or fixing is missing, or the facility has a deterministic draw schedule (only stochastic facilities simulate paths).
    #[wasm_bindgen(js_name = priceWithPaths, unchecked_return_type = "EnhancedMonteCarloResult")]
    pub fn price_with_paths(
        &self,
        #[wasm_bindgen(js_name = "marketJson", unchecked_param_type = "JsonInput")]
        market_json: JsValue,
        #[wasm_bindgen(js_name = "asOf", unchecked_param_type = "string")] as_of: JsValue,
    ) -> Result<JsValue, JsValue> {
        let market = parse_market(&market_json)?;
        let as_of = js_date(&as_of, "asOf")?;
        let result =
            finstack_quant_valuations::instruments::fixed_income::revolving_credit::RevolvingCreditPricer::price_with_paths(
                &self.inner,
                &market,
                as_of,
            )
            .map_err(to_js_err)?;
        to_js_value(&result)
    }
}

factories!(JsRevolvingCredit, "RevolvingCredit", finstack_quant_valuations::instruments::fixed_income::revolving_credit::RevolvingCredit, {
        /// Canonical example facility (mirrors Rust `RevolvingCredit::example`).
        ///
        /// Returns a three-year USD 50M SOFR + 250bp facility with USD 10M drawn
        /// and a scheduled draw and repayment, useful as a starting point and in tests.
        /// @returns The example facility.
        /// @throws If construction fails (should not occur).
        example as example,
});

instrument_surface!(
    JsRevolvingCredit,
    "RevolvingCredit",
    JsRevolvingCreditBuilder,
    "generated.valuations.RevolvingCredit"
);

getters!(JsRevolvingCredit, "RevolvingCredit", |i| {
        /// Total committed amount.
        /// @returns Currency-tagged commitment.
        commitment as commitment: "Money" => money(i.commitment),
        /// Drawn balance at the simulation anchor (the later of the commitment and valuation dates), in both deterministic and stochastic mode.
        /// @returns Currency-tagged drawn balance.
        drawn as drawn: "Money" => money(i.drawn),
        /// Date the facility becomes available.
        /// @returns The commitment date.
        issue_date as issueDate: "string" => date(i.issue_date),
        /// Expiry of the commitment.
        /// @returns The maturity date.
        maturity as maturity: "string" => date(i.maturity),
        /// Coupon specification as its `RateSpec` serde plain object.
        /// @returns `{"fixed": {"rate": r}}` with a decimal rate, or `{"floating": {...}}`.
        rate as rate: "generated.valuations.RevolvingCredit['rate']" => json(i.rate),
        /// Interest accrual day count.
        /// @returns The accrual convention.
        day_count as dayCount: "DayCount" => day_count(i.day_count),
        /// Annual basis of the yield quotes and EIR accretion (resolution O11), separate from accrual.
        /// @returns `"act_365f"` or `"act_360"`.
        yield_basis as yieldBasis: "generated.valuations.RevolvingCredit['yield_basis']" => json(i.yield_basis),
        /// Payment frequency for interest and fees.
        /// @returns The payment tenor.
        frequency as frequency: "Tenor" => tenor(i.frequency),
        /// Quote compounding of the discount margin and z-spread (pricing, quote inversion and CS01), stated independently of the payment frequency.
        /// @returns The `Compounding` wire value.
        quote_compounding as quoteCompounding: "generated.valuations.RevolvingCredit['quote_compounding']" => json(i.quote_compounding),
        /// Fee structure as its serde plain object (including dated `steps`).
        /// @returns `upfront_fee`, `commitment_fee_tiers`, `usage_fee_tiers`, `facility_fee_bp` and `steps`.
        fees as fees: "generated.valuations.RevolvingCredit['fees']" => json(i.fees),
        /// Scheduled commitment changes as serde plain object rows (`date`, `amount`, `reduction_fee_bp`); empty when the commitment is flat.
        /// @returns One row per commitment step, in date order.
        commitment_steps as commitmentSteps: "NonNullable<generated.valuations.RevolvingCredit['commitment_steps']>" => json(i.commitment_steps),
        /// Dated margin steps as serde plain object rows (`date`, `delta_bp`); empty when the margin is flat.
        /// @returns One row per margin step, in date order.
        margin_steps as marginSteps: "NonNullable<generated.valuations.RevolvingCredit['margin_steps']>" => json(i.margin_steps),
        /// Dated fixed fees as serde plain object rows (`date`, `amount`); empty when none are scheduled.
        /// @returns One row per scheduled fee, in date order.
        scheduled_fees as scheduledFees: "NonNullable<generated.valuations.RevolvingCredit['scheduled_fees']>" => json(i.scheduled_fees),
        /// Effective-interest-rate reporting switch (`{"include_fees": boolean}`), or `null` for the default (fees included).
        /// @returns The switch, or `null`.
        oid_eir as oidEir: "NonNullable<generated.valuations.RevolvingCredit['oid_eir']> | null" => json(i.oid_eir),
        /// Letter-of-credit sub-facility as its serde plain object (`sublimit`, `outstanding`, `events`, `fee_bp`, `fronting_fee_bp`, `leq`), or `null` without an LC sublimit.
        /// @returns The LC sub-facility, or `null`.
        lc as lc: "NonNullable<generated.valuations.RevolvingCredit['lc']> | null" => json(i.lc),
        /// Draw/repay specification as its serde plain object.
        /// @returns `{"deterministic": [...]}` or `{"stochastic": {...}}`.
        draw_repay_spec as drawRepaySpec: "generated.valuations.RevolvingCredit['draw_repay_spec']" => json(i.draw_repay_spec),
        /// Discount curve identifier.
        /// @returns The curve id.
        discount_curve_id as discountCurveId: "string" => text(i.discount_curve_id),
        /// Credit (hazard) curve identifier, or `null`.
        /// @returns The curve id when the facility carries credit risk.
        credit_curve_id as creditCurveId: "string | null" => opt_text(i.credit_curve_id),
        /// Recovery rate on default, as a decimal in `[0, 1]`.
        /// @returns The recovery fraction.
        recovery_rate as recoveryRate: "generated.valuations.RevolvingCredit['recovery_rate']" => json(i.recovery_rate),
        /// Loan-equivalent exposure: fraction of the undrawn commitment drawn at default, as a decimal in `[0, 1]`.
        /// @returns The loan-equivalent exposure.
        leq as leq: "NonNullable<generated.valuations.RevolvingCredit['leq']>" => json(i.leq),
        /// Stub rule for schedule generation.
        /// @returns The stub kind.
        stub as stub: "NonNullable<generated.valuations.RevolvingCredit['stub']>" => json(i.stub),
        /// Business-day convention applied to payment dates (serde string, e.g. `"modified_following"`).
        /// @returns `"modified_following"` unless overridden.
        business_day_convention as businessDayConvention: "NonNullable< generated.valuations.RevolvingCredit['business_day_convention'] >" => json(i.business_day_convention),
        /// Holiday calendar identifier used for payment, fixing and settlement rolls, or `null` for weekends only.
        /// @returns The calendar identifier, or `null`.
        calendar_id as calendarId: "string | null" => opt_text(i.calendar_id),
        /// Business days between an accrual end and its payment date.
        /// @returns The payment lag; `0` pays on the adjusted accrual end.
        payment_lag_days as paymentLagDays: "NonNullable<generated.valuations.RevolvingCredit['payment_lag_days']>" => json(i.payment_lag_days),
        /// Business days from the valuation date to the settlement date used by quote metrics.
        /// @returns The settlement lag; `0` settles on the valuation date.
        settlement_days as settlementDays: "NonNullable<generated.valuations.RevolvingCredit['settlement_days']>" => json(i.settlement_days),
        /// Scenario-selection attributes.
        /// @returns The attribute map.
        attributes as attributes: "generated.valuations.RevolvingCredit['attributes']" => json(i.attributes),
        /// Default pricing model key from the `Instrument` trait.
        /// @returns The model key.
        default_model as defaultModel: "string" => text(Instrument::default_model(i)),
        /// Expiry date exposed by the `Instrument` trait, or `null`.
        /// @returns The expiry date.
        expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
});

common_getters!(JsRevolvingCredit, "RevolvingCredit", overrides);

builder_class!(
    /// Fluent builder for `RevolvingCredit`; one setter per Rust `FinancialBuilder` setter.
    ///
    /// Every setter returns the builder, so calls chain; `build()` validates
    /// and consumes it.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const builder = valuations.instruments.RevolvingCredit.builder();
    /// console.log(typeof builder.build);
    /// builder.free();
    /// ```
    JsRevolvingCreditBuilder,
    "RevolvingCreditBuilder",
    finstack_quant_valuations::instruments::fixed_income::revolving_credit::RevolvingCreditBuilder,
    JsRevolvingCredit,
    |b| b.build()
);

setters!(JsRevolvingCreditBuilder, "RevolvingCreditBuilder", {
        /// Set the quote compounding of the discount margin and z-spread (required; never derived from the payment frequency).
        /// @param value - `Compounding` wire value: `"continuous"`, `"simple"` or `{ periodic: n }`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        quote_compounding as quoteCompounding: "generated.valuations.RevolvingCredit['quote_compounding']" => from_js_json,
        /// Set the instrument identifier.
        /// @param value - Unique identifier for the facility.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        id as id: "string" => id,
        /// Set the date the facility becomes available.
        /// @param value - Commitment date as an ISO-8601 string.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        issue_date as issueDate: "string" => js_date,
        /// Set the expiry of the commitment.
        /// @param value - Maturity date as an ISO-8601 string.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        maturity as maturity: "string" => js_date,
        /// Set the facility coupon.
        /// @param value - `RateSpec` plain object or JSON string: `{"fixed": {"rate": 0.06}}` for a fixed all-in rate (`0.06` = 6%) or `{"floating": {...}}` with a `FloatingRateSpec`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        rate as rate: "generated.valuations.RevolvingCredit['rate']" => from_js_json,
        /// Set the fee structure from its serde shape.
        /// @param value - `RevolvingCreditFees` as a plain object or JSON string (`upfront_fee` as `null`, `{"amount": `Money` plain object}` or `{"fraction_of_commitment": 0.02}`; `commitment_fee_tiers`, `usage_fee_tiers`, `facility_fee_bp` and the dated `steps` list of `{"date", "commitment_delta_bp", "usage_delta_bp", "facility_delta_bp"}` rows).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        fees as fees: "generated.valuations.RevolvingCredit['fees']" => from_js_json,
        /// Set the scheduled commitment changes.
        /// @param value - Rows of `{"date": "YYYY-MM-DD", "amount": `Money` plain object, "reduction_fee_bp": string}` (an array of plain objects or a JSON string), each the commitment in force from its date; `reduction_fee_bp` is the one-off reduction fee on a step down, a decimal string in basis points of the reduced amount (default `"0"`). Dates must be strictly increasing, after the commitment date and on or before maturity.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        commitment_steps as commitmentSteps: "NonNullable<generated.valuations.RevolvingCredit['commitment_steps']>" => from_js_json,
        /// Set the dated fixed fees (amendment, waiver, extension, consent).
        /// @param value - Rows of `{"date": "YYYY-MM-DD", "amount": `Money` plain object}` (a array of plain objects or a JSON string), each paid on its date. Dates must lie after the commitment date and on or before maturity.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        scheduled_fees as scheduledFees: "NonNullable<generated.valuations.RevolvingCredit['scheduled_fees']>" => from_js_json,
        /// Set the effective-interest-rate reporting switch.
        /// @param value - `{"include_fees": boolean}` as a plain object or JSON string; `null` (the default) includes fees in the effective yield.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        oid_eir as oidEir: "NonNullable<generated.valuations.RevolvingCredit['oid_eir']>" => from_js_json,
        /// Set the letter-of-credit sub-facility.
        /// @param value - `LetterOfCreditSpec` as a plain object or JSON string with `sublimit` and `outstanding` (`Money` plain objects), `events` (rows of `{"date", "amount", "is_issue"}`), `fee_bp` (`null` accrues the floating margin), `fronting_fee_bp` and `leq` (the fraction of the LC face drawn at default). `null` removes the sublimit.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        lc as lc: "NonNullable<generated.valuations.RevolvingCredit['lc']>" => from_js_json,
        /// Set the dated margin steps.
        /// @param value - Rows of `{"date": "YYYY-MM-DD", "delta_bp": string}` (a decimal string in basis points; a array of plain objects or a JSON string); each delta shifts the floating spread or the fixed rate from its date, cumulatively. Dates must be strictly increasing and strictly inside the facility life.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        margin_steps as marginSteps: "NonNullable<generated.valuations.RevolvingCredit['margin_steps']>" => from_js_json,
        /// Set the draw/repay specification from its serde shape.
        /// @param value - `DrawRepaySpec` as a plain object or JSON string: `{"deterministic": [{"date": ..., "amount": Money, "is_draw": boolean}, ...]}` or `{"stochastic": {"utilization_process": {...}, "mc_config": ...}}`. The estimator count, antithetic flag and seed label come from `instrument_pricing_overrides.model_config` (`mc_paths`, `mc_antithetic`, `mc_seed_scenario`).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        draw_repay_spec as drawRepaySpec: "generated.valuations.RevolvingCredit['draw_repay_spec']" => from_js_json,
        /// Set the discount curve identifier.
        /// @param value - Discount curve id in the market context.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        discount_curve_id as discountCurveId: "string" => id,
        /// Set (or clear) the credit curve identifier.
        /// @param value - Hazard curve id used for survival weighting; `null` prices without credit risk. A stochastic facility with a credit curve must use a market-anchored spread process on the same curve.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        credit_curve_id as creditCurveId: "string" => id,
        /// Set the recovery rate on default.
        /// @param value - Recovery fraction as a decimal in `[0, 1]`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        recovery_rate as recoveryRate: "number" => js_f64,
        /// Set the loan-equivalent exposure drawn at default.
        /// @param value - Fraction of the undrawn commitment assumed drawn at default, as a decimal in `[0, 1]` (default `0.0`).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        leq as leq: "number" => js_f64,
        /// Set the stub rule for schedule generation.
        /// @param value - Stub kind object or serde name (`"short_front"`, `"short_back"`, `"long_front"`, `"long_back"`, `"none"`); default `"short_front"`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        stub as stub: "NonNullable<generated.valuations.RevolvingCredit['stub']>" => js_wire,
        /// Set the business-day convention for payment dates.
        /// @param value - Convention object or serde name (`"modified_following"`, `"following"`, `"preceding"`, ...); default `"modified_following"`. Accrual boundaries stay unadjusted.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        business_day_convention as businessDayConvention: "NonNullable<generated.valuations.RevolvingCredit['business_day_convention']>" => js_wire,
        /// Set the required annual basis of yield quotes and EIR accretion (resolution O11).
        /// @param value - `"act_365f"` or `"act_360"`; independent of the accrual day count. `build()` throws when it is not set.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it is not a supported basis or the builder was already consumed by `build()`.
        yield_basis as yieldBasis: "generated.valuations.RevolvingCredit['yield_basis']" => js_wire,
        /// Set the holiday calendar used for payment, fixing and settlement rolls.
        /// @param value - Calendar identifier such as `"usny"`; `null` (the default) adjusts for weekends only. An unknown identifier fails at `build()`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        calendar_id as calendarId: "string" => id,
        /// Set the payment lag in business days after each accrual end.
        /// @param value - Business days on the facility calendar; `0` (the default) pays on the adjusted accrual end.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        payment_lag_days as paymentLagDays: "number" => js_uint,
        /// Set the settlement lag used by quote metrics.
        /// @param value - Business days from the valuation date to settlement; `0` (the default) settles on the valuation date. The base present value is always anchored at the valuation date.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        settlement_days as settlementDays: "number" => js_uint,
        /// Set scenario-selection attributes.
        /// @param value - Attribute map; `null` clears it.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        attributes as attributes: "generated.valuations.RevolvingCredit['attributes']" => from_js_json,
});

common_setters!(
    JsRevolvingCreditBuilder,
    "RevolvingCreditBuilder",
    overrides
);

handle_setters!(JsRevolvingCreditBuilder, "RevolvingCreditBuilder", {
        /// Set the total commitment.
        /// @param value - Total commitment, as a `Money` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        commitment as commitment => JsMoney,
        /// Set the drawn balance at the simulation anchor, the later of the commitment date and the valuation date, in both modes. Deterministic draw/repay events must be dated after that anchor.
        /// @param value - Drawn balance at the simulation anchor, the later of the commitment date and the valuation date, in both modes. Deterministic draw/repay events must be dated after that anchor, as a `Money` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        drawn as drawn => JsMoney,
        /// Set the interest accrual day count.
        /// @param value - Interest accrual day count, as a `DayCount` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        day_count as dayCount => JsDayCount,
        /// Set the payment frequency for interest and fees.
        /// @param value - Payment frequency for interest and fees, as a `Tenor` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        frequency as frequency => JsTenor,
});
