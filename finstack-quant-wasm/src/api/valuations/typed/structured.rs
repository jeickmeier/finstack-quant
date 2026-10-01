//! Typed `StructuredCredit` class, its fluent builder, and `TrancheBuilder`.
//!
//! Pools, tranches, waterfalls and the simulation results cross the boundary
//! as plain objects (the schema-generated TypeScript types).

use crate::api::core::dates::{JsDayCount, JsTenor};
use crate::api::core::money::JsMoney;
use crate::utils::input::js_string;
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_valuations::instruments::fixed_income::loan_terms::RateSpec;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    AssetPool, StructuredCredit, TrancheStructure,
};
use finstack_quant_valuations::instruments::Instrument;
use wasm_bindgen::prelude::*;

/// A deal-type constructor `(id, pool, tranches, closingDate, maturity, discountCurveId, calendarId?)`.
macro_rules! deal_constructor {
    ($(#[$doc:meta])* $name:ident as $js_name:ident) => {
        #[wasm_bindgen(js_class = StructuredCredit)]
        impl JsStructuredCredit {
            $(#[$doc])*
            /// @param id - Unique instrument identifier.
            /// @param pool - Collateral `AssetPool` (plain object or JSON string).
            /// @param tranches - Liability `TrancheStructure` (plain object or JSON string).
            /// @param closing_date - Deal closing date as an ISO-8601 string.
            /// @param maturity - Legal final maturity as an ISO-8601 string.
            /// @param discount_curve_id - Discount curve identifier used to value tranche cashflows.
            /// @param calendar_id - Optional holiday calendar for payment-date adjustment; omit to leave payment dates unadjusted.
            /// @returns The validated deal with the deal type's standard waterfall and assumptions.
            /// @throws Error - Throws with kind `validation` if `pool` or `tranches` does not match its schema, a date is malformed, or the deal fails pricing validation; and kind `invalid_type` for a wrong argument type.
            #[allow(clippy::too_many_arguments)]
            #[wasm_bindgen(js_name = $js_name)]
            pub fn $name(
                id: JsValue,
                pool: JsValue,
                tranches: JsValue,
                closing_date: JsValue,
                maturity: JsValue,
                discount_curve_id: JsValue,
                calendar_id: Option<JsValue>,
            ) -> Result<JsStructuredCredit, JsValue> {
                let pool: AssetPool = super::arg::json(&pool, "pool")?;
                let tranches: TrancheStructure = super::arg::json(&tranches, "tranches")?;
                let mut inner = StructuredCredit::$name(
                    js_string(&id, "id")?,
                    pool,
                    tranches,
                    super::arg::date(&closing_date, "closingDate")?,
                    super::arg::date(&maturity, "maturity")?,
                    js_string(&discount_curve_id, "discountCurveId")?,
                );
                if let Some(calendar_id) =
                    crate::utils::input::js_opt_string(calendar_id.as_ref(), "calendarId")?
                {
                    inner = inner.with_calendar_id(calendar_id);
                }
                inner.validate_for_pricing().map_err(to_js_err)?;
                Ok(Self { inner })
            }
        }
    };
}

instrument_class!(
    /// Typed wrapper for the Rust `StructuredCredit` instrument (ABS, CLO, CMBS or RMBS deal).
    JsStructuredCredit,
    "StructuredCredit",
    finstack_quant_valuations::instruments::StructuredCredit
);

instrument_builder_entry!(
    JsStructuredCredit,
    "StructuredCredit",
    JsStructuredCreditBuilder
);

instrument_to_dict!(JsStructuredCredit, "StructuredCredit");

getters!(JsStructuredCredit, "StructuredCredit", |i| {
        /// Deal classification (serde string).
        /// @returns `"abs"`, `"clo"`, `"cmbs"`, `"rmbs"` ...
        deal_type as dealType => json(i.deal_type),
        /// Collateral pool.
        /// @returns Independent copy of the pool.
        pool as pool => json(i.pool),
        /// Capital structure.
        /// @returns Independent copy of the tranche structure.
        tranches as tranches => json(i.tranches),
        /// Deal closing (issuance) date.
        /// @returns The closing date.
        closing_date as closingDate => date(i.closing_date),
        /// First tranche payment date.
        /// @returns The first payment date.
        first_payment_date as firstPaymentDate => date(i.first_payment_date),
        /// Buyer settlement date for note price and spread calculations.
        /// @returns Settlement date; `null` uses the valuation date. Payments on or before settlement belong to the seller. Model PV stays at valuation.
        quote_settlement_date as quoteSettlementDate => opt_date(i.quote_settlement_date),
        /// Legal final maturity date.
        /// @returns The maturity date.
        maturity as maturity => date(i.maturity),
        /// Discount curve identifier.
        /// @returns Curve id used for discounting.
        discount_curve_id as discountCurveId => text(i.discount_curve_id),
        /// Payment frequency.
        /// @returns The tranche payment tenor.
        frequency as frequency => tenor(i.frequency),
        /// Payment calendar identifier.
        /// @returns `null` when no calendar is set.
        calendar_id as calendarId => opt_text(i.calendar_id),
        /// Payment business-day convention string.
        /// @returns `null` for the default convention.
        business_day_convention as businessDayConvention => json(i.business_day_convention),
        /// Credit model as its `CreditModelConfig` serde plain object.
        /// @returns Prepayment, default, recovery, stochastic, delinquency and card specs.
        credit_model as creditModel => json(i.credit_model),
        /// Market conditions as their serde plain object.
        /// @returns `MarketConditions` fields.
        market_conditions as marketConditions => json(i.market_conditions),
        /// Deal metadata as its serde plain object.
        /// @returns `Metadata` fields.
        deal_metadata as dealMetadata => json(i.deal_metadata),
        /// Hedges settled through the waterfall.
        /// @returns One typed `HedgeSwap` per hedge (empty when unhedged).
        hedge_swaps as hedgeSwaps => json(i.hedge_swaps),
        /// Senior transaction fees as their `DealFees` serde plain object.
        /// @returns `null` when no fee tier is attached.
        fees as fees => json(i.fees),
        /// Deal-level coverage tests as `CoverageTestSpec` serde plain objects.
        /// @returns One plain object per test (empty when none run).
        coverage_triggers as coverageTriggers => json(i.coverage_triggers),
        /// Clean-up call pool-factor threshold (decimal).
        /// @returns `null` when no clean-up call is set.
        cleanup_call_decimal as cleanupCallDecimal => json(i.cleanup_call_decimal),
        /// Assumed optional redemption.
        /// @returns `null` without a call assumption.
        call_assumption as callAssumption => json(i.call_assumption),
        /// Collateral liquidation price in percent of par.
        /// @returns `null` for par.
        liquidation_price_pct as liquidationPricePct => json(i.liquidation_price_pct),
        /// Explicit loss-allocation policy (`"write_down"` / `"par_preserving"`).
        /// @returns `null` for the deal-type default.
        loss_allocation as lossAllocation => json(i.loss_allocation),
        /// Explicit loss-recognition timing (`"at_default"` / `"at_liquidation"`).
        /// @returns `null` for the deal-type default (at liquidation for RMBS/CMBS, at default otherwise).
        loss_recognition as lossRecognition => json(i.loss_recognition),
        /// Scheduled lender draws on notes as `TrancheDraw` serde plain objects (`tranche_id`, `date`, `amount`).
        /// @returns Empty for a fully funded structure.
        tranche_draws as trancheDraws => json(i.tranche_draws),
        /// Per-period re-advance rule as its `TrancheReadvance` serde plain object (`tranche_id`, `commitment`).
        /// @returns `null` for no re-advances.
        tranche_readvance as trancheReadvance => json(i.tranche_readvance),
        /// Explicit principal-covers-senior-interest flag.
        /// @returns `null` for the deal-type default.
        principal_covers_senior_interest as principalCoversSeniorInterest => json(i.principal_covers_senior_interest),
        /// Collateral valuation rules for the coverage tests.
        /// @returns `null` when performing collateral is carried at par.
        coverage_rules as coverageRules => json(i.coverage_rules),
        /// Declarative waterfall rules as their serde plain object.
        /// @returns `null` when no rules are layered on the base waterfall.
        waterfall_rules as waterfallRules => json(i.waterfall_rules),
        /// Custom priority of payments.
        /// @returns `null` when the deal-type template applies.
        waterfall as waterfall => json(i.waterfall),
        /// Free-form attributes (tags and metadata).
        /// @returns The attribute bag.
        attributes as attributes => json(i.attributes),
        /// Deterministic prepayment model.
        /// @returns A typed copy of the spec.
        prepayment_spec as prepaymentSpec => json(i.credit_model.prepayment_spec),
        /// Deterministic default model.
        /// @returns A typed copy of the spec.
        default_spec as defaultSpec => json(i.credit_model.default_spec),
        /// Recovery rate and lag model for defaulted collateral.
        /// @returns A typed copy of the spec.
        recovery_spec as recoverySpec => json(i.credit_model.recovery_spec),
        /// Stochastic prepayment specification as its serde plain object.
        /// @returns `null` until set or enabled.
        stochastic_prepay_spec as stochasticPrepaySpec => json(i.credit_model.stochastic_prepay_spec),
        /// Stochastic default specification as its serde plain object.
        /// @returns `null` until set or enabled.
        stochastic_default_spec as stochasticDefaultSpec => json(i.credit_model.stochastic_default_spec),
        /// Stochastic recovery specification as its `RecoverySpec` serde plain object.
        /// @returns `{"type": "constant", "rate": ...}` or `{"type": "market_correlated", "mean_recovery": ..., "recovery_volatility": ..., "factor_correlation": ...}`; `null` for constant recoveries at the deterministic rate.
        stochastic_recovery_spec as stochasticRecoverySpec => json(i.credit_model.stochastic_recovery_spec),
        /// Default correlation structure as its serde plain object.
        /// @returns `null` until set or enabled.
        correlation_structure as correlationStructure => json(i.credit_model.correlation_structure),
        /// Delinquency model as its `DelinquencyModel` serde plain object.
        /// @returns `null` when no roll-rate model is set.
        delinquency as delinquency => json(i.credit_model.delinquency),
        /// Card portfolio model as its `CardPortfolioSpec` serde plain object.
        /// @returns `null` for non-card deals.
        card as card => json(i.credit_model.card),
});

builder_class!(
    /// Fluent builder for `StructuredCredit`; one setter per Rust `FinancialBuilder` setter.
    ///
    /// Every setter returns the builder, so calls chain; `build()` validates
    /// and consumes it.
    JsStructuredCreditBuilder,
    "StructuredCreditBuilder",
    finstack_quant_valuations::instruments::fixed_income::structured_credit::StructuredCreditBuilder,
    JsStructuredCredit,
    |b| b.build()
);

setters!(JsStructuredCreditBuilder, "StructuredCreditBuilder", {
        /// Set the instrument identifier.
        /// @param value - Unique identifier for the deal.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        id as id => id,
        /// Set the deal-type classification.
        /// @param value - Deal classification.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        deal_type as dealType => en,
        /// Set the structured-credit asset pool backing the deal.
        /// @param value - Asset pool definition.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        pool as pool => json,
        /// Set the tranche capital structure.
        /// @param value - Tranche capital structure.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        tranches as tranches => json,
        /// Set the deal closing (issuance) date.
        /// @param value - Deal closing date.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        closing_date as closingDate => date,
        /// Set the first payment date to tranches.
        /// @param value - First payment date.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        first_payment_date as firstPaymentDate => date,
        /// Set buyer settlement for prices, yields and spreads.
        /// @param value - Settlement on or after valuation and closing. Payments on or before this date are excluded. Omission uses the valuation date.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        quote_settlement_date as quoteSettlementDate => date,
        /// Set the legal final maturity date.
        /// @param value - Legal final maturity date.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        maturity as maturity => date,
        /// Set the payment calendar identifier for schedule adjustments.
        /// @param value - Holiday calendar identifier (e.g. `"nyse"`). Required for accurate schedule generation.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        calendar_id as calendarId => id,
        /// Set the business day convention for tranche payments.
        /// @param value - Business day convention (e.g. `"following"`, `"modified_following"`). Defaults to `"following"` when never set.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        business_day_convention as businessDayConvention => en,
        /// Set the discount curve identifier for valuation.
        /// @param value - Discount curve identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        discount_curve_id as discountCurveId => id,
        /// Set market conditions from a JSON object.
        /// @param value - `MarketConditions` object with finite annual decimal `refi_rate` for Richard-Roll refinancing incentives. Negative rates are accepted. This replaces the registry default; unknown macro-factor fields fail.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        market_conditions as marketConditions => json,
        /// Set declarative waterfall rules from a JSON object.
        /// @param value - JSON-encoded `WaterfallRules` object (available-funds caps, step-down, shifting interest, controlled accumulation), layered onto the base waterfall.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        waterfall_rules as waterfallRules => json,
        /// Set senior transaction fees from a JSON object.
        /// @param value - JSON-encoded `DealFees` object (trustee, senior management, servicing, and optional master/special servicer fees), paid ahead of every note. Optional `workout_fee_pct` (percent of the P&I collected on specially serviced loans) and `liquidation_fee_pct` (percent of liquidation proceeds) are taken inside the collateral flows. Skipped (`null`) by default.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        fees as fees => json,
        /// Replace the whole credit model (prepayment, default, recovery, stochastic and correlation specs, delinquency and card models).
        /// @param value - `CreditModelConfig` serde object. Later per-field setters (`prepayment_spec` ...) modify this model.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        credit_model as creditModel => json,
        /// Set the deterministic prepayment model.
        /// @param value - Typed spec (`PrepaymentModelSpec.constant_cpr` / `psa` / `abs` / `vector` / `cmbs_with_lockout`) or its serde form.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        prepayment_spec as prepaymentSpec => json,
        /// Set the deterministic default model.
        /// @param value - Typed spec (`DefaultModelSpec.constant_cdr` / `sda` / `vector` / `cumulative_loss` / `timing`) or its serde form.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        default_spec as defaultSpec => json,
        /// Set the recovery model.
        /// @param value - Typed spec (rate, lag, optional severity vector) or its serde form.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        recovery_spec as recoverySpec => json,
        /// Set the stochastic prepayment specification used by `StructuredCredit.price_stochastic`.
        /// @param value - `StochasticPrepaySpec` serde object (Richard-Roll or factor parameters).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        stochastic_prepay_spec as stochasticPrepaySpec => json,
        /// Set the stochastic default specification used by `StructuredCredit.price_stochastic`.
        /// @param value - `StochasticDefaultSpec` serde object.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        stochastic_default_spec as stochasticDefaultSpec => json,
        /// Set the stochastic recovery specification used by `StructuredCredit.price_stochastic`.
        /// @param value - `RecoverySpec` serde object: `{"type": "constant", "rate": 0.4}` or `{"type": "market_correlated", "mean_recovery": 0.4, "recovery_volatility": 0.25, "factor_correlation": 0.4}` (recovery falls with the systematic factor, so heavy-default paths recover less).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        stochastic_recovery_spec as stochasticRecoverySpec => json,
        /// Set the default correlation structure used by `StructuredCredit.price_stochastic`.
        /// @param value - `CorrelationStructure` serde object (factor loadings).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        correlation_structure as correlationStructure => json,
        /// Set the delinquency roll-rate, advancing and modification model (ABS/RMBS asset and rep-line pools).
        /// @param value - `DelinquencyModel` serde object: `roll_rates` (per bucket, the last rolls to charge-off), `cure_rates`, `advancing` (`{"policy": "none"}` or `{"policy": "principal_and_interest", "recoverability_cap_pct": ..., "reimburse_from_collections": false}`) and optional `modification`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        delinquency as delinquency => json,
        /// Set the card master-trust portfolio model.
        /// @param value - `CardPortfolioSpec` serde object: `monthly_payment_rate`, `portfolio_yield` and `charge_off_rate` (annual decimals), plus the optional `seller_interest` (`Money` serde object in the pool currency) and `fixed_allocation_decimal` (decimal in `(0, 1]`) that fix the investor allocation of trust collections once the revolving period ends.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        card as card => json,
        /// Set the deal-level OC / IC coverage tests.
        /// @param value - `CoverageTestSpec` objects (`id`, `tranche_id`, `kind` (`"oc"` / `"ic"`), `trigger_level` ratio, `action`, optional `placement` (`{"kind": "after_tranche", "tranche_id": ...}` or `{"kind": "after_junior_fees"}`) and `divert_pct`).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        coverage_triggers as coverageTriggers => json,
        /// Set the collateral valuation rules for the coverage tests.
        /// @param value - Typed `CoverageRules` or its serde form.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        coverage_rules as coverageRules => json,
        /// Set the assumed optional redemption for price-to-call analytics.
        /// @param value - Typed `CallAssumption` or its serde form (`date`, `price_pct`, `scope`).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        call_assumption as callAssumption => json,
        /// Set a custom priority of payments in place of the deal-type template.
        /// @param value - Typed `Waterfall` or its serde form (`tiers`, `currency`, optional `coverage_rules`).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        waterfall as waterfall => json,
        /// Set the interest-rate hedges settled through the waterfall.
        /// @param value - Typed `HedgeSwap` objects, their serde plain objects, or a JSON array string.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        hedge_swaps as hedgeSwaps => json,
        /// Set the clean-up call pool-factor threshold.
        /// @param value - Pool factor (decimal in `(0, 1)`, typically `0.10`) below which the deal is redeemed when the liquidation proceeds cover the notes.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        cleanup_call_decimal as cleanupCallDecimal => num,
        /// Set the collateral liquidation price used by deal calls and clean-up calls.
        /// @param value - Percent of par the collateral realizes (`100.0` = par).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        liquidation_price_pct as liquidationPricePct => num,
        /// Set how collateral losses reach the note balances.
        /// @param value - `"write_down"` allocates realized losses junior-first at default (RMBS/CMBS convention); `"par_preserving"` keeps note balances at par and realizes shortfalls at legal final (CLO/ABS convention). The deal type's default applies when never set.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        loss_allocation as lossAllocation => en,
        /// Set when collateral losses are booked.
        /// @param value - `"at_default"` books the expected net loss on the default date (CLO/ABS convention); `"at_liquidation"` books the realized loss when the claim settles after the recovery lag (RMBS/CMBS convention), which delays write-downs and every cumulative-loss trigger. The deal type's default applies when never set.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        loss_recognition as lossRecognition => en,
        /// Set the scheduled lender draws on notes after closing.
        /// @param value - `TrancheDraw` serde objects `{"tranche_id": "A", "date": "2025-01-01", "amount": {"amount": "5000000", "currency": "USD"}}`, ascending by date; each is applied on the first payment date at or after its date, lifting the note's balance and adding the cash to principal proceeds.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        tranche_draws as trancheDraws => json,
        /// Set the per-period re-advance of one note up to its commitment and the borrowing base while the deal revolves.
        /// @param value - `TrancheReadvance` serde object `{"tranche_id": "A", "commitment": {"amount": "70000000", "currency": "USD"}}`; requires `coverage_rules.borrowing_base`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        tranche_readvance as trancheReadvance => json,
        /// Set whether principal proceeds cover senior fees and senior interest shortfalls before any note is redeemed.
        /// @param value - `true` for the CLO principal-waterfall convention, `false` for strictly separate accounts. The deal type's default applies when never set.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        principal_covers_senior_interest as principalCoversSeniorInterest => flag,
        /// Set deal metadata (counterparties, identifiers).
        /// @param value - `Metadata` serde object.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        deal_metadata as dealMetadata => json,
        /// Set free-form attributes (tags and metadata) on the deal.
        /// @param value - Attribute bag; a plain object populates `meta` (an optional `"tags"` list populates `tags`).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        attributes as attributes => json,
});

handle_setters!(JsStructuredCreditBuilder, "StructuredCreditBuilder", {
        /// Set the payment frequency for the structure.
        /// @param value - Payment frequency for the structure, as a `Tenor` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        frequency as frequency => JsTenor,
});

setters!(JsTrancheBuilder, "TrancheBuilder", {
        /// Set the tranche identifier.
        /// @param value - Unique identifier for the tranche.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        id as id => text,
        /// Set the attachment point.
        /// @param value - Attachment point quoted in percent on a 0-100 scale (e.g. `0.0` for equity, `10.0` for a tranche attaching at 10%).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        attach_pct as attachPct => num,
        /// Set the detachment point.
        /// @param value - Detachment point quoted in percent on a 0-100 scale (e.g. `100.0` for the most senior tranche).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        detach_pct as detachPct => num,
        /// Set the tranche seniority.
        /// @param value - Structural seniority of the tranche.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        seniority as seniority => en,
        /// Set a floating-rate coupon from a JSON `RateSpec::Floating` payload.
        /// @param value - JSON-encoded, externally-tagged `RateSpec` value, e.g. `{"floating": {...FloatingRateSpec fields...}}`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        coupon_floating as couponFloating => json via coupon,
        /// Set the legal final maturity date.
        /// @param value - Legal final maturity date.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        maturity as maturity => date,
        /// Enable payment-in-kind accretion of interest shortfalls.
        /// @param value - `true` capitalizes unpaid interest into the balance; `false` (the default) defers it as a claim.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        pik_enabled as pikEnabled => flag,
        /// Mark the coupon non-deferrable or deferrable, overriding the seniority convention.
        /// @param value - `true` for a coupon the template pays from principal proceeds when interest falls short (the senior default); `false` for one that defers (the default for every other class).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        non_deferrable as nonDeferrable => flag,
        /// Set the credit rating.
        /// @param value - Rating string (`"AAA"`, `"BBB"`, `"NR"` ...).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        rating as rating => en,
        /// Attach a per-tranche overcollateralization trigger.
        /// @param value - `CoverageTrigger` serde object: `trigger_level` (ratio), optional `cure_level`, `consequence` and breach memory fields.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        oc_trigger as ocTrigger => json,
        /// Attach a per-tranche interest-coverage trigger.
        /// @param value - `CoverageTrigger` serde object (see `oc_trigger`).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        ic_trigger as icTrigger => json,
        /// Set free-form attributes (tags and metadata).
        /// @param value - Attribute bag; a plain object populates `meta` (an optional `"tags"` list populates `tags`).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        attributes as attributes => json,
});

handle_setters!(JsTrancheBuilder, "TrancheBuilder", {
        /// Set the original tranche balance.
        /// @param value - Original tranche balance, as a `Money` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        original_balance as originalBalance => JsMoney via balance,
        /// Set the payment frequency.
        /// @param value - Payment frequency, as a `Tenor` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        frequency as frequency => JsTenor,
        /// Set the day count convention for interest accrual.
        /// @param value - Day count convention for interest accrual, as a `DayCount` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        day_count as dayCount => JsDayCount,
        /// Set the current (factored) balance.
        /// @param value - Current (factored) balance, as a `Money` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        current_balance as currentBalance => JsMoney,
        /// Set interest already deferred (unpaid, still owed) at closing.
        /// @param value - Interest already deferred (unpaid, still owed) at closing, as a `Money` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        deferred_interest as deferredInterest => JsMoney,
});

deal_constructor!(
    /// Create an asset-backed security deal (mirrors Rust `StructuredCredit::new_abs`).
    new_abs as newAbs
);
deal_constructor!(
    /// Create a collateralized loan obligation (mirrors Rust `StructuredCredit::new_clo`).
    new_clo as newClo
);
deal_constructor!(
    /// Create a commercial mortgage-backed security deal (mirrors Rust `StructuredCredit::new_cmbs`).
    new_cmbs as newCmbs
);
deal_constructor!(
    /// Create a residential mortgage-backed security deal (mirrors Rust `StructuredCredit::new_rmbs`).
    new_rmbs as newRmbs
);

#[wasm_bindgen(js_class = StructuredCredit)]
impl JsStructuredCredit {
    /// Return a copy of this deal with the deal type's standard fee schedule.
    ///
    /// Mirrors Rust `StructuredCredit::with_standard_fees`; the receiver is not modified.
    /// @returns A new deal carrying the standard trustee, servicing and management fees.
    #[wasm_bindgen(js_name = withStandardFees)]
    pub fn with_standard_fees(&self) -> JsStructuredCredit {
        Self {
            inner: self.inner.clone().with_standard_fees(),
        }
    }

    /// Return a copy of this deal with the default stochastic specifications.
    ///
    /// Mirrors Rust `StructuredCredit::enable_stochastic`: fills the
    /// stochastic prepayment, default and correlation specs for the deal
    /// type. The receiver is not modified.
    /// @returns A new deal ready for `priceStochastic`.
    /// @throws Error - Throws with kind `validation` if the deal type has no stochastic defaults or the deal is invalid.
    #[wasm_bindgen(js_name = enableStochastic)]
    pub fn enable_stochastic(&self) -> Result<JsStructuredCredit, JsValue> {
        let mut inner = self.inner.clone();
        inner.enable_stochastic().map_err(to_js_err)?;
        Ok(Self { inner })
    }

    /// The waterfall this deal runs (mirrors Rust `StructuredCredit::create_waterfall`).
    ///
    /// Returns the explicit waterfall when one is set, otherwise the deal
    /// type's template built from the tranches, fees and coverage triggers.
    /// @returns `Waterfall` plain object with the payment tiers and coverage rules.
    /// @throws Error - Throws with kind `validation` if the deal's fees or tranches cannot form a waterfall.
    #[wasm_bindgen(js_name = createWaterfall)]
    pub fn create_waterfall(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.create_waterfall().map_err(to_js_err)?)
    }

    /// Projected cashflows of one tranche from the deterministic simulation.
    ///
    /// Mirrors Rust `StructuredCredit::tranche_cashflows`.
    /// @param tranche_id - Identifier of a tranche of this deal.
    /// @param market_json - Canonical market-context JSON (string or plain object) supplying the discount and forward curves.
    /// @param as_of - ISO-8601 valuation date.
    /// @returns `TrancheCashflows` plain object with total, interest, principal, PIK, deferred and writedown flows.
    /// @throws Error - Throws with kind `not_found` if `trancheId` is not a class of the deal or a curve is missing, kind `validation` if an input is malformed, and kind `computation` if the simulation fails.
    #[wasm_bindgen(js_name = trancheCashflows)]
    pub fn tranche_cashflows(
        &self,
        tranche_id: JsValue,
        market_json: JsValue,
        as_of: JsValue,
    ) -> Result<JsValue, JsValue> {
        let tranche_id = js_string(&tranche_id, "trancheId")?;
        let market = super::market(&market_json)?;
        let as_of = super::as_of(&as_of)?;
        to_js_value(
            &self
                .inner
                .tranche_cashflows(&tranche_id, &market, as_of)
                .map_err(to_js_err)?,
        )
    }

    /// Equity-tranche return metrics (mirrors Rust `calculate_equity_metrics`).
    /// @param market_json - Canonical market-context JSON (string or plain object) supplying the discount and forward curves.
    /// @param as_of - ISO-8601 valuation date.
    /// @param purchase_price_pct - Optional equity purchase price in percent of its balance (`60` = 60%); omit to invest at the model net asset value.
    /// @returns `EquityMetrics` plain object with IRR, MOIC, NAV in percent of balance and cash-on-cash yields.
    /// @throws Error - Throws with kind `not_found` if a curve is missing, kind `validation` if an input is malformed or the deal has no equity tranche, and kind `computation` if the simulation fails.
    #[wasm_bindgen(js_name = equityMetrics)]
    pub fn equity_metrics(
        &self,
        market_json: JsValue,
        as_of: JsValue,
        purchase_price_pct: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let market = super::market(&market_json)?;
        let as_of = super::as_of(&as_of)?;
        let purchase_price_pct =
            crate::utils::input::js_opt_f64(purchase_price_pct.as_ref(), "purchasePricePct")?;
        to_js_value(
            &finstack_quant_valuations::instruments::fixed_income::structured_credit::calculate_equity_metrics(
                &self.inner,
                &market,
                as_of,
                purchase_price_pct,
            )
            .map_err(to_js_err)?,
        )
    }

    /// Monte Carlo stochastic price of the deal.
    ///
    /// Mirrors Rust `StructuredCredit::price_stochastic_monte_carlo`. Call
    /// `enableStochastic()` first when the deal carries no stochastic specs.
    /// @param market_json - Canonical market-context JSON (string or plain object) supplying the discount and forward curves.
    /// @param as_of - ISO-8601 valuation date.
    /// @param num_paths - Optional number of independent estimators; omit to use `model_config.mc_paths` (default 5,000).
    /// @param antithetic - Optional antithetic pairing; `true` (the default) simulates `2 x numPaths` scenario paths.
    /// @returns `StochasticPricingResult` plain object with NPV, expected and unexpected loss, expected shortfall and per-tranche results.
    /// @throws Error - Throws with kind `not_found` if a curve is missing, kind `validation` if an input is malformed or the deal has no stochastic specs, and kind `computation` if the simulation fails.
    #[wasm_bindgen(js_name = priceStochastic)]
    pub fn price_stochastic(
        &self,
        market_json: JsValue,
        as_of: JsValue,
        num_paths: Option<JsValue>,
        antithetic: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let market = super::market(&market_json)?;
        let as_of = super::as_of(&as_of)?;
        let num_paths = crate::utils::input::js_opt_uint(num_paths.as_ref(), "numPaths")?;
        let antithetic =
            crate::utils::input::js_opt_bool(antithetic.as_ref(), "antithetic")?.unwrap_or(true);
        crate::utils::to_js_value(
            &self
                .inner
                .price_stochastic_monte_carlo(&market, as_of, num_paths, antithetic)
                .map_err(to_js_err)?,
        )
    }

    /// Run the deterministic simulation and return the deal-level accounting.
    ///
    /// Mirrors Rust `run_simulation_with_diagnostics`.
    /// @param market_json - Canonical market-context JSON (string or plain object) supplying the discount and forward curves.
    /// @param as_of - ISO-8601 valuation date.
    /// @returns `SimulationDiagnostics` plain object with per-period pool, account and coverage-test records.
    /// @throws Error - Throws with kind `not_found` if a curve is missing, kind `validation` if an input is malformed, and kind `computation` if the simulation fails.
    #[wasm_bindgen(js_name = runSimulationWithDiagnostics)]
    pub fn run_simulation_with_diagnostics(
        &self,
        market_json: JsValue,
        as_of: JsValue,
    ) -> Result<JsValue, JsValue> {
        let market = super::market(&market_json)?;
        let as_of = super::as_of(&as_of)?;
        let run = finstack_quant_valuations::instruments::fixed_income::structured_credit::run_simulation_with_diagnostics(
            &self.inner,
            &market,
            as_of,
        )
        .map_err(to_js_err)?;
        to_js_value(&run.diagnostics)
    }
}

/// Fluent builder for a `Tranche`; one setter per Rust `TrancheBuilder` setter.
///
/// Every setter returns the builder, so calls chain; `build()` validates and
/// consumes it. Create one with `valuations.instruments.trancheBuilder()`.
#[wasm_bindgen(js_name = TrancheBuilder)]
pub struct JsTrancheBuilder {
    inner: super::Staged<
        finstack_quant_valuations::instruments::fixed_income::structured_credit::TrancheBuilder,
    >,
}

impl JsTrancheBuilder {
    /// An empty builder.
    pub(crate) fn new() -> Self {
        Self {
            inner: super::Staged::new(
                finstack_quant_valuations::instruments::fixed_income::structured_credit::TrancheBuilder::new(),
            ),
        }
    }
}

#[wasm_bindgen(js_class = TrancheBuilder)]
impl JsTrancheBuilder {
    /// Set a fixed coupon.
    /// @param rate - Annual coupon as a decimal (`0.05` = 5%).
    /// @returns The builder, for chaining.
    /// @throws Error - Throws with kind `invalid_type` if `rate` is not a number, and kind `validation` if the builder was already consumed by `build()`.
    #[wasm_bindgen(js_name = couponFixed)]
    pub fn coupon_fixed(&self, rate: JsValue) -> Result<JsTrancheBuilder, JsValue> {
        let rate = super::arg::num(&rate, "rate")?;
        let inner = self.inner.apply(|b| b.coupon(RateSpec::Fixed { rate }))?;
        Ok(Self { inner })
    }

    /// Validate the staged fields and build the tranche.
    ///
    /// Runs the Rust `TrancheBuilder::build` validation and consumes the
    /// builder. Attachment and detachment points may both be omitted, in
    /// which case `TrancheStructure` derives them from the balances.
    /// @returns The validated `Tranche` plain object.
    /// @throws Error - Throws with kind `validation` if the builder was already consumed, a required field is missing, only one of the attachment and detachment points is set, or validation fails.
    pub fn build(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.take()?.build().map_err(to_js_err)?)
    }
}
