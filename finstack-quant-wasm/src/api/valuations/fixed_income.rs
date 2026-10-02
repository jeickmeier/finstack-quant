//! Typed fixed-income instrument classes (`Bond`, `TermLoan`,
//! `RevolvingCredit`, `AssetBackedFacility`).
//!
//! Thin wrappers over the canonical Rust structs
//! [`finstack_quant_valuations::instruments::Bond`],
//! [`finstack_quant_valuations::instruments::TermLoan`],
//! [`finstack_quant_valuations::instruments::RevolvingCredit`] and
//! [`finstack_quant_valuations::instruments::AssetBackedFacility`]. Construction
//! and validation stay in Rust; the wrappers convert to and from the canonical
//! `finstack_quant.instrument/1` envelope accepted by the JSON loader.
//!
//! To price a typed instrument, pass its `toJson()` output to the generic
//! pricing entry points (`valuations.instruments.priceInstrument`,
//! `priceInstrument`, `instrumentCashflowsJson`).

use crate::api::core::dates::{JsDayCount, JsTenor};
use crate::api::core::money::JsMoney;
use crate::api::core::types::{JsBps, JsRate};
use crate::utils::input::{js_f64, js_string, json_text};
use crate::utils::{parse_iso_date, to_js_err, to_js_value};
use finstack_quant_core::dates::StubKind;
use finstack_quant_valuations::instruments::{InstrumentEnvelope, InstrumentJson};
use wasm_bindgen::prelude::*;

/// Parse a canonical instrument envelope into one concrete Rust instrument;
/// Rust rejects a different instrument type with a `validation` error.
fn parse_typed<T>(json: &str) -> Result<T, JsValue>
where
    T: TryFrom<InstrumentJson, Error = finstack_quant_core::Error>,
{
    finstack_quant_valuations::pricer::parse_typed_instrument_json(json).map_err(to_js_err)
}

/// Typed wrapper for the Rust `Bond` instrument.
#[wasm_bindgen(js_name = Bond)]
#[derive(Clone)]
pub struct JsBond {
    pub(crate) inner: finstack_quant_valuations::instruments::Bond,
}

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
        let issue_date: &str = &js_string(&issue_date, "issueDate")?;
        let maturity: &str = &js_string(&maturity, "maturity")?;
        let stub: &str = &js_string(&stub, "stub")?;
        let discount_curve_id: &str = &js_string(&discount_curve_id, "discountCurveId")?;
        let inner = finstack_quant_valuations::instruments::Bond::fixed(
            id,
            notional.inner,
            coupon_rate.inner,
            parse_iso_date(issue_date)?,
            parse_iso_date(maturity)?,
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
        let issue_date: &str = &js_string(&issue_date, "issueDate")?;
        let maturity: &str = &js_string(&maturity, "maturity")?;
        let convention: &str = &js_string(&convention, "convention")?;
        let discount_curve_id: &str = &js_string(&discount_curve_id, "discountCurveId")?;
        let inner = finstack_quant_valuations::instruments::Bond::with_convention(
            id,
            notional.inner,
            coupon_rate.inner,
            parse_iso_date(issue_date)?,
            parse_iso_date(maturity)?,
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
        let issue_date: &str = &js_string(&issue_date, "issueDate")?;
        let maturity: &str = &js_string(&maturity, "maturity")?;
        let discount_curve_id: &str = &js_string(&discount_curve_id, "discountCurveId")?;
        let inner = finstack_quant_valuations::instruments::Bond::floating(
            id,
            notional.inner,
            forward_curve_id,
            spread_bp.inner,
            parse_iso_date(issue_date)?,
            parse_iso_date(maturity)?,
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
        let inner = finstack_quant_valuations::instruments::Bond::floating_with_convention(
            js_string(&id, "id")?,
            notional.inner,
            js_string(&forward_curve_id, "forwardCurveId")?,
            spread_bp.inner,
            parse_iso_date(&js_string(&issue_date, "issueDate")?)?,
            parse_iso_date(&js_string(&maturity, "maturity")?)?,
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
        let inner = finstack_quant_valuations::instruments::Bond::zero_coupon(
            js_string(&id, "id")?,
            notional.inner,
            parse_iso_date(&js_string(&issue_date, "issueDate")?)?,
            parse_iso_date(&js_string(&maturity, "maturity")?)?,
            js_string(&discount_curve_id, "discountCurveId")?,
        )
        .map_err(to_js_err)?;
        Ok(JsBond { inner })
    }

    /// Canonical example bond (mirrors Rust `Bond::example`): a 10-year US
    /// Treasury, USD 1,000,000 at 4.25%, issued 2024-01-15.
    /// @returns The example bond.
    /// @throws Error - Throws if the canonical example fails validation (does not occur for a released build).
    pub fn example() -> Result<JsBond, JsValue> {
        finstack_quant_valuations::instruments::Bond::example()
            .map(|inner| JsBond { inner })
            .map_err(to_js_err)
    }

    /// Canonical example floating-rate note (mirrors Rust `Bond::example_floating`):
    /// USD SOFR 3M + 150bp with a 0% index floor.
    /// @returns The example floating-rate note.
    /// @throws Error - Throws if the canonical example fails validation (does not occur for a released build).
    #[wasm_bindgen(js_name = exampleFloating)]
    pub fn example_floating() -> Result<JsBond, JsValue> {
        finstack_quant_valuations::instruments::Bond::example_floating()
            .map(|inner| JsBond { inner })
            .map_err(to_js_err)
    }

    /// Canonical example callable bond (mirrors Rust `Bond::example_callable`):
    /// a 5% semi-annual bond with a declining call schedule.
    /// @returns The example callable bond.
    /// @throws Error - Throws if the canonical example fails validation (does not occur for a released build).
    #[wasm_bindgen(js_name = exampleCallable)]
    pub fn example_callable() -> Result<JsBond, JsValue> {
        finstack_quant_valuations::instruments::Bond::example_callable()
            .map(|inner| JsBond { inner })
            .map_err(to_js_err)
    }

    /// Canonical example amortizing bond (mirrors Rust `Bond::example_amortizing`):
    /// a 4% semi-annual bond with a scheduled principal amortization.
    /// @returns The example amortizing bond.
    /// @throws Error - Throws if the canonical example fails validation (does not occur for a released build).
    #[wasm_bindgen(js_name = exampleAmortizing)]
    pub fn example_amortizing() -> Result<JsBond, JsValue> {
        finstack_quant_valuations::instruments::Bond::example_amortizing()
            .map(|inner| JsBond { inner })
            .map_err(to_js_err)
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

    /// Deserialize a bond from its canonical v1 instrument envelope.
    ///
    /// Bare payloads are rejected; the loader's validation runs on the result.
    /// @param json - A `finstack_quant.instrument/1` envelope containing type `"bond"`.
    /// @returns The validated bond.
    /// @throws If the JSON is malformed, has a different instrument type, or fails validation.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<JsBond, JsValue> {
        let json: &str = &json_text(&json, "json")?;
        parse_typed(json).map(|inner| JsBond { inner })
    }

    /// Serialize to a canonical `finstack_quant.instrument/1` envelope.
    ///
    /// Pass the result to `valuations.instruments.priceInstrument` (or the
    /// other generic pricing entry points) to price this bond.
    /// @returns Canonical instrument envelope accepted by `priceInstrument` and `Bond.fromJson`.
    /// @throws If serialization fails.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&InstrumentEnvelope::new(InstrumentJson::Bond(
            self.inner.clone(),
        )))
        .map_err(to_js_err)
    }

    /// Instrument identifier.
    #[wasm_bindgen(getter)]
    pub fn id(&self) -> String {
        self.inner.id.to_string()
    }
}

/// Typed wrapper for the Rust `TermLoan` instrument.
///
/// Rust has no `fixed`/`floating` convenience constructors for term loans;
/// construct via `TermLoan.fromJson` with a canonical v1 instrument envelope
/// or start from `TermLoan.example()`.
#[wasm_bindgen(js_name = TermLoan)]
#[derive(Clone)]
pub struct JsTermLoan {
    pub(crate) inner: finstack_quant_valuations::instruments::TermLoan,
}

#[wasm_bindgen(js_class = TermLoan)]
impl JsTermLoan {
    /// Deserialize a term loan from its canonical v1 instrument envelope.
    ///
    /// Bare payloads are rejected; the loader's validation runs on the result.
    /// @param json - A `finstack_quant.instrument/1` envelope containing type `"term_loan"`.
    /// @returns The validated term loan.
    /// @throws If the JSON is malformed, has a different instrument type, or fails validation.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<JsTermLoan, JsValue> {
        let json: &str = &json_text(&json, "json")?;
        parse_typed(json).map(|inner| JsTermLoan { inner })
    }

    /// Canonical example term loan (mirrors Rust `TermLoan::example`).
    ///
    /// Returns a 5-year USD fixed-rate loan (6%, quarterly, Act/360, 2.5%
    /// per-period amortization) useful as a starting point and in tests.
    /// @returns The example loan.
    /// @throws If construction fails (should not occur).
    pub fn example() -> Result<JsTermLoan, JsValue> {
        finstack_quant_valuations::instruments::TermLoan::example()
            .map(|inner| JsTermLoan { inner })
            .map_err(to_js_err)
    }

    /// Canonical example floating-rate term loan with a delayed-draw tranche
    /// (mirrors Rust `TermLoan::example_floating_with_ddtl`).
    /// @returns The example loan.
    /// @throws Error - Throws if the canonical example fails validation (does not occur for a released build).
    #[wasm_bindgen(js_name = exampleFloatingWithDdtl)]
    pub fn example_floating_with_ddtl() -> Result<JsTermLoan, JsValue> {
        finstack_quant_valuations::instruments::TermLoan::example_floating_with_ddtl()
            .map(|inner| JsTermLoan { inner })
            .map_err(to_js_err)
    }

    /// Canonical example callable term loan (mirrors Rust `TermLoan::example_callable`).
    /// @returns The example loan.
    /// @throws Error - Throws if the canonical example fails validation (does not occur for a released build).
    #[wasm_bindgen(js_name = exampleCallable)]
    pub fn example_callable() -> Result<JsTermLoan, JsValue> {
        finstack_quant_valuations::instruments::TermLoan::example_callable()
            .map(|inner| JsTermLoan { inner })
            .map_err(to_js_err)
    }

    /// Serialize to a canonical `finstack_quant.instrument/1` envelope.
    ///
    /// Pass the result to `valuations.instruments.priceInstrument` (or the
    /// other generic pricing entry points) to price this loan.
    /// @returns Canonical instrument envelope accepted by `priceInstrument` and `TermLoan.fromJson`.
    /// @throws If serialization fails.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&InstrumentEnvelope::new(InstrumentJson::TermLoan(
            self.inner.clone(),
        )))
        .map_err(to_js_err)
    }

    /// Instrument identifier.
    #[wasm_bindgen(getter)]
    pub fn id(&self) -> String {
        self.inner.id.to_string()
    }
}

/// Typed wrapper for the Rust `AssetBackedFacility` instrument (a warehouse
/// line against a collateral pool under advance rates, concentration limits
/// and a borrowing-base test).
///
/// Construct via `AssetBackedFacility.fromJson` with a canonical v1
/// instrument envelope or start from `AssetBackedFacility.example()`; price
/// by passing `toJson()` to `valuations.instruments.priceInstrument`.
#[wasm_bindgen(js_name = AssetBackedFacility)]
#[derive(Clone)]
pub struct JsAssetBackedFacility {
    pub(crate) inner: finstack_quant_valuations::instruments::AssetBackedFacility,
}

#[wasm_bindgen(js_class = AssetBackedFacility)]
impl JsAssetBackedFacility {
    /// Parse a canonical `finstack_quant.instrument/1` envelope whose
    /// instrument is an `asset_backed_facility`.
    /// @param json - Canonical instrument envelope JSON.
    /// @returns The typed facility.
    /// @throws If the JSON is malformed, has a different instrument type, or fails validation.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<JsAssetBackedFacility, JsValue> {
        let json: &str = &json_text(&json, "json")?;
        parse_typed(json).map(|inner| JsAssetBackedFacility { inner })
    }

    /// The canonical example facility: the example CLO pool financed by a
    /// USD 80M commitment drawn USD 70M.
    /// @returns The example facility.
    /// @throws If construction fails (should not occur).
    pub fn example() -> Result<JsAssetBackedFacility, JsValue> {
        finstack_quant_valuations::instruments::AssetBackedFacility::example()
            .map(|inner| JsAssetBackedFacility { inner })
            .map_err(to_js_err)
    }

    /// Serialize to a canonical `finstack_quant.instrument/1` envelope.
    /// @returns Canonical instrument envelope accepted by `priceInstrument` and `AssetBackedFacility.fromJson`.
    /// @throws If serialization fails.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&InstrumentEnvelope::new(
            InstrumentJson::AssetBackedFacility(Box::new(self.inner.clone())),
        ))
        .map_err(to_js_err)
    }

    /// Instrument identifier.
    /// @returns Stable instrument identifier.
    #[wasm_bindgen(getter)]
    pub fn id(&self) -> String {
        self.inner.id.to_string()
    }

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
        let market = super::pricing::parse_market_json(&json_text(&market_json, "marketJson")?)?;
        let as_of = parse_iso_date(&js_string(&as_of, "asOf")?)?;
        to_js_value(&self.inner.project(&market, as_of).map_err(to_js_err)?)
    }

    /// Lender IRR: XIRR of `-drawn` on `asOf` against every projected interest,
    /// principal and fee receipt (mirrors Rust `AssetBackedFacility::facility_irr`).
    /// @param marketJson - Serialized `MarketContext` with the curves and fixings for the projection.
    /// @param asOf - Valuation date as an ISO-8601 string; the investment is dated here.
    /// @returns Annual IRR as a decimal (`0.08` = 8%).
    /// @throws Error - Throws with kind `validation` if an input is malformed, kind `not_found` if market data is missing, and kind `computation` if the XIRR does not converge.
    #[wasm_bindgen(js_name = facilityIrr)]
    pub fn facility_irr(&self, market_json: JsValue, as_of: JsValue) -> Result<f64, JsValue> {
        let market = super::pricing::parse_market_json(&json_text(&market_json, "marketJson")?)?;
        let as_of = parse_iso_date(&js_string(&as_of, "asOf")?)?;
        self.inner.facility_irr(&market, as_of).map_err(to_js_err)
    }

    /// The two-class structured-credit deal the engine runs for this facility
    /// (mirrors Rust `AssetBackedFacility::synthesized_deal`): facility note
    /// plus residual, borrowing-base test, reinvestment window,
    /// early-amortization rules and the term-out call.
    /// @returns Canonical `finstack_quant.instrument/1` envelope JSON of type `structured_credit`, accepted by `priceInstrument`.
    /// @throws Error - Throws with kind `validation` if the facility or the synthetic deal fails validation.
    #[wasm_bindgen(js_name = synthesizedDealJson)]
    pub fn synthesized_deal_json(&self) -> Result<String, JsValue> {
        let deal = self.inner.synthesized_deal().map_err(to_js_err)?;
        serde_json::to_string(&InstrumentEnvelope::new(InstrumentJson::StructuredCredit(
            Box::new(deal),
        )))
        .map_err(to_js_err)
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

/// Typed wrapper for the Rust `RevolvingCredit` instrument.
///
/// Construct via `RevolvingCredit.fromJson` with a canonical v1 instrument
/// envelope or start from `RevolvingCredit.example()`; price by passing
/// `toJson()` to the generic pricing entry points.
#[wasm_bindgen(js_name = RevolvingCredit)]
#[derive(Clone)]
pub struct JsRevolvingCredit {
    pub(crate) inner: finstack_quant_valuations::instruments::RevolvingCredit,
}

#[wasm_bindgen(js_class = RevolvingCredit)]
impl JsRevolvingCredit {
    /// Deserialize a revolving credit facility from its canonical v1 instrument envelope.
    ///
    /// Bare payloads are rejected; the loader's validation runs on the result.
    /// @param json - A `finstack_quant.instrument/1` envelope containing type `"revolving_credit"`.
    /// @returns The validated facility.
    /// @throws If the JSON is malformed, has a different instrument type, or fails validation.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<JsRevolvingCredit, JsValue> {
        let json: &str = &json_text(&json, "json")?;
        parse_typed(json).map(|inner| JsRevolvingCredit { inner })
    }

    /// Canonical example facility (mirrors Rust `RevolvingCredit::example`).
    ///
    /// Returns a three-year USD 50M SOFR + 250bp facility with USD 10M drawn
    /// and a scheduled draw and repayment, useful as a starting point and in tests.
    /// @returns The example facility.
    /// @throws If construction fails (should not occur).
    pub fn example() -> Result<JsRevolvingCredit, JsValue> {
        finstack_quant_valuations::instruments::RevolvingCredit::example()
            .map(|inner| JsRevolvingCredit { inner })
            .map_err(to_js_err)
    }

    /// Serialize to a canonical `finstack_quant.instrument/1` envelope.
    ///
    /// Pass the result to `valuations.instruments.priceInstrument` (or the
    /// other generic pricing entry points) to price this facility.
    /// @returns Canonical instrument envelope accepted by `priceInstrument` and `RevolvingCredit.fromJson`.
    /// @throws If serialization fails.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&InstrumentEnvelope::new(InstrumentJson::RevolvingCredit(
            self.inner.clone(),
        )))
        .map_err(to_js_err)
    }

    /// Instrument identifier.
    #[wasm_bindgen(getter)]
    pub fn id(&self) -> String {
        self.inner.id.to_string()
    }

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
        use finstack_quant_cashflows::traits::CashflowScheduleSource;
        let market = super::pricing::parse_market_json(&json_text(&market_json, "marketJson")?)?;
        let as_of = parse_iso_date(&js_string(&as_of, "asOf")?)?;
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
        let market_json: &str = &json_text(&market_json, "marketJson")?;
        let as_of: &str = &js_string(&as_of, "asOf")?;
        let market = super::pricing::parse_market_json(market_json)?;
        let as_of = parse_iso_date(as_of)?;
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
