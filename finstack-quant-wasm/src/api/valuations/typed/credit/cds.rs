//! Typed `CreditDefaultSwap` and `CreditDefaultSwapBuilder` WASM bindings.

use crate::api::core::money::JsMoney;
use finstack_quant_valuations::instruments::Instrument;
use wasm_bindgen::prelude::*;

instrument_class!(
    /// Typed wrapper for the Rust `CreditDefaultSwap` instrument (single-name CDS).
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const creditDefaultSwap = valuations.instruments.CreditDefaultSwap.example();
    /// console.log(creditDefaultSwap.id, creditDefaultSwap.toDict());
    /// creditDefaultSwap.free();
    /// ```
    JsCreditDefaultSwap,
    "CreditDefaultSwap",
    finstack_quant_valuations::instruments::credit_derivatives::cds::CreditDefaultSwap
);

instrument_surface!(
    JsCreditDefaultSwap,
    "CreditDefaultSwap",
    JsCreditDefaultSwapBuilder,
    "generated.valuations.CreditDefaultSwap"
);

getters!(JsCreditDefaultSwap, "CreditDefaultSwap", |i| {
        /// Same-day premium and actual upfront cash election.
        /// @returns True when payments dated on the valuation date are still owed.
        include_valuation_date_cashflows as includeValuationDateCashflows: "boolean" => json(i.include_valuation_date_cashflows),
        /// Canonical model key used when `model="default"` is passed to `price`.
        /// @returns Registered model key such as `"hazard_rate"` or `"black76"`.
        default_model as defaultModel: "string" => text(Instrument::default_model(i)),
        /// Notional amount of protection.
        /// @returns Currency-tagged notional.
        notional as notional: "Money" => money(i.notional),
        /// Protection perspective.
        /// @returns `"pay"` (buy protection) or `"receive"` (sell protection).
        side as side: "generated.valuations.CreditDefaultSwap['side']" => json(i.side),
        /// ISDA regional convention (serde name).
        /// @returns `"isda_na"`, `"isda_eu"`, `"isda_as"` or `"custom"`.
        convention as convention: "generated.valuations.CreditDefaultSwap['convention']" => json(i.convention),
        /// Premium (fixed coupon) leg specification.
        /// @returns The premium leg.
        premium_leg as premiumLeg: "generated.valuations.CreditDefaultSwap['premium_leg']" => json(i.premium_leg),
        /// Protection (default-contingent) leg specification.
        /// @returns The protection leg.
        protection_leg as protectionLeg: "generated.valuations.CreditDefaultSwap['protection_leg']" => json(i.protection_leg),
        /// Valuation presentation convention (serde name).
        /// @returns `"bloomberg_cdsw_clean"` (default), `"bloomberg_cdsw_clean_full_premium"`, `"isda_dirty"` or `"quant_lib_isda_parity"`.
        valuation_convention as valuationConvention: "NonNullable< generated.valuations.CreditDefaultSwap['valuation_convention'] >" => json(i.valuation_convention),
        /// Points-upfront payment as `(payment_date, amount)`; positive means the protection buyer pays.
        /// @returns The upfront pair, or `null` when the trade has no upfront.
        upfront as upfront: "[string, Money] | null" => dated_money(i.upfront),
        /// Explicit ISDA documentation clause (serde name).
        /// @returns The clause, or `null` when derived from the convention.
        doc_clause as docClause: "NonNullable<generated.valuations.CreditDefaultSwap['doc_clause']> | null" => json(i.doc_clause),
        /// Effective documentation clause after convention-based resolution (mirrors Rust `doc_clause_effective`).
        /// @returns `"xr14"` for `isda_na` / `isda_as` / `custom`, `"mm14"` for `isda_eu`, or the explicit clause resolved to its 2014 variant.
        doc_clause_effective as docClauseEffective: "string" => json(i.doc_clause_effective()),
        /// Protection effective date for a forward-starting CDS.
        /// @returns The date, or `null` when protection starts with the premium leg.
        protection_effective_date as protectionEffectiveDate: "string | null" => opt_date(i.protection_effective_date),
        /// Date protection starts (mirrors Rust `protection_start`).
        /// @returns `protection_effective_date` when set, else the premium start date.
        protection_start as protectionStart: "string" => date(i.protection_start()),
        /// OTC margin specification in serde form.
        /// @returns The `OtcMarginSpec` plain object, or `null` for unmargined trades.
        margin_spec as marginSpec: "NonNullable<generated.valuations.CreditDefaultSwap['margin_spec']> | null" => json(i.margin_spec),
        /// Premium-leg end date as seen by the pricer.
        /// @returns The scheduled maturity, or `null`.
        expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
});

common_getters!(JsCreditDefaultSwap, "CreditDefaultSwap");

factories!(JsCreditDefaultSwap, "CreditDefaultSwap", finstack_quant_valuations::instruments::credit_derivatives::cds::CreditDefaultSwap, {
        /// Canonical 5-year USD 10,000,000 investment-grade payer CDS (mirrors Rust `CreditDefaultSwap::example`): `isda_na` convention, 100bp running spread, 40% recovery, curves `USD-OIS` / `CORP-HAZARD`, premium 2024-03-20 to 2029-03-20.
        /// @returns The example CDS.
        /// @throws Error - Throws if the canonical example fails validation (does not occur for a released build).
        example as example,
});

market_metrics!(JsCreditDefaultSwap, "CreditDefaultSwap", {
        /// Par spread implied by the market, in basis points (mirrors Rust `CreditDefaultSwap::par_spread`): the running spread at which the contract is worth zero under this CDS's valuation convention, premium schedule, discount curve, hazard curve and recovery assumption.
        /// @param marketJson - Canonical market-context JSON (string or plain object) supplying the curves and quotes this instrument needs.
        /// @param asOf - ISO-8601 valuation date.
        /// @returns Par spread in basis points.
        /// @throws Error - Throws with kind `not_found` if required market data is missing, kind `validation` if the market JSON or `asOf` is malformed, and kind `computation` if the calculation fails.
        par_spread as parSpread,
});

builder_class!(
    /// Fluent builder for `CreditDefaultSwap`; one setter per Rust `FinancialBuilder` setter.
    ///
    /// Every setter returns the builder, so calls chain; `build()` validates
    /// and consumes it.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const builder = valuations.instruments.CreditDefaultSwap.builder();
    /// console.log(typeof builder.build);
    /// builder.free();
    /// ```
    JsCreditDefaultSwapBuilder,
    "CreditDefaultSwapBuilder",
    finstack_quant_valuations::instruments::credit_derivatives::cds::CreditDefaultSwapBuilder,
    JsCreditDefaultSwap,
    |b| b.build()
);

setters!(JsCreditDefaultSwapBuilder, "CreditDefaultSwapBuilder", {
        /// Elect whether to include payments due on the valuation date.
        /// @param value - True when same-day premium and actual upfront are still owed.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` for non-boolean inputs or `validation` once the builder is consumed.
        include_valuation_date_cashflows as includeValuationDateCashflows: "boolean" => js_bool,
        /// Set the instrument identifier.
        /// @param value - Unique identifier for the CDS.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        id as id: "string" => id,
        /// Set the protection buyer/seller perspective.
        /// @param value - `"pay"` to buy protection (pay premium), `"receive"` to sell protection (receive premium).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        side as side: "generated.valuations.CreditDefaultSwap['side']" => js_wire,
        /// Set the ISDA regional convention.
        /// @param value - `"isda_na"` is the SNAC / post-Big-Bang North American standard (ACT/360, quarterly IMM, T+3); `"isda_eu"` the European standard (T+1, TARGET2); `"isda_as"` Asian (ACT/365F, Tokyo); `"custom"` for a manually configured convention.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        convention as convention: "generated.valuations.CreditDefaultSwap['convention']" => js_wire,
        /// Set the premium leg specification.
        /// @param value - Premium leg specification.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        premium_leg as premiumLeg: "generated.valuations.CreditDefaultSwap['premium_leg']" => from_js_json,
        /// Set the protection leg specification.
        /// @param value - Protection leg specification.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        protection_leg as protectionLeg: "generated.valuations.CreditDefaultSwap['protection_leg']" => from_js_json,
        /// Set the valuation presentation convention.
        /// @param value - `"bloomberg_cdsw_clean"` (default) reports Bloomberg CDSW clean principal; `"isda_dirty"` the academic ISDA dirty PV; `"quant_lib_isda_parity"` reproduces QuantLib `IsdaCdsEngine`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        valuation_convention as valuationConvention: "NonNullable<generated.valuations.CreditDefaultSwap['valuation_convention']>" => js_wire,
        /// Set the ISDA documentation clause for restructuring credit events.
        /// @param value - One of the four 2014 ISDA restructuring elections, a regional ISDA corporate default, or `"custom"`. If never set, the effective clause is derived from the CDS convention (see `CreditDefaultSwap.doc_clause_effective`).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        doc_clause as docClause: "NonNullable<generated.valuations.CreditDefaultSwap['doc_clause']>" => js_wire,
        /// Set the protection effective date for a forward-starting CDS.
        /// @param value - Date on which credit protection begins; must satisfy `premium.start <= value <= premium.end` as an ISO-8601 string.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        protection_effective_date as protectionEffectiveDate: "string" => js_date,
        /// Set the OTC margin (CSA / initial-margin) specification.
        /// @param value - Rust `OtcMarginSpec` in serde form (plain object or JSON string); cleared CDS use the `cleared` form, bilateral CDS need a SIMM credit classification.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        margin_spec as marginSpec: "NonNullable<generated.valuations.CreditDefaultSwap['margin_spec']>" => from_js_json,
});

common_setters!(JsCreditDefaultSwapBuilder, "CreditDefaultSwapBuilder");

handle_setters!(JsCreditDefaultSwapBuilder, "CreditDefaultSwapBuilder", {
        /// Set the notional amount.
        /// @param value - Notional amount, as a `Money` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        notional as notional => JsMoney,
});

upfront_setter!(JsCreditDefaultSwapBuilder, "CreditDefaultSwapBuilder");
