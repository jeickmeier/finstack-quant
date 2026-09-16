//! Typed fixed-income instrument classes (`Bond`, `TermLoan`,
//! `RevolvingCredit`).
//!
//! Thin wrappers over the canonical Rust structs
//! [`finstack_quant_valuations::instruments::Bond`],
//! [`finstack_quant_valuations::instruments::TermLoan`] and
//! [`finstack_quant_valuations::instruments::RevolvingCredit`]. Construction
//! and validation stay in Rust; the wrappers convert to and from the canonical
//! `finstack_quant.instrument/1` envelope accepted by the JSON loader.
//!
//! To price a typed instrument, pass its `toJson()` output to the generic
//! pricing entry points (`valuations.instruments.priceInstrument`,
//! `priceInstrument`, `instrumentCashflowsJson`).

use crate::api::core::dates::{JsDayCount, JsTenor};
use crate::api::core::money::JsMoney;
use crate::api::core::types::{JsBps, JsRate};
use crate::utils::{parse_iso_date, to_js_err, to_js_value};
use finstack_quant_core::dates::StubKind;
use finstack_quant_valuations::instruments::{InstrumentEnvelope, InstrumentJson};
use wasm_bindgen::prelude::*;

/// Parse a canonical instrument envelope through the shared JSON-loader path.
fn parse_envelope(json: &str) -> Result<InstrumentJson, JsValue> {
    finstack_quant_valuations::pricer::json::parse_instrument_from_json(json).map_err(to_js_err)
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
    /// @param issue - Issue date as an ISO-8601 string (`"YYYY-MM-DD"`).
    /// @param maturity - Maturity date as an ISO-8601 string (`"YYYY-MM-DD"`).
    /// @param stub - Stub policy: `none`, `short_front`, `short_back`,
    /// `long_front`, or `long_back`.
    /// @param discountCurveId - Discount curve identifier used for pricing.
    /// @returns The validated fixed-rate bond.
    /// @throws If validation fails (e.g. maturity not after issue).
    pub fn fixed(
        id: &str,
        notional: &JsMoney,
        coupon_rate: &JsRate,
        issue: &str,
        maturity: &str,
        stub: &str,
        discount_curve_id: &str,
    ) -> Result<JsBond, JsValue> {
        let inner = finstack_quant_valuations::instruments::Bond::fixed(
            id,
            notional.inner,
            coupon_rate.inner,
            parse_iso_date(issue)?,
            parse_iso_date(maturity)?,
            stub.parse::<StubKind>().map_err(to_js_err)?,
            discount_curve_id,
        )
        .map_err(to_js_err)?;
        Ok(JsBond { inner })
    }

    /// Create a floating-rate bond (FRN) linked to a forward index.
    ///
    /// Mirrors Rust `Bond::floating`. Settlement, calendar, and
    /// business-day convention come from the notional currency: USD
    /// UsCorporate (T+1, usny), EUR EurCorporate (T+2, target2), GBP
    /// UkGilt (T+1), JPY Jgb (T+2). Unmapped currencies throw.
    /// @param id - Unique instrument identifier.
    /// @param notional - Principal amount of the bond.
    /// @param indexId - Forward curve identifier (e.g. `"USD-SOFR-3M"`).
    /// @param marginBp - Spread over the index in whole basis points
    /// (`Bps` rejects fractional values; use `Bond.fromJson` for sub-bp
    /// margins, which preserves the exact decimal spread).
    /// @param issue - Issue date as an ISO-8601 string (`"YYYY-MM-DD"`).
    /// @param maturity - Maturity date as an ISO-8601 string (`"YYYY-MM-DD"`).
    /// @param frequency - Payment frequency (e.g. `Tenor.quarterly()`).
    /// @param dayCount - Day count convention (e.g. `DayCount.act360()`).
    /// @param discountCurveId - Discount curve identifier used for pricing.
    /// @returns The validated floating-rate note.
    /// @throws If the notional currency has no mapped settlement convention
    /// or validation fails.
    #[allow(clippy::too_many_arguments)]
    pub fn floating(
        id: &str,
        notional: &JsMoney,
        index_id: &str,
        margin_bp: &JsBps,
        issue: &str,
        maturity: &str,
        frequency: &JsTenor,
        day_count: &JsDayCount,
        discount_curve_id: &str,
    ) -> Result<JsBond, JsValue> {
        let inner = finstack_quant_valuations::instruments::Bond::floating(
            id,
            notional.inner,
            index_id,
            margin_bp.inner,
            parse_iso_date(issue)?,
            parse_iso_date(maturity)?,
            frequency.inner,
            day_count.inner,
            discount_curve_id,
        )
        .map_err(to_js_err)?;
        Ok(JsBond { inner })
    }

    /// Deserialize a bond from its canonical v1 instrument envelope.
    ///
    /// Bare payloads are rejected; the loader's validation runs on the result.
    /// @param json - A `finstack_quant.instrument/1` envelope containing type `"bond"`.
    /// @returns The validated bond.
    /// @throws If the JSON is malformed, has a different instrument type, or fails validation.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: &str) -> Result<JsBond, JsValue> {
        match parse_envelope(json)? {
            InstrumentJson::Bond(inner) => Ok(JsBond { inner }),
            _ => Err(JsValue::from_str(
                "expected instrument type \"bond\", got a different instrument type",
            )),
        }
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
    pub fn from_json(json: &str) -> Result<JsTermLoan, JsValue> {
        match parse_envelope(json)? {
            InstrumentJson::TermLoan(inner) => Ok(JsTermLoan { inner }),
            _ => Err(JsValue::from_str(
                "expected instrument type \"term_loan\", got a different instrument type",
            )),
        }
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
    pub fn from_json(json: &str) -> Result<JsRevolvingCredit, JsValue> {
        match parse_envelope(json)? {
            InstrumentJson::RevolvingCredit(inner) => Ok(JsRevolvingCredit { inner }),
            _ => Err(JsValue::from_str(
                "expected instrument type \"revolving_credit\", got a different instrument type",
            )),
        }
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
    pub fn price_with_paths(&self, market_json: &str, as_of: &str) -> Result<JsValue, JsValue> {
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
