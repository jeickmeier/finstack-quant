//! Member parity for `Bond`, `TermLoan`, `RevolvingCredit` and
//! `AssetBackedFacility`, and their fluent builders.
//!
//! The classes themselves (constructors, `fromJson`, `toJson`) live in
//! [`crate::api::valuations::fixed_income`]; this module adds one getter per
//! public Rust field, the `Instrument` trait surface, `price` / `metric`, and
//! the `FinancialBuilder` setters.

use super::data::JsMertonMcConfig;
use crate::api::core::dates::{JsDayCount, JsTenor};
use crate::api::core::money::JsMoney;
use crate::api::valuations::fixed_income::{
    JsAssetBackedFacility, JsBond, JsRevolvingCredit, JsTermLoan,
};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_valuations::instruments::Instrument;
use wasm_bindgen::prelude::*;

instrument_builder_entry!(JsBond, "Bond", JsBondBuilder);

instrument_to_dict!(JsBond, "Bond");

instrument_market_dependencies!(JsBond, "Bond");

instrument_pricing!(JsBond, "Bond");

getters!(JsBond, "Bond", |i| {
        /// Principal amount of the bond.
        /// @returns Currency-tagged principal.
        notional as notional => money(i.notional),
        /// Issue date of the bond.
        /// @returns The contractual issue date.
        issue_date as issueDate => date(i.issue_date),
        /// Maturity (final redemption) date.
        /// @returns The contractual maturity date.
        maturity as maturity => date(i.maturity),
        /// Coupon/cashflow specification in serde form.
        /// @returns One-key plain object: `{"fixed": {...}}`, `{"floating": {...}}`, `{"step_up": {...}}` or `{"amortizing": {...}}`.
        cashflow_spec as cashflowSpec => json(i.cashflow_spec),
        /// Discount curve identifier.
        /// @returns Curve id used for discounting.
        discount_curve_id as discountCurveId => text(i.discount_curve_id),
        /// Hazard curve identifier for `hazard_rate` and `rates_credit` pricing.
        /// @returns Curve id, or `null` when no credit-consuming model is configured.
        credit_curve_id as creditCurveId => opt_text(i.credit_curve_id),
        /// Repo (financing) discount curve identifier.
        /// @returns Curve id, or `null`.
        repo_curve_id as repoCurveId => opt_text(i.repo_curve_id),
        /// Call/put schedule in serde form.
        /// @returns `{"calls": [...], "puts": [...]}` or `null` for a bullet bond.
        call_put as callPut => json(i.call_put),
        /// Return-floor specification (minimum MOIC / XIRR) in serde form.
        /// @returns The spec plain object, or `null`.
        return_floor as returnFloor => json(i.return_floor),
        /// Explicit cashflow schedule overriding generated coupons, in serde form.
        /// @returns The schedule plain object, or `null`.
        custom_cashflows as customCashflows => json(i.custom_cashflows),
        /// Accrual method (serde string).
        /// @returns `"linear"` unless overridden.
        accrual_method as accrualMethod => json(i.accrual_method),
        /// Settlement convention (settlement lag, ex-coupon period) in serde form.
        /// @returns `{"settlement_days": ..., "ex_coupon_days": ..., "ex_coupon_calendar_id": ...}` or `null`.
        settlement_convention as settlementConvention => json(i.settlement_convention),
        /// Settlement lag in business days.
        /// @returns The lag, or `null` when no settlement convention is set.
        settlement_days as settlementDays => json(i.settlement_days()),
        /// Ex-coupon period in business days.
        /// @returns The period, or `null` when no settlement convention is set.
        ex_coupon_days as exCouponDays => json(i.ex_coupon_days()),
        /// Whether coupons depend on forward-curve projection (FRNs).
        /// @returns `true` for floating (or amortizing-floating) cashflow specs.
        has_floating_coupons as hasFloatingCoupons => json(i.has_floating_coupons()),
        /// Canonical model key used when `model="default"` is passed to `price`.
        /// @returns Registered model key such as `"discounting"` or `"black76"`.
        default_model as defaultModel => text(Instrument::default_model(i)),
        /// Instrument attributes (tags and metadata) used for scenario selection.
        /// @returns The attribute bag; empty when none were set.
        attributes as attributes => json(i.attributes),
        /// Expiry date exposed by the Rust `Instrument` trait.
        /// @returns The expiry/maturity date, or `null` when the instrument type reports none.
        expiry as expiry => opt_date(Instrument::expiry(i)),
});

builder_class!(
    /// Fluent builder for `Bond`; one setter per Rust `FinancialBuilder` setter.
    ///
    /// Every setter returns the builder, so calls chain; `build()` validates
    /// and consumes it.
    JsBondBuilder,
    "BondBuilder",
    finstack_quant_valuations::instruments::fixed_income::bond::BondBuilder,
    JsBond,
    |b| b.build()
);

setters!(JsBondBuilder, "BondBuilder", {
        /// Set the instrument identifier.
        /// @param value - Instrument identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        id as id => id,
        /// Set the required contractual issue date.
        /// @param value - Required contractual issue date as an ISO-8601 string. `build()` throws when it is not set.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        issue_date as issueDate => date,
        /// Set the maturity date.
        /// @param value - Maturity date as an ISO-8601 string.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        maturity as maturity => date,
        /// Set the coupon/cashflow specification.
        /// @param value - Rust `CashflowSpec` in serde form (plain object or JSON string), e.g. `{"fixed": {"coupon_type": "cash", "rate": "0.05", "schedule": {...}}}` (copy `Bond.example().cashflow_spec`).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        cashflow_spec as cashflowSpec => json,
        /// Set the discount curve identifier.
        /// @param value - Discount curve identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        discount_curve_id as discountCurveId => id,
        /// Set the hazard curve identifier for `hazard_rate` and `rates_credit` pricing.
        /// @param value - Hazard curve identifier for scalar or joint rates-credit pricing.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        credit_curve_id as creditCurveId => id,
        /// Set the repo (financing) discount curve identifier.
        /// @param value - Repo (financing) discount curve identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        repo_curve_id as repoCurveId => id,
        /// Set the call/put schedule.
        /// @param value - Rust `CallPutSchedule` in serde form (plain object or JSON string), e.g. `{"calls": [{"start": "2027-01-15", "end": "2029-01-15", "price_pct_of_par": 100.0}], "puts": []}`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        call_put as callPut => json,
        /// Set the return-floor specification (minimum MOIC / XIRR on early redemption).
        /// @param value - Rust `ReturnFloorSpec` in serde form (plain object or JSON string), e.g. `Bond.example().min_moic(1.25).return_floor`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        return_floor as returnFloor => json,
        /// Set the explicit cashflow schedule that overrides generated coupons.
        /// @param value - Rust `CashFlowSchedule` in serde form (plain object or JSON string), e.g. the `custom_cashflows` value of a bond built from cashflows.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        custom_cashflows as customCashflows => json,
        /// Set the accrual method.
        /// @param value - Accrual method (serde string). `"linear"` is the default.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        accrual_method as accrualMethod => en,
        /// Set the settlement convention (settlement lag and ex-coupon period).
        /// @param value - Rust `BondSettlementConvention` in serde form (plain object or JSON string), e.g. `{"settlement_days": 2, "ex_coupon_days": 0, "ex_coupon_calendar_id": `null`}`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        settlement_convention as settlementConvention => json,
        /// Set instrument attributes (tags and metadata).
        /// @param value - Attribute bag; a plain object populates `meta` and an optional `"tags"` entry holding a list of strings populates `tags`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        attributes as attributes => json,
});

handle_setters!(JsBondBuilder, "BondBuilder", {
        /// Set the principal amount.
        /// @param value - Principal amount, as a `Money` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        notional as notional => JsMoney,
});

instrument_builder_entry!(JsTermLoan, "TermLoan", JsTermLoanBuilder);

instrument_to_dict!(JsTermLoan, "TermLoan");

instrument_market_dependencies!(JsTermLoan, "TermLoan");

instrument_pricing!(JsTermLoan, "TermLoan");

getters!(JsTermLoan, "TermLoan", |i| {
        /// Currency the loan is denominated in.
        /// @returns ISO-4217 currency code such as `"USD"`.
        currency as currency => text(i.currency),
        /// Committed notional (facility limit).
        /// @returns Currency-tagged commitment.
        notional_limit as notionalLimit => money(i.notional_limit),
        /// Issue / funding date.
        /// @returns The issue date.
        issue_date as issueDate => date(i.issue_date),
        /// Maturity (final redemption) date.
        /// @returns The contractual maturity date.
        maturity as maturity => date(i.maturity),
        /// Rate specification in serde form.
        /// @returns `{"fixed": {"rate": 0.06}}` (decimal rate) or `{"floating": {...}}`.
        rate as rate => json(i.rate),
        /// Payment frequency.
        /// @returns The payment tenor.
        frequency as frequency => tenor(i.frequency),
        /// Accrual day-count convention.
        /// @returns The day count.
        day_count as dayCount => day_count(i.day_count),
        /// Business day convention (serde string).
        /// @returns `"modified_following"` unless overridden.
        business_day_convention as businessDayConvention => json(i.business_day_convention),
        /// Holiday calendar identifier.
        /// @returns Calendar id, or `null`.
        calendar_id as calendarId => opt_text(i.calendar_id),
        /// Stub-period handling rule for the schedule.
        /// @returns Stub rule serde name (`"none"`, `"short_front"`, ...).
        stub as stub => json(i.stub),
        /// Discount curve identifier.
        /// @returns Curve id used for discounting.
        discount_curve_id as discountCurveId => text(i.discount_curve_id),
        /// Hazard curve identifier.
        /// @returns Curve id, or `null`.
        credit_curve_id as creditCurveId => opt_text(i.credit_curve_id),
        /// Amortization specification in serde form.
        /// @returns `"none"`, `{"percent_of_remaining_per_period": {"pct": 0.025}}`, `{"linear_between": {...}}`, ...
        amortization as amortization => json(i.amortization),
        /// Coupon type (serde string).
        /// @returns `"cash"`, `"pik"`, ...
        coupon_type as couponType => json(i.coupon_type),
        /// Upfront (arrangement or OID) fee paid on the issue date.
        /// @returns `UpfrontFee` serde plain object (`{"amount": `Money` plain object}` or `{"fraction_of_commitment": 0.02}`), or `null` when the loan has no upfront fee.
        upfront_fee as upfrontFee => json(i.upfront_fee),
        /// Delayed-draw term loan specification in serde form.
        /// @returns The spec plain object, or `null`.
        ddtl as ddtl => json(i.ddtl),
        /// Covenant event schedule in serde form.
        /// @returns The events plain object, or `null`.
        covenants as covenants => json(i.covenants),
        /// OID / effective-interest-rate specification in serde form.
        /// @returns The spec plain object, or `null`.
        oid_eir as oidEir => json(i.oid_eir),
        /// Prepayment (call) schedule in serde form.
        /// @returns The schedule plain object, or `null`.
        call_schedule as callSchedule => json(i.call_schedule),
        /// Settlement lag in business days.
        /// @returns The lag (Rust default 2).
        settlement_days as settlementDays => json(i.settlement_days),
        /// Canonical model key used when `model="default"` is passed to `price`.
        /// @returns Registered model key such as `"discounting"` or `"black76"`.
        default_model as defaultModel => text(Instrument::default_model(i)),
        /// Instrument attributes (tags and metadata) used for scenario selection.
        /// @returns The attribute bag; empty when none were set.
        attributes as attributes => json(i.attributes),
        /// Expiry date exposed by the Rust `Instrument` trait.
        /// @returns The expiry/maturity date, or `null` when the instrument type reports none.
        expiry as expiry => opt_date(Instrument::expiry(i)),
});

builder_class!(
    /// Fluent builder for `TermLoan`; one setter per Rust `FinancialBuilder` setter.
    ///
    /// Every setter returns the builder, so calls chain; `build()` validates
    /// and consumes it.
    JsTermLoanBuilder,
    "TermLoanBuilder",
    finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoanBuilder,
    JsTermLoan,
    |b| b.build()
);

setters!(JsTermLoanBuilder, "TermLoanBuilder", {
        /// Set the instrument identifier.
        /// @param value - Instrument identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        id as id => id,
        /// Set the loan currency.
        /// @param value - ISO-4217 currency code of the loan, e.g. `"USD"`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        currency as currency => en,
        /// Set the required contractual issue / funding date.
        /// @param value - Required contractual issue / funding date as an ISO-8601 string. `build()` throws when it is not set.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        issue_date as issueDate => date,
        /// Set the maturity date.
        /// @param value - Maturity date as an ISO-8601 string.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        maturity as maturity => date,
        /// Set the interest rate specification.
        /// @param value - `RateSpec` plain object or JSON string: `{"fixed": {"rate": 0.06}}` for a fixed all-in rate (`0.06` = 6%) or `{"floating": {...}}` with a `FloatingRateSpec`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        rate as rate => json,
        /// Set the business day convention.
        /// @param value - Business day convention (serde string). Default `"modified_following"`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        business_day_convention as businessDayConvention => en,
        /// Set the holiday calendar identifier (e.g. `"usny"`).
        /// @param value - Holiday calendar identifier (e.g. `"usny"`).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        calendar_id as calendarId => id,
        /// Set the stub rule.
        /// @param value - Stub rule (serde string). Default `"short_front"`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        stub as stub => en,
        /// Set the discount curve identifier.
        /// @param value - Discount curve identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        discount_curve_id as discountCurveId => id,
        /// Set the hazard curve identifier for credit-risky pricing.
        /// @param value - Hazard curve identifier for credit-risky pricing.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        credit_curve_id as creditCurveId => id,
        /// Set the amortization schedule.
        /// @param value - Amortization as the serde name of a unit variant (`"none"`) or an `AmortizationSpec` plain object / JSON string.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        amortization as amortization => json_or_name,
        /// Set the coupon type.
        /// @param value - Coupon type (serde string). `"cash"` (default), `"pik"`, ...
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        coupon_type as couponType => en,
        /// Set the upfront (arrangement or OID) fee paid on the issue date.
        /// @param value - `UpfrontFee` plain object or JSON string: `{"amount": Money}` for a cash fee or `{"fraction_of_commitment": 0.02}`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        upfront_fee as upfrontFee => json,
        /// Set the delayed-draw (DDTL) specification.
        /// @param value - Rust `DdtlSpec` in serde form (plain object or JSON string), e.g. `TermLoan.example_floating_with_ddtl().ddtl`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        ddtl as ddtl => json,
        /// Set the covenant event schedule.
        /// @param value - Rust `TermLoanCovenantEvents` in serde form (plain object or JSON string), e.g. `{"events": [...]}`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        covenants as covenants => json,
        /// Set the OID / effective-interest-rate specification.
        /// @param value - Rust `OidEirSpec` in serde form (plain object or JSON string), e.g. `{"issue_price_pct": 99.0, ...}`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        oid_eir as oidEir => json,
        /// Set the prepayment (call) schedule.
        /// @param value - Rust `LoanCallSchedule` in serde form (plain object or JSON string), e.g. `TermLoan.example_callable().call_schedule`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        call_schedule as callSchedule => json,
        /// Set the settlement lag in business days (default 2).
        /// @param value - Settlement lag.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        settlement_days as settlementDays => uint,
        /// Set instrument attributes (tags and metadata).
        /// @param value - Attribute bag; a plain object populates `meta` and an optional `"tags"` entry holding a list of strings populates `tags`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        attributes as attributes => json,
});

handle_setters!(JsTermLoanBuilder, "TermLoanBuilder", {
        /// Set the committed notional (facility limit).
        /// @param value - Committed notional (facility limit), as a `Money` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        notional_limit as notionalLimit => JsMoney,
        /// Set the payment frequency.
        /// @param value - Payment frequency, as a `Tenor` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        frequency as frequency => JsTenor,
        /// Set the accrual day-count convention.
        /// @param value - Accrual day-count convention, as a `DayCount` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        day_count as dayCount => JsDayCount,
});

instrument_builder_entry!(
    JsRevolvingCredit,
    "RevolvingCredit",
    JsRevolvingCreditBuilder
);

instrument_to_dict!(JsRevolvingCredit, "RevolvingCredit");

instrument_market_dependencies!(JsRevolvingCredit, "RevolvingCredit");

instrument_pricing!(JsRevolvingCredit, "RevolvingCredit");

getters!(JsRevolvingCredit, "RevolvingCredit", |i| {
        /// Total committed amount.
        /// @returns Currency-tagged commitment.
        commitment as commitment => money(i.commitment),
        /// Drawn balance at the simulation anchor (the later of the commitment and valuation dates), in both deterministic and stochastic mode.
        /// @returns Currency-tagged drawn balance.
        drawn as drawn => money(i.drawn),
        /// Date the facility becomes available.
        /// @returns The commitment date.
        issue_date as issueDate => date(i.issue_date),
        /// Expiry of the commitment.
        /// @returns The maturity date.
        maturity as maturity => date(i.maturity),
        /// Coupon specification as its `RateSpec` serde plain object.
        /// @returns `{"fixed": {"rate": r}}` with a decimal rate, or `{"floating": {...}}`.
        rate as rate => json(i.rate),
        /// Interest accrual day count.
        /// @returns The accrual convention.
        day_count as dayCount => day_count(i.day_count),
        /// Payment frequency for interest and fees.
        /// @returns The payment tenor.
        frequency as frequency => tenor(i.frequency),
        /// Fee structure as its serde plain object (including dated `steps`).
        /// @returns `upfront_fee`, `commitment_fee_tiers`, `usage_fee_tiers`, `facility_fee_bp` and `steps`.
        fees as fees => json(i.fees),
        /// Scheduled commitment changes as serde plain object rows (`date`, `amount`, `reduction_fee_bp`); empty when the commitment is flat.
        /// @returns One row per commitment step, in date order.
        commitment_steps as commitmentSteps => json(i.commitment_steps),
        /// Dated margin steps as serde plain object rows (`date`, `delta_bp`); empty when the margin is flat.
        /// @returns One row per margin step, in date order.
        margin_steps as marginSteps => json(i.margin_steps),
        /// Dated fixed fees as serde plain object rows (`date`, `amount`); empty when none are scheduled.
        /// @returns One row per scheduled fee, in date order.
        scheduled_fees as scheduledFees => json(i.scheduled_fees),
        /// Effective-interest-rate reporting switch (`{"include_fees": boolean}`), or `null` for the default (fees included).
        /// @returns The switch, or `null`.
        oid_eir as oidEir => json(i.oid_eir),
        /// Letter-of-credit sub-facility as its serde plain object (`sublimit`, `outstanding`, `events`, `fee_bp`, `fronting_fee_bp`, `leq`), or `null` without an LC sublimit.
        /// @returns The LC sub-facility, or `null`.
        lc as lc => json(i.lc),
        /// Draw/repay specification as its serde plain object.
        /// @returns `{"deterministic": [...]}` or `{"stochastic": {...}}`.
        draw_repay_spec as drawRepaySpec => json(i.draw_repay_spec),
        /// Discount curve identifier.
        /// @returns The curve id.
        discount_curve_id as discountCurveId => text(i.discount_curve_id),
        /// Credit (hazard) curve identifier, or `null`.
        /// @returns The curve id when the facility carries credit risk.
        credit_curve_id as creditCurveId => opt_text(i.credit_curve_id),
        /// Recovery rate on default, as a decimal in `[0, 1]`.
        /// @returns The recovery fraction.
        recovery_rate as recoveryRate => json(i.recovery_rate),
        /// Loan-equivalent exposure: fraction of the undrawn commitment drawn at default, as a decimal in `[0, 1]`.
        /// @returns The loan-equivalent exposure.
        leq as leq => json(i.leq),
        /// Stub rule for schedule generation.
        /// @returns The stub kind.
        stub as stub => json(i.stub),
        /// Business-day convention applied to payment dates (serde string, e.g. `"modified_following"`).
        /// @returns `"modified_following"` unless overridden.
        business_day_convention as businessDayConvention => json(i.business_day_convention),
        /// Holiday calendar identifier used for payment, fixing and settlement rolls, or `null` for weekends only.
        /// @returns The calendar identifier, or `null`.
        calendar_id as calendarId => opt_text(i.calendar_id),
        /// Business days between an accrual end and its payment date.
        /// @returns The payment lag; `0` pays on the adjusted accrual end.
        payment_lag_days as paymentLagDays => json(i.payment_lag_days),
        /// Business days from the valuation date to the settlement date used by quote metrics.
        /// @returns The settlement lag; `0` settles on the valuation date.
        settlement_days as settlementDays => json(i.settlement_days),
        /// Scenario-selection attributes.
        /// @returns The attribute map.
        attributes as attributes => json(i.attributes),
        /// Default pricing model key from the `Instrument` trait.
        /// @returns The model key.
        default_model as defaultModel => text(Instrument::default_model(i)),
        /// Expiry date exposed by the `Instrument` trait, or `null`.
        /// @returns The expiry date.
        expiry as expiry => opt_date(Instrument::expiry(i)),
});

builder_class!(
    /// Fluent builder for `RevolvingCredit`; one setter per Rust `FinancialBuilder` setter.
    ///
    /// Every setter returns the builder, so calls chain; `build()` validates
    /// and consumes it.
    JsRevolvingCreditBuilder,
    "RevolvingCreditBuilder",
    finstack_quant_valuations::instruments::fixed_income::revolving_credit::RevolvingCreditBuilder,
    JsRevolvingCredit,
    |b| b.build()
);

setters!(JsRevolvingCreditBuilder, "RevolvingCreditBuilder", {
        /// Set the instrument identifier.
        /// @param value - Unique identifier for the facility.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        id as id => id,
        /// Set the date the facility becomes available.
        /// @param value - Commitment date as an ISO-8601 string.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        issue_date as issueDate => date,
        /// Set the expiry of the commitment.
        /// @param value - Maturity date as an ISO-8601 string.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        maturity as maturity => date,
        /// Set the facility coupon.
        /// @param value - `RateSpec` plain object or JSON string: `{"fixed": {"rate": 0.06}}` for a fixed all-in rate (`0.06` = 6%) or `{"floating": {...}}` with a `FloatingRateSpec`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        rate as rate => json,
        /// Set the fee structure from its serde shape.
        /// @param value - `RevolvingCreditFees` as a plain object or JSON string (`upfront_fee` as `null`, `{"amount": `Money` plain object}` or `{"fraction_of_commitment": 0.02}`; `commitment_fee_tiers`, `usage_fee_tiers`, `facility_fee_bp` and the dated `steps` list of `{"date", "commitment_delta_bp", "usage_delta_bp", "facility_delta_bp"}` rows).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        fees as fees => json,
        /// Set the scheduled commitment changes.
        /// @param value - Rows of `{"date": "YYYY-MM-DD", "amount": `Money` plain object, "reduction_fee_bp": string}` (an array of plain objects or a JSON string), each the commitment in force from its date; `reduction_fee_bp` is the one-off reduction fee on a step down, a decimal string in basis points of the reduced amount (default `"0"`). Dates must be strictly increasing, after the commitment date and on or before maturity.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        commitment_steps as commitmentSteps => json,
        /// Set the dated fixed fees (amendment, waiver, extension, consent).
        /// @param value - Rows of `{"date": "YYYY-MM-DD", "amount": `Money` plain object}` (a array of plain objects or a JSON string), each paid on its date. Dates must lie after the commitment date and on or before maturity.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        scheduled_fees as scheduledFees => json,
        /// Set the effective-interest-rate reporting switch.
        /// @param value - `{"include_fees": boolean}` as a plain object or JSON string; `null` (the default) includes fees in the effective yield.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        oid_eir as oidEir => json,
        /// Set the letter-of-credit sub-facility.
        /// @param value - `LetterOfCreditSpec` as a plain object or JSON string with `sublimit` and `outstanding` (`Money` plain objects), `events` (rows of `{"date", "amount", "is_issue"}`), `fee_bp` (`null` accrues the floating margin), `fronting_fee_bp` and `leq` (the fraction of the LC face drawn at default). `null` removes the sublimit.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        lc as lc => json,
        /// Set the dated margin steps.
        /// @param value - Rows of `{"date": "YYYY-MM-DD", "delta_bp": string}` (a decimal string in basis points; a array of plain objects or a JSON string); each delta shifts the floating spread or the fixed rate from its date, cumulatively. Dates must be strictly increasing and strictly inside the facility life.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        margin_steps as marginSteps => json,
        /// Set the draw/repay specification from its serde shape.
        /// @param value - `DrawRepaySpec` as a plain object or JSON string: `{"deterministic": [{"date": ..., "amount": Money, "is_draw": boolean}, ...]}` or `{"stochastic": {"utilization_process": {...}, "use_sobol_qmc": ..., "mc_config": ...}}`. The estimator count, antithetic flag and seed label come from `instrument_pricing_overrides.model_config` (`mc_paths`, `mc_antithetic`, `mc_seed_scenario`).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        draw_repay_spec as drawRepaySpec => json,
        /// Set the discount curve identifier.
        /// @param value - Discount curve id in the market context.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        discount_curve_id as discountCurveId => id,
        /// Set (or clear) the credit curve identifier.
        /// @param value - Hazard curve id used for survival weighting; `null` prices without credit risk. A stochastic facility with a credit curve must use a market-anchored spread process on the same curve.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        credit_curve_id as creditCurveId => id,
        /// Set the recovery rate on default.
        /// @param value - Recovery fraction as a decimal in `[0, 1]`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        recovery_rate as recoveryRate => num,
        /// Set the loan-equivalent exposure drawn at default.
        /// @param value - Fraction of the undrawn commitment assumed drawn at default, as a decimal in `[0, 1]` (default `0.0`).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        leq as leq => num,
        /// Set the stub rule for schedule generation.
        /// @param value - Stub kind object or serde name (`"short_front"`, `"short_back"`, `"long_front"`, `"long_back"`, `"none"`); default `"short_front"`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        stub as stub => en,
        /// Set the business-day convention for payment dates.
        /// @param value - Convention object or serde name (`"modified_following"`, `"following"`, `"preceding"`, ...); default `"modified_following"`. Accrual boundaries stay unadjusted.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        business_day_convention as businessDayConvention => en,
        /// Set the holiday calendar used for payment, fixing and settlement rolls.
        /// @param value - Calendar identifier such as `"usny"`; `null` (the default) adjusts for weekends only. An unknown identifier fails at `build()`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        calendar_id as calendarId => id,
        /// Set the payment lag in business days after each accrual end.
        /// @param value - Business days on the facility calendar; `0` (the default) pays on the adjusted accrual end.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        payment_lag_days as paymentLagDays => uint,
        /// Set the settlement lag used by quote metrics.
        /// @param value - Business days from the valuation date to settlement; `0` (the default) settles on the valuation date. The base present value is always anchored at the valuation date.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        settlement_days as settlementDays => uint,
        /// Set scenario-selection attributes.
        /// @param value - Attribute map; `null` clears it.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        attributes as attributes => json,
});

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

instrument_builder_entry!(
    JsAssetBackedFacility,
    "AssetBackedFacility",
    JsAssetBackedFacilityBuilder
);

instrument_to_dict!(JsAssetBackedFacility, "AssetBackedFacility");

instrument_market_dependencies!(JsAssetBackedFacility, "AssetBackedFacility");

instrument_pricing!(JsAssetBackedFacility, "AssetBackedFacility");

getters!(JsAssetBackedFacility, "AssetBackedFacility", |i| {
        /// Collateral pool the facility lends against.
        /// @returns The pool (asset rows, rep lines or instrument collateral).
        collateral as collateral => json(i.collateral),
        /// Advance rates and concentration limits as their serde plain object.
        /// @returns `BorrowingBaseRules` with `advance_rates` and `concentration_limits`.
        borrowing_base_rules as borrowingBaseRules => json(i.borrowing_base_rules),
        /// Total commitment.
        /// @returns Currency-tagged commitment.
        commitment as commitment => money(i.commitment),
        /// Amount drawn at closing.
        /// @returns Currency-tagged drawn balance.
        drawn as drawn => money(i.drawn),
        /// Facility coupon as its `RateSpec` serde plain object.
        /// @returns `{"fixed": {"rate": r}}` with a decimal all-in rate, or `{"floating": {...}}` with a `FloatingRateSpec`.
        rate as rate => json(i.rate),
        /// Commitment fee on the undrawn commitment in basis points per annum.
        /// @returns Basis points per annum.
        commitment_fee_bp as commitmentFeeBp => dec(i.commitment_fee_bp),
        /// Closing date; the first payment date is one frequency later.
        /// @returns The closing date.
        closing_date as closingDate => date(i.closing_date),
        /// Scheduled end of the revolving period.
        /// @returns The scheduled revolving end.
        revolving_end as revolvingEnd => date(i.revolving_end),
        /// Final repayment date: the revolving end plus the term-out window, capped at maturity.
        /// @returns The repayment date.
        repayment_date as repaymentDate => date(i.repayment_date()),
        /// Legal final maturity.
        /// @returns The maturity date.
        maturity as maturity => date(i.maturity),
        /// Payment frequency of interest, fees and the borrowing-base test.
        /// @returns The payment frequency.
        frequency as frequency => tenor(i.frequency),
        /// Accrual day count of the facility interest and the commitment fee.
        /// @returns The accrual convention.
        day_count as dayCount => day_count(i.day_count),
        /// Payment calendar identifier, or `null`.
        /// @returns Holiday calendar id (e.g. `"nyse"`).
        calendar_id as calendarId => opt_text(i.calendar_id),
        /// Events that end revolving early, as `AmortizationEvent` serde plain objects.
        /// @returns Each `{"kind": "date", "date": ...}`, `{"kind": "cumulative_loss", "max_cumulative_loss": ...}` or `{"kind": "excess_spread", "min_excess_spread_3m": ...}`.
        amortization_events as amortizationEvents => json(i.amortization_events),
        /// Term-out window as its serde plain object (`{"months": ...}`), or `null`.
        /// @returns Months after the revolving end at which the collateral is liquidated.
        term_out as termOut => json(i.term_out),
        /// Collateral liquidation price at the term-out end in percent of par, or `null` for par.
        /// @returns Percent of par.
        liquidation_price_pct as liquidationPricePct => json(i.liquidation_price_pct),
        /// Transaction fees paid ahead of the facility's interest as the `DealFees` serde plain object.
        /// @returns `null` when the facility carries no fees.
        fees as fees => json(i.fees),
        /// Scheduled draws after closing as `DrawEvent` serde plain objects (`date`, `amount`).
        /// @returns Empty when the line is fully funded at closing.
        draws as draws => json(i.draws),
        /// Whether the line is re-advanced up to the borrowing base each revolving period.
        /// @returns `true` when re-advances apply.
        readvance_to_borrowing_base as readvanceToBorrowingBase => json(i.readvance_to_borrowing_base),
        /// Discount curve identifier.
        /// @returns The curve id.
        discount_curve_id as discountCurveId => text(i.discount_curve_id),
        /// Collateral behavior as its `CreditModelConfig` serde plain object.
        /// @returns Prepayment, default, recovery and delinquency assumptions.
        credit_model as creditModel => json(i.credit_model),
        /// Deterministic prepayment model of the collateral.
        /// @returns The typed spec.
        prepayment_spec as prepaymentSpec => json(i.credit_model.prepayment_spec),
        /// Deterministic default model of the collateral.
        /// @returns The typed spec.
        default_spec as defaultSpec => json(i.credit_model.default_spec),
        /// Recovery model of the collateral.
        /// @returns The typed spec.
        recovery_spec as recoverySpec => json(i.credit_model.recovery_spec),
        /// Scenario-selection attributes.
        /// @returns The attribute map.
        attributes as attributes => json(i.attributes),
        /// Default pricing model key from the `Instrument` trait (`"discounting"`).
        /// @returns The model key.
        default_model as defaultModel => text(Instrument::default_model(i)),
});

builder_class!(
    /// Fluent builder for `AssetBackedFacility`; one setter per Rust `FinancialBuilder` setter.
    ///
    /// Every setter returns the builder, so calls chain; `build()` validates
    /// and consumes it.
    JsAssetBackedFacilityBuilder,
    "AssetBackedFacilityBuilder",
    finstack_quant_valuations::instruments::fixed_income::asset_backed_facility::AssetBackedFacilityBuilder,
    JsAssetBackedFacility,
    |b| b.build().and_then(|facility| facility.validate().map(|()| facility))
);

setters!(JsAssetBackedFacilityBuilder, "AssetBackedFacilityBuilder", {
        /// Set the facility identifier.
        /// @param value - Stable facility identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        id as id => id,
        /// Set the collateral pool.
        /// @param value - Collateral pool (asset rows, rep lines or instrument collateral).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        collateral as collateral => json,
        /// Set the advance rates, eligibility and concentration limits.
        /// @param value - `BorrowingBaseRules` serde object: `advance_rates` (each with an `asset_class` wire name or `"*"`, a decimal `rate` and an optional `eligibility`) and `concentration_limits` (each with `scope` `"obligor"` / `"industry"` / `"asset_class"` and a percent `max_pct`).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        borrowing_base_rules as borrowingBaseRules => json,
        /// Set the facility coupon.
        /// @param value - `RateSpec` plain object or JSON string: `{"fixed": {"rate": 0.06}}` for a fixed all-in rate (`0.06` = 6%) or `{"floating": {...}}` with a `FloatingRateSpec`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        rate as rate => json,
        /// Set the commitment fee on the undrawn commitment.
        /// @param value - Basis points per annum (`50.0` = 0.50%).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        commitment_fee_bp as commitmentFeeBp => dec,
        /// Set the closing date.
        /// @param value - Closing date; the first payment date is one frequency later.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        closing_date as closingDate => date,
        /// Set the scheduled end of the revolving period.
        /// @param value - Revolving end; after it, collateral principal repays the facility.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        revolving_end as revolvingEnd => date,
        /// Set the legal final maturity.
        /// @param value - Legal final maturity.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        maturity as maturity => date,
        /// Set the payment calendar.
        /// @param value - Holiday calendar identifier (e.g. `"nyse"`); required for pricing.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        calendar_id as calendarId => id,
        /// Set the events that end revolving early.
        /// @param value - `AmortizationEvent` objects: `{"kind": "date", "date": ...}`, `{"kind": "cumulative_loss", "max_cumulative_loss": ...}` or `{"kind": "excess_spread", "min_excess_spread_3m": ...}`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        amortization_events as amortizationEvents => json,
        /// Set the term-out window after revolving.
        /// @param value - `TermOutSpec` plain object, e.g. `{"months": 24}`: the amortization term after the revolving period ends.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        term_out as termOut => json,
        /// Set the transaction fees paid through the waterfall ahead of the facility's interest.
        /// @param value - `DealFees` serde object (`trustee_fee` Money per annum, `senior_mgmt_fee_bp`, `subordinated_mgmt_fee_bp`, `servicing_fee_bp`, optional `master_servicer_fee_bp` / `workout_fee_pct` / `liquidation_fee_pct` / `special_servicer_fee_bp` / `incentive_fee`).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        fees as fees => json,
        /// Set the scheduled draws after closing.
        /// @param value - `DrawEvent` serde objects `{"date": "2025-01-01", "amount": {"amount": "10000000", "currency": "USD"}}`, ascending by date; each is applied on the first payment date at or after its date.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        draws as draws => json,
        /// Set whether the line is re-advanced up to the borrowing base each revolving period.
        /// @param value - `true` draws `min(commitment, borrowing base) − balance` every revolving period.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        readvance_to_borrowing_base as readvanceToBorrowingBase => flag,
        /// Set the collateral liquidation price at the term-out end.
        /// @param value - Percent of par (`100.0` = par).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        liquidation_price_pct as liquidationPricePct => num,
        /// Set the discount curve.
        /// @param value - Discount curve identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        discount_curve_id as discountCurveId => id,
        /// Replace the whole collateral behavior model.
        /// @param value - `CreditModelConfig` serde object; the per-field setters (`prepayment_spec` ...) then modify it.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        credit_model as creditModel => json,
        /// Set the deterministic prepayment model of the collateral.
        /// @param value - Typed spec or its serde form.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        prepayment_spec as prepaymentSpec => json,
        /// Set the deterministic default model of the collateral.
        /// @param value - Typed spec or its serde form.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        default_spec as defaultSpec => json,
        /// Set the recovery model of the collateral.
        /// @param value - Typed spec or its serde form.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        recovery_spec as recoverySpec => json,
        /// Set scenario-selection attributes.
        /// @param value - Attribute map; `null` clears it.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        attributes as attributes => json,
});

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
    /// @param discount_rate - Flat continuously compounded risk-free rate as a decimal (`0.04` = 4%).
    /// @param as_of - ISO-8601 valuation date.
    /// @returns `MertonMcResult` with clean and dirty price in percent of par, loss statistics and path statistics.
    /// @throws Error - Throws with kind `validation` if `asOf` is malformed or the bond or configuration is invalid, kind `invalid_type` if `discountRate` is not a number, and kind `computation` if the simulation fails.
    #[wasm_bindgen(js_name = priceMertonMc)]
    pub fn price_merton_mc(
        &self,
        config: &JsMertonMcConfig,
        discount_rate: JsValue,
        as_of: JsValue,
    ) -> Result<JsValue, JsValue> {
        let discount_rate = super::arg::num(&discount_rate, "discountRate")?;
        let as_of = super::as_of(&as_of)?;
        to_js_value(
            &self
                .inner
                .price_merton_mc(&config.inner, discount_rate, as_of)
                .map_err(to_js_err)?,
        )
    }
}
