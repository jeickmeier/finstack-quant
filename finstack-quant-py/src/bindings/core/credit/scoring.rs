//! Python bindings for `finstack_quant_core::credit::scoring`.

use finstack_quant_core::credit::scoring::{
    altman_z_double_prime as core_altman_z_double_prime,
    altman_z_double_prime_with_pd as core_altman_z_double_prime_with_pd,
    altman_z_prime as core_altman_z_prime, altman_z_prime_with_pd as core_altman_z_prime_with_pd,
    altman_z_score as core_altman_z_score, altman_z_score_with_pd as core_altman_z_score_with_pd,
    ohlson_o_score as core_ohlson_o_score, zmijewski_score as core_zmijewski_score,
    AltmanPdCalibration, AltmanZDoublePrimeInput, AltmanZPrimeInput, AltmanZScoreInput,
    OhlsonOScoreInput, ScoringZone, ZmijewskiInput,
};
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};

use crate::errors::display_to_py;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a `ScoringZone` into a lowercase Python string.
fn zone_to_str(zone: ScoringZone) -> &'static str {
    match zone {
        ScoringZone::Safe => "safe",
        ScoringZone::Grey => "grey",
        ScoringZone::Distress => "distress",
    }
}

// ---------------------------------------------------------------------------
// Altman Z-Score family
// ---------------------------------------------------------------------------

/// Explicit versioned Altman score-to-PD heuristics.
#[pyclass(
    name = "AltmanPdCalibration",
    module = "finstack_quant.core.credit.scoring",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PyAltmanPdCalibration {
    inner: AltmanPdCalibration,
}

#[pymethods]
impl PyAltmanPdCalibration {
    /// Legacy uncalibrated house heuristic.
    #[classattr]
    const HEURISTIC_V1: Self = Self {
        inner: AltmanPdCalibration::HeuristicV1,
    };
}

/// Compute the original Altman Z-Score (1968) for publicly traded manufacturing firms.
///
/// Z = 1.2 * X1 + 1.4 * X2 + 3.3 * X3 + 0.6 * X4 + 1.0 * X5
///
/// Zone cutoffs: Z > 2.99 Safe, 1.81 <= Z <= 2.99 Grey, Z < 1.81 Distress.
///
/// Returns ``implied_pd=None`` unless an explicit versioned heuristic is supplied.
#[pyfunction]
#[pyo3(
    signature = (working_capital_to_total_assets, retained_earnings_to_total_assets, ebit_to_total_assets, market_equity_to_total_liabilities, sales_to_total_assets, pd_calibration=None),
    text_signature = "(working_capital_to_total_assets, retained_earnings_to_total_assets, ebit_to_total_assets, market_equity_to_total_liabilities, sales_to_total_assets, pd_calibration=None)"
)]
fn altman_z_score(
    working_capital_to_total_assets: f64,
    retained_earnings_to_total_assets: f64,
    ebit_to_total_assets: f64,
    market_equity_to_total_liabilities: f64,
    sales_to_total_assets: f64,
    pd_calibration: Option<&PyAltmanPdCalibration>,
) -> PyResult<(f64, String, Option<f64>)> {
    let input = AltmanZScoreInput {
        working_capital_to_total_assets,
        retained_earnings_to_total_assets,
        ebit_to_total_assets,
        market_equity_to_total_liabilities,
        sales_to_total_assets,
    };
    let r = match pd_calibration {
        Some(calibration) => core_altman_z_score_with_pd(&input, calibration.inner),
        None => core_altman_z_score(&input),
    }
    .map_err(display_to_py)?;
    Ok((r.score, zone_to_str(r.zone).to_string(), r.implied_pd))
}

/// Compute the Altman Z'-Score for private firms.
///
/// Z' = 0.717 * X1 + 0.847 * X2 + 3.107 * X3 + 0.420 * X4 + 0.998 * X5
///
/// Zone cutoffs: Z' > 2.90 Safe, 1.23 <= Z' <= 2.90 Grey, Z' < 1.23 Distress.
///
/// Returns a tuple ``(score, zone, implied_pd)``.
#[pyfunction]
#[pyo3(
    signature = (working_capital_to_total_assets, retained_earnings_to_total_assets, ebit_to_total_assets, book_equity_to_total_liabilities, sales_to_total_assets, pd_calibration=None),
    text_signature = "(working_capital_to_total_assets, retained_earnings_to_total_assets, ebit_to_total_assets, book_equity_to_total_liabilities, sales_to_total_assets, pd_calibration=None)"
)]
fn altman_z_prime(
    working_capital_to_total_assets: f64,
    retained_earnings_to_total_assets: f64,
    ebit_to_total_assets: f64,
    book_equity_to_total_liabilities: f64,
    sales_to_total_assets: f64,
    pd_calibration: Option<&PyAltmanPdCalibration>,
) -> PyResult<(f64, String, Option<f64>)> {
    let input = AltmanZPrimeInput {
        working_capital_to_total_assets,
        retained_earnings_to_total_assets,
        ebit_to_total_assets,
        book_equity_to_total_liabilities,
        sales_to_total_assets,
    };
    let r = match pd_calibration {
        Some(calibration) => core_altman_z_prime_with_pd(&input, calibration.inner),
        None => core_altman_z_prime(&input),
    }
    .map_err(display_to_py)?;
    Ok((r.score, zone_to_str(r.zone).to_string(), r.implied_pd))
}

/// Compute the Altman Z''-Score for non-manufacturing firms (non-EM model;
/// the emerging-market EM-Score variant with the +3.25 constant is not
/// implemented).
///
/// Z'' = 6.56 * X1 + 3.26 * X2 + 6.72 * X3 + 1.05 * X4
///
/// Zone cutoffs: Z'' > 2.60 Safe, 1.10 <= Z'' <= 2.60 Grey, Z'' < 1.10 Distress.
///
/// Returns a tuple ``(score, zone, implied_pd)``.
#[pyfunction]
#[pyo3(
    signature = (working_capital_to_total_assets, retained_earnings_to_total_assets, ebit_to_total_assets, book_equity_to_total_liabilities, pd_calibration=None),
    text_signature = "(working_capital_to_total_assets, retained_earnings_to_total_assets, ebit_to_total_assets, book_equity_to_total_liabilities, pd_calibration=None)"
)]
fn altman_z_double_prime(
    working_capital_to_total_assets: f64,
    retained_earnings_to_total_assets: f64,
    ebit_to_total_assets: f64,
    book_equity_to_total_liabilities: f64,
    pd_calibration: Option<&PyAltmanPdCalibration>,
) -> PyResult<(f64, String, Option<f64>)> {
    let input = AltmanZDoublePrimeInput {
        working_capital_to_total_assets,
        retained_earnings_to_total_assets,
        ebit_to_total_assets,
        book_equity_to_total_liabilities,
    };
    let r = match pd_calibration {
        Some(calibration) => core_altman_z_double_prime_with_pd(&input, calibration.inner),
        None => core_altman_z_double_prime(&input),
    }
    .map_err(display_to_py)?;
    Ok((r.score, zone_to_str(r.zone).to_string(), r.implied_pd))
}

// ---------------------------------------------------------------------------
// Ohlson O-Score
// ---------------------------------------------------------------------------

/// Compute the Ohlson O-Score (1980) nine-predictor logistic bankruptcy model.
///
/// O = -1.32 - 0.407 * X1 + 6.03 * X2 - 1.43 * X3 + 0.0757 * X4
///     - 1.72 * X5 - 2.37 * X6 - 1.83 * X7 + 0.285 * X8 - 0.521 * X9
///
/// PD = 1 / (1 + exp(-O)).  Zone based on implied PD: < 0.019 Safe,
/// [0.019, 0.038] Grey, > 0.038 Distress (Ohlson's published optimal cutoff
/// P* = 0.038, O ≈ -3.23, is the Distress boundary).
///
/// Returns a tuple ``(score, zone, implied_pd)``.
#[pyfunction]
#[pyo3(
    text_signature = "(log_total_assets_adjusted, total_liabilities_to_total_assets, working_capital_to_total_assets, current_liabilities_to_current_assets, liabilities_exceed_assets, net_income_to_total_assets, funds_from_operations_to_total_liabilities, negative_net_income_two_years, net_income_change)"
)]
#[allow(clippy::too_many_arguments)]
fn ohlson_o_score(
    log_total_assets_adjusted: f64,
    total_liabilities_to_total_assets: f64,
    working_capital_to_total_assets: f64,
    current_liabilities_to_current_assets: f64,
    liabilities_exceed_assets: f64,
    net_income_to_total_assets: f64,
    funds_from_operations_to_total_liabilities: f64,
    negative_net_income_two_years: f64,
    net_income_change: f64,
) -> PyResult<(f64, String, Option<f64>)> {
    let input = OhlsonOScoreInput {
        log_total_assets_adjusted,
        total_liabilities_to_total_assets,
        working_capital_to_total_assets,
        current_liabilities_to_current_assets,
        liabilities_exceed_assets,
        net_income_to_total_assets,
        funds_from_operations_to_total_liabilities,
        negative_net_income_two_years,
        net_income_change,
    };
    let r = core_ohlson_o_score(&input).map_err(display_to_py)?;
    Ok((r.score, zone_to_str(r.zone).to_string(), r.implied_pd))
}

// ---------------------------------------------------------------------------
// Zmijewski probit
// ---------------------------------------------------------------------------

/// Compute the Zmijewski (1984) probit bankruptcy score.
///
/// Y = -4.336 - 4.513 * ROA + 5.679 * DebtRatio + 0.004 * CurrentRatio
///
/// PD = Phi(Y).  Zone based on implied PD: < 0.10 Safe, [0.10, 0.50] Grey, > 0.50 Distress.
///
/// Returns a tuple ``(score, zone, implied_pd)`` matching the Altman / Ohlson
/// bindings.
#[pyfunction]
#[pyo3(
    text_signature = "(net_income_to_total_assets, total_liabilities_to_total_assets, current_assets_to_current_liabilities)"
)]
fn zmijewski_score(
    net_income_to_total_assets: f64,
    total_liabilities_to_total_assets: f64,
    current_assets_to_current_liabilities: f64,
) -> PyResult<(f64, String, Option<f64>)> {
    let input = ZmijewskiInput {
        net_income_to_total_assets,
        total_liabilities_to_total_assets,
        current_assets_to_current_liabilities,
    };
    let r = core_zmijewski_score(&input).map_err(display_to_py)?;
    Ok((r.score, zone_to_str(r.zone).to_string(), r.implied_pd))
}

// ---------------------------------------------------------------------------
// Register
// ---------------------------------------------------------------------------

/// Build the `finstack_quant.core.credit.scoring` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "scoring")?;
    m.setattr(
        "__doc__",
        "Academic credit scoring models: Altman Z-Score family, Ohlson O-Score, Zmijewski probit.",
    )?;

    m.add_class::<PyAltmanPdCalibration>()?;
    m.add_function(wrap_pyfunction!(altman_z_score, &m)?)?;
    m.add_function(wrap_pyfunction!(altman_z_prime, &m)?)?;
    m.add_function(wrap_pyfunction!(altman_z_double_prime, &m)?)?;
    m.add_function(wrap_pyfunction!(ohlson_o_score, &m)?)?;
    m.add_function(wrap_pyfunction!(zmijewski_score, &m)?)?;

    let all = PyList::new(
        py,
        [
            "AltmanPdCalibration",
            "altman_z_score",
            "altman_z_prime",
            "altman_z_double_prime",
            "ohlson_o_score",
            "zmijewski_score",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "scoring",
        "finstack_quant.core.credit",
        crate::bindings::module_utils::ParentNameSource::Package,
    )?;

    Ok(())
}
