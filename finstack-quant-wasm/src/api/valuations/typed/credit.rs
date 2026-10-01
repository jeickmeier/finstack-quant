//! Typed `CreditDefaultSwap`, `CdsIndex`, `CdsTranche` and `ConvertibleBond`
//! classes and their fluent builders.

use crate::api::core::dates::{JsDayCount, JsTenor};
use crate::api::core::money::JsMoney;
use crate::utils::input::js_string;
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_valuations::instruments::Instrument;
use wasm_bindgen::prelude::*;

/// The `upfront(paymentDate, amount)` setter shared by the CDS-family builders.
macro_rules! upfront_setter {
    ($bty:ident, $bjs:literal) => {
        #[wasm_bindgen(js_class = $bjs)]
        impl $bty {
            /// Set the upfront payment exchanged at inception.
            /// @param payment_date - Upfront payment date as an ISO-8601 string.
            /// @param amount - Upfront amount; positive when the protection buyer pays.
            /// @returns The builder, for chaining.
            /// @throws Error - Throws with kind `validation` if `paymentDate` is malformed or the builder was already consumed by `build()`, and kind `invalid_type` if `paymentDate` is not a string.
            pub fn upfront(
                &self,
                payment_date: JsValue,
                amount: &JsMoney,
            ) -> Result<$bty, JsValue> {
                let upfront = (
                    super::arg::date(&payment_date, "paymentDate")?,
                    amount.inner,
                );
                let inner = self.inner.apply(|b| b.upfront(upfront))?;
                Ok(Self { inner })
            }
        }
    };
}

instrument_class!(
    /// Typed wrapper for the Rust `CreditDefaultSwap` instrument (single-name CDS).
    JsCreditDefaultSwap,
    "CreditDefaultSwap",
    finstack_quant_valuations::instruments::CreditDefaultSwap
);

instrument_builder_entry!(
    JsCreditDefaultSwap,
    "CreditDefaultSwap",
    JsCreditDefaultSwapBuilder
);

instrument_to_dict!(JsCreditDefaultSwap, "CreditDefaultSwap");

instrument_market_dependencies!(JsCreditDefaultSwap, "CreditDefaultSwap");

instrument_pricing!(JsCreditDefaultSwap, "CreditDefaultSwap");

getters!(JsCreditDefaultSwap, "CreditDefaultSwap", |i| {
        /// Canonical model key used when `model="default"` is passed to `price`.
        /// @returns Registered model key such as `"hazard_rate"` or `"black76"`.
        default_model as defaultModel => text(Instrument::default_model(i)),
        /// Instrument attributes (tags and metadata) used for scenario selection.
        /// @returns The attribute bag; empty when none were set.
        attributes as attributes => json(i.attributes),
        /// Notional amount of protection.
        /// @returns Currency-tagged notional.
        notional as notional => money(i.notional),
        /// Protection perspective.
        /// @returns `"pay"` (buy protection) or `"receive"` (sell protection).
        side as side => json(i.side),
        /// ISDA regional convention (serde name).
        /// @returns `"isda_na"`, `"isda_eu"`, `"isda_as"` or `"custom"`.
        convention as convention => json(i.convention),
        /// Premium (fixed coupon) leg specification.
        /// @returns The premium leg.
        premium_leg as premiumLeg => json(i.premium_leg),
        /// Protection (default-contingent) leg specification.
        /// @returns The protection leg.
        protection_leg as protectionLeg => json(i.protection_leg),
        /// Valuation presentation convention (serde name).
        /// @returns `"bloomberg_cdsw_clean"` (default), `"bloomberg_cdsw_clean_full_premium"`, `"isda_dirty"` or `"quant_lib_isda_parity"`.
        valuation_convention as valuationConvention => json(i.valuation_convention),
        /// Points-upfront payment as `(payment_date, amount)`; positive means the protection buyer pays.
        /// @returns The upfront pair, or `null` when the trade has no upfront.
        upfront as upfront => dated_money(i.upfront),
        /// Explicit ISDA documentation clause (serde name).
        /// @returns The clause, or `null` when derived from the convention.
        doc_clause as docClause => json(i.doc_clause),
        /// Effective documentation clause after convention-based resolution (mirrors Rust `doc_clause_effective`).
        /// @returns `"xr14"` for `isda_na` / `isda_as` / `custom`, `"mm14"` for `isda_eu`, or the explicit clause resolved to its 2014 variant.
        doc_clause_effective as docClauseEffective => json(i.doc_clause_effective()),
        /// Protection effective date for a forward-starting CDS.
        /// @returns The date, or `null` when protection starts with the premium leg.
        protection_effective_date as protectionEffectiveDate => opt_date(i.protection_effective_date),
        /// Date protection starts (mirrors Rust `protection_start`).
        /// @returns `protection_effective_date` when set, else the premium start date.
        protection_start as protectionStart => date(i.protection_start()),
        /// OTC margin specification in serde form.
        /// @returns The `OtcMarginSpec` plain object, or `null` for unmargined trades.
        margin_spec as marginSpec => json(i.margin_spec),
        /// Premium-leg end date as seen by the pricer.
        /// @returns The scheduled maturity, or `null`.
        expiry as expiry => opt_date(Instrument::expiry(i)),
});

factories!(JsCreditDefaultSwap, "CreditDefaultSwap", finstack_quant_valuations::instruments::CreditDefaultSwap, {
        /// Canonical 5-year USD 10,000,000 investment-grade payer CDS (mirrors Rust `CreditDefaultSwap::example`): `isda_na` convention, 100bp running spread, 40% recovery, curves `USD-OIS` / `CORP-HAZARD`, premium 2024-03-20 to 2029-03-20.
        /// @returns The example CDS.
        /// @throws Error - Throws if the canonical example fails validation (does not occur for a released build).
        example as example,
});

market_metrics!(JsCreditDefaultSwap, "CreditDefaultSwap", {
        /// Par spread implied by the market, in basis points (mirrors Rust `CreditDefaultSwap::par_spread`): the running spread at which the contract is worth zero under this CDS's valuation convention, premium schedule, discount curve, hazard curve and recovery assumption.
        /// @param market_json - Canonical market-context JSON (string or plain object) supplying the curves and quotes this instrument needs.
        /// @param as_of - ISO-8601 valuation date.
        /// @returns Par spread in basis points.
        /// @throws Error - Throws with kind `not_found` if required market data is missing, kind `validation` if the market JSON or `asOf` is malformed, and kind `computation` if the calculation fails.
        par_spread as parSpread,
});

builder_class!(
    /// Fluent builder for `CreditDefaultSwap`; one setter per Rust `FinancialBuilder` setter.
    ///
    /// Every setter returns the builder, so calls chain; `build()` validates
    /// and consumes it.
    JsCreditDefaultSwapBuilder,
    "CreditDefaultSwapBuilder",
    finstack_quant_valuations::instruments::credit_derivatives::cds::CreditDefaultSwapBuilder,
    JsCreditDefaultSwap,
    |b| b.build()
);

setters!(JsCreditDefaultSwapBuilder, "CreditDefaultSwapBuilder", {
        /// Set the instrument identifier.
        /// @param value - Unique identifier for the CDS.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        id as id => id,
        /// Set the protection buyer/seller perspective.
        /// @param value - `"pay"` to buy protection (pay premium), `"receive"` to sell protection (receive premium).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        side as side => en,
        /// Set the ISDA regional convention.
        /// @param value - `"isda_na"` is the SNAC / post-Big-Bang North American standard (ACT/360, quarterly IMM, T+3); `"isda_eu"` the European standard (T+1, TARGET2); `"isda_as"` Asian (ACT/365F, Tokyo); `"custom"` for a manually configured convention.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        convention as convention => en,
        /// Set the premium leg specification.
        /// @param value - Premium leg specification.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        premium_leg as premiumLeg => json,
        /// Set the protection leg specification.
        /// @param value - Protection leg specification.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        protection_leg as protectionLeg => json,
        /// Set the valuation presentation convention.
        /// @param value - `"bloomberg_cdsw_clean"` (default) reports Bloomberg CDSW clean principal; `"isda_dirty"` the academic ISDA dirty PV; `"quant_lib_isda_parity"` reproduces QuantLib `IsdaCdsEngine`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        valuation_convention as valuationConvention => en,
        /// Set the ISDA documentation clause for restructuring credit events.
        /// @param value - One of the four 2014 ISDA restructuring elections, a regional ISDA corporate default, or `"custom"`. If never set, the effective clause is derived from the CDS convention (see `CreditDefaultSwap.doc_clause_effective`).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        doc_clause as docClause => en,
        /// Set the protection effective date for a forward-starting CDS.
        /// @param value - Date on which credit protection begins; must satisfy `premium.start <= value <= premium.end` as an ISO-8601 string.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        protection_effective_date as protectionEffectiveDate => date,
        /// Set the OTC margin (CSA / initial-margin) specification.
        /// @param value - Rust `OtcMarginSpec` in serde form (plain object or JSON string); cleared CDS use the `cleared` form, bilateral CDS need a SIMM credit classification.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        margin_spec as marginSpec => json,
        /// Set instrument attributes (tags and metadata).
        /// @param value - Attribute bag; a plain object populates `meta` and an optional `"tags"` entry holding a list of strings populates `tags`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        attributes as attributes => json,
});

handle_setters!(JsCreditDefaultSwapBuilder, "CreditDefaultSwapBuilder", {
        /// Set the notional amount.
        /// @param value - Notional amount, as a `Money` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        notional as notional => JsMoney,
});

instrument_class!(
    /// Typed wrapper for the Rust `CdsIndex` instrument (CDX / iTraxx style index swap).
    JsCdsIndex,
    "CdsIndex",
    finstack_quant_valuations::instruments::CdsIndex
);

instrument_builder_entry!(JsCdsIndex, "CdsIndex", JsCdsIndexBuilder);

instrument_to_dict!(JsCdsIndex, "CdsIndex");

instrument_market_dependencies!(JsCdsIndex, "CdsIndex");

instrument_pricing!(JsCdsIndex, "CdsIndex");

getters!(JsCdsIndex, "CdsIndex", |i| {
        /// Canonical model key used when `model="default"` is passed to `price`.
        /// @returns Registered model key such as `"hazard_rate"` or `"black76"`.
        default_model as defaultModel => text(Instrument::default_model(i)),
        /// Instrument attributes (tags and metadata) used for scenario selection.
        /// @returns The attribute bag; empty when none were set.
        attributes as attributes => json(i.attributes),
        /// Ticker of the credit index family this contract references.
        /// @returns Index family ticker as supplied at construction, for example `"CDX.NA.IG"` or `"iTraxx Europe"`. The value is stored verbatim and is not normalised or validated against a registry.
        index_name as indexName => json(i.index_name),
        /// Roll series of the credit index, incremented each semi-annual roll.
        /// @returns Series number as an unsigned integer (for example `41` for CDX.NA.IG series 41). Higher numbers denote more recent on-the-run rolls.
        series as series => json(i.series),
        /// Version within the series.
        /// @returns The version number.
        version as version => json(i.version),
        /// Traded notional of the index position, carrying its own currency.
        /// @returns Currency-tagged notional in the index deal currency (USD for CDX, EUR for iTraxx). It is the full original notional and is not scaled by the index factor; apply `index_factor` to obtain the current outstanding amount.
        notional as notional => money(i.notional),
        /// Fraction of surviving notional.
        /// @returns `1.0` when no constituent has defaulted since inception.
        index_factor as indexFactor => json(i.index_factor),
        /// Protection perspective.
        /// @returns `"pay"` (buy protection) or `"receive"` (sell protection).
        side as side => json(i.side),
        /// Regional ISDA convention (serde name).
        /// @returns `"isda_na"`, `"isda_eu"`, `"isda_as"` or `"custom"`.
        convention as convention => json(i.convention),
        /// Premium leg specification.
        /// @returns The premium leg.
        premium_leg as premiumLeg => json(i.premium_leg),
        /// Protection leg specification.
        /// @returns The protection leg.
        protection_leg as protectionLeg => json(i.protection_leg),
        /// Pricing aggregation mode.
        /// @returns `"single_curve"` or `"constituents"`.
        pricing as pricing => json(i.pricing),
        /// Constituent rows.
        /// @returns Typed rows; empty in `single_curve` mode.
        constituents as constituents => json(i.constituents),
        /// Number of names in the pool.
        /// @returns The count, or `null` when unset.
        num_constituents as numConstituents => json(i.num_constituents),
        /// Contractual upfront payment as `(payment_date, amount)`; positive means the protection buyer pays.
        /// @returns The upfront pair, or `null` when the trade has no upfront.
        upfront as upfront => dated_money(i.upfront),
        /// OTC margin specification in serde form.
        /// @returns The `OtcMarginSpec` plain object, or `null` for unmargined trades.
        margin_spec as marginSpec => json(i.margin_spec),
        /// Premium-leg end date as seen by the pricer.
        /// @returns The scheduled maturity, or `null`.
        expiry as expiry => opt_date(Instrument::expiry(i)),
});

factories!(JsCdsIndex, "CdsIndex", finstack_quant_valuations::instruments::CdsIndex, {
        /// Canonical CDX.NA.IG series 42 USD 10,000,000 payer (mirrors Rust `CdsIndex::example`): 60bp running coupon, `single_curve` pricing off `CDX.NA.IG.HAZARD` discounted on `USD-OIS`, premium 2024-03-20 to 2029-12-20, 125 names.
        /// @returns The example index trade.
        /// @throws Error - Throws if the canonical example fails validation (does not occur for a released build).
        example as example,
});

market_metrics!(JsCdsIndex, "CdsIndex", {
        /// Par spread of the index in basis points (mirrors Rust `CdsIndex::par_spread`; risky-annuity denominator in `single_curve` mode, weighted constituents otherwise).
        /// @param market_json - Canonical market-context JSON (string or plain object) supplying the curves and quotes this instrument needs.
        /// @param as_of - ISO-8601 valuation date.
        /// @returns Par spread in basis points.
        /// @throws Error - Throws with kind `not_found` if required market data is missing, kind `validation` if the market JSON or `asOf` is malformed, and kind `computation` if the calculation fails.
        par_spread as parSpread,
        /// Risky PV01 (risky annuity) of the premium leg (mirrors Rust `CdsIndex::risky_pv01`): PV of 1bp running on the surviving notional.
        /// @param market_json - Canonical market-context JSON (string or plain object) supplying the curves and quotes this instrument needs.
        /// @param as_of - ISO-8601 valuation date.
        /// @returns Risky PV01 in notional currency units per basis point.
        /// @throws Error - Throws with kind `not_found` if required market data is missing, kind `validation` if the market JSON or `asOf` is malformed, and kind `computation` if the calculation fails.
        risky_pv01 as riskyPv01,
});

builder_class!(
    /// Fluent builder for `CdsIndex`; one setter per Rust `FinancialBuilder` setter.
    ///
    /// Every setter returns the builder, so calls chain; `build()` validates
    /// and consumes it.
    JsCdsIndexBuilder,
    "CdsIndexBuilder",
    finstack_quant_valuations::instruments::credit_derivatives::cds_index::CdsIndexBuilder,
    JsCdsIndex,
    |b| b.build()
);

setters!(JsCdsIndexBuilder, "CdsIndexBuilder", {
        /// Set the instrument identifier.
        /// @param value - Unique identifier for the index trade.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        id as id => id,
        /// Set the index name.
        /// @param value - Index name, e.g. `"CDX.NA.IG"`, `"CDX.NA.HY"`, `"iTraxx Europe"`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        index_name as indexName => id,
        /// Set the series number.
        /// @param value - Series number, e.g. `42`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        series as series => uint,
        /// Set the version number within the series.
        /// @param value - Version number, e.g. `1`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        version as version => uint,
        /// Set the index factor (fraction of surviving notional).
        /// @param value - Index factor in `[0.0, 1.0]`; `1.0` means no constituent has defaulted since series inception.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        index_factor as indexFactor => num,
        /// Set the protection buyer/seller perspective.
        /// @param value - `"pay"` to buy protection (pay premium), `"receive"` to sell protection (receive premium).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        side as side => en,
        /// Set the ISDA regional convention.
        /// @param value - `"isda_na"` is the SNAC / post-Big-Bang North American standard; `"isda_eu"` European; `"isda_as"` Asian; `"custom"` for a manually configured convention.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        convention as convention => en,
        /// Set the premium leg specification.
        /// @param value - Premium leg specification (coupon schedule and discounting).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        premium_leg as premiumLeg => json,
        /// Set the protection leg specification.
        /// @param value - Protection leg specification (credit curve and settlement).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        protection_leg as protectionLeg => json,
        /// Set the pricing aggregation mode.
        /// @param value - `"single_curve"` prices the index against a single index hazard curve (synthetic CDS). `"constituents"` prices each issuer separately and aggregates by weight; requires `CdsIndexBuilder.constituents` to be set.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        pricing as pricing => en,
        /// Set the index constituents.
        /// @param value - Constituent rows as typed `CdsIndexConstituent` objects, plain objects with `credit` (`reference_entity`, `recovery_rate`, `credit_curve_id`), `weight` and optional `defaulted`, or a JSON array of the same shape.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        constituents as constituents => json,
        /// Set the number of reference entities in the index pool.
        /// @param value - Number of names in the index pool, e.g. `125` for CDX.NA.IG; required for portfolio-level analytics (e.g. jump-to-default) when `constituents` is empty.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        num_constituents as numConstituents => uint,
        /// Set the OTC margin (CSA / initial-margin) specification.
        /// @param value - Rust `OtcMarginSpec` in serde form (plain object or JSON string).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        margin_spec as marginSpec => json,
        /// Set instrument attributes (tags and metadata).
        /// @param value - Attribute bag; a plain object populates `meta` and an optional `"tags"` entry holding a list of strings populates `tags`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        attributes as attributes => json,
});

handle_setters!(JsCdsIndexBuilder, "CdsIndexBuilder", {
        /// Set the notional amount of the index.
        /// @param value - Notional amount of the index, as a `Money` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        notional as notional => JsMoney,
});

instrument_class!(
    /// Typed wrapper for the Rust `CdsTranche` instrument (synthetic index tranche).
    JsCdsTranche,
    "CdsTranche",
    finstack_quant_valuations::instruments::CdsTranche
);

instrument_builder_entry!(JsCdsTranche, "CdsTranche", JsCdsTrancheBuilder);

instrument_to_dict!(JsCdsTranche, "CdsTranche");

instrument_market_dependencies!(JsCdsTranche, "CdsTranche");

instrument_pricing!(JsCdsTranche, "CdsTranche");

getters!(JsCdsTranche, "CdsTranche", |i| {
        /// Canonical model key used when `model="default"` is passed to `price`.
        /// @returns Registered model key such as `"hazard_rate"` or `"black76"`.
        default_model as defaultModel => text(Instrument::default_model(i)),
        /// Instrument attributes (tags and metadata) used for scenario selection.
        /// @returns The attribute bag; empty when none were set.
        attributes as attributes => json(i.attributes),
        /// Underlying index name.
        /// @returns The index name.
        index_name as indexName => json(i.index_name),
        /// Index series number.
        /// @returns The series number.
        series as series => json(i.series),
        /// Attachment point in percent.
        /// @returns Attachment (`3.0` = 3%).
        attach_pct as attachPct => json(i.attach_pct),
        /// Detachment point in percent.
        /// @returns Detachment (`7.0` = 7%).
        detach_pct as detachPct => json(i.detach_pct),
        /// Tranche notional.
        /// @returns Currency-tagged notional.
        notional as notional => money(i.notional),
        /// Scheduled maturity.
        /// @returns The maturity date.
        maturity as maturity => date(i.maturity),
        /// Contractual running coupon (bp) paid on the tranche premium leg.
        /// @returns Coupon quoted in basis points per annum on the outstanding tranche notional (for example `100.0` for a 100 bp coupon), not as a decimal rate. Accrues on the premium-leg day count.
        coupon_bp as couponBp => json(i.coupon_bp),
        /// Payment frequency.
        /// @returns The coupon tenor (typically quarterly).
        frequency as frequency => tenor(i.frequency),
        /// Day count convention.
        /// @returns Act/360 for standard tranches.
        day_count as dayCount => day_count(i.day_count),
        /// Business day convention (serde name).
        /// @returns `"modified_following"` unless set otherwise.
        business_day_convention as businessDayConvention => json(i.business_day_convention),
        /// Holiday calendar identifier.
        /// @returns The calendar id, or `null`.
        calendar_id as calendarId => opt_text(i.calendar_id),
        /// Discount curve identifier.
        /// @returns The curve id.
        discount_curve_id as discountCurveId => text(i.discount_curve_id),
        /// Credit index identifier for the loss distribution.
        /// @returns The credit index id.
        credit_index_id as creditIndexId => text(i.credit_index_id),
        /// Direction of the tranche position from the holder's perspective.
        /// @returns Serde string, either `"pay"` (buys protection: pays the running coupon and receives tranche loss payments) or `"receive"` (sells protection: receives the coupon and pays losses), matching the CDS and CDS index `side`.
        side as side => json(i.side),
        /// Explicit contract start (effective) date for schedule anchoring.
        /// @returns The date, or `null`.
        start_date as startDate => opt_date(i.start_date),
        /// Realized portfolio loss so far.
        /// @returns Fraction of the original portfolio notional.
        realized_loss as realizedLoss => json(i.realized_loss),
        /// Coupon roll-date grid.
        /// @returns `"cds_imm"` for the CDS roll dates (20th of Mar/Jun/Sep/Dec); `"none"` for a bespoke `frequency`/`stub` schedule.
        roll_rule as rollRule => json(i.roll_rule),
        /// Stub convention for a bespoke (`"none"`) coupon schedule.
        /// @returns The stub rule (`short_front` by default).
        stub as stub => json(i.stub),
        /// Upfront payment as `(payment_date, amount)`.
        /// @returns The pair, or `null`.
        upfront as upfront => dated_money(i.upfront),
        /// Maturity as seen by the pricer.
        /// @returns The maturity, or `null`.
        expiry as expiry => opt_date(Instrument::expiry(i)),
});

factories!(JsCdsTranche, "CdsTranche", finstack_quant_valuations::instruments::CdsTranche, {
        /// Canonical CDX.NA.IG 42 equity (0–3%) tranche, USD 10,000,000 (mirrors Rust `CdsTranche::example`): buy protection, 100bp running, maturity 2029-12-20, curves `USD-OIS` / `CDX.NA.IG.HAZARD`.
        /// @returns The example tranche.
        /// @throws Error - Throws if the canonical example fails validation (does not occur for a released build).
        example as example,
});

market_metrics!(JsCdsTranche, "CdsTranche", {
        /// Jump-to-default exposure (mirrors Rust `CdsTranche::jump_to_default`): PV impact of one constituent defaulting immediately.
        /// @param market_json - Canonical market-context JSON (string or plain object) supplying the curves and quotes this instrument needs.
        /// @param as_of - ISO-8601 valuation date.
        /// @returns Jump-to-default PV change in notional currency units.
        /// @throws Error - Throws with kind `not_found` if required market data is missing, kind `validation` if the market JSON or `asOf` is malformed, and kind `computation` if the calculation fails.
        jump_to_default as jumpToDefault,
});

builder_class!(
    /// Fluent builder for `CdsTranche`; one setter per Rust `FinancialBuilder` setter.
    ///
    /// Every setter returns the builder, so calls chain; `build()` validates
    /// and consumes it.
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
        id as id => id,
        /// Set the underlying index name.
        /// @param value - Index name, e.g. `"CDX.NA.IG"`, `"CDX.NA.HY"`, `"iTraxx EUR"`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        index_name as indexName => id,
        /// Set the series number.
        /// @param value - Series number, e.g. `42`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        series as series => uint,
        /// Set the attachment point.
        /// @param value - Attachment point quoted in percent (`0.0` for equity; `3.0` for a tranche attaching at 3%).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        attach_pct as attachPct => num,
        /// Set the detachment point.
        /// @param value - Detachment point quoted in percent (`3.0` for a 0-3% tranche).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        detach_pct as detachPct => num,
        /// Set the maturity date of the tranche.
        /// @param value - Maturity date as an ISO-8601 string.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        maturity as maturity => date,
        /// Set the running coupon.
        /// @param value - Running coupon in basis points (`100.0` = 1.00%).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        coupon_bp as couponBp => num,
        /// Set the business day convention for coupon dates.
        /// @param value - Roll rule (`"modified_following"` when never set).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        business_day_convention as businessDayConvention => en,
        /// Set the holiday calendar identifier.
        /// @param value - Holiday calendar identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        calendar_id as calendarId => id,
        /// Set the discount curve identifier (by quote currency).
        /// @param value - Discount curve identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        discount_curve_id as discountCurveId => id,
        /// Set the credit index identifier for survival/loss modeling.
        /// @param value - Credit index identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        credit_index_id as creditIndexId => id,
        /// Set the tranche side (`"pay"` buys protection, `"receive"` sells it).
        /// @param value - Tranche side.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        side as side => en,
        /// Set the contract start (effective) date for schedule anchoring.
        /// @param value - Effective date; if never set, uses the as-of date (or the prior CDS roll date when `roll_rule` is `"cds_imm"`).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        start_date as startDate => date,
        /// Set the realized (settled) loss.
        /// @param value - Realized loss as a fraction of the original portfolio notional; `0.0` when never set.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        realized_loss as realizedLoss => num,
        /// Set the coupon roll-date grid.
        /// @param value - `"cds_imm"` for the CDS roll dates (20th of Mar, Jun, Sep, Dec) or `"none"` (the default) for a schedule from `frequency` and `stub`; `"imm"` is rejected by `build()`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        roll_rule as rollRule => en,
        /// Set the stub convention for a bespoke coupon schedule.
        /// @param value - Stub rule used when `roll_rule` is `"none"`; `"short_front"` when never set.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        stub as stub => en,
        /// Set instrument attributes (tags and metadata).
        /// @param value - Attribute bag; a plain object populates `meta` and an optional `"tags"` entry holding a list of strings populates `tags`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        attributes as attributes => json,
});

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

instrument_class!(
    /// Typed wrapper for the Rust `ConvertibleBond` instrument.
    JsConvertibleBond,
    "ConvertibleBond",
    finstack_quant_valuations::instruments::ConvertibleBond
);

instrument_builder_entry!(
    JsConvertibleBond,
    "ConvertibleBond",
    JsConvertibleBondBuilder
);

instrument_to_dict!(JsConvertibleBond, "ConvertibleBond");

instrument_market_dependencies!(JsConvertibleBond, "ConvertibleBond");

instrument_pricing!(JsConvertibleBond, "ConvertibleBond");

getters!(JsConvertibleBond, "ConvertibleBond", |i| {
        /// Canonical model key used when `model="default"` is passed to `price`.
        /// @returns Registered model key such as `"hazard_rate"` or `"black76"`.
        default_model as defaultModel => text(Instrument::default_model(i)),
        /// Instrument attributes (tags and metadata) used for scenario selection.
        /// @returns The attribute bag; empty when none were set.
        attributes as attributes => json(i.attributes),
        /// Principal amount.
        /// @returns Currency-tagged principal.
        notional as notional => money(i.notional),
        /// Dated date from which the bond starts accruing interest.
        /// @returns Calendar date, unadjusted for business days. It anchors the coupon schedule and the first accrual period.
        issue_date as issueDate => date(i.issue_date),
        /// Scheduled redemption date on which principal is repaid.
        /// @returns Unadjusted calendar maturity; payment dates derived from it are rolled by the instrument's business-day convention.
        maturity as maturity => date(i.maturity),
        /// Discount curve identifier for the debt component.
        /// @returns The curve id.
        discount_curve_id as discountCurveId => text(i.discount_curve_id),
        /// Issuer hazard curve identifier.
        /// @returns The hazard curve id, or `null` (the cash component discounts at the risk-free curve).
        credit_curve_id as creditCurveId => opt_text(i.credit_curve_id),
        /// Conversion terms.
        /// @returns The typed conversion spec.
        conversion as conversion => json(i.conversion),
        /// Base conversion ratio (shares per bond), derived from ratio or price (mirrors Rust `conversion_ratio`).
        /// @returns The ratio, or `null` when neither ratio nor price is set.
        conversion_ratio as conversionRatio => json(i.conversion_ratio()),
        /// Conversion ratio after anti-dilution adjustments (mirrors Rust `effective_conversion_ratio`).
        /// @returns The adjusted ratio, or `null`.
        effective_conversion_ratio as effectiveConversionRatio => json(i.effective_conversion_ratio()),
        /// Market-scalar id of the underlying share price.
        /// @returns The `MarketContext` price id read for the equity spot.
        spot_id as spotId => text(i.spot_id),
        /// Equity volatility id: a volatility surface, or a unitless scalar holding a flat volatility.
        /// @returns The volatility id.
        vol_surface_id as volSurfaceId => text(i.vol_surface_id),
        /// Unitless continuous dividend-yield scalar id.
        /// @returns The id, or `null` for a zero dividend yield.
        div_yield_id as divYieldId => opt_text(i.div_yield_id),
        /// Call/put schedule.
        /// @returns The typed schedule, or `null`.
        call_put as callPut => json(i.call_put),
        /// Soft-call trigger (`threshold_pct`, `observation_days`, `required_days_above`).
        /// @returns The trigger plain object, or `null`.
        soft_call_trigger as softCallTrigger => json(i.soft_call_trigger),
        /// Settlement lag in business days.
        /// @returns The lag, or `null` for same-day.
        settlement_days as settlementDays => json(i.settlement_days),
        /// Assumed recovery rate on default as a fraction.
        /// @returns The recovery, or `null`.
        recovery_rate as recoveryRate => json(i.recovery_rate),
        /// Coupon/cashflow specification in serde form, the same shape as `Bond.cashflow_spec`.
        /// @returns `{"fixed": {...}}`, `{"floating": {...}}`, `{"step_up": {...}}` or `{"amortizing": {...}}`; a zero-coupon convertible is a fixed spec with rate `"0"`.
        cashflow_spec as cashflowSpec => json(i.cashflow_spec),
        /// Maturity as seen by the pricer.
        /// @returns The maturity, or `null`.
        expiry as expiry => opt_date(Instrument::expiry(i)),
});

factories!(JsConvertibleBond, "ConvertibleBond", finstack_quant_valuations::instruments::ConvertibleBond, {
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
        id as id => id,
        /// Set the issue date.
        /// @param value - Issue date as an ISO-8601 string.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        issue_date as issueDate => date,
        /// Set the maturity date.
        /// @param value - Maturity date as an ISO-8601 string.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        maturity as maturity => date,
        /// Set the discount curve identifier for the debt component.
        /// @param value - Discount curve identifier (risk-free or funding).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        discount_curve_id as discountCurveId => id,
        /// Set the issuer hazard curve identifier.
        /// @param value - `HazardCurve` identifier; the zero-recovery risky discount factor is the risk-free discount factor times its survival probability. If never set, the cash component discounts at the risk-free curve (no credit spread). Requires an explicit `recovery_rate`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        credit_curve_id as creditCurveId => id,
        /// Set the conversion terms.
        /// @param value - Typed `ConversionSpec`, a plain object, or a JSON object string with `ratio`, `price`, `policy`, `anti_dilution`, `dividend_adjustment` and `dilution_events`; at least one of `ratio` / `price` must be set.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        conversion as conversion => json,
        /// Set the market-scalar id of the underlying share price (required).
        /// @param value - `MarketContext` price id holding the share price in the bond's currency, or a unitless level.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        spot_id as spotId => id,
        /// Set the equity volatility id (required).
        /// @param value - Volatility surface id read at the conversion strike and maturity, or the id of a unitless scalar holding a flat volatility.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        vol_surface_id as volSurfaceId => id,
        /// Set the continuous dividend-yield scalar id.
        /// @param value - Id of a unitless decimal dividend yield (`0.02` = 2%). When never set the dividend yield is zero; a set id must exist in the market.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        div_yield_id as divYieldId => id,
        /// Set the call/put schedule.
        /// @param value - Typed `CallPutSchedule`, a plain object, or a JSON object string with `calls` and `puts` arrays of windows.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        call_put as callPut => json,
        /// Set the soft-call trigger condition.
        /// @param value - `PriceTrigger` as a plain object or JSON object string with `threshold_pct` (percent of conversion price, e.g. `130.0`), `observation_days` and `required_days_above`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        soft_call_trigger as softCallTrigger => json,
        /// Set the settlement lag.
        /// @param value - Business days from trade date to settlement (e.g. `2` for US corporate convertibles); same-day when never set.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        settlement_days as settlementDays => uint,
        /// Set the assumed recovery rate on default.
        /// @param value - Recovery rate as a fraction (`0.40` = 40%); only relevant when `credit_curve_id` is set.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        recovery_rate as recoveryRate => num,
        /// Set the coupon/cashflow specification.
        /// @param value - Rust `CashflowSpec` as a plain object or JSON object string (the same shape as `Bond.cashflow_spec`, e.g. `{"fixed": {"coupon_type": "cash", "rate": "0.05", ...schedule}}`); a zero-coupon convertible uses a fixed spec with rate `"0"`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        cashflow_spec as cashflowSpec => json,
        /// Set instrument attributes (tags and metadata).
        /// @param value - Attribute bag; a plain object populates `meta` and an optional `"tags"` entry holding a list of strings populates `tags`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        attributes as attributes => json,
});

handle_setters!(JsConvertibleBondBuilder, "ConvertibleBondBuilder", {
        /// Set the principal amount.
        /// @param value - Principal amount, as a `Money` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        notional as notional => JsMoney,
});

upfront_setter!(JsCreditDefaultSwapBuilder, "CreditDefaultSwapBuilder");
upfront_setter!(JsCdsIndexBuilder, "CdsIndexBuilder");
upfront_setter!(JsCdsTrancheBuilder, "CdsTrancheBuilder");

#[wasm_bindgen(js_class = CdsIndex)]
impl JsCdsIndex {
    /// Create an index swap from a standard series preset.
    ///
    /// Mirrors Rust `CdsIndex::from_preset`: the preset supplies the index
    /// name, series, version, running coupon and regional ISDA convention;
    /// the remaining arguments are the trade terms.
    /// @param preset - `CdsIndexParams` preset, e.g. from `valuations.instruments.cdsIndexParamsCdxNaIg`.
    /// @param id - Unique instrument identifier.
    /// @param notional - Index notional on which the premium and protection legs are computed.
    /// @param side - Protection direction: `pay` (buy protection) or `receive`.
    /// @param start - Effective date as an ISO-8601 string.
    /// @param end - Maturity date as an ISO-8601 string.
    /// @param recovery_rate - Assumed recovery as a decimal fraction of par (`0.4` = 40%).
    /// @param discount_curve_id - Discount curve identifier.
    /// @param credit_curve_id - Index hazard curve identifier.
    /// @returns The validated index swap.
    /// @throws Error - Throws with kind `validation` if `preset`, `side` or a date is malformed or the instrument fails validation, and kind `invalid_type` for a wrong argument type.
    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = fromPreset)]
    pub fn from_preset(
        preset: JsValue,
        id: JsValue,
        notional: &JsMoney,
        side: JsValue,
        start: JsValue,
        end: JsValue,
        recovery_rate: JsValue,
        discount_curve_id: JsValue,
        credit_curve_id: JsValue,
    ) -> Result<JsCdsIndex, JsValue> {
        let preset: finstack_quant_valuations::instruments::credit_derivatives::cds_index::CdsIndexParams =
            super::arg::json(&preset, "preset")?;
        finstack_quant_valuations::instruments::CdsIndex::from_preset(
            &preset,
            js_string(&id, "id")?,
            notional.inner,
            super::arg::en(&side, "side")?,
            super::arg::date(&start, "start")?,
            super::arg::date(&end, "end")?,
            super::arg::num(&recovery_rate, "recoveryRate")?,
            js_string(&discount_curve_id, "discountCurveId")?,
            js_string(&credit_curve_id, "creditCurveId")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Credit spread sensitivity of the index (mirrors Rust `CdsIndex::cs01`).
    ///
    /// Bumps the index par spread by one basis point, recalibrates the hazard
    /// curve and reprices.
    /// @param market_json - Canonical market-context JSON (string or plain object) supplying the discount and index hazard curves.
    /// @param as_of - ISO-8601 valuation date.
    /// @returns Change in present value for a one basis point spread widening, in the notional currency.
    /// @throws Error - Throws with kind `not_found` if a curve is missing, kind `validation` if the market JSON or `asOf` is malformed, and kind `computation` if recalibration or pricing fails.
    pub fn cs01(&self, market_json: JsValue, as_of: JsValue) -> Result<f64, JsValue> {
        let market = super::market(&market_json)?;
        let as_of = super::as_of(&as_of)?;
        let provider =
            finstack_quant_calibration::recalibration::CachedRecalibrationProvider::new();
        self.inner
            .cs01(&market, as_of, &provider)
            .map_err(to_js_err)
    }
}

#[wasm_bindgen(js_class = CdsTranche)]
impl JsCdsTranche {
    /// Create a standard index tranche (mirrors Rust `CdsTranche::standard`).
    ///
    /// Uses the standard quarterly, Act/360, following, CDS-IMM schedule.
    /// @param id - Unique instrument identifier.
    /// @param params - `CdsTrancheParams`, e.g. from `valuations.instruments.cdsTrancheParamsEquityTranche`.
    /// @param discount_curve_id - Discount curve identifier.
    /// @param credit_index_id - Credit index data identifier (index hazard curve and base correlation).
    /// @param side - Protection direction: `pay` (buy protection) or `receive`.
    /// @returns The validated tranche.
    /// @throws Error - Throws with kind `validation` if `params` or `side` is malformed or the tranche fails validation, and kind `invalid_type` for a wrong argument type.
    pub fn standard(
        id: JsValue,
        params: JsValue,
        discount_curve_id: JsValue,
        credit_index_id: JsValue,
        side: JsValue,
    ) -> Result<JsCdsTranche, JsValue> {
        let params: finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CdsTrancheParams =
            super::arg::json(&params, "params")?;
        finstack_quant_valuations::instruments::CdsTranche::standard(
            js_string(&id, "id")?,
            &params,
            js_string(&discount_curve_id, "discountCurveId")?,
            js_string(&credit_index_id, "creditIndexId")?,
            super::arg::en(&side, "side")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Expected tranche loss at maturity (mirrors Rust `CdsTranche::expected_loss`).
    /// @param market_json - Canonical market-context JSON (string or plain object) supplying the credit index data.
    /// @returns Expected loss of the tranche in the notional currency.
    /// @throws Error - Throws with kind `not_found` if the credit index data is missing, kind `validation` if the market JSON is malformed, and kind `computation` if the loss model fails.
    #[wasm_bindgen(js_name = expectedLoss)]
    pub fn expected_loss(&self, market_json: JsValue) -> Result<f64, JsValue> {
        let market = super::market(&market_json)?;
        self.inner.expected_loss(&market).map_err(to_js_err)
    }
}

#[wasm_bindgen(js_class = ConvertibleBond)]
impl JsConvertibleBond {
    /// Conversion parity (mirrors Rust `ConvertibleBond::parity`).
    /// @param market_json - Canonical market-context JSON (string or plain object) carrying the underlying spot under `spot_id`.
    /// @returns Value of the shares received on conversion, in the bond's currency.
    /// @throws Error - Throws with kind `not_found` if the spot is missing, and kind `validation` if the market JSON is malformed or the bond fails validation.
    pub fn parity(&self, market_json: JsValue) -> Result<f64, JsValue> {
        let market = super::market(&market_json)?;
        self.inner.parity(&market).map_err(to_js_err)
    }

    /// Conversion premium over parity (mirrors Rust `ConvertibleBond::conversion_premium`).
    /// @param market_json - Canonical market-context JSON (string or plain object) carrying the underlying spot under `spot_id`.
    /// @param bond_price - Bond price in currency units for the whole notional, on the same scale as parity.
    /// @returns Premium as a decimal fraction of parity (`0.25` = 25% above parity).
    /// @throws Error - Throws with kind `not_found` if the spot is missing, kind `validation` if the market JSON is malformed or the bond fails validation, and kind `invalid_type` if `bondPrice` is not a number.
    #[wasm_bindgen(js_name = conversionPremium)]
    pub fn conversion_premium(
        &self,
        market_json: JsValue,
        bond_price: JsValue,
    ) -> Result<f64, JsValue> {
        let market = super::market(&market_json)?;
        let bond_price = super::arg::num(&bond_price, "bondPrice")?;
        self.inner
            .conversion_premium(&market, bond_price)
            .map_err(to_js_err)
    }

    /// Tree Greeks of the convertible (mirrors Rust `ConvertibleBond::greeks`).
    ///
    /// Uses the default binomial tree; its step count is
    /// `instrument_pricing_overrides.model_config.tree_steps`.
    /// @param market_json - Canonical market-context JSON (string or plain object) supplying the curves, spot and volatility.
    /// @param as_of - ISO-8601 valuation date.
    /// @returns `ConvertibleGreeks` plain object with the price and its delta, gamma, vega, rho and theta.
    /// @throws Error - Throws with kind `not_found` if required market data is missing, kind `validation` if the market JSON or `asOf` is malformed, and kind `computation` if the tree fails.
    pub fn greeks(&self, market_json: JsValue, as_of: JsValue) -> Result<JsValue, JsValue> {
        let market = super::market(&market_json)?;
        let as_of = super::as_of(&as_of)?;
        to_js_value(&self.inner.greeks(&market, None, as_of).map_err(to_js_err)?)
    }
}
