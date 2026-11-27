//! FRTB Sensitivity-Based Approach: sensitivity builder, engine and charge.
//!
//! `FrtbSensitivities` (a mutable builder) and `FrtbSbaEngine` are WASM
//! classes whose members mirror the Python classes; `FrtbSbaResult` is a plain
//! JSON value typed by the generated TypeScript.

use crate::utils::input::js_currency;
use crate::utils::input::{from_js_json, js_bool, js_f64, js_opt_string_seq, js_string, js_uint};
use crate::utils::wire::parse_label;
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_margin::regulatory::frtb::{
    CorrelationScenario, DrcPosition, FrtbRiskClass, FrtbSbaEngine, FrtbSensitivities,
};
use wasm_bindgen::prelude::*;

/// FRTB sensitivity portfolio for the Sensitivity-Based Approach.
///
/// Build up delta, vega, curvature, DRC and RRAO inputs with the `add*`
/// methods, then pass the handle to `FrtbSbaEngine.calculate` to compute the
/// capital charge under one or more correlation scenarios per BCBS d457.
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
/// sens.addGirrDelta("USD", "USD-OIS", "5Y", 100_000);
/// new margin.FrtbSbaEngine().calculate(sens).total; // capital charge in USD
/// ```
#[wasm_bindgen(js_name = FrtbSensitivities)]
#[derive(Clone)]
pub struct JsFrtbSensitivities {
    inner: FrtbSensitivities,
}

json_round_trip!(JsFrtbSensitivities, FrtbSensitivities);

macro_rules! frtb_wasm_adders {
    (
        ccy1: [$( $(#[$c_doc:meta])* ($c_method:ident, $c_js:ident) ),* $(,)?],
        delta5: [$( $(#[$d_doc:meta])* ($d_method:ident, $d_js:ident, $d_id:ident) ),* $(,)?],
        vega4: [$( $(#[$v_doc:meta])* ($v_method:ident, $v_js:ident, $v_id:ident) ),* $(,)?],
        cvr4: [$( $(#[$k_doc:meta])* ($k_method:ident, $k_js:ident, $k_id:ident) ),* $(,)?],
        eq3: [$( $(#[$e_doc:meta])* ($e_method:ident, $e_js:ident) ),* $(,)?] $(,)?
    ) => {
        #[finstack_quant_wasm_macros::plain_docs]
        #[wasm_bindgen(js_class = FrtbSensitivities)]
        impl JsFrtbSensitivities {
            $(
                $(#[$c_doc])*
                #[wasm_bindgen(js_name = $c_js)]
                pub fn $c_method(
                    &mut self,
                    #[wasm_bindgen(unchecked_param_type = "string")] currency: JsValue,
                    #[wasm_bindgen(unchecked_param_type = "number")] amount: JsValue,
                ) -> Result<(), JsValue> {
                    let amount = js_f64(&amount, "amount")?;
                    let currency = js_currency(&currency, "currency")?;
                    self.inner.$c_method(currency, amount);
                    Ok(())
                }
            )*
            $(
                $(#[$d_doc])*
                #[wasm_bindgen(js_name = $d_js)]
                pub fn $d_method(
                    &mut self,
                    #[wasm_bindgen(unchecked_param_type = "string")] $d_id: JsValue,
                    #[wasm_bindgen(unchecked_param_type = "number")] bucket: JsValue,
                    #[wasm_bindgen(unchecked_param_type = "string")] tenor: JsValue,
                    #[wasm_bindgen(unchecked_param_type = "string")] basis: JsValue,
                    #[wasm_bindgen(unchecked_param_type = "number")] amount: JsValue,
                ) -> Result<(), JsValue> {
                    let $d_id = js_string(&$d_id, stringify!($d_id))?;
                    let bucket: u8 = js_uint(&bucket, "bucket")?;
                    let tenor = js_string(&tenor, "tenor")?;
                    let basis = js_string(&basis, "basis")?;
                    let amount = js_f64(&amount, "amount")?;
                    self.inner.$d_method(&$d_id, bucket, &tenor, &basis, amount);
                    Ok(())
                }
            )*
            $(
                $(#[$v_doc])*
                #[wasm_bindgen(js_name = $v_js)]
                pub fn $v_method(
                    &mut self,
                    #[wasm_bindgen(unchecked_param_type = "string")] $v_id: JsValue,
                    #[wasm_bindgen(unchecked_param_type = "number")] bucket: JsValue,
                    #[wasm_bindgen(unchecked_param_type = "string")] maturity: JsValue,
                    #[wasm_bindgen(unchecked_param_type = "number")] amount: JsValue,
                ) -> Result<(), JsValue> {
                    let $v_id = js_string(&$v_id, stringify!($v_id))?;
                    let bucket: u8 = js_uint(&bucket, "bucket")?;
                    let maturity = js_string(&maturity, "maturity")?;
                    let amount = js_f64(&amount, "amount")?;
                    self.inner.$v_method(&$v_id, bucket, &maturity, amount);
                    Ok(())
                }
            )*
            $(
                $(#[$k_doc])*
                #[wasm_bindgen(js_name = $k_js)]
                pub fn $k_method(
                    &mut self,
                    #[wasm_bindgen(unchecked_param_type = "string")] $k_id: JsValue,
                    #[wasm_bindgen(unchecked_param_type = "number")] bucket: JsValue,
                    #[wasm_bindgen(js_name = "cvrUp", unchecked_param_type = "number")] cvr_up: JsValue,
                    #[wasm_bindgen(js_name = "cvrDown", unchecked_param_type = "number")] cvr_down: JsValue,
                ) -> Result<(), JsValue> {
                    let $k_id = js_string(&$k_id, stringify!($k_id))?;
                    let bucket: u8 = js_uint(&bucket, "bucket")?;
                    let cvr_up = js_f64(&cvr_up, "cvrUp")?;
                    let cvr_down = js_f64(&cvr_down, "cvrDown")?;
                    self.inner.$k_method(&$k_id, bucket, cvr_up, cvr_down);
                    Ok(())
                }
            )*
            $(
                $(#[$e_doc])*
                #[wasm_bindgen(js_name = $e_js)]
                pub fn $e_method(
                    &mut self,
                    #[wasm_bindgen(unchecked_param_type = "string")] underlier: JsValue,
                    #[wasm_bindgen(unchecked_param_type = "number")] bucket: JsValue,
                    #[wasm_bindgen(unchecked_param_type = "number")] amount: JsValue,
                ) -> Result<(), JsValue> {
                    let underlier = js_string(&underlier, "underlier")?;
                    let bucket: u8 = js_uint(&bucket, "bucket")?;
                    let amount = js_f64(&amount, "amount")?;
                    self.inner.$e_method(&underlier, bucket, amount);
                    Ok(())
                }
            )*
        }
    };
}

frtb_wasm_adders!(
    ccy1: [
        /// Add a GIRR inflation delta sensitivity.
        /// @param amount - Base-currency P&L per 1 percentage point of inflation shift.
        /// @param currency - ISO-4217 currency of the inflation curve of the shocked risk factor.
        ///
        /// # Errors
        ///
        /// Throws if the currency is not a known ISO-4217 code.
        (add_girr_inflation_delta, addGirrInflationDelta),
        /// Add a GIRR cross-currency basis delta sensitivity.
        /// @param amount - Base-currency P&L per 1 percentage point of basis shift.
        /// @param currency - ISO-4217 currency whose basis moves of the shocked risk factor.
        ///
        /// # Errors
        ///
        /// Throws if the currency is not a known ISO-4217 code.
        (add_girr_xccy_basis_delta, addGirrXccyBasisDelta),
    ],
    delta5: [
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
        (add_csr_nonsec_delta, addCsrNonsecDelta, issuer),
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
        (add_csr_sec_ctp_delta, addCsrSecCtpDelta, tranche),
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
        (add_csr_sec_nonctp_delta, addCsrSecNonctpDelta, tranche),
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
        (add_commodity_delta, addCommodityDelta, name),
    ],
    vega4: [
        /// Add a CSR non-securitisation vega sensitivity.
        /// @param issuer - Issuer or reference-entity identifier.
        /// @param bucket - 1-based CSR non-securitisation bucket (MAR21.51).
        /// @param maturity - Option maturity label such as `"1Y"`.
        /// @param amount - Volatility-scaled vega (sigma times dV/dsigma) in the base currency.
        ///
        /// # Errors
        ///
        /// Throws a `TypeError` for an argument of the wrong type, including a bucket that is not an integer in `0..=255`.
        (add_csr_nonsec_vega, addCsrNonsecVega, issuer),
        /// Add a CSR securitisation (correlation trading portfolio) vega sensitivity.
        /// @param tranche - Tranche or index identifier.
        /// @param bucket - 1-based CSR securitisation CTP bucket (MAR21.59).
        /// @param maturity - Option maturity label such as `"1Y"`.
        /// @param amount - Volatility-scaled vega (sigma times dV/dsigma) in the base currency.
        ///
        /// # Errors
        ///
        /// Throws a `TypeError` for an argument of the wrong type, including a bucket that is not an integer in `0..=255`.
        (add_csr_sec_ctp_vega, addCsrSecCtpVega, tranche),
        /// Add a CSR securitisation (non-CTP) vega sensitivity.
        /// @param tranche - Tranche identifier.
        /// @param bucket - 1-based CSR securitisation non-CTP bucket (MAR21.64).
        /// @param maturity - Option maturity label such as `"1Y"`.
        /// @param amount - Volatility-scaled vega (sigma times dV/dsigma) in the base currency.
        ///
        /// # Errors
        ///
        /// Throws a `TypeError` for an argument of the wrong type, including a bucket that is not an integer in `0..=255`.
        (add_csr_sec_nonctp_vega, addCsrSecNonctpVega, tranche),
        /// Add a commodity vega sensitivity.
        /// @param name - Commodity identifier.
        /// @param bucket - 1-based commodity bucket (MAR21.82).
        /// @param maturity - Option maturity label such as `"1Y"`.
        /// @param amount - Volatility-scaled vega (sigma times dV/dsigma) in the base currency.
        ///
        /// # Errors
        ///
        /// Throws a `TypeError` for an argument of the wrong type, including a bucket that is not an integer in `0..=255`.
        (add_commodity_vega, addCommodityVega, name),
        /// Add an equity vega sensitivity.
        /// @param underlier - Equity underlier or index identifier.
        /// @param bucket - 1-based equity bucket (MAR21.72).
        /// @param maturity - Option maturity label such as `"1Y"`.
        /// @param amount - Volatility-scaled vega (sigma times dV/dsigma) in the base currency.
        ///
        /// # Errors
        ///
        /// Throws a `TypeError` for an argument of the wrong type, including a bucket that is not an integer in `0..=255`.
        (add_equity_vega, addEquityVega, underlier),
    ],
    cvr4: [
        /// Add a CSR non-securitisation curvature pair.
        /// @param issuer - Issuer or reference-entity identifier.
        /// @param bucket - 1-based CSR non-securitisation bucket (MAR21.51).
        /// @param cvrUp - Curvature risk position under the upward spread shock, in the base currency.
        /// @param cvrDown - Curvature risk position under the downward spread shock, in the base currency.
        ///
        /// # Errors
        ///
        /// Throws a `TypeError` for an argument of the wrong type, including a bucket that is not an integer in `0..=255`.
        (add_csr_nonsec_curvature, addCsrNonsecCurvature, issuer),
        /// Add a CSR securitisation (correlation trading portfolio) curvature pair.
        /// @param tranche - Tranche or index identifier.
        /// @param bucket - 1-based CSR securitisation CTP bucket (MAR21.59).
        /// @param cvrUp - Curvature risk position under the upward spread shock, in the base currency.
        /// @param cvrDown - Curvature risk position under the downward spread shock, in the base currency.
        ///
        /// # Errors
        ///
        /// Throws a `TypeError` for an argument of the wrong type, including a bucket that is not an integer in `0..=255`.
        (add_csr_sec_ctp_curvature, addCsrSecCtpCurvature, tranche),
        /// Add a CSR securitisation (non-CTP) curvature pair.
        /// @param tranche - Tranche identifier.
        /// @param bucket - 1-based CSR securitisation non-CTP bucket (MAR21.64).
        /// @param cvrUp - Curvature risk position under the upward spread shock, in the base currency.
        /// @param cvrDown - Curvature risk position under the downward spread shock, in the base currency.
        ///
        /// # Errors
        ///
        /// Throws a `TypeError` for an argument of the wrong type, including a bucket that is not an integer in `0..=255`.
        (add_csr_sec_nonctp_curvature, addCsrSecNonctpCurvature, tranche),
        /// Add a commodity curvature pair.
        /// @param name - Commodity identifier.
        /// @param bucket - 1-based commodity bucket (MAR21.82).
        /// @param cvrUp - Curvature risk position under the upward price shock, in the base currency.
        /// @param cvrDown - Curvature risk position under the downward price shock, in the base currency.
        ///
        /// # Errors
        ///
        /// Throws a `TypeError` for an argument of the wrong type, including a bucket that is not an integer in `0..=255`.
        (add_commodity_curvature, addCommodityCurvature, name),
        /// Add an equity curvature pair.
        /// @param underlier - Equity underlier or index identifier.
        /// @param bucket - 1-based equity bucket (MAR21.72).
        /// @param cvrUp - Curvature risk position under the upward price shock, in the base currency.
        /// @param cvrDown - Curvature risk position under the downward price shock, in the base currency.
        ///
        /// # Errors
        ///
        /// Throws a `TypeError` for an argument of the wrong type, including a bucket that is not an integer in `0..=255`.
        (add_equity_curvature, addEquityCurvature, underlier),
    ],
    eq3: [
        /// Add an equity delta sensitivity.
        /// @param underlier - Equity underlier or index identifier.
        /// @param bucket - 1-based equity bucket (MAR21.72).
        /// @param amount - Base-currency P&L per 1 percentage point move in the underlier.
        ///
        /// # Errors
        ///
        /// Throws a `TypeError` for an argument of the wrong type, including a bucket that is not an integer in `0..=255`.
        (add_equity_delta, addEquityDelta),
        /// Add an equity repo-rate delta sensitivity.
        /// @param underlier - Equity underlier or index identifier.
        /// @param bucket - 1-based equity bucket (MAR21.72).
        /// @param amount - Base-currency P&L per 1 percentage point parallel repo-rate shift.
        ///
        /// # Errors
        ///
        /// Throws a `TypeError` for an argument of the wrong type, including a bucket that is not an integer in `0..=255`.
        (add_equity_repo_delta, addEquityRepoDelta),
    ],
);

#[wasm_bindgen(js_class = FrtbSensitivities)]
impl JsFrtbSensitivities {
    /// Create an empty FRTB sensitivity set in one reporting currency.
    /// @param baseCurrency - ISO-4217 currency used for all SBA sensitivities and capital amounts.
    ///
    /// # Errors
    ///
    /// Throws if `base_currency` is not a known ISO-4217 code.
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(js_name = "baseCurrency", unchecked_param_type = "string")]
        base_currency: JsValue,
    ) -> Result<JsFrtbSensitivities, JsValue> {
        Ok(Self {
            inner: FrtbSensitivities::new(js_currency(&base_currency, "baseCurrency")?),
        })
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

    /// Add an ordinary GIRR yield delta at one curve vertex.
    /// @param currency - ISO-4217 risk currency of the curve.
    /// @param curveId - Exact yield curve identifier distinguishing OIS or a specific term-index curve.
    /// @param tenor - Case-sensitive regulatory vertex such as "5Y".
    /// @param amount - Signed reporting-currency P&L from the upward one-bp reprice, multiplied by 100.
    /// @throws TypeError for mistyped arguments or FinstackError for an invalid currency, missing curve identity, unknown tenor or non-finite amount or aggregate.
    #[wasm_bindgen(js_name = addGirrDelta)]
    pub fn add_girr_delta(
        &mut self,
        #[wasm_bindgen(unchecked_param_type = "string")] currency: JsValue,
        #[wasm_bindgen(js_name = "curveId", unchecked_param_type = "string")] curve_id: JsValue,
        #[wasm_bindgen(unchecked_param_type = "string")] tenor: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] amount: JsValue,
    ) -> Result<(), JsValue> {
        self.inner
            .add_girr_delta(
                js_currency(&currency, "currency")?,
                js_string(&curve_id, "curveId")?,
                &js_string(&tenor, "tenor")?,
                js_f64(&amount, "amount")?,
            )
            .map_err(to_js_err)
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
        #[wasm_bindgen(unchecked_param_type = "string")] ccy1: JsValue,
        #[wasm_bindgen(unchecked_param_type = "string")] ccy2: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] amount: JsValue,
    ) -> Result<(), JsValue> {
        let ccy1 = js_currency(&ccy1, "ccy1")?;
        let ccy2 = js_currency(&ccy2, "ccy2")?;
        let amount = js_f64(&amount, "amount")?;
        self.inner.add_fx_delta(ccy1, ccy2, amount);
        Ok(())
    }

    /// Add a GIRR vega sensitivity.
    /// @param optionMaturity - Option maturity label such as `"1Y"`.
    /// @param underlyingTenor - Underlying swap tenor label such as `"5Y"`.
    /// @param amount - Volatility-scaled vega (sigma times dV/dsigma) in the base currency.
    /// @param currency - ISO-4217 currency of the curve of the shocked risk factor.
    ///
    /// # Errors
    ///
    /// Throws if the currency is not a known ISO-4217 code.
    #[wasm_bindgen(js_name = addGirrVega)]
    pub fn add_girr_vega(
        &mut self,
        #[wasm_bindgen(unchecked_param_type = "string")] currency: JsValue,
        #[wasm_bindgen(js_name = "optionMaturity", unchecked_param_type = "string")]
        option_maturity: JsValue,
        #[wasm_bindgen(js_name = "underlyingTenor", unchecked_param_type = "string")]
        underlying_tenor: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] amount: JsValue,
    ) -> Result<(), JsValue> {
        let option_maturity = js_string(&option_maturity, "optionMaturity")?;
        let underlying_tenor = js_string(&underlying_tenor, "underlyingTenor")?;
        let amount = js_f64(&amount, "amount")?;
        let currency = js_currency(&currency, "currency")?;
        self.inner
            .add_girr_vega(currency, &option_maturity, &underlying_tenor, amount);
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
        #[wasm_bindgen(unchecked_param_type = "string")] ccy1: JsValue,
        #[wasm_bindgen(unchecked_param_type = "string")] ccy2: JsValue,
        #[wasm_bindgen(unchecked_param_type = "string")] maturity: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] amount: JsValue,
    ) -> Result<(), JsValue> {
        let ccy1 = js_currency(&ccy1, "ccy1")?;
        let ccy2 = js_currency(&ccy2, "ccy2")?;
        let maturity = js_string(&maturity, "maturity")?;
        let amount = js_f64(&amount, "amount")?;
        self.inner.add_fx_vega(ccy1, ccy2, &maturity, amount);
        Ok(())
    }

    /// Add a GIRR curvature pair.
    /// @param cvrUp - Curvature risk position under the upward rate shock, in the base currency.
    /// @param cvrDown - Curvature risk position under the downward rate shock, in the base currency.
    /// @param currency - ISO-4217 currency of the curve of the shocked risk factor.
    ///
    /// # Errors
    ///
    /// Throws if the currency is not a known ISO-4217 code.
    #[wasm_bindgen(js_name = addGirrCurvature)]
    pub fn add_girr_curvature(
        &mut self,
        #[wasm_bindgen(unchecked_param_type = "string")] currency: JsValue,
        #[wasm_bindgen(js_name = "cvrUp", unchecked_param_type = "number")] cvr_up: JsValue,
        #[wasm_bindgen(js_name = "cvrDown", unchecked_param_type = "number")] cvr_down: JsValue,
    ) -> Result<(), JsValue> {
        let cvr_up = js_f64(&cvr_up, "cvrUp")?;
        let cvr_down = js_f64(&cvr_down, "cvrDown")?;
        let currency = js_currency(&currency, "currency")?;
        self.inner.add_girr_curvature(currency, cvr_up, cvr_down);
        Ok(())
    }

    /// Add an FX curvature pair for a currency pair.
    /// @param ccy1 - First ISO-4217 currency of the FX pair.
    /// @param ccy2 - Second ISO-4217 currency of the FX pair.
    /// @param cvrUp - Curvature risk position under the upward FX shock, in the base currency.
    /// @param cvrDown - Curvature risk position under the downward FX shock, in the base currency.
    ///
    /// # Errors
    ///
    /// Throws if `ccy1` or `ccy2` is not a known ISO-4217 code.
    #[wasm_bindgen(js_name = addFxCurvature)]
    pub fn add_fx_curvature(
        &mut self,
        #[wasm_bindgen(unchecked_param_type = "string")] ccy1: JsValue,
        #[wasm_bindgen(unchecked_param_type = "string")] ccy2: JsValue,
        #[wasm_bindgen(js_name = "cvrUp", unchecked_param_type = "number")] cvr_up: JsValue,
        #[wasm_bindgen(js_name = "cvrDown", unchecked_param_type = "number")] cvr_down: JsValue,
    ) -> Result<(), JsValue> {
        let ccy1 = js_currency(&ccy1, "ccy1")?;
        let ccy2 = js_currency(&ccy2, "ccy2")?;
        let cvr_up = js_f64(&cvr_up, "cvrUp")?;
        let cvr_down = js_f64(&cvr_down, "cvrDown")?;
        self.inner.add_fx_curvature(ccy1, ccy2, cvr_up, cvr_down);
        Ok(())
    }

    /// Add a canonical non-securitisation DRC position.
    /// @param position - DrcPosition object or JSON: issuer, signed jtd_amount in reporting currency before LGD, rating_bucket 1..9, sector, seniority, non-negative maturity_years and optional pnl_adjustment (default 0). Equity seniority requires corporate sector; securitisations are unsupported.
    /// @throws Error - If fields are unknown, malformed or fail Rust position validation.
    #[wasm_bindgen(js_name = addDrcPosition)]
    pub fn add_drc_position(
        &mut self,
        #[wasm_bindgen(
            unchecked_param_type = "FrtbSensitivitiesWire['drc_positions'][number] | string"
        )]
        position: JsValue,
    ) -> Result<(), JsValue> {
        let position: DrcPosition = from_js_json(&position, "position")?;
        position.validate().map_err(to_js_err)?;
        self.inner.add_drc_position(position);
        Ok(())
    }

    /// Add a Residual Risk Add-On position.
    /// @param instrumentId - Instrument identifier.
    /// @param notional - Gross notional in the base currency.
    /// @param isExotic - Required: `true` for an exotic underlying (1.0% weight); `false` for other residual risk such as gap, correlation or behavioural risk (0.1% weight).
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` for an argument of the wrong type.
    #[wasm_bindgen(js_name = addRraoPosition)]
    pub fn add_rrao_position(
        &mut self,
        #[wasm_bindgen(js_name = "instrumentId", unchecked_param_type = "string")]
        instrument_id: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] notional: JsValue,
        #[wasm_bindgen(js_name = "isExotic", unchecked_param_type = "boolean")] is_exotic: JsValue,
    ) -> Result<(), JsValue> {
        let instrument_id = js_string(&instrument_id, "instrumentId")?;
        let notional = js_f64(&notional, "notional")?;
        let is_exotic = js_bool(&is_exotic, "isExotic")?;
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
/// sens.addGirrDelta("USD", "USD-OIS", "5Y", 100_000);
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
    /// @param riskClasses - Lower-case risk-class labels to include (`"girr"`, `"csr_non_sec"`, `"csr_sec_ctp"`, `"csr_sec_non_ctp"`, `"equity"`, `"commodity"`, `"fx"`); omitted includes all seven.
    ///
    /// # Errors
    ///
    /// Throws if a label is unknown or either list is empty.
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(unchecked_optional_param_type = "readonly CorrelationScenario[] | null")]
        scenarios: Option<JsValue>,
        #[wasm_bindgen(
            js_name = "riskClasses",
            unchecked_optional_param_type = "readonly FrtbRiskClass[] | null"
        )]
        risk_classes: Option<JsValue>,
    ) -> Result<JsFrtbSbaEngine, JsValue> {
        let scenarios = js_opt_string_seq(scenarios.as_ref(), "scenarios")?
            .map(|labels| {
                labels
                    .iter()
                    .map(|label| parse_label::<CorrelationScenario>(label))
                    .collect::<Result<Vec<_>, JsValue>>()
            })
            .transpose()?;
        let risk_classes = js_opt_string_seq(risk_classes.as_ref(), "riskClasses")?
            .map(|labels| {
                labels
                    .iter()
                    .map(|label| parse_label::<FrtbRiskClass>(label))
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
    #[wasm_bindgen(getter, unchecked_return_type = "CorrelationScenario[]")]
    pub fn scenarios(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.scenarios())
    }

    /// Risk-class labels included, in configured order.
    ///
    /// # Errors
    ///
    /// Throws if the labels cannot be converted to a JavaScript value.
    #[wasm_bindgen(getter, js_name = riskClasses, unchecked_return_type = "FrtbRiskClass[]")]
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
    #[wasm_bindgen(unchecked_return_type = "FrtbSbaResult")]
    pub fn calculate(&self, sensitivities: &JsFrtbSensitivities) -> Result<JsValue, JsValue> {
        to_js_value(
            &self
                .inner
                .calculate(&sensitivities.inner)
                .map_err(to_js_err)?,
        )
    }
}

/// Construct a checked non-securitisation Default Risk Charge position.
/// @param position - Native issuer, signed JTD basis, residual maturity, rating, sector, seniority and optional signed P&L adjustment.
/// @returns Independent validated position in the reporting currency designated by the caller.
/// @throws FinstackError for missing, unknown or invalid position fields.
#[wasm_bindgen(js_name = drcPosition, unchecked_return_type = "generated.margin.DrcPosition")]
pub fn drc_position(
    #[wasm_bindgen(unchecked_param_type = "generated.margin.DrcPosition")] position: JsValue,
) -> Result<JsValue, JsValue> {
    let position: DrcPosition = from_js_json(&position, "position")?;
    position.validate().map_err(to_js_err)?;
    to_js_value(&position)
}
