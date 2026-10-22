//! Typed fixed-income instrument classes (`Bond`, `TermLoan`,
//! `RevolvingCredit`, `AssetBackedFacility`) and their fluent builders.
//!
//! Thin wrappers over the canonical Rust structs
//! [`finstack_quant_valuations::instruments::fixed_income::bond::Bond`],
//! [`finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan`],
//! [`finstack_quant_valuations::instruments::fixed_income::revolving_credit::RevolvingCredit`] and
//! [`finstack_quant_valuations::instruments::fixed_income::asset_backed_facility::AssetBackedFacility`]. Construction
//! and validation stay in Rust; the wrappers convert to and from the canonical
//! `finstack_quant.instrument/1` envelope accepted by the JSON loader.
//!
//! Each class exposes its constructors, `fromJson` / `toJson`, one getter per
//! public Rust field, the `Instrument` trait surface, `price` / `metric`, and
//! the `FinancialBuilder` setters.

use super::data::JsMertonMcConfig;
use crate::api::core::dates::{JsDayCount, JsTenor};
use crate::api::core::money::JsMoney;
use crate::api::core::types::{JsBps, JsRate};
use crate::api::valuations::pricing::parse_market;
use crate::utils::input::{js_f64, js_string};
use crate::utils::wire::js_date;
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_core::dates::StubKind;
use finstack_quant_valuations::instruments::Instrument;
use wasm_bindgen::prelude::*;

instrument_class!(
    /// Typed wrapper for the canonical Rust Bond instrument.
    JsBond, "Bond", finstack_quant_valuations::instruments::fixed_income::bond::Bond
);

#[wasm_bindgen(js_class = Bond)]
impl JsBond {
    /// Create a US corporate fixed-rate bond (semi-annual, 30/360, T+1).
    ///
    /// Mirrors Rust `Bond::fixed` and requires an explicit stub policy.
    /// @param id - Unique instrument identifier.
    /// @param notional - Principal amount of the bond.
    /// @param couponRate - Annual coupon rate.
    /// @param issue_date - Issue date as an ISO-8601 string (`"YYYY-MM-DD"`).
    /// @param maturity - Maturity date as an ISO-8601 string (`"YYYY-MM-DD"`).
    /// @param stub - Stub policy: `none`, `short_front`, `short_back`,
    /// `long_front`, or `long_back`.
    /// @param discountCurveId - Discount curve identifier used for pricing.
    /// @returns The validated fixed-rate bond.
    /// @throws If validation fails (e.g. maturity not after issue_date).
    pub fn fixed(
        id: JsValue,
        notional: &JsMoney,
        coupon_rate: &JsRate,
        issue_date: JsValue,
        maturity: JsValue,
        stub: JsValue,
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
    /// @param issue_date - Issue date as an ISO-8601 string (`"YYYY-MM-DD"`).
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
        id: JsValue,
        notional: &JsMoney,
        coupon_rate: &JsRate,
        issue_date: JsValue,
        maturity: JsValue,
        convention: JsValue,
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
                    .parse::<finstack_quant_valuations::instruments::BondConvention>()
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
    pub fn with_stub(&self, stub: JsValue) -> Result<JsBond, JsValue> {
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
    /// @param id - Unique instrument identifier.
    /// @param notional - Principal amount of the bond.
    /// @param forwardCurveId - Forward curve identifier (e.g. `"USD-SOFR-3M"`).
    /// @param spreadBp - Spread over the index in whole basis points
    /// (`Bps` rejects fractional values; use `Bond.fromJson` for sub-bp
    /// margins, which preserves the exact decimal spread).
    /// @param issue_date - Issue date as an ISO-8601 string (`"YYYY-MM-DD"`).
    /// @param maturity - Maturity date as an ISO-8601 string (`"YYYY-MM-DD"`).
    /// @param frequency - Payment frequency (e.g. `Tenor.quarterly()`).
    /// @param dayCount - Day count convention (e.g. `DayCount.act360()`).
    /// @param discountCurveId - Discount curve identifier used for pricing.
    /// @returns The validated floating-rate note.
    /// @throws If the notional currency has no mapped settlement convention
    /// or validation fails.
    #[allow(clippy::too_many_arguments)]
    pub fn floating(
        id: JsValue,
        notional: &JsMoney,
        forward_curve_id: JsValue,
        spread_bp: &JsBps,
        issue_date: JsValue,
        maturity: JsValue,
        frequency: &JsTenor,
        day_count: &JsDayCount,
        discount_curve_id: JsValue,
    ) -> Result<JsBond, JsValue> {
        let id: &str = &js_string(&id, "id")?;
        let forward_curve_id: &str = &js_string(&forward_curve_id, "forwardCurveId")?;
        let issue_date = js_date(&issue_date, "issueDate")?;
        let maturity = js_date(&maturity, "maturity")?;
        let discount_curve_id: &str = &js_string(&discount_curve_id, "discountCurveId")?;
        let inner = finstack_quant_valuations::instruments::fixed_income::bond::Bond::floating(
            id,
            notional.inner,
            forward_curve_id,
            spread_bp.inner,
            issue_date,
            maturity,
            frequency.inner,
            day_count.inner,
            discount_curve_id,
        )
        .map_err(to_js_err)?;
        Ok(JsBond { inner })
    }

    /// Create a floating-rate bond (FRN) from a named market convention preset.
    ///
    /// Mirrors Rust `Bond::floating_with_convention`: calendar, business-day
    /// convention, settlement lag and stub rule come from the preset; the
    /// coupon is `forward index + spreadBp`.
    /// @param id - Unique instrument identifier.
    /// @param notional - Principal amount of the bond.
    /// @param forwardCurveId - Forward curve identifier (e.g. `"USD-SOFR-3M"`).
    /// @param spreadBp - Spread over the index in whole basis points.
    /// @param issue_date - Issue date as an ISO-8601 string (`"YYYY-MM-DD"`).
    /// @param maturity - Maturity date as an ISO-8601 string (`"YYYY-MM-DD"`).
    /// @param frequency - Payment frequency (e.g. `Tenor.quarterly()`).
    /// @param dayCount - Day count convention (e.g. `DayCount.act360()`).
    /// @param convention - Bond convention preset: `us_treasury`, `us_agency`,
    /// `german_bund`, `uk_gilt`, `french_oat`, `jgb`, `us_corporate`, or
    /// `eur_corporate`.
    /// @param discountCurveId - Discount curve identifier used for pricing.
    /// @returns The validated floating-rate note.
    /// @throws Error - Throws with kind `validation` if `convention` is not a
    /// known preset, a date is malformed, or bond validation fails.
    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = floatingWithConvention)]
    pub fn floating_with_convention(
        id: JsValue,
        notional: &JsMoney,
        forward_curve_id: JsValue,
        spread_bp: &JsBps,
        issue_date: JsValue,
        maturity: JsValue,
        frequency: &JsTenor,
        day_count: &JsDayCount,
        convention: JsValue,
        discount_curve_id: JsValue,
    ) -> Result<JsBond, JsValue> {
        let inner = finstack_quant_valuations::instruments::fixed_income::bond::Bond::floating_with_convention(
            js_string(&id, "id")?,
            notional.inner,
            js_string(&forward_curve_id, "forwardCurveId")?,
            spread_bp.inner,
            js_date(&issue_date, "issueDate")?,
            js_date(&maturity, "maturity")?,
            frequency.inner,
            day_count.inner,
            js_string(&convention, "convention")?
                .parse::<finstack_quant_valuations::instruments::BondConvention>()
                .map_err(to_js_err)?,
            js_string(&discount_curve_id, "discountCurveId")?,
        )
        .map_err(to_js_err)?;
        Ok(JsBond { inner })
    }

    /// Create a zero-coupon bond that pays `notional` at maturity.
    ///
    /// Mirrors Rust `Bond::zero_coupon`.
    /// @param id - Unique instrument identifier.
    /// @param notional - Principal repaid at maturity.
    /// @param issue_date - Issue date as an ISO-8601 string (`"YYYY-MM-DD"`).
    /// @param maturity - Maturity date as an ISO-8601 string (`"YYYY-MM-DD"`).
    /// @param discountCurveId - Discount curve identifier used for pricing.
    /// @returns The validated zero-coupon bond.
    /// @throws Error - Throws with kind `validation` if a date is malformed or
    /// bond validation fails (e.g. maturity not after issue_date).
    #[wasm_bindgen(js_name = zeroCoupon)]
    pub fn zero_coupon(
        id: JsValue,
        notional: &JsMoney,
        issue_date: JsValue,
        maturity: JsValue,
        discount_curve_id: JsValue,
    ) -> Result<JsBond, JsValue> {
        let inner = finstack_quant_valuations::instruments::fixed_income::bond::Bond::zero_coupon(
            js_string(&id, "id")?,
            notional.inner,
            js_date(&issue_date, "issueDate")?,
            js_date(&maturity, "maturity")?,
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
    pub fn min_moic(&self, multiple: JsValue) -> Result<JsBond, JsValue> {
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

instrument_class!(
    /// Typed wrapper for the canonical Rust TermLoan instrument.
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

instrument_class!(
    /// Typed wrapper for the canonical Rust AssetBackedFacility instrument.
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
    pub fn project(&self, market_json: JsValue, as_of: JsValue) -> Result<JsValue, JsValue> {
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
        super::envelope_json(self.inner.synthesized_deal().map_err(to_js_err)?)
    }

    /// Borrowing base on the closing collateral: eligible collateral,
    /// concentration excess and the advance-rate-weighted base.
    /// @returns Plain `BorrowingBaseReport` object with `eligible_collateral`, `concentration_excess` and `borrowing_base` Money values.
    /// @throws If the borrowing-base rules are malformed.
    #[wasm_bindgen(js_name = borrowingBase)]
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

instrument_class!(
    /// Typed wrapper for the canonical Rust RevolvingCredit instrument.
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
    #[wasm_bindgen(js_name = expectedCashflows)]
    pub fn expected_cashflows(
        &self,
        market_json: JsValue,
        as_of: JsValue,
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
    #[wasm_bindgen(js_name = priceWithPaths)]
    pub fn price_with_paths(
        &self,
        market_json: JsValue,
        as_of: JsValue,
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

instrument_surface!(JsBond, "Bond", JsBondBuilder);

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
        /// Expiry date exposed by the Rust `Instrument` trait.
        /// @returns The expiry/maturity date, or `null` when the instrument type reports none.
        expiry as expiry => opt_date(Instrument::expiry(i)),
});

common_getters!(JsBond, "Bond");

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
});

common_setters!(JsBondBuilder, "BondBuilder");

handle_setters!(JsBondBuilder, "BondBuilder", {
        /// Set the principal amount.
        /// @param value - Principal amount, as a `Money` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        notional as notional => JsMoney,
});

instrument_surface!(JsTermLoan, "TermLoan", JsTermLoanBuilder);

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
        /// Expiry date exposed by the Rust `Instrument` trait.
        /// @returns The expiry/maturity date, or `null` when the instrument type reports none.
        expiry as expiry => opt_date(Instrument::expiry(i)),
});

common_getters!(JsTermLoan, "TermLoan");

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
        amortization as amortization => en,
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

instrument_surface!(
    JsRevolvingCredit,
    "RevolvingCredit",
    JsRevolvingCreditBuilder
);

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

common_getters!(JsRevolvingCredit, "RevolvingCredit", overrides);

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
        /// @param value - `DrawRepaySpec` as a plain object or JSON string: `{"deterministic": [{"date": ..., "amount": Money, "is_draw": boolean}, ...]}` or `{"stochastic": {"utilization_process": {...}, "mc_config": ...}}`. The estimator count, antithetic flag and seed label come from `instrument_pricing_overrides.model_config` (`mc_paths`, `mc_antithetic`, `mc_seed_scenario`).
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

instrument_surface!(
    JsAssetBackedFacility,
    "AssetBackedFacility",
    JsAssetBackedFacilityBuilder
);

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
        /// @throws `FinstackError` (kind `validation`) if the term-out window
        /// exceeds the supported date range.
        repayment_date as repaymentDate => date(i.repayment_date().map_err(to_js_err)?),
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

common_getters!(JsAssetBackedFacility, "AssetBackedFacility", overrides);

builder_class!(
    /// Fluent builder for `AssetBackedFacility`; one setter per Rust `FinancialBuilder` setter.
    ///
    /// Every setter returns the builder, so calls chain; `build()` validates
    /// and consumes it.
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
