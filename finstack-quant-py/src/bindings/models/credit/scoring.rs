//! Python bindings for `finstack_quant_models::credit::scoring`.

use finstack_quant_models::credit::scoring::{
    altman_em_score as core_altman_em_score, altman_z_double_prime as core_altman_z_double_prime,
    altman_z_prime as core_altman_z_prime, altman_z_score as core_altman_z_score,
    ohlson_o_score as core_ohlson_o_score, zmijewski_score as core_zmijewski_score,
    AltmanZDoublePrimeInput, AltmanZPrimeInput, AltmanZScoreInput, OhlsonOScoreInput,
    ScoringResult, ZmijewskiInput,
};
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};

use crate::bindings::pandas_utils::serde_object_to_single_row_dataframe_with_schema;
use crate::errors::{core_to_py, scoring_to_py, serde_json_to_py};

/// Outcome of one academic credit-scoring model.
///
/// ``score`` is the raw discriminant / regression output, ``zone`` is the
/// published risk classification (``"safe"``, ``"grey"`` or ``"distress"``),
/// ``implied_pd`` is the model's native probability of default (Ohlson,
/// Zmijewski) or ``None`` for the Altman family, and ``model`` names the
/// producing model. Feed it to ``pd.MasterScale.map_score`` to grade the
/// implied PD.
#[pyclass(
    module = "finstack_quant.models.credit.scoring",
    name = "ScoringResult",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub(crate) struct PyScoringResult {
    pub(crate) inner: ScoringResult,
}

impl PyScoringResult {
    pub(crate) fn from_inner(inner: ScoringResult) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyScoringResult {
    /// Raw score value (Z, Z', Z'', EM, O, or Zmijewski Y).
    #[getter]
    fn score(&self) -> f64 {
        self.inner.score
    }

    /// Risk zone: ``"safe"``, ``"grey"`` or ``"distress"``.
    #[getter]
    fn zone(&self) -> PyResult<String> {
        finstack_quant_core::wire::serde_label(&self.inner.zone).map_err(core_to_py)
    }

    /// Native implied probability of default as a decimal, or ``None`` when
    /// the model has no probability transform (Altman family).
    #[getter]
    fn implied_pd(&self) -> Option<f64> {
        self.inner.implied_pd
    }

    /// Name of the model that produced this result.
    #[getter]
    fn model(&self) -> String {
        self.inner.model.clone()
    }

    /// Deserialize a scoring result from canonical JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json)
                .map_err(|err| serde_json_to_py(err, "invalid ScoringResult JSON"))?,
        })
    }

    /// Serialize this result to compact canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "ScoringResult serialization failed"))
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Export as a single-row pandas ``DataFrame``.
    ///
    /// Columns: ``model``, ``score``, ``zone``, ``implied_pd`` (``None`` for
    /// the Altman family). ``pd.concat`` over several results builds a
    /// model-comparison table directly.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_object_to_single_row_dataframe_with_schema(
            py,
            &self.inner,
            &["model", "score", "zone", "implied_pd"],
        )
    }

    /// Identify this value in notebooks and logs.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("ScoringResult", &self.inner)
    }
}

/// Compute the original Altman Z-Score (1968) for publicly traded manufacturing firms.
///
/// Z = 1.2 * X1 + 1.4 * X2 + 3.3 * X3 + 0.6 * X4 + 1.0 * X5
///
/// Zone cutoffs: Z > 2.99 safe, 1.81 <= Z <= 2.99 grey, Z < 1.81 distress.
///
/// Parameters
/// ----------
/// working_capital_to_total_assets : float
///     Working capital / total assets (X1).
/// retained_earnings_to_total_assets : float
///     Retained earnings / total assets (X2).
/// ebit_to_total_assets : float
///     EBIT / total assets (X3).
/// market_equity_to_total_liabilities : float
///     Market value of equity / total liabilities (X4).
/// sales_to_total_assets : float
///     Sales / total assets (X5).
///
/// Returns
/// -------
/// ScoringResult
///     ``implied_pd`` is ``None``; calibrate score-to-PD separately.
///
/// Raises ``ValueError`` if any ratio is non-finite.
#[pyfunction]
#[pyo3(
    text_signature = "(working_capital_to_total_assets, retained_earnings_to_total_assets, ebit_to_total_assets, market_equity_to_total_liabilities, sales_to_total_assets)"
)]
fn altman_z_score(
    working_capital_to_total_assets: f64,
    retained_earnings_to_total_assets: f64,
    ebit_to_total_assets: f64,
    market_equity_to_total_liabilities: f64,
    sales_to_total_assets: f64,
) -> PyResult<PyScoringResult> {
    let input = AltmanZScoreInput {
        working_capital_to_total_assets,
        retained_earnings_to_total_assets,
        ebit_to_total_assets,
        market_equity_to_total_liabilities,
        sales_to_total_assets,
    };
    core_altman_z_score(&input)
        .map(PyScoringResult::from_inner)
        .map_err(scoring_to_py)
}

/// Compute the Altman Z'-Score for private firms.
///
/// Z' = 0.717 * X1 + 0.847 * X2 + 3.107 * X3 + 0.420 * X4 + 0.998 * X5
///
/// Zone cutoffs: Z' > 2.90 safe, 1.23 <= Z' <= 2.90 grey, Z' < 1.23 distress.
///
/// Parameters
/// ----------
/// working_capital_to_total_assets : float
///     Working capital / total assets (X1).
/// retained_earnings_to_total_assets : float
///     Retained earnings / total assets (X2).
/// ebit_to_total_assets : float
///     EBIT / total assets (X3).
/// book_equity_to_total_liabilities : float
///     Book value of equity / total liabilities (X4).
/// sales_to_total_assets : float
///     Sales / total assets (X5).
///
/// Returns
/// -------
/// ScoringResult
///     ``implied_pd`` is ``None``.
///
/// Raises ``ValueError`` if any ratio is non-finite.
#[pyfunction]
#[pyo3(
    text_signature = "(working_capital_to_total_assets, retained_earnings_to_total_assets, ebit_to_total_assets, book_equity_to_total_liabilities, sales_to_total_assets)"
)]
fn altman_z_prime(
    working_capital_to_total_assets: f64,
    retained_earnings_to_total_assets: f64,
    ebit_to_total_assets: f64,
    book_equity_to_total_liabilities: f64,
    sales_to_total_assets: f64,
) -> PyResult<PyScoringResult> {
    let input = AltmanZPrimeInput {
        working_capital_to_total_assets,
        retained_earnings_to_total_assets,
        ebit_to_total_assets,
        book_equity_to_total_liabilities,
        sales_to_total_assets,
    };
    core_altman_z_prime(&input)
        .map(PyScoringResult::from_inner)
        .map_err(scoring_to_py)
}

/// Compute the Altman Z''-Score for non-manufacturing firms.
///
/// Z'' = 6.56 * X1 + 3.26 * X2 + 6.72 * X3 + 1.05 * X4
///
/// Zone cutoffs: Z'' > 2.60 safe, 1.10 <= Z'' <= 2.60 grey, Z'' < 1.10
/// distress. The emerging-market variant with the +3.25 constant is
/// ``altman_em_score``.
///
/// Parameters
/// ----------
/// working_capital_to_total_assets : float
///     Working capital / total assets (X1).
/// retained_earnings_to_total_assets : float
///     Retained earnings / total assets (X2).
/// ebit_to_total_assets : float
///     EBIT / total assets (X3).
/// book_equity_to_total_liabilities : float
///     Book value of equity / total liabilities (X4).
///
/// Returns
/// -------
/// ScoringResult
///     ``implied_pd`` is ``None``.
///
/// Raises ``ValueError`` if any ratio is non-finite.
#[pyfunction]
#[pyo3(
    text_signature = "(working_capital_to_total_assets, retained_earnings_to_total_assets, ebit_to_total_assets, book_equity_to_total_liabilities)"
)]
fn altman_z_double_prime(
    working_capital_to_total_assets: f64,
    retained_earnings_to_total_assets: f64,
    ebit_to_total_assets: f64,
    book_equity_to_total_liabilities: f64,
) -> PyResult<PyScoringResult> {
    let input = AltmanZDoublePrimeInput {
        working_capital_to_total_assets,
        retained_earnings_to_total_assets,
        ebit_to_total_assets,
        book_equity_to_total_liabilities,
    };
    core_altman_z_double_prime(&input)
        .map(PyScoringResult::from_inner)
        .map_err(scoring_to_py)
}

/// Compute the Altman EM-Score for emerging-market corporates.
///
/// EM = 3.25 + Z'' = 3.25 + 6.56 * X1 + 3.26 * X2 + 6.72 * X3 + 1.05 * X4
///
/// Zone cutoffs: EM > 5.85 safe, 4.35 <= EM <= 5.85 grey, EM < 4.35 distress
/// (Altman, Hartzell & Peck 1995; Altman 2005, Emerging Markets Review 6(4)).
///
/// Parameters
/// ----------
/// working_capital_to_total_assets : float
///     Working capital / total assets (X1).
/// retained_earnings_to_total_assets : float
///     Retained earnings / total assets (X2).
/// ebit_to_total_assets : float
///     EBIT / total assets (X3).
/// book_equity_to_total_liabilities : float
///     Book value of equity / total liabilities (X4).
///
/// Returns
/// -------
/// ScoringResult
///     ``implied_pd`` is ``None``.
///
/// Raises ``ValueError`` if any ratio is non-finite.
#[pyfunction]
#[pyo3(
    text_signature = "(working_capital_to_total_assets, retained_earnings_to_total_assets, ebit_to_total_assets, book_equity_to_total_liabilities)"
)]
fn altman_em_score(
    working_capital_to_total_assets: f64,
    retained_earnings_to_total_assets: f64,
    ebit_to_total_assets: f64,
    book_equity_to_total_liabilities: f64,
) -> PyResult<PyScoringResult> {
    let input = AltmanZDoublePrimeInput {
        working_capital_to_total_assets,
        retained_earnings_to_total_assets,
        ebit_to_total_assets,
        book_equity_to_total_liabilities,
    };
    core_altman_em_score(&input)
        .map(PyScoringResult::from_inner)
        .map_err(scoring_to_py)
}

/// Compute the Ohlson O-Score (1980) nine-predictor logistic bankruptcy model.
///
/// O = -1.32 - 0.407 * X1 + 6.03 * X2 - 1.43 * X3 + 0.0757 * X4
///     - 1.72 * X5 - 2.37 * X6 - 1.83 * X7 + 0.285 * X8 - 0.521 * X9
///
/// PD = 1 / (1 + exp(-O)). Zone based on implied PD: < 0.019 safe,
/// [0.019, 0.038] grey, > 0.038 distress (Ohlson's published optimal cutoff
/// P* = 0.038, O ~ -3.23, is the distress boundary).
///
/// Parameters
/// ----------
/// log_total_assets_adjusted : float
///     log(total assets / GNP price-level index) (X1).
/// total_liabilities_to_total_assets : float
///     Total liabilities / total assets (X2).
/// working_capital_to_total_assets : float
///     Working capital / total assets (X3).
/// current_liabilities_to_current_assets : float
///     Current liabilities / current assets (X4).
/// liabilities_exceed_assets : float
///     Indicator, exactly ``1.0`` if total liabilities exceed total assets else ``0.0`` (X5).
/// net_income_to_total_assets : float
///     Net income / total assets (X6).
/// funds_from_operations_to_total_liabilities : float
///     Funds from operations / total liabilities (X7).
/// negative_net_income_two_years : float
///     Indicator, exactly ``1.0`` if net income was negative in each of the last two years (X8).
/// net_income_change : float
///     (NI_t - NI_t-1) / (|NI_t| + |NI_t-1|) (X9).
///
/// Returns
/// -------
/// ScoringResult
///     ``implied_pd`` carries the logistic probability.
///
/// Raises ``ValueError`` if any ratio is non-finite or an indicator is not
/// exactly 0 or 1.
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
) -> PyResult<PyScoringResult> {
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
    core_ohlson_o_score(&input)
        .map(PyScoringResult::from_inner)
        .map_err(scoring_to_py)
}

/// Compute the Zmijewski (1984) probit bankruptcy score.
///
/// Y = -4.336 - 4.513 * ROA + 5.679 * DebtRatio + 0.004 * CurrentRatio
///
/// PD = Phi(Y). Zone based on implied PD: < 0.10 safe, [0.10, 0.50] grey,
/// > 0.50 distress.
///
/// Parameters
/// ----------
/// net_income_to_total_assets : float
///     Net income / total assets (ROA).
/// total_liabilities_to_total_assets : float
///     Total liabilities / total assets (debt ratio).
/// current_assets_to_current_liabilities : float
///     Current assets / current liabilities (current ratio).
///
/// Returns
/// -------
/// ScoringResult
///     ``implied_pd`` carries the probit probability.
///
/// Raises ``ValueError`` if any ratio is non-finite.
#[pyfunction]
#[pyo3(
    text_signature = "(net_income_to_total_assets, total_liabilities_to_total_assets, current_assets_to_current_liabilities)"
)]
fn zmijewski_score(
    net_income_to_total_assets: f64,
    total_liabilities_to_total_assets: f64,
    current_assets_to_current_liabilities: f64,
) -> PyResult<PyScoringResult> {
    let input = ZmijewskiInput {
        net_income_to_total_assets,
        total_liabilities_to_total_assets,
        current_assets_to_current_liabilities,
    };
    core_zmijewski_score(&input)
        .map(PyScoringResult::from_inner)
        .map_err(scoring_to_py)
}

/// Build the `finstack_quant.models.credit.scoring` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "scoring")?;
    m.setattr(
        "__doc__",
        "Academic credit scoring models: Altman Z-Score family, Ohlson O-Score, Zmijewski probit.",
    )?;

    m.add_class::<PyScoringResult>()?;
    m.add_function(wrap_pyfunction!(altman_z_score, &m)?)?;
    m.add_function(wrap_pyfunction!(altman_z_prime, &m)?)?;
    m.add_function(wrap_pyfunction!(altman_z_double_prime, &m)?)?;
    m.add_function(wrap_pyfunction!(altman_em_score, &m)?)?;
    m.add_function(wrap_pyfunction!(ohlson_o_score, &m)?)?;
    m.add_function(wrap_pyfunction!(zmijewski_score, &m)?)?;

    let all = PyList::new(
        py,
        [
            "ScoringResult",
            "altman_em_score",
            "altman_z_double_prime",
            "altman_z_prime",
            "altman_z_score",
            "ohlson_o_score",
            "zmijewski_score",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Compiled,
    )?;

    Ok(())
}
