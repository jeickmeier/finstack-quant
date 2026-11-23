//! Typed `TermLoan` and `TermLoanBuilder` WASM bindings.

use crate::api::core::dates::{JsDayCount, JsTenor};
use crate::api::core::money::JsMoney;
use finstack_quant_valuations::instruments::Instrument;
use wasm_bindgen::prelude::*;

instrument_class!(
    /// Typed wrapper for the canonical Rust TermLoan instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { core, valuations } from "finstack-quant-wasm";
    /// await init();
    /// const loan = valuations.instruments.TermLoan.example();
    /// const market = new core.MarketContext();
    /// market.insert(core.DiscountCurve.flat("USD-OIS", "2024-06-30", 0.03, "act_365f"));
    /// const result = valuations.instruments.priceInstrument(loan.toJson(), market.toJson(), "2024-06-30", "default");
    /// console.log(result.value.amount, result.value.currency);
    /// ```
    JsTermLoan, "TermLoan", finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan
);

factories!(JsTermLoan, "TermLoan", finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan, {
        /// Canonical example term loan (mirrors Rust `TermLoan::example`).
        ///
        /// Returns a 5-year USD fixed-rate loan (6%, quarterly, Act/360, 2.5%
        /// per-period amortization) useful as a starting point and in tests.
        /// @returns The example loan.
        /// @throws If construction fails (should not occur).
        example as example,
        /// Canonical example floating-rate term loan with a delayed-draw tranche
        /// (mirrors Rust `TermLoan::example_floating_with_ddtl`).
        /// @returns The example loan.
        /// @throws Error - Throws if the canonical example fails validation (does not occur for a released build).
        example_floating_with_ddtl as exampleFloatingWithDdtl,
        /// Canonical example callable term loan (mirrors Rust `TermLoan::example_callable`).
        /// @returns The example loan.
        /// @throws Error - Throws if the canonical example fails validation (does not occur for a released build).
        example_callable as exampleCallable,
});

instrument_surface!(
    JsTermLoan,
    "TermLoan",
    JsTermLoanBuilder,
    "generated.valuations.TermLoan"
);

getters!(JsTermLoan, "TermLoan", |i| {
        /// Currency the loan is denominated in.
        /// @returns ISO-4217 currency code such as `"USD"`.
        currency as currency: "string" => text(i.currency),
        /// Committed notional (facility limit).
        /// @returns Currency-tagged commitment.
        notional_limit as notionalLimit: "Money" => money(i.notional_limit),
        /// Issue / funding date.
        /// @returns The issue date.
        issue_date as issueDate: "string" => date(i.issue_date),
        /// Maturity (final redemption) date.
        /// @returns The contractual maturity date.
        maturity as maturity: "string" => date(i.maturity),
        /// Rate specification in serde form.
        /// @returns `{"fixed": {"rate": 0.06}}` (decimal rate) or `{"floating": {...}}`.
        rate as rate: "generated.valuations.TermLoan['rate']" => json(i.rate),
        /// Payment frequency.
        /// @returns The payment tenor.
        frequency as frequency: "Tenor" => tenor(i.frequency),
        /// Quote compounding of the discount margin and z-spread (pricing, quote inversion and CS01), stated independently of the payment frequency.
        /// @returns The `Compounding` wire value.
        quote_compounding as quoteCompounding: "generated.valuations.TermLoan['quote_compounding']" => json(i.quote_compounding),
        /// Accrual day-count convention.
        /// @returns The day count.
        day_count as dayCount: "DayCount" => day_count(i.day_count),
        /// Annual basis of the yield quotes and EIR accretion (resolution O11), separate from accrual.
        /// @returns `"act_365f"` or `"act_360"`.
        yield_basis as yieldBasis: "generated.valuations.TermLoan['yield_basis']" => json(i.yield_basis),
        /// Business day convention (serde string).
        /// @returns `"modified_following"` unless overridden.
        business_day_convention as businessDayConvention: "NonNullable< generated.valuations.TermLoan['business_day_convention'] >" => json(i.business_day_convention),
        /// Holiday calendar identifier.
        /// @returns Calendar id, or `null`.
        calendar_id as calendarId: "string | null" => opt_text(i.calendar_id),
        /// Stub-period handling rule for the schedule.
        /// @returns Stub rule serde name (`"none"`, `"short_front"`, ...).
        stub as stub: "NonNullable<generated.valuations.TermLoan['stub']>" => json(i.stub),
        /// Discount curve identifier.
        /// @returns Curve id used for discounting.
        discount_curve_id as discountCurveId: "string" => text(i.discount_curve_id),
        /// Hazard curve identifier.
        /// @returns Curve id, or `null`.
        credit_curve_id as creditCurveId: "string | null" => opt_text(i.credit_curve_id),
        /// Amortization specification in serde form.
        /// @returns `"none"`, `{"percent_of_remaining_per_period": {"pct": 0.025}}`, `{"linear_between": {...}}`, ...
        amortization as amortization: "generated.valuations.TermLoan['amortization']" => json(i.amortization),
        /// Coupon type (serde string).
        /// @returns `"cash"`, `"pik"`, ...
        coupon_type as couponType: "NonNullable<generated.valuations.TermLoan['coupon_type']>" => json(i.coupon_type),
        /// Upfront (arrangement or OID) fee paid on the issue date.
        /// @returns `UpfrontFee` serde plain object (`{"amount": `Money` plain object}` or `{"fraction_of_commitment": 0.02}`), or `null` when the loan has no upfront fee.
        upfront_fee as upfrontFee: "NonNullable<generated.valuations.TermLoan['upfront_fee']> | null" => json(i.upfront_fee),
        /// Delayed-draw term loan specification in serde form.
        /// @returns The spec plain object, or `null`.
        ddtl as ddtl: "NonNullable<generated.valuations.TermLoan['ddtl']> | null" => json(i.ddtl),
        /// Covenant event schedule in serde form.
        /// @returns The events plain object, or `null`.
        covenants as covenants: "NonNullable<generated.valuations.TermLoan['covenants']> | null" => json(i.covenants),
        /// OID / effective-interest-rate specification in serde form.
        /// @returns The spec plain object, or `null`.
        oid_eir as oidEir: "NonNullable<generated.valuations.TermLoan['oid_eir']> | null" => json(i.oid_eir),
        /// Prepayment (call) schedule in serde form.
        /// @returns The schedule plain object, or `null`.
        call_schedule as callSchedule: "NonNullable<generated.valuations.TermLoan['call_schedule']> | null" => json(i.call_schedule),
        /// Settlement lag in business days.
        /// @returns The lag (Rust default 2).
        settlement_days as settlementDays: "NonNullable<generated.valuations.TermLoan['settlement_days']>" => json(i.settlement_days),
        /// Canonical model key used when `model="default"` is passed to `price`.
        /// @returns Registered model key such as `"discounting"` or `"black76"`.
        default_model as defaultModel: "string" => text(Instrument::default_model(i)),
        /// Expiry date exposed by the Rust `Instrument` trait.
        /// @returns The expiry/maturity date, or `null` when the instrument type reports none.
        expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
});

common_getters!(JsTermLoan, "TermLoan");

builder_class!(
    /// Fluent builder for `TermLoan`; one setter per Rust `FinancialBuilder` setter.
    ///
    /// Every setter returns the builder, so calls chain; `build()` validates
    /// and consumes it.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const builder = valuations.instruments.TermLoan.builder();
    /// console.log(typeof builder.build);
    /// builder.free();
    /// ```
    JsTermLoanBuilder,
    "TermLoanBuilder",
    finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoanBuilder,
    JsTermLoan,
    |b| b.build()
);

setters!(JsTermLoanBuilder, "TermLoanBuilder", {
        /// Set the quote compounding of the discount margin and z-spread (required; never derived from the payment frequency).
        /// @param value - `Compounding` wire value: `"continuous"`, `"simple"` or `{ periodic: n }`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        quote_compounding as quoteCompounding: "generated.valuations.TermLoan['quote_compounding']" => from_js_json,
        /// Set the instrument identifier.
        /// @param value - Instrument identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        id as id: "string" => id,
        /// Set the loan currency.
        /// @param value - ISO-4217 currency code of the loan, e.g. `"USD"`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        currency as currency: "generated.valuations.TermLoan['currency']" => js_wire,
        /// Set the required contractual issue / funding date.
        /// @param value - Required contractual issue / funding date as an ISO-8601 string. `build()` throws when it is not set.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        issue_date as issueDate: "string" => js_date,
        /// Set the maturity date.
        /// @param value - Maturity date as an ISO-8601 string.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        maturity as maturity: "string" => js_date,
        /// Set the interest rate specification.
        /// @param value - `RateSpec` plain object or JSON string: `{"fixed": {"rate": 0.06}}` for a fixed all-in rate (`0.06` = 6%) or `{"floating": {...}}` with a `FloatingRateSpec`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        rate as rate: "generated.valuations.TermLoan['rate']" => from_js_json,
        /// Set the business day convention.
        /// @param value - Business day convention (serde string). Default `"modified_following"`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        business_day_convention as businessDayConvention: "NonNullable<generated.valuations.TermLoan['business_day_convention']>" => js_wire,
        /// Set the required annual basis of yield quotes and EIR accretion (resolution O11).
        /// @param value - `"act_365f"` or `"act_360"`; independent of the accrual day count. `build()` throws when it is not set.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it is not a supported basis or the builder was already consumed by `build()`.
        yield_basis as yieldBasis: "generated.valuations.TermLoan['yield_basis']" => js_wire,
        /// Set the holiday calendar identifier (e.g. `"usny"`).
        /// @param value - Holiday calendar identifier (e.g. `"usny"`).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        calendar_id as calendarId: "string" => id,
        /// Set the stub rule.
        /// @param value - Stub rule (serde string). Default `"short_front"`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        stub as stub: "NonNullable<generated.valuations.TermLoan['stub']>" => js_wire,
        /// Set the discount curve identifier.
        /// @param value - Discount curve identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        discount_curve_id as discountCurveId: "string" => id,
        /// Set the hazard curve identifier for credit-risky pricing.
        /// @param value - Hazard curve identifier for credit-risky pricing.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        credit_curve_id as creditCurveId: "string" => id,
        /// Set the amortization schedule.
        /// @param value - Amortization as the serde name of a unit variant (`"none"`) or an `AmortizationSpec` plain object / JSON string.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        amortization as amortization: "generated.valuations.TermLoan['amortization']" => js_wire,
        /// Set the coupon type.
        /// @param value - Coupon type (serde string). `"cash"` (default), `"pik"`, ...
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        coupon_type as couponType: "NonNullable<generated.valuations.TermLoan['coupon_type']>" => js_wire,
        /// Set the upfront (arrangement or OID) fee paid on the issue date.
        /// @param value - `UpfrontFee` plain object or JSON string: `{"amount": Money}` for a cash fee or `{"fraction_of_commitment": 0.02}`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        upfront_fee as upfrontFee: "NonNullable<generated.valuations.TermLoan['upfront_fee']>" => from_js_json,
        /// Set the delayed-draw (DDTL) specification.
        /// @param value - Rust `DdtlSpec` in serde form (plain object or JSON string), e.g. `TermLoan.example_floating_with_ddtl().ddtl`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        ddtl as ddtl: "NonNullable<generated.valuations.TermLoan['ddtl']>" => from_js_json,
        /// Set the covenant event schedule.
        /// @param value - Rust `TermLoanCovenantEvents` in serde form (plain object or JSON string), e.g. `{"events": [...]}`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        covenants as covenants: "NonNullable<generated.valuations.TermLoan['covenants']>" => from_js_json,
        /// Set the OID / effective-interest-rate specification.
        /// @param value - Rust `OidEirSpec` in serde form (plain object or JSON string), e.g. `{"issue_price_pct": 99.0, ...}`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        oid_eir as oidEir: "NonNullable<generated.valuations.TermLoan['oid_eir']>" => from_js_json,
        /// Set the prepayment (call) schedule.
        /// @param value - Rust `LoanCallSchedule` in serde form (plain object or JSON string), e.g. `TermLoan.example_callable().call_schedule`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        call_schedule as callSchedule: "NonNullable<generated.valuations.TermLoan['call_schedule']>" => from_js_json,
        /// Set the settlement lag in business days (default 2).
        /// @param value - Settlement lag.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        settlement_days as settlementDays: "number" => js_uint,
});

common_setters!(JsTermLoanBuilder, "TermLoanBuilder");

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
