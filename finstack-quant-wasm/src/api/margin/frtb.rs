//! FRTB Sensitivity-Based Approach: sensitivity builder, engine and charge.
//!
//! `FrtbSensitivities` (a mutable builder) and `FrtbSbaEngine` are WASM
//! classes whose members mirror the Python classes; `FrtbSbaResult` is a plain
//! JSON value typed by the generated TypeScript.

use super::js_currency;
use crate::utils::input::{
    from_js_json, js_f64, js_opt_bool, js_opt_f64, js_opt_string, js_opt_string_seq, js_string,
    js_uint,
};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_core::currency::Currency;
use finstack_quant_margin::regulatory::frtb::{
    frtb_sba_charge as frtb_sba_charge_rs, CorrelationScenario, DrcAssetType, DrcPosition,
    DrcSector, DrcSeniority, FrtbRiskClass, FrtbSbaEngine, FrtbSensitivities,
};
use wasm_bindgen::prelude::*;

/// Parse a lower-case serde label (scenario, risk class, DRC sector, …).
fn serde_label<T: serde::de::DeserializeOwned>(label: &str) -> Result<T, JsValue> {
    finstack_quant_core::wire::serde_parse(label).map_err(to_js_err)
}

/// FRTB sensitivity portfolio for the Sensitivity-Based Approach.
///
/// Build up delta, vega, curvature, DRC and RRAO inputs with the `add*`
/// methods, then pass the handle to `frtbSbaCharge` or
/// `FrtbSbaEngine.calculate` to compute the capital charge under one or more
/// correlation scenarios per BCBS d457.
///
/// Units: GIRR deltas are base-currency P&L per 1 percentage point of curve
/// shift (`100 * DV01`); CSR deltas are per 1 percentage point of spread;
/// equity, commodity and FX deltas are per 1 percentage point of the
/// underlying; vegas are volatility-scaled; curvature pairs are the up/down
/// shocked P&L positions; DRC amounts are signed JTD notionals before LGD;
/// RRAO amounts are gross notionals. Bucket numbers are 1-based FRTB buckets.
///
/// @example
/// ```javascript
/// import init, { margin } from "finstack-quant-wasm";
/// await init();
/// const sens = new margin.FrtbSensitivities("USD");
/// sens.addGirrDelta("5Y", 100_000);
/// margin.frtbSbaCharge(sens).total; // capital charge in USD
/// ```
#[wasm_bindgen(js_name = FrtbSensitivities)]
#[derive(Clone)]
pub struct JsFrtbSensitivities {
    inner: FrtbSensitivities,
}

impl JsFrtbSensitivities {
    fn currency_or_base(
        &self,
        currency: Option<&JsValue>,
        label: &str,
    ) -> Result<Currency, JsValue> {
        match js_opt_string(currency, label)? {
            Some(code) => code.parse().map_err(to_js_err),
            None => Ok(self.inner.base_currency),
        }
    }
}

#[wasm_bindgen(js_class = FrtbSensitivities)]
impl JsFrtbSensitivities {
    /// Create an empty FRTB sensitivity set in one reporting currency.
    /// @param base_currency - ISO-4217 currency used for all SBA sensitivities and capital amounts.
    ///
    /// # Errors
    ///
    /// Throws if `base_currency` is not a known ISO-4217 code.
    #[wasm_bindgen(constructor)]
    pub fn new(base_currency: JsValue) -> Result<JsFrtbSensitivities, JsValue> {
        Ok(Self {
            inner: FrtbSensitivities::new(js_currency(&base_currency, "baseCurrency")?),
        })
    }

    /// Deserialize sensitivities from the canonical Rust JSON shape.
    /// @param json - `FrtbSensitivities` JSON text or plain object.
    /// @returns A `FrtbSensitivities` handle.
    ///
    /// # Errors
    ///
    /// Throws if the JSON is malformed or has unknown fields.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<JsFrtbSensitivities, JsValue> {
        Ok(Self {
            inner: from_js_json(&json, "json")?,
        })
    }

    /// Serialize these sensitivities to the canonical Rust JSON shape.
    /// @returns Compact JSON text accepted by `FrtbSensitivities.fromJson`.
    ///
    /// # Errors
    ///
    /// Throws if serialization fails.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.inner).map_err(to_js_err)
    }

    /// Validate labels, buckets, identifiers and amounts without pricing.
    ///
    /// The engine runs this automatically; call it directly to check a
    /// container built from external data.
    ///
    /// # Errors
    ///
    /// Throws a validation error naming the first invalid field when a tenor
    /// or bucket is unsupported, an identifier is empty, or a value is
    /// non-finite.
    pub fn validate(&self) -> Result<(), JsValue> {
        self.inner.validate().map_err(to_js_err)
    }

    /// Add a GIRR delta sensitivity.
    /// @param tenor - GIRR tenor bucket, such as `"5Y"`.
    /// @param amount - Signed base-currency P&L per 1 percentage point of curve shift (`100 * DV01`).
    /// @param currency - ISO-4217 currency of the curve; omitted uses the base currency.
    ///
    /// # Errors
    ///
    /// Throws if a supplied currency is not a known ISO-4217 code.
    #[wasm_bindgen(js_name = addGirrDelta)]
    pub fn add_girr_delta(
        &mut self,
        tenor: JsValue,
        amount: JsValue,
        currency: Option<JsValue>,
    ) -> Result<(), JsValue> {
        let tenor = js_string(&tenor, "tenor")?;
        let amount = js_f64(&amount, "amount")?;
        let currency = self.currency_or_base(currency.as_ref(), "currency")?;
        self.inner.add_girr_delta(currency, &tenor, amount);
        Ok(())
    }

    /// Add a GIRR inflation delta sensitivity.
    /// @param amount - Base-currency P&L per 1 percentage point of inflation shift.
    /// @param currency - ISO-4217 currency of the inflation curve; omitted uses the base currency.
    ///
    /// # Errors
    ///
    /// Throws if a supplied currency is not a known ISO-4217 code.
    #[wasm_bindgen(js_name = addGirrInflationDelta)]
    pub fn add_girr_inflation_delta(
        &mut self,
        amount: JsValue,
        currency: Option<JsValue>,
    ) -> Result<(), JsValue> {
        let amount = js_f64(&amount, "amount")?;
        let currency = self.currency_or_base(currency.as_ref(), "currency")?;
        self.inner.add_girr_inflation_delta(currency, amount);
        Ok(())
    }

    /// Add a GIRR cross-currency basis delta sensitivity.
    /// @param amount - Base-currency P&L per 1 percentage point of basis shift.
    /// @param currency - ISO-4217 currency whose basis moves; omitted uses the base currency.
    ///
    /// # Errors
    ///
    /// Throws if a supplied currency is not a known ISO-4217 code.
    #[wasm_bindgen(js_name = addGirrXccyBasisDelta)]
    pub fn add_girr_xccy_basis_delta(
        &mut self,
        amount: JsValue,
        currency: Option<JsValue>,
    ) -> Result<(), JsValue> {
        let amount = js_f64(&amount, "amount")?;
        let currency = self.currency_or_base(currency.as_ref(), "currency")?;
        self.inner.add_girr_xccy_basis_delta(currency, amount);
        Ok(())
    }

    /// Add a CSR non-securitisation delta sensitivity.
    /// @param issuer - Issuer or reference-entity identifier.
    /// @param bucket - 1-based CSR non-securitisation bucket (MAR21.51).
    /// @param tenor - Credit-spread tenor label such as `"5Y"`.
    /// @param basis - Spread curve identifier (bond or CDS) or commodity delivery location; equal labels identify the same basis for correlation.
    /// @param amount - Base-currency P&L per 1 percentage point of spread move (`100 * CS01`).
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` for an argument of the wrong type, including a bucket that is not an integer in `0..=255`.
    #[wasm_bindgen(js_name = addCsrNonsecDelta)]
    pub fn add_csr_nonsec_delta(
        &mut self,
        issuer: JsValue,
        bucket: JsValue,
        tenor: JsValue,
        basis: JsValue,
        amount: JsValue,
    ) -> Result<(), JsValue> {
        let issuer = js_string(&issuer, "issuer")?;
        let bucket: u8 = js_uint(&bucket, "bucket")?;
        let tenor = js_string(&tenor, "tenor")?;
        let basis = js_string(&basis, "basis")?;
        let amount = js_f64(&amount, "amount")?;
        self.inner
            .add_csr_nonsec_delta(&issuer, bucket, &tenor, &basis, amount);
        Ok(())
    }

    /// Add a CSR non-securitisation vega sensitivity.
    /// @param issuer - Issuer or reference-entity identifier.
    /// @param bucket - 1-based CSR non-securitisation bucket (MAR21.51).
    /// @param maturity - Option maturity label such as `"1Y"`.
    /// @param amount - Volatility-scaled vega (sigma times dV/dsigma) in the base currency.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` for an argument of the wrong type, including a bucket that is not an integer in `0..=255`.
    #[wasm_bindgen(js_name = addCsrNonsecVega)]
    pub fn add_csr_nonsec_vega(
        &mut self,
        issuer: JsValue,
        bucket: JsValue,
        maturity: JsValue,
        amount: JsValue,
    ) -> Result<(), JsValue> {
        let issuer = js_string(&issuer, "issuer")?;
        let bucket: u8 = js_uint(&bucket, "bucket")?;
        let maturity = js_string(&maturity, "maturity")?;
        let amount = js_f64(&amount, "amount")?;
        self.inner
            .add_csr_nonsec_vega(&issuer, bucket, &maturity, amount);
        Ok(())
    }

    /// Add a CSR non-securitisation curvature pair.
    /// @param issuer - Issuer or reference-entity identifier.
    /// @param bucket - 1-based CSR non-securitisation bucket (MAR21.51).
    /// @param cvr_up - Curvature risk position under the upward spread shock, in the base currency.
    /// @param cvr_down - Curvature risk position under the downward spread shock, in the base currency.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` for an argument of the wrong type, including a bucket that is not an integer in `0..=255`.
    #[wasm_bindgen(js_name = addCsrNonsecCurvature)]
    pub fn add_csr_nonsec_curvature(
        &mut self,
        issuer: JsValue,
        bucket: JsValue,
        cvr_up: JsValue,
        cvr_down: JsValue,
    ) -> Result<(), JsValue> {
        let issuer = js_string(&issuer, "issuer")?;
        let bucket: u8 = js_uint(&bucket, "bucket")?;
        let cvr_up = js_f64(&cvr_up, "cvrUp")?;
        let cvr_down = js_f64(&cvr_down, "cvrDown")?;
        self.inner
            .add_csr_nonsec_curvature(&issuer, bucket, cvr_up, cvr_down);
        Ok(())
    }

    /// Add a CSR securitisation (correlation trading portfolio) delta sensitivity.
    /// @param tranche - Tranche or index identifier.
    /// @param bucket - 1-based CSR securitisation CTP bucket (MAR21.59).
    /// @param tenor - Credit-spread tenor label such as `"5Y"`.
    /// @param basis - Spread curve identifier (bond or CDS) or commodity delivery location; equal labels identify the same basis for correlation.
    /// @param amount - Base-currency P&L per 1 percentage point of spread move (`100 * CS01`).
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` for an argument of the wrong type, including a bucket that is not an integer in `0..=255`.
    #[wasm_bindgen(js_name = addCsrSecCtpDelta)]
    pub fn add_csr_sec_ctp_delta(
        &mut self,
        tranche: JsValue,
        bucket: JsValue,
        tenor: JsValue,
        basis: JsValue,
        amount: JsValue,
    ) -> Result<(), JsValue> {
        let tranche = js_string(&tranche, "tranche")?;
        let bucket: u8 = js_uint(&bucket, "bucket")?;
        let tenor = js_string(&tenor, "tenor")?;
        let basis = js_string(&basis, "basis")?;
        let amount = js_f64(&amount, "amount")?;
        self.inner
            .add_csr_sec_ctp_delta(&tranche, bucket, &tenor, &basis, amount);
        Ok(())
    }

    /// Add a CSR securitisation (correlation trading portfolio) vega sensitivity.
    /// @param tranche - Tranche or index identifier.
    /// @param bucket - 1-based CSR securitisation CTP bucket (MAR21.59).
    /// @param maturity - Option maturity label such as `"1Y"`.
    /// @param amount - Volatility-scaled vega (sigma times dV/dsigma) in the base currency.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` for an argument of the wrong type, including a bucket that is not an integer in `0..=255`.
    #[wasm_bindgen(js_name = addCsrSecCtpVega)]
    pub fn add_csr_sec_ctp_vega(
        &mut self,
        tranche: JsValue,
        bucket: JsValue,
        maturity: JsValue,
        amount: JsValue,
    ) -> Result<(), JsValue> {
        let tranche = js_string(&tranche, "tranche")?;
        let bucket: u8 = js_uint(&bucket, "bucket")?;
        let maturity = js_string(&maturity, "maturity")?;
        let amount = js_f64(&amount, "amount")?;
        self.inner
            .add_csr_sec_ctp_vega(&tranche, bucket, &maturity, amount);
        Ok(())
    }

    /// Add a CSR securitisation (correlation trading portfolio) curvature pair.
    /// @param tranche - Tranche or index identifier.
    /// @param bucket - 1-based CSR securitisation CTP bucket (MAR21.59).
    /// @param cvr_up - Curvature risk position under the upward spread shock, in the base currency.
    /// @param cvr_down - Curvature risk position under the downward spread shock, in the base currency.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` for an argument of the wrong type, including a bucket that is not an integer in `0..=255`.
    #[wasm_bindgen(js_name = addCsrSecCtpCurvature)]
    pub fn add_csr_sec_ctp_curvature(
        &mut self,
        tranche: JsValue,
        bucket: JsValue,
        cvr_up: JsValue,
        cvr_down: JsValue,
    ) -> Result<(), JsValue> {
        let tranche = js_string(&tranche, "tranche")?;
        let bucket: u8 = js_uint(&bucket, "bucket")?;
        let cvr_up = js_f64(&cvr_up, "cvrUp")?;
        let cvr_down = js_f64(&cvr_down, "cvrDown")?;
        self.inner
            .add_csr_sec_ctp_curvature(&tranche, bucket, cvr_up, cvr_down);
        Ok(())
    }

    /// Add a CSR securitisation (non-CTP) delta sensitivity.
    /// @param tranche - Tranche identifier.
    /// @param bucket - 1-based CSR securitisation non-CTP bucket (MAR21.64).
    /// @param tenor - Credit-spread tenor label such as `"5Y"`.
    /// @param basis - Spread curve identifier (bond or CDS) or commodity delivery location; equal labels identify the same basis for correlation.
    /// @param amount - Base-currency P&L per 1 percentage point of spread move (`100 * CS01`).
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` for an argument of the wrong type, including a bucket that is not an integer in `0..=255`.
    #[wasm_bindgen(js_name = addCsrSecNonctpDelta)]
    pub fn add_csr_sec_nonctp_delta(
        &mut self,
        tranche: JsValue,
        bucket: JsValue,
        tenor: JsValue,
        basis: JsValue,
        amount: JsValue,
    ) -> Result<(), JsValue> {
        let tranche = js_string(&tranche, "tranche")?;
        let bucket: u8 = js_uint(&bucket, "bucket")?;
        let tenor = js_string(&tenor, "tenor")?;
        let basis = js_string(&basis, "basis")?;
        let amount = js_f64(&amount, "amount")?;
        self.inner
            .add_csr_sec_nonctp_delta(&tranche, bucket, &tenor, &basis, amount);
        Ok(())
    }

    /// Add a CSR securitisation (non-CTP) vega sensitivity.
    /// @param tranche - Tranche identifier.
    /// @param bucket - 1-based CSR securitisation non-CTP bucket (MAR21.64).
    /// @param maturity - Option maturity label such as `"1Y"`.
    /// @param amount - Volatility-scaled vega (sigma times dV/dsigma) in the base currency.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` for an argument of the wrong type, including a bucket that is not an integer in `0..=255`.
    #[wasm_bindgen(js_name = addCsrSecNonctpVega)]
    pub fn add_csr_sec_nonctp_vega(
        &mut self,
        tranche: JsValue,
        bucket: JsValue,
        maturity: JsValue,
        amount: JsValue,
    ) -> Result<(), JsValue> {
        let tranche = js_string(&tranche, "tranche")?;
        let bucket: u8 = js_uint(&bucket, "bucket")?;
        let maturity = js_string(&maturity, "maturity")?;
        let amount = js_f64(&amount, "amount")?;
        self.inner
            .add_csr_sec_nonctp_vega(&tranche, bucket, &maturity, amount);
        Ok(())
    }

    /// Add a CSR securitisation (non-CTP) curvature pair.
    /// @param tranche - Tranche identifier.
    /// @param bucket - 1-based CSR securitisation non-CTP bucket (MAR21.64).
    /// @param cvr_up - Curvature risk position under the upward spread shock, in the base currency.
    /// @param cvr_down - Curvature risk position under the downward spread shock, in the base currency.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` for an argument of the wrong type, including a bucket that is not an integer in `0..=255`.
    #[wasm_bindgen(js_name = addCsrSecNonctpCurvature)]
    pub fn add_csr_sec_nonctp_curvature(
        &mut self,
        tranche: JsValue,
        bucket: JsValue,
        cvr_up: JsValue,
        cvr_down: JsValue,
    ) -> Result<(), JsValue> {
        let tranche = js_string(&tranche, "tranche")?;
        let bucket: u8 = js_uint(&bucket, "bucket")?;
        let cvr_up = js_f64(&cvr_up, "cvrUp")?;
        let cvr_down = js_f64(&cvr_down, "cvrDown")?;
        self.inner
            .add_csr_sec_nonctp_curvature(&tranche, bucket, cvr_up, cvr_down);
        Ok(())
    }

    /// Add an equity delta sensitivity.
    /// @param underlier - Equity underlier or index identifier.
    /// @param bucket - 1-based equity bucket (MAR21.72).
    /// @param amount - Base-currency P&L per 1 percentage point move in the underlier.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` for an argument of the wrong type, including a bucket that is not an integer in `0..=255`.
    #[wasm_bindgen(js_name = addEquityDelta)]
    pub fn add_equity_delta(
        &mut self,
        underlier: JsValue,
        bucket: JsValue,
        amount: JsValue,
    ) -> Result<(), JsValue> {
        let underlier = js_string(&underlier, "underlier")?;
        let bucket: u8 = js_uint(&bucket, "bucket")?;
        let amount = js_f64(&amount, "amount")?;
        self.inner.add_equity_delta(&underlier, bucket, amount);
        Ok(())
    }

    /// Add an equity repo-rate delta sensitivity.
    /// @param underlier - Equity underlier or index identifier.
    /// @param bucket - 1-based equity bucket (MAR21.72).
    /// @param amount - Base-currency P&L per 1 percentage point parallel repo-rate shift.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` for an argument of the wrong type, including a bucket that is not an integer in `0..=255`.
    #[wasm_bindgen(js_name = addEquityRepoDelta)]
    pub fn add_equity_repo_delta(
        &mut self,
        underlier: JsValue,
        bucket: JsValue,
        amount: JsValue,
    ) -> Result<(), JsValue> {
        let underlier = js_string(&underlier, "underlier")?;
        let bucket: u8 = js_uint(&bucket, "bucket")?;
        let amount = js_f64(&amount, "amount")?;
        self.inner.add_equity_repo_delta(&underlier, bucket, amount);
        Ok(())
    }

    /// Add an FX delta sensitivity for a currency pair.
    /// @param ccy1 - First ISO-4217 currency of the FX pair.
    /// @param ccy2 - Second ISO-4217 currency of the FX pair.
    /// @param amount - Base-currency P&L per 1 percentage point move in the exchange rate.
    ///
    /// # Errors
    ///
    /// Throws if `ccy1` or `ccy2` is not a known ISO-4217 code.
    #[wasm_bindgen(js_name = addFxDelta)]
    pub fn add_fx_delta(
        &mut self,
        ccy1: JsValue,
        ccy2: JsValue,
        amount: JsValue,
    ) -> Result<(), JsValue> {
        let ccy1 = js_currency(&ccy1, "ccy1")?;
        let ccy2 = js_currency(&ccy2, "ccy2")?;
        let amount = js_f64(&amount, "amount")?;
        self.inner.add_fx_delta(ccy1, ccy2, amount);
        Ok(())
    }

    /// Add a commodity delta sensitivity.
    /// @param name - Commodity identifier.
    /// @param bucket - 1-based commodity bucket (MAR21.82).
    /// @param tenor - Commodity tenor label such as `"1Y"`.
    /// @param basis - Spread curve identifier (bond or CDS) or commodity delivery location; equal labels identify the same basis for correlation.
    /// @param amount - Base-currency P&L per 1 percentage point move in the commodity price.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` for an argument of the wrong type, including a bucket that is not an integer in `0..=255`.
    #[wasm_bindgen(js_name = addCommodityDelta)]
    pub fn add_commodity_delta(
        &mut self,
        name: JsValue,
        bucket: JsValue,
        tenor: JsValue,
        basis: JsValue,
        amount: JsValue,
    ) -> Result<(), JsValue> {
        let name = js_string(&name, "name")?;
        let bucket: u8 = js_uint(&bucket, "bucket")?;
        let tenor = js_string(&tenor, "tenor")?;
        let basis = js_string(&basis, "basis")?;
        let amount = js_f64(&amount, "amount")?;
        self.inner
            .add_commodity_delta(&name, bucket, &tenor, &basis, amount);
        Ok(())
    }

    /// Add a commodity vega sensitivity.
    /// @param name - Commodity identifier.
    /// @param bucket - 1-based commodity bucket (MAR21.82).
    /// @param maturity - Option maturity label such as `"1Y"`.
    /// @param amount - Volatility-scaled vega (sigma times dV/dsigma) in the base currency.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` for an argument of the wrong type, including a bucket that is not an integer in `0..=255`.
    #[wasm_bindgen(js_name = addCommodityVega)]
    pub fn add_commodity_vega(
        &mut self,
        name: JsValue,
        bucket: JsValue,
        maturity: JsValue,
        amount: JsValue,
    ) -> Result<(), JsValue> {
        let name = js_string(&name, "name")?;
        let bucket: u8 = js_uint(&bucket, "bucket")?;
        let maturity = js_string(&maturity, "maturity")?;
        let amount = js_f64(&amount, "amount")?;
        self.inner
            .add_commodity_vega(&name, bucket, &maturity, amount);
        Ok(())
    }

    /// Add a commodity curvature pair.
    /// @param name - Commodity identifier.
    /// @param bucket - 1-based commodity bucket (MAR21.82).
    /// @param cvr_up - Curvature risk position under the upward price shock, in the base currency.
    /// @param cvr_down - Curvature risk position under the downward price shock, in the base currency.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` for an argument of the wrong type, including a bucket that is not an integer in `0..=255`.
    #[wasm_bindgen(js_name = addCommodityCurvature)]
    pub fn add_commodity_curvature(
        &mut self,
        name: JsValue,
        bucket: JsValue,
        cvr_up: JsValue,
        cvr_down: JsValue,
    ) -> Result<(), JsValue> {
        let name = js_string(&name, "name")?;
        let bucket: u8 = js_uint(&bucket, "bucket")?;
        let cvr_up = js_f64(&cvr_up, "cvrUp")?;
        let cvr_down = js_f64(&cvr_down, "cvrDown")?;
        self.inner
            .add_commodity_curvature(&name, bucket, cvr_up, cvr_down);
        Ok(())
    }

    /// Add a GIRR vega sensitivity.
    /// @param option_maturity - Option maturity label such as `"1Y"`.
    /// @param underlying_tenor - Underlying swap tenor label such as `"5Y"`.
    /// @param amount - Volatility-scaled vega (sigma times dV/dsigma) in the base currency.
    /// @param currency - ISO-4217 currency of the curve; omitted uses the base currency.
    ///
    /// # Errors
    ///
    /// Throws if a supplied currency is not a known ISO-4217 code.
    #[wasm_bindgen(js_name = addGirrVega)]
    pub fn add_girr_vega(
        &mut self,
        option_maturity: JsValue,
        underlying_tenor: JsValue,
        amount: JsValue,
        currency: Option<JsValue>,
    ) -> Result<(), JsValue> {
        let option_maturity = js_string(&option_maturity, "optionMaturity")?;
        let underlying_tenor = js_string(&underlying_tenor, "underlyingTenor")?;
        let amount = js_f64(&amount, "amount")?;
        let currency = self.currency_or_base(currency.as_ref(), "currency")?;
        self.inner
            .add_girr_vega(currency, &option_maturity, &underlying_tenor, amount);
        Ok(())
    }

    /// Add an equity vega sensitivity.
    /// @param underlier - Equity underlier or index identifier.
    /// @param bucket - 1-based equity bucket (MAR21.72).
    /// @param maturity - Option maturity label such as `"1Y"`.
    /// @param amount - Volatility-scaled vega (sigma times dV/dsigma) in the base currency.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` for an argument of the wrong type, including a bucket that is not an integer in `0..=255`.
    #[wasm_bindgen(js_name = addEquityVega)]
    pub fn add_equity_vega(
        &mut self,
        underlier: JsValue,
        bucket: JsValue,
        maturity: JsValue,
        amount: JsValue,
    ) -> Result<(), JsValue> {
        let underlier = js_string(&underlier, "underlier")?;
        let bucket: u8 = js_uint(&bucket, "bucket")?;
        let maturity = js_string(&maturity, "maturity")?;
        let amount = js_f64(&amount, "amount")?;
        self.inner
            .add_equity_vega(&underlier, bucket, &maturity, amount);
        Ok(())
    }

    /// Add an FX vega sensitivity for a currency pair.
    /// @param ccy1 - First ISO-4217 currency of the FX pair.
    /// @param ccy2 - Second ISO-4217 currency of the FX pair.
    /// @param maturity - Option maturity label such as `"1Y"`.
    /// @param amount - Volatility-scaled vega (sigma times dV/dsigma) in the base currency.
    ///
    /// # Errors
    ///
    /// Throws if `ccy1` or `ccy2` is not a known ISO-4217 code.
    #[wasm_bindgen(js_name = addFxVega)]
    pub fn add_fx_vega(
        &mut self,
        ccy1: JsValue,
        ccy2: JsValue,
        maturity: JsValue,
        amount: JsValue,
    ) -> Result<(), JsValue> {
        let ccy1 = js_currency(&ccy1, "ccy1")?;
        let ccy2 = js_currency(&ccy2, "ccy2")?;
        let maturity = js_string(&maturity, "maturity")?;
        let amount = js_f64(&amount, "amount")?;
        self.inner.add_fx_vega(ccy1, ccy2, &maturity, amount);
        Ok(())
    }

    /// Add a GIRR curvature pair.
    /// @param cvr_up - Curvature risk position under the upward rate shock, in the base currency.
    /// @param cvr_down - Curvature risk position under the downward rate shock, in the base currency.
    /// @param currency - ISO-4217 currency of the curve; omitted uses the base currency.
    ///
    /// # Errors
    ///
    /// Throws if a supplied currency is not a known ISO-4217 code.
    #[wasm_bindgen(js_name = addGirrCurvature)]
    pub fn add_girr_curvature(
        &mut self,
        cvr_up: JsValue,
        cvr_down: JsValue,
        currency: Option<JsValue>,
    ) -> Result<(), JsValue> {
        let cvr_up = js_f64(&cvr_up, "cvrUp")?;
        let cvr_down = js_f64(&cvr_down, "cvrDown")?;
        let currency = self.currency_or_base(currency.as_ref(), "currency")?;
        self.inner.add_girr_curvature(currency, cvr_up, cvr_down);
        Ok(())
    }

    /// Add an equity curvature pair.
    /// @param underlier - Equity underlier or index identifier.
    /// @param bucket - 1-based equity bucket (MAR21.72).
    /// @param cvr_up - Curvature risk position under the upward price shock, in the base currency.
    /// @param cvr_down - Curvature risk position under the downward price shock, in the base currency.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` for an argument of the wrong type, including a bucket that is not an integer in `0..=255`.
    #[wasm_bindgen(js_name = addEquityCurvature)]
    pub fn add_equity_curvature(
        &mut self,
        underlier: JsValue,
        bucket: JsValue,
        cvr_up: JsValue,
        cvr_down: JsValue,
    ) -> Result<(), JsValue> {
        let underlier = js_string(&underlier, "underlier")?;
        let bucket: u8 = js_uint(&bucket, "bucket")?;
        let cvr_up = js_f64(&cvr_up, "cvrUp")?;
        let cvr_down = js_f64(&cvr_down, "cvrDown")?;
        self.inner
            .add_equity_curvature(&underlier, bucket, cvr_up, cvr_down);
        Ok(())
    }

    /// Add an FX curvature pair for a currency pair.
    /// @param ccy1 - First ISO-4217 currency of the FX pair.
    /// @param ccy2 - Second ISO-4217 currency of the FX pair.
    /// @param cvr_up - Curvature risk position under the upward FX shock, in the base currency.
    /// @param cvr_down - Curvature risk position under the downward FX shock, in the base currency.
    ///
    /// # Errors
    ///
    /// Throws if `ccy1` or `ccy2` is not a known ISO-4217 code.
    #[wasm_bindgen(js_name = addFxCurvature)]
    pub fn add_fx_curvature(
        &mut self,
        ccy1: JsValue,
        ccy2: JsValue,
        cvr_up: JsValue,
        cvr_down: JsValue,
    ) -> Result<(), JsValue> {
        let ccy1 = js_currency(&ccy1, "ccy1")?;
        let ccy2 = js_currency(&ccy2, "ccy2")?;
        let cvr_up = js_f64(&cvr_up, "cvrUp")?;
        let cvr_down = js_f64(&cvr_down, "cvrDown")?;
        self.inner.add_fx_curvature(ccy1, ccy2, cvr_up, cvr_down);
        Ok(())
    }

    /// Add a Default Risk Charge position.
    /// @param issuer - Issuer identifier; long and short JTD net per issuer at charge time.
    /// @param jtd_amount - Signed jump-to-default notional in the base currency (positive long, negative short), before the seniority LGD.
    /// @param rating_bucket - Credit-rating bucket, 1 (AAA) to 9 (defaulted) per MAR22.24.
    /// @param sector - `"corporate"`, `"sovereign"` or `"local_government"`.
    /// @param seniority - `"senior_unsecured"`, `"subordinated"`, `"equity"` or `"covered_bond"`; selects the LGD.
    /// @param asset_type - DRC asset type label such as `"corporate"`, `"sovereign"`, `"local_government"` or `"equity"`.
    /// @param maturity_years - Residual maturity in years, finite and non-negative; JTD scales by maturity clipped to `[0.25, 1.0]`.
    /// @param pnl_adjustment - Mark-to-market adjustment per MAR22.9 (negative for a long position carrying an unrealised loss); defaults to `0`.
    ///
    /// # Errors
    ///
    /// Throws if `sector`, `seniority` or `asset_type` is not a known label,
    /// or `rating_bucket` is not an integer in `0..=255`.
    #[wasm_bindgen(js_name = addDrcPosition)]
    #[allow(clippy::too_many_arguments)]
    pub fn add_drc_position(
        &mut self,
        issuer: JsValue,
        jtd_amount: JsValue,
        rating_bucket: JsValue,
        sector: JsValue,
        seniority: JsValue,
        asset_type: JsValue,
        maturity_years: JsValue,
        pnl_adjustment: Option<JsValue>,
    ) -> Result<(), JsValue> {
        self.inner.add_drc_position(DrcPosition {
            maturity_years: js_f64(&maturity_years, "maturityYears")?,
            issuer: js_string(&issuer, "issuer")?,
            jtd_amount: js_f64(&jtd_amount, "jtdAmount")?,
            rating_bucket: js_uint(&rating_bucket, "ratingBucket")?,
            sector: serde_label::<DrcSector>(&js_string(&sector, "sector")?)?,
            seniority: serde_label::<DrcSeniority>(&js_string(&seniority, "seniority")?)?,
            asset_type: serde_label::<DrcAssetType>(&js_string(&asset_type, "assetType")?)?,
            pnl_adjustment: js_opt_f64(pnl_adjustment.as_ref(), "pnlAdjustment")?
                .unwrap_or_default(),
        });
        Ok(())
    }

    /// Add a Residual Risk Add-On position.
    /// @param instrument_id - Instrument identifier.
    /// @param notional - Gross notional in the base currency.
    /// @param is_exotic - `true` for an exotic underlying (1.0% weight); `false` (the default) for other residual risk such as gap, correlation or behavioural risk (0.1% weight).
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` for an argument of the wrong type.
    #[wasm_bindgen(js_name = addRraoPosition)]
    pub fn add_rrao_position(
        &mut self,
        instrument_id: JsValue,
        notional: JsValue,
        is_exotic: Option<JsValue>,
    ) -> Result<(), JsValue> {
        let instrument_id = js_string(&instrument_id, "instrumentId")?;
        let notional = js_f64(&notional, "notional")?;
        let is_exotic = js_opt_bool(is_exotic.as_ref(), "isExotic")?.unwrap_or_default();
        self.inner
            .add_rrao_position(&instrument_id, notional, is_exotic);
        Ok(())
    }

    /// ISO-4217 reporting currency of every sensitivity and capital amount.
    #[wasm_bindgen(getter, js_name = baseCurrency)]
    pub fn base_currency(&self) -> String {
        self.inner.base_currency.to_string()
    }
}

// ---------------------------------------------------------------------------
// FrtbSbaEngine
// ---------------------------------------------------------------------------

/// FRTB SBA engine.
///
/// Evaluates delta, vega and curvature under each configured correlation
/// scenario, takes the maximum, then adds DRC and RRAO (BCBS d457).
///
/// @example
/// ```javascript
/// import init, { margin } from "finstack-quant-wasm";
/// await init();
/// const engine = new margin.FrtbSbaEngine(["low", "high"], ["girr", "fx"]);
/// engine.scenarios; // ["low", "high"]
/// const sens = new margin.FrtbSensitivities("USD");
/// sens.addGirrDelta("5Y", 100_000);
/// engine.calculate(sens).binding_scenario;
/// ```
#[wasm_bindgen(js_name = FrtbSbaEngine)]
pub struct JsFrtbSbaEngine {
    inner: FrtbSbaEngine,
}

#[wasm_bindgen(js_class = FrtbSbaEngine)]
impl JsFrtbSbaEngine {
    /// Select the correlation scenarios and risk classes the engine evaluates.
    /// @param scenarios - Lower-case scenario labels (`"low"`, `"medium"`, `"high"`); the charge is the maximum across them. Omitted evaluates all three.
    /// @param risk_classes - Lower-case risk-class labels to include (`"girr"`, `"csr_non_sec"`, `"csr_sec_ctp"`, `"csr_sec_non_ctp"`, `"equity"`, `"commodity"`, `"fx"`); omitted includes all seven.
    ///
    /// # Errors
    ///
    /// Throws if a label is unknown or either list is empty.
    #[wasm_bindgen(constructor)]
    pub fn new(
        scenarios: Option<JsValue>,
        risk_classes: Option<JsValue>,
    ) -> Result<JsFrtbSbaEngine, JsValue> {
        let scenarios = js_opt_string_seq(scenarios.as_ref(), "scenarios")?
            .map(|labels| {
                labels
                    .iter()
                    .map(|label| serde_label::<CorrelationScenario>(label))
                    .collect::<Result<Vec<_>, JsValue>>()
            })
            .transpose()?;
        let risk_classes = js_opt_string_seq(risk_classes.as_ref(), "riskClasses")?
            .map(|labels| {
                labels
                    .iter()
                    .map(|label| serde_label::<FrtbRiskClass>(label))
                    .collect::<Result<Vec<_>, JsValue>>()
            })
            .transpose()?;
        Ok(Self {
            inner: FrtbSbaEngine::with_selection(scenarios, risk_classes).map_err(to_js_err)?,
        })
    }

    /// Correlation scenario labels evaluated, in configured order.
    ///
    /// # Errors
    ///
    /// Throws if the labels cannot be converted to a JavaScript value.
    #[wasm_bindgen(getter)]
    pub fn scenarios(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.scenarios())
    }

    /// Risk-class labels included, in configured order.
    ///
    /// # Errors
    ///
    /// Throws if the labels cannot be converted to a JavaScript value.
    #[wasm_bindgen(getter, js_name = riskClasses)]
    pub fn risk_classes(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.risk_classes())
    }

    /// Calculate the FRTB SBA charge for a sensitivity portfolio.
    /// @param sensitivities - Portfolio of FRTB sensitivities (delta, vega, curvature, DRC, RRAO).
    /// @returns The `FrtbSbaResult` as a plain object: `total`, the per-risk-class delta, vega and curvature breakdown, `drc`, `rrao`, and the per-scenario charges with the binding one named.
    ///
    /// # Errors
    ///
    /// Throws if a sensitivity has an unsupported tenor or bucket, an empty
    /// required identifier, or a non-finite value.
    pub fn calculate(&self, sensitivities: &JsFrtbSensitivities) -> Result<JsValue, JsValue> {
        to_js_value(
            &self
                .inner
                .calculate(&sensitivities.inner)
                .map_err(to_js_err)?,
        )
    }
}

/// FRTB SBA capital charge for one sensitivity portfolio.
/// @param sensitivities - Portfolio of FRTB sensitivities (delta, vega, curvature, DRC, RRAO).
/// @param correlation_scenario - `"low"`, `"medium"` or `"high"` to evaluate only that scenario; omitted runs all three and reports the binding (maximum) one per BCBS d457.
/// @returns The `FrtbSbaResult` as a plain object: `total`, the per-risk-class delta, vega and curvature breakdown, `drc`, `rrao`, and the per-scenario charges with the binding one named.
///
/// # Errors
///
/// Throws if `correlation_scenario` is unknown, or a sensitivity has an
/// unsupported tenor or bucket, an empty required identifier, or a non-finite
/// value.
#[wasm_bindgen(js_name = frtbSbaCharge)]
pub fn frtb_sba_charge(
    sensitivities: &JsFrtbSensitivities,
    correlation_scenario: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let scenario = js_opt_string(correlation_scenario.as_ref(), "correlationScenario")?
        .map(|label| serde_label::<CorrelationScenario>(&label))
        .transpose()?;
    to_js_value(&frtb_sba_charge_rs(&sensitivities.inner, scenario).map_err(to_js_err)?)
}
