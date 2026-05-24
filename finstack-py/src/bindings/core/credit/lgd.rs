//! Python bindings for `finstack_core::credit::lgd`.
//!
//! Exposes a function-based API covering:
//!
//! - Seniority-based recovery statistics (Moody's / S&P calibrations).
//! - Beta-recovery sampling and quantiles.
//! - Workout (collateral-waterfall) LGD.
//! - Downturn LGD adjustments (Frye-Jacobs, regulatory floor).
//! - Exposure-at-default for term loans and revolvers.

use finstack_core::credit::lgd;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyModule};

use crate::errors::display_to_py;

// ---------------------------------------------------------------------------
// Seniority recovery stats
// ---------------------------------------------------------------------------

/// Return historical recovery statistics for a given seniority class.
///
/// Arguments:
///     seniority: One of "senior_secured", "senior_unsecured",
///         "subordinated", "junior_subordinated".
///     rating_agency: Optional agency id such as "moodys" or "sp". If omitted,
///         the Rust credit-assumptions registry default is used.
///
/// Returns a dict with keys ``{"mean", "std", "alpha", "beta"}`` where
/// ``alpha``/``beta`` are the Beta-distribution shape parameters derived
/// from the (mean, std) moment-matching parameterization.
#[pyfunction]
#[pyo3(signature = (seniority, rating_agency = None))]
#[pyo3(text_signature = "(seniority, rating_agency=None)")]
fn seniority_recovery_stats<'py>(
    py: Python<'py>,
    seniority: &str,
    rating_agency: Option<&str>,
) -> PyResult<Bound<'py, PyDict>> {
    let br = match rating_agency {
        Some(agency) => lgd::seniority_recovery_stats(seniority, agency),
        None => lgd::seniority_recovery_stats_default(seniority),
    }
    .map_err(display_to_py)?;

    let d = PyDict::new(py);
    d.set_item("mean", br.mean())?;
    d.set_item("std", br.std_dev())?;
    d.set_item("alpha", br.alpha())?;
    d.set_item("beta", br.beta_param())?;
    Ok(d)
}

// ---------------------------------------------------------------------------
// Beta recovery sampling / quantiles
// ---------------------------------------------------------------------------

/// Draw ``n_samples`` recovery rates from a Beta(alpha, beta) distribution
/// parameterized by (``mean``, ``std``) using a deterministic PCG64 RNG.
///
/// Arguments:
///     mean: Mean recovery rate in (0, 1).
///     std: Standard deviation; must satisfy std^2 < mean * (1 - mean).
///     n_samples: Number of draws to produce.
///     seed: RNG seed. The same seed yields the same sequence.
#[pyfunction]
#[pyo3(text_signature = "(mean, std, n_samples, seed)")]
fn beta_recovery_sample(mean: f64, std: f64, n_samples: usize, seed: u64) -> PyResult<Vec<f64>> {
    lgd::beta_recovery_sample(mean, std, n_samples, seed).map_err(display_to_py)
}

/// Return the value at quantile ``q`` for a Beta recovery distribution
/// parameterized by (``mean``, ``std``).
///
/// Arguments:
///     mean: Mean recovery rate in (0, 1).
///     std: Standard deviation; must satisfy std^2 < mean * (1 - mean).
///     q: Probability in (0, 1).
#[pyfunction]
#[pyo3(text_signature = "(mean, std, q)")]
fn beta_recovery_quantile(mean: f64, std: f64, q: f64) -> PyResult<f64> {
    lgd::beta_recovery_quantile(mean, std, q).map_err(display_to_py)
}

// ---------------------------------------------------------------------------
// Workout LGD
// ---------------------------------------------------------------------------

/// Compute workout LGD from a collateral waterfall, costs, and a
/// time-to-resolution discount.
///
/// Arguments:
///     ead: Exposure at default (> 0).
///     collateral: List of ``(type, value, haircut)`` triples where ``type``
///         is a collateral-type string, ``value`` is the pre-haircut book
///         value, and ``haircut`` is in [0, 1].
///     direct_cost_pct: Direct resolution costs as fraction of EAD (>= 0).
///     indirect_cost_pct: Indirect resolution costs as fraction of EAD (>= 0).
///     time_to_resolution_years: Expected workout duration in years (>= 0).
///     discount_rate: Annual discount rate for the workout period (>= 0).
///
/// Returns ``(net_recovery, lgd)`` where ``net_recovery`` is the
/// post-discount, post-cost recovery amount (floored at 0) and
/// ``lgd = 1 - net_recovery / ead`` clamped to [0, 1].
#[pyfunction]
#[pyo3(
    text_signature = "(ead, collateral, direct_cost_pct, indirect_cost_pct, time_to_resolution_years, discount_rate)"
)]
fn workout_lgd(
    ead: f64,
    collateral: Vec<(String, f64, f64)>,
    direct_cost_pct: f64,
    indirect_cost_pct: f64,
    time_to_resolution_years: f64,
    discount_rate: f64,
) -> PyResult<(f64, f64)> {
    lgd::workout_lgd(
        ead,
        collateral,
        direct_cost_pct,
        indirect_cost_pct,
        time_to_resolution_years,
        discount_rate,
    )
    .map_err(display_to_py)
}

// ---------------------------------------------------------------------------
// Downturn LGD
// ---------------------------------------------------------------------------

/// Apply a Frye-Jacobs (2012) downturn adjustment to a base LGD.
///
/// ```text
/// LGD_downturn = LGD_base + sqrt(rho) * Phi^-1(q) * sqrt(LGD_base * (1 - LGD_base))
/// ```
///
/// with the LGD sensitivity fixed at 1.0. The result is clamped to [0, 1].
///
/// Arguments:
///     base_lgd: Through-the-cycle LGD in [0, 1].
///     asset_correlation: Asset correlation rho in (0, 1). Basel: 0.12-0.24.
///     stress_quantile: Downturn quantile in (0, 1), e.g. 0.999.
#[pyfunction]
#[pyo3(text_signature = "(base_lgd, asset_correlation, stress_quantile)")]
fn downturn_lgd_frye_jacobs(
    base_lgd: f64,
    asset_correlation: f64,
    stress_quantile: f64,
) -> PyResult<f64> {
    lgd::downturn_lgd_frye_jacobs(base_lgd, asset_correlation, stress_quantile)
        .map_err(display_to_py)
}

/// Apply a regulatory floor downturn adjustment to a base LGD.
///
/// ```text
/// LGD_downturn = max(LGD_base + add_on, floor)
/// ```
///
/// The result is clamped to [0, 1].
///
/// Arguments:
///     base_lgd: Through-the-cycle LGD in [0, 1].
///     add_on: Flat add-on (>= 0). Typical: 0.05-0.10.
///     floor: Absolute floor in [0, 1]. Typical: 0.10 secured / 0.25 unsecured.
#[pyfunction]
#[pyo3(text_signature = "(base_lgd, add_on, floor)")]
fn downturn_lgd_regulatory_floor(base_lgd: f64, add_on: f64, floor: f64) -> PyResult<f64> {
    lgd::downturn_lgd_regulatory_floor(base_lgd, add_on, floor).map_err(display_to_py)
}

// ---------------------------------------------------------------------------
// EAD
// ---------------------------------------------------------------------------

/// Exposure at default for a fully drawn term loan.
///
/// Equivalent to ``principal`` itself (no undrawn component).
#[pyfunction]
#[pyo3(text_signature = "(principal)")]
fn ead_term_loan(principal: f64) -> PyResult<f64> {
    lgd::ead_term_loan(principal).map_err(display_to_py)
}

/// Exposure at default for a revolving facility.
///
/// ```text
/// EAD = drawn + undrawn * CCF
/// ```
///
/// Arguments:
///     drawn: Currently drawn amount (>= 0).
///     undrawn: Undrawn commitment (>= 0).
///     ccf: Credit conversion factor in [0, 1]. Basel IRB: 0.75.
#[pyfunction]
#[pyo3(text_signature = "(drawn, undrawn, ccf)")]
fn ead_revolver(drawn: f64, undrawn: f64, ccf: f64) -> PyResult<f64> {
    lgd::ead_revolver(drawn, undrawn, ccf).map_err(display_to_py)
}

// ---------------------------------------------------------------------------
// Register
// ---------------------------------------------------------------------------

/// Build the `finstack.core.credit.lgd` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "lgd")?;
    m.setattr(
        "__doc__",
        "Loss-given-default modeling: seniority recovery, workout LGD, downturn adjustments, EAD.",
    )?;

    m.add_function(wrap_pyfunction!(seniority_recovery_stats, &m)?)?;
    m.add_function(wrap_pyfunction!(beta_recovery_sample, &m)?)?;
    m.add_function(wrap_pyfunction!(beta_recovery_quantile, &m)?)?;
    m.add_function(wrap_pyfunction!(workout_lgd, &m)?)?;
    m.add_function(wrap_pyfunction!(downturn_lgd_frye_jacobs, &m)?)?;
    m.add_function(wrap_pyfunction!(downturn_lgd_regulatory_floor, &m)?)?;
    m.add_function(wrap_pyfunction!(ead_term_loan, &m)?)?;
    m.add_function(wrap_pyfunction!(ead_revolver, &m)?)?;

    let all = PyList::new(
        py,
        [
            "seniority_recovery_stats",
            "beta_recovery_sample",
            "beta_recovery_quantile",
            "workout_lgd",
            "downturn_lgd_frye_jacobs",
            "downturn_lgd_regulatory_floor",
            "ead_term_loan",
            "ead_revolver",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "lgd",
        "finstack.core.credit",
        crate::bindings::module_utils::ParentNameSource::Package,
    )?;

    Ok(())
}
