//! Typed fixed-income instrument classes (`Bond`, `TermLoan`).
//!
//! Thin wrappers over the canonical Rust structs
//! [`finstack_quant_valuations::instruments::Bond`] and
//! [`finstack_quant_valuations::instruments::TermLoan`]. Construction and
//! validation stay in Rust; the wrappers convert to and from the tagged
//! instrument JSON accepted by the JSON loader (`{"type": "bond", "spec":
//! ...}` / `{"type": "term_loan", "spec": ...}`).
//!
//! To price a typed instrument, pass its `toJson()` output to the generic
//! pricing entry points (`valuations.instruments.priceInstrument`,
//! `priceInstrumentWithMetrics`, `instrumentCashflowsJson`).

use crate::api::core::dates::{JsDayCount, JsTenor};
use crate::api::core::money::JsMoney;
use crate::api::core::types::{JsBps, JsRate};
use crate::utils::{parse_iso_date, to_js_err};
use finstack_quant_valuations::instruments::{Instrument, InstrumentJson};
use wasm_bindgen::prelude::*;

/// Parse tagged instrument JSON through the JSON-loader path.
fn parse_tagged(json: &str) -> Result<InstrumentJson, JsValue> {
    serde_json::from_str::<InstrumentJson>(json).map_err(to_js_err)
}

// ---------------------------------------------------------------------------
// Bond
// ---------------------------------------------------------------------------

/// Typed wrapper for the Rust `Bond` instrument.
#[wasm_bindgen(js_name = Bond)]
#[derive(Clone)]
pub struct JsBond {
    #[wasm_bindgen(skip)]
    pub(crate) inner: finstack_quant_valuations::instruments::Bond,
}

#[wasm_bindgen(js_class = Bond)]
impl JsBond {
    /// Create a standard fixed-rate bond (semi-annual, 30/360, T+2).
    ///
    /// Mirrors Rust `Bond::fixed`.
    /// @param id - Unique instrument identifier.
    /// @param notional - Principal amount of the bond.
    /// @param coupon_rate - Annual coupon rate.
    /// @param issue - Issue date as an ISO-8601 string (`"YYYY-MM-DD"`).
    /// @param maturity - Maturity date as an ISO-8601 string (`"YYYY-MM-DD"`).
    /// @param discount_curve_id - Discount curve identifier used for pricing.
    /// @returns The validated fixed-rate bond.
    /// @throws If validation fails (e.g. maturity not after issue).
    pub fn fixed(
        id: &str,
        notional: &JsMoney,
        coupon_rate: &JsRate,
        issue: &str,
        maturity: &str,
        discount_curve_id: &str,
    ) -> Result<JsBond, JsValue> {
        let inner = finstack_quant_valuations::instruments::Bond::fixed(
            id,
            notional.inner,
            coupon_rate.inner,
            parse_iso_date(issue)?,
            parse_iso_date(maturity)?,
            discount_curve_id,
        )
        .map_err(to_js_err)?;
        Ok(JsBond { inner })
    }

    /// Create a floating-rate bond (FRN) linked to a forward index.
    ///
    /// Mirrors Rust `Bond::floating`.
    /// @param id - Unique instrument identifier.
    /// @param notional - Principal amount of the bond.
    /// @param index_id - Forward curve identifier (e.g. `"USD-SOFR-3M"`).
    /// @param margin_bp - Spread over the index in basis points.
    /// @param issue - Issue date as an ISO-8601 string (`"YYYY-MM-DD"`).
    /// @param maturity - Maturity date as an ISO-8601 string (`"YYYY-MM-DD"`).
    /// @param freq - Payment frequency (e.g. `Tenor.quarterly()`).
    /// @param dc - Day count convention (e.g. `DayCount.act360()`).
    /// @param discount_curve_id - Discount curve identifier used for pricing.
    /// @returns The validated floating-rate note.
    /// @throws If validation fails.
    #[allow(clippy::too_many_arguments)]
    pub fn floating(
        id: &str,
        notional: &JsMoney,
        index_id: &str,
        margin_bp: &JsBps,
        issue: &str,
        maturity: &str,
        freq: &JsTenor,
        dc: &JsDayCount,
        discount_curve_id: &str,
    ) -> Result<JsBond, JsValue> {
        let inner = finstack_quant_valuations::instruments::Bond::floating(
            id,
            notional.inner,
            index_id,
            margin_bp.inner,
            parse_iso_date(issue)?,
            parse_iso_date(maturity)?,
            freq.inner,
            dc.inner,
            discount_curve_id,
        )
        .map_err(to_js_err)?;
        Ok(JsBond { inner })
    }

    /// Deserialize a bond from tagged instrument JSON.
    ///
    /// Accepts the same `{"type": "bond", "spec": {...}}` payload the JSON
    /// loader accepts; the loader's validation runs on the result.
    /// @param json - Tagged instrument JSON with type `"bond"`.
    /// @returns The validated bond.
    /// @throws If the JSON is malformed, has a different instrument type, or fails validation.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: &str) -> Result<JsBond, JsValue> {
        match parse_tagged(json)? {
            InstrumentJson::Bond(inner) => {
                inner.validate_for_pricing().map_err(to_js_err)?;
                Ok(JsBond { inner })
            }
            _ => Err(JsValue::from_str(
                "expected instrument type \"bond\", got a different instrument type",
            )),
        }
    }

    /// Serialize to tagged instrument JSON (`{"type": "bond", "spec": ...}`).
    ///
    /// Pass the result to `valuations.instruments.priceInstrument` (or the
    /// other generic pricing entry points) to price this bond.
    /// @returns Tagged instrument JSON accepted by `priceInstrument` and `Bond.fromJson`.
    /// @throws If serialization fails.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&InstrumentJson::Bond(self.inner.clone())).map_err(to_js_err)
    }

    /// Instrument identifier.
    #[wasm_bindgen(getter)]
    pub fn id(&self) -> String {
        self.inner.id.to_string()
    }
}

// ---------------------------------------------------------------------------
// TermLoan
// ---------------------------------------------------------------------------

/// Typed wrapper for the Rust `TermLoan` instrument.
///
/// Rust has no `fixed`/`floating` convenience constructors for term loans;
/// construct via `TermLoan.fromJson` with tagged JSON
/// (`{"type": "term_loan", "spec": ...}`) or start from `TermLoan.example()`.
#[wasm_bindgen(js_name = TermLoan)]
#[derive(Clone)]
pub struct JsTermLoan {
    #[wasm_bindgen(skip)]
    pub(crate) inner: finstack_quant_valuations::instruments::TermLoan,
}

#[wasm_bindgen(js_class = TermLoan)]
impl JsTermLoan {
    /// Deserialize a term loan from tagged instrument JSON.
    ///
    /// Accepts the same `{"type": "term_loan", "spec": {...}}` payload the
    /// JSON loader accepts; the loader's validation runs on the result.
    /// @param json - Tagged instrument JSON with type `"term_loan"`.
    /// @returns The validated term loan.
    /// @throws If the JSON is malformed, has a different instrument type, or fails validation.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: &str) -> Result<JsTermLoan, JsValue> {
        match parse_tagged(json)? {
            InstrumentJson::TermLoan(inner) => {
                inner.validate_for_pricing().map_err(to_js_err)?;
                Ok(JsTermLoan { inner })
            }
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

    /// Serialize to tagged instrument JSON (`{"type": "term_loan", "spec": ...}`).
    ///
    /// Pass the result to `valuations.instruments.priceInstrument` (or the
    /// other generic pricing entry points) to price this loan.
    /// @returns Tagged instrument JSON accepted by `priceInstrument` and `TermLoan.fromJson`.
    /// @throws If serialization fails.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&InstrumentJson::TermLoan(self.inner.clone())).map_err(to_js_err)
    }

    /// Instrument identifier.
    #[wasm_bindgen(getter)]
    pub fn id(&self) -> String {
        self.inner.id.to_string()
    }
}
