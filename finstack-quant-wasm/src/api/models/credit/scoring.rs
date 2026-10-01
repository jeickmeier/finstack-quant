//! WASM bindings for accounting-based credit scoring models.
//!
//! Mirrors `finstack-quant-py/src/bindings/models/credit/scoring.rs`. Every
//! function returns the canonical `ScoringResult` as a plain object
//! (`score`, `zone`, `implied_pd`, `model`).

use crate::utils::input::js_f64;
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_models::credit::scoring::{
    self, AltmanZDoublePrimeInput, AltmanZPrimeInput, AltmanZScoreInput, OhlsonOScoreInput,
    ZmijewskiInput,
};
use wasm_bindgen::prelude::*;

/// Original Altman (1968) Z-Score for publicly traded manufacturers.
/// @param working_capital_to_total_assets - Working capital divided by total assets (ratio X1).
/// @param retained_earnings_to_total_assets - Retained earnings divided by total assets (ratio X2).
/// @param ebit_to_total_assets - Earnings before interest and taxes divided by total assets (ratio X3).
/// @param market_equity_to_total_liabilities - Market value of equity divided by total liabilities (ratio X4).
/// @param sales_to_total_assets - Sales divided by total assets (ratio X5).
/// @returns The `ScoringResult` object (`score`, `zone`, `implied_pd`, `model`).
///
/// # Errors
///
/// Throws a `validation` error if any ratio is non-finite.
#[wasm_bindgen(js_name = altmanZScore)]
pub fn altman_z_score(
    working_capital_to_total_assets: JsValue,
    retained_earnings_to_total_assets: JsValue,
    ebit_to_total_assets: JsValue,
    market_equity_to_total_liabilities: JsValue,
    sales_to_total_assets: JsValue,
) -> Result<JsValue, JsValue> {
    let input = AltmanZScoreInput {
        working_capital_to_total_assets: js_f64(
            &working_capital_to_total_assets,
            "workingCapitalToTotalAssets",
        )?,
        retained_earnings_to_total_assets: js_f64(
            &retained_earnings_to_total_assets,
            "retainedEarningsToTotalAssets",
        )?,
        ebit_to_total_assets: js_f64(&ebit_to_total_assets, "ebitToTotalAssets")?,
        market_equity_to_total_liabilities: js_f64(
            &market_equity_to_total_liabilities,
            "marketEquityToTotalLiabilities",
        )?,
        sales_to_total_assets: js_f64(&sales_to_total_assets, "salesToTotalAssets")?,
    };
    to_js_value(&scoring::altman_z_score(&input).map_err(to_js_err)?)
}

/// Altman Z'-Score for private firms (book equity replaces market equity).
/// @param working_capital_to_total_assets - Working capital divided by total assets (ratio X1).
/// @param retained_earnings_to_total_assets - Retained earnings divided by total assets (ratio X2).
/// @param ebit_to_total_assets - Earnings before interest and taxes divided by total assets (ratio X3).
/// @param book_equity_to_total_liabilities - Book value of equity divided by total liabilities (ratio X4).
/// @param sales_to_total_assets - Sales divided by total assets (ratio X5).
/// @returns The `ScoringResult` object (`score`, `zone`, `implied_pd`, `model`).
///
/// # Errors
///
/// Throws a `validation` error if any ratio is non-finite.
#[wasm_bindgen(js_name = altmanZPrime)]
pub fn altman_z_prime(
    working_capital_to_total_assets: JsValue,
    retained_earnings_to_total_assets: JsValue,
    ebit_to_total_assets: JsValue,
    book_equity_to_total_liabilities: JsValue,
    sales_to_total_assets: JsValue,
) -> Result<JsValue, JsValue> {
    let input = AltmanZPrimeInput {
        working_capital_to_total_assets: js_f64(
            &working_capital_to_total_assets,
            "workingCapitalToTotalAssets",
        )?,
        retained_earnings_to_total_assets: js_f64(
            &retained_earnings_to_total_assets,
            "retainedEarningsToTotalAssets",
        )?,
        ebit_to_total_assets: js_f64(&ebit_to_total_assets, "ebitToTotalAssets")?,
        book_equity_to_total_liabilities: js_f64(
            &book_equity_to_total_liabilities,
            "bookEquityToTotalLiabilities",
        )?,
        sales_to_total_assets: js_f64(&sales_to_total_assets, "salesToTotalAssets")?,
    };
    to_js_value(&scoring::altman_z_prime(&input).map_err(to_js_err)?)
}

fn double_prime_input(
    working_capital_to_total_assets: &JsValue,
    retained_earnings_to_total_assets: &JsValue,
    ebit_to_total_assets: &JsValue,
    book_equity_to_total_liabilities: &JsValue,
) -> Result<AltmanZDoublePrimeInput, JsValue> {
    Ok(AltmanZDoublePrimeInput {
        working_capital_to_total_assets: js_f64(
            working_capital_to_total_assets,
            "workingCapitalToTotalAssets",
        )?,
        retained_earnings_to_total_assets: js_f64(
            retained_earnings_to_total_assets,
            "retainedEarningsToTotalAssets",
        )?,
        ebit_to_total_assets: js_f64(ebit_to_total_assets, "ebitToTotalAssets")?,
        book_equity_to_total_liabilities: js_f64(
            book_equity_to_total_liabilities,
            "bookEquityToTotalLiabilities",
        )?,
    })
}

/// Altman Z''-Score for non-manufacturers (no sales ratio).
/// @param working_capital_to_total_assets - Working capital divided by total assets (ratio X1).
/// @param retained_earnings_to_total_assets - Retained earnings divided by total assets (ratio X2).
/// @param ebit_to_total_assets - Earnings before interest and taxes divided by total assets (ratio X3).
/// @param book_equity_to_total_liabilities - Book value of equity divided by total liabilities (ratio X4).
/// @returns The `ScoringResult` object (`score`, `zone`, `implied_pd`, `model`).
///
/// # Errors
///
/// Throws a `validation` error if any ratio is non-finite.
#[wasm_bindgen(js_name = altmanZDoublePrime)]
pub fn altman_z_double_prime(
    working_capital_to_total_assets: JsValue,
    retained_earnings_to_total_assets: JsValue,
    ebit_to_total_assets: JsValue,
    book_equity_to_total_liabilities: JsValue,
) -> Result<JsValue, JsValue> {
    let input = double_prime_input(
        &working_capital_to_total_assets,
        &retained_earnings_to_total_assets,
        &ebit_to_total_assets,
        &book_equity_to_total_liabilities,
    )?;
    to_js_value(&scoring::altman_z_double_prime(&input).map_err(to_js_err)?)
}

/// Altman emerging-market score: the Z''-Score plus a constant of 3.25.
/// @param working_capital_to_total_assets - Working capital divided by total assets (ratio X1).
/// @param retained_earnings_to_total_assets - Retained earnings divided by total assets (ratio X2).
/// @param ebit_to_total_assets - Earnings before interest and taxes divided by total assets (ratio X3).
/// @param book_equity_to_total_liabilities - Book value of equity divided by total liabilities (ratio X4).
/// @returns The `ScoringResult` object (`score`, `zone`, `implied_pd`, `model`).
///
/// # Errors
///
/// Throws a `validation` error if any ratio is non-finite.
#[wasm_bindgen(js_name = altmanEmScore)]
pub fn altman_em_score(
    working_capital_to_total_assets: JsValue,
    retained_earnings_to_total_assets: JsValue,
    ebit_to_total_assets: JsValue,
    book_equity_to_total_liabilities: JsValue,
) -> Result<JsValue, JsValue> {
    let input = double_prime_input(
        &working_capital_to_total_assets,
        &retained_earnings_to_total_assets,
        &ebit_to_total_assets,
        &book_equity_to_total_liabilities,
    )?;
    to_js_value(&scoring::altman_em_score(&input).map_err(to_js_err)?)
}

/// Ohlson (1980) O-Score logit bankruptcy model.
/// @param log_total_assets_adjusted - Natural log of total assets deflated by the GNP price-level index.
/// @param total_liabilities_to_total_assets - Total liabilities divided by total assets.
/// @param working_capital_to_total_assets - Working capital divided by total assets.
/// @param current_liabilities_to_current_assets - Current liabilities divided by current assets.
/// @param liabilities_exceed_assets - Indicator, exactly 1 when total liabilities exceed total assets and 0 otherwise.
/// @param net_income_to_total_assets - Net income divided by total assets.
/// @param funds_from_operations_to_total_liabilities - Funds from operations divided by total liabilities.
/// @param negative_net_income_two_years - Indicator, exactly 1 when net income was negative in each of the last two years and 0 otherwise.
/// @param net_income_change - Scaled change in net income, `(NI_t - NI_{t-1}) / (|NI_t| + |NI_{t-1}|)`.
/// @returns The `ScoringResult` object (`score`, `zone`, `implied_pd`, `model`).
///
/// # Errors
///
/// Throws a `validation` error if any input is non-finite or an indicator is
/// not exactly 0 or 1.
#[wasm_bindgen(js_name = ohlsonOScore)]
#[allow(clippy::too_many_arguments)]
pub fn ohlson_o_score(
    log_total_assets_adjusted: JsValue,
    total_liabilities_to_total_assets: JsValue,
    working_capital_to_total_assets: JsValue,
    current_liabilities_to_current_assets: JsValue,
    liabilities_exceed_assets: JsValue,
    net_income_to_total_assets: JsValue,
    funds_from_operations_to_total_liabilities: JsValue,
    negative_net_income_two_years: JsValue,
    net_income_change: JsValue,
) -> Result<JsValue, JsValue> {
    let input = OhlsonOScoreInput {
        log_total_assets_adjusted: js_f64(&log_total_assets_adjusted, "logTotalAssetsAdjusted")?,
        total_liabilities_to_total_assets: js_f64(
            &total_liabilities_to_total_assets,
            "totalLiabilitiesToTotalAssets",
        )?,
        working_capital_to_total_assets: js_f64(
            &working_capital_to_total_assets,
            "workingCapitalToTotalAssets",
        )?,
        current_liabilities_to_current_assets: js_f64(
            &current_liabilities_to_current_assets,
            "currentLiabilitiesToCurrentAssets",
        )?,
        liabilities_exceed_assets: js_f64(&liabilities_exceed_assets, "liabilitiesExceedAssets")?,
        net_income_to_total_assets: js_f64(&net_income_to_total_assets, "netIncomeToTotalAssets")?,
        funds_from_operations_to_total_liabilities: js_f64(
            &funds_from_operations_to_total_liabilities,
            "fundsFromOperationsToTotalLiabilities",
        )?,
        negative_net_income_two_years: js_f64(
            &negative_net_income_two_years,
            "negativeNetIncomeTwoYears",
        )?,
        net_income_change: js_f64(&net_income_change, "netIncomeChange")?,
    };
    to_js_value(&scoring::ohlson_o_score(&input).map_err(to_js_err)?)
}

/// Zmijewski (1984) probit financial-distress model.
/// @param net_income_to_total_assets - Net income divided by total assets.
/// @param total_liabilities_to_total_assets - Total liabilities divided by total assets.
/// @param current_assets_to_current_liabilities - Current assets divided by current liabilities.
/// @returns The `ScoringResult` object (`score`, `zone`, `implied_pd`, `model`).
///
/// # Errors
///
/// Throws a `validation` error if any ratio is non-finite.
#[wasm_bindgen(js_name = zmijewskiScore)]
pub fn zmijewski_score(
    net_income_to_total_assets: JsValue,
    total_liabilities_to_total_assets: JsValue,
    current_assets_to_current_liabilities: JsValue,
) -> Result<JsValue, JsValue> {
    let input = ZmijewskiInput {
        net_income_to_total_assets: js_f64(&net_income_to_total_assets, "netIncomeToTotalAssets")?,
        total_liabilities_to_total_assets: js_f64(
            &total_liabilities_to_total_assets,
            "totalLiabilitiesToTotalAssets",
        )?,
        current_assets_to_current_liabilities: js_f64(
            &current_assets_to_current_liabilities,
            "currentAssetsToCurrentLiabilities",
        )?,
    };
    to_js_value(&scoring::zmijewski_score(&input).map_err(to_js_err)?)
}
