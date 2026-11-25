//! A fixed-term repurchase agreement with explicit collateral terms.

use crate::api::core::dates::JsDayCount;
use crate::api::core::money::JsMoney;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::rates::repo::{Repo, RepoBuilder};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// A fixed-term repurchase agreement with explicit collateral terms.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.Repo.example();
    /// console.log(future.id);
    /// future.free();
    /// ```
    JsRepo, "Repo", Repo
);
instrument_surface!(JsRepo, "Repo", JsRepoBuilder, "generated.valuations.Repo");
factories!(JsRepo, "Repo", Repo, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsRepo, "Repo", |i| {
    /// Effective expiry of the contract.
    /// @returns The ISO expiry, or null if the instrument defines none.
    expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's cash amount.
    /// @returns Cash amount being lent/borrowed
    cash_amount as cashAmount: "Money" => money(i.cash_amount),
    /// Read the contract's collateral.
    /// @returns Collateral specification
    collateral as collateral: "generated.valuations.Repo['collateral']" => json(i.collateral),
    /// Read the contract's repo rate.
    /// @returns Repo rate (annual, as decimal)
    repo_rate as repoRate: "string" => text(i.repo_rate),
    /// Read the contract's start date.
    /// @returns Start date of the repo
    start_date as startDate: "string" => date(i.start_date),
    /// Read the contract's maturity.
    /// @returns Maturity date of the repo
    maturity as maturity: "string" => date(i.maturity),
    /// Read the contract's haircut.
    /// @returns Haircut percentage (as decimal, e.g., 0.02 = 2%)
    haircut as haircut: "number" => json(i.haircut),
    /// Read the contract's repo type.
    /// @returns Type of repo
    repo_type as repoType: "generated.valuations.Repo['repo_type']" => json(i.repo_type),
    /// Read the contract's triparty.
    /// @returns Whether this is a tri-party repo
    triparty as triparty: "boolean" => json(i.triparty),
    /// Read the contract's day count.
    /// @returns Day count convention for interest calculations
    day_count as dayCount: "DayCount" => day_count(i.day_count),
    /// Read the contract's business day convention.
    /// @returns Business day convention
    business_day_convention as businessDayConvention: "generated.valuations.Repo['business_day_convention']" => json(i.business_day_convention),
    /// Read the contract's calendar id.
    /// @returns Optional calendar for business day adjustments
    calendar_id as calendarId: "string | null" => opt_text(i.calendar_id),
    /// Read the contract's discount curve id.
    /// @returns Discount curve identifier for valuation
    discount_curve_id as discountCurveId: "string" => text(i.discount_curve_id),
    /// Read the contract's margin spec.
    /// @returns Optional margin specification for mark-to-market margining. When present, enables margin call generation, collateral valuation, and margin interest calculations. See RepoMarginSpec for details.
    margin_spec as marginSpec: "NonNullable<generated.valuations.Repo['margin_spec']> | null" => json(i.margin_spec),
});
common_getters!(JsRepo, "Repo");
to_builder_method!(
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    JsRepo,
    "Repo",
    JsRepoBuilder
);

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.Repo.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsRepoBuilder, "RepoBuilder", RepoBuilder, JsRepo, |b| b.build()
);

setters!(JsRepoBuilder, "RepoBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's collateral.
    /// @param value - Collateral specification
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    collateral as collateral: "generated.valuations.Repo['collateral']" => js_wire,
    /// Set the contract's repo rate.
    /// @param value - Repo rate (annual, as decimal)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    repo_rate as repoRate: "string" => exact_decimal,
    /// Set the contract's start date.
    /// @param value - Start date of the repo
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    start_date as startDate: "string" => js_date,
    /// Set the contract's maturity.
    /// @param value - Maturity date of the repo
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    maturity as maturity: "string" => js_date,
    /// Set the contract's haircut.
    /// @param value - Haircut percentage (as decimal, e.g., 0.02 = 2%)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    haircut as haircut: "number" => js_f64,
    /// Set the contract's repo type.
    /// @param value - Type of repo
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    repo_type as repoType: "generated.valuations.Repo['repo_type']" => js_wire,
    /// Set the contract's triparty.
    /// @param value - Whether this is a tri-party repo
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    triparty as triparty: "boolean" => js_bool,
    /// Set the contract's business day convention.
    /// @param value - Business day convention
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    business_day_convention as businessDayConvention: "generated.valuations.Repo['business_day_convention']" => js_wire,
    /// Set the contract's calendar id.
    /// @param value - Optional calendar for business day adjustments
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    calendar_id as calendarId: "string | null" => optional_id via calendar_id_opt,
    /// Set the contract's discount curve id.
    /// @param value - Discount curve identifier for valuation
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    discount_curve_id as discountCurveId: "string" => id,
    /// Set the contract's margin spec.
    /// @param value - Optional margin specification for mark-to-market margining. When present, enables margin call generation, collateral valuation, and margin interest calculations. See RepoMarginSpec for details.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    margin_spec as marginSpec: "NonNullable<generated.valuations.Repo['margin_spec']> | null" => opt_wire via margin_spec_opt,
});
common_setters!(JsRepoBuilder, "RepoBuilder");

handle_setters!(JsRepoBuilder, "RepoBuilder", {
    /// Set the contract's cash amount.
    /// @param value - Cash amount being lent/borrowed
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    cash_amount as cashAmount => JsMoney,
    /// Set the contract's day count.
    /// @param value - Day count convention for interest calculations
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    day_count as dayCount => JsDayCount,
});
