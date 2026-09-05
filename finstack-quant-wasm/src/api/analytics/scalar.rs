//! WASM bindings for the freestanding scalar metrics
//! (`finstack_quant_analytics::scalar`).
//!
//! Sharpe, Sortino, annualized volatility and maximum drawdown over one
//! simple-return series, without constructing a `Performance` panel. Inputs
//! travel as `number[]` / `Float64Array`; outputs are plain numbers.

use wasm_bindgen::prelude::*;

use super::support::parse_f64_vec;

const DEFAULT_PERIODS_PER_YEAR: f64 = 252.0;

/// Sharpe ratio of one return series (annualized excess mean over
/// annualized sample volatility; the same kernel as `Performance.sharpe`).
///
/// # Arguments
///
/// * `returns` - Per-period simple decimal returns in date order.
/// * `rf` - Annualized risk-free rate as a decimal (`0.02` for 2%); defaults
///   to `0`.
/// * `periods_per_year` - Observations per year used to annualize; defaults
///   to `252`.
///
/// # Errors
///
/// Rejects a `returns` value that is not a numeric array.
/// @param returns - Per-period simple decimal returns in date order.
/// @param rf - Annualized risk-free rate as a decimal; defaults to `0`.
/// @param periodsPerYear - Observations per year used to annualize; defaults to `252`.
/// @returns The Sharpe ratio; `±Infinity` when volatility is zero with a non-zero excess return, `NaN` for fewer than two observations, non-finite inputs, or an invalid `periodsPerYear`.
/// @throws Error - Rejects a `returns` value that is not a numeric array.
#[wasm_bindgen(js_name = sharpe)]
pub fn sharpe(
    returns: JsValue,
    rf: Option<f64>,
    periods_per_year: Option<f64>,
) -> Result<f64, JsValue> {
    let returns = parse_f64_vec(returns)?;
    Ok(finstack_quant_analytics::sharpe(
        &returns,
        rf.unwrap_or(0.0),
        periods_per_year.unwrap_or(DEFAULT_PERIODS_PER_YEAR),
    ))
}

/// Annualized Sortino ratio of one return series.
///
/// # Arguments
///
/// * `returns` - Per-period simple decimal returns in date order.
/// * `mar` - Minimum acceptable return per period as a decimal (not
///   annualized); defaults to `0`.
/// * `periods_per_year` - Observations per year used to annualize; defaults
///   to `252`.
///
/// # Errors
///
/// Rejects a `returns` value that is not a numeric array.
/// @param returns - Per-period simple decimal returns in date order.
/// @param mar - Minimum acceptable return per period as a decimal; defaults to `0`.
/// @param periodsPerYear - Observations per year used to annualize; defaults to `252`.
/// @returns The Sortino ratio; `±Infinity` with no downside deviation but a non-zero excess mean, `NaN` for fewer than two observations, non-finite inputs, or an invalid `periodsPerYear`.
/// @throws Error - Rejects a `returns` value that is not a numeric array.
#[wasm_bindgen(js_name = sortino)]
pub fn sortino(
    returns: JsValue,
    mar: Option<f64>,
    periods_per_year: Option<f64>,
) -> Result<f64, JsValue> {
    let returns = parse_f64_vec(returns)?;
    Ok(finstack_quant_analytics::sortino(
        &returns,
        mar.unwrap_or(0.0),
        periods_per_year.unwrap_or(DEFAULT_PERIODS_PER_YEAR),
    ))
}

/// Annualized sample volatility (n−1 denominator) of one return series.
///
/// # Arguments
///
/// * `returns` - Per-period simple decimal returns in date order.
/// * `periods_per_year` - Observations per year; the per-period standard
///   deviation is scaled by its square root. Defaults to `252`.
///
/// # Errors
///
/// Rejects a `returns` value that is not a numeric array.
/// @param returns - Per-period simple decimal returns in date order.
/// @param periodsPerYear - Observations per year; the per-period standard deviation is scaled by its square root. Defaults to `252`.
/// @returns Annualized volatility as a decimal; `0` for an empty array, `NaN` for non-finite inputs or an invalid `periodsPerYear`.
/// @throws Error - Rejects a `returns` value that is not a numeric array.
#[wasm_bindgen(js_name = volatility)]
pub fn volatility(returns: JsValue, periods_per_year: Option<f64>) -> Result<f64, JsValue> {
    let returns = parse_f64_vec(returns)?;
    Ok(finstack_quant_analytics::volatility(
        &returns,
        periods_per_year.unwrap_or(DEFAULT_PERIODS_PER_YEAR),
    ))
}

/// Maximum peak-to-trough drawdown of one return series.
///
/// # Arguments
///
/// * `returns` - Per-period simple decimal returns in date order; they are
///   compounded into a wealth path before the running-peak decline is
///   measured.
///
/// # Errors
///
/// Rejects a `returns` value that is not a numeric array.
/// @param returns - Per-period simple decimal returns in date order.
/// @returns Non-positive fraction (`-0.25` is a 25% loss); `0` when the series never falls below its running peak or is empty. Non-finite returns or reconstructed drawdowns yield `NaN`.
/// @throws Error - Rejects a `returns` value that is not a numeric array.
#[wasm_bindgen(js_name = maxDrawdown)]
pub fn max_drawdown(returns: JsValue) -> Result<f64, JsValue> {
    let returns = parse_f64_vec(returns)?;
    Ok(finstack_quant_analytics::max_drawdown(&returns))
}
