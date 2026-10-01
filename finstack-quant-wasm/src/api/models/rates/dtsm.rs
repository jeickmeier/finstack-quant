//! WASM bindings for dynamic term-structure models: Diebold-Li dynamic
//! Nelson-Siegel and yield-curve PCA.
//!
//! Mirrors `finstack-quant-py/src/bindings/models/rates/dtsm.rs`. The result
//! types `FactorTimeSeries`, `YieldForecast` and `YieldPcaView` cross the
//! boundary as plain objects in their canonical serde form.

use crate::utils::input::{
    js_f64, js_f64_matrix, js_f64_seq, js_opt_f64, js_opt_string_seq, js_opt_uint, js_uint,
};
use crate::utils::{parse_iso_date, to_js_err, to_js_value};
use finstack_quant_core::Error;
use finstack_quant_models::rates::dtsm::{self, DieboldLi, YieldPanel, YieldPca};
use wasm_bindgen::prelude::*;

/// Evaluate the static Nelson-Siegel (1987) yield curve for one factor triple.
///
/// This is the Diebold-Li cross-sectional equation for a single date:
/// `y(tau) = b1 + b2 * s(tau) + b3 * (s(tau) - exp(-lambda * tau))` with
/// `s(tau) = (1 - exp(-lambda * tau)) / (lambda * tau)`. Returns one yield per
/// tenor, in decimal units and in input order.
/// @param lambda - Exponential decay parameter for tenors in years; must be finite and greater than zero (0.7308 is the years-equivalent of Diebold-Li's 0.0609 months value).
/// @param factors - Nelson-Siegel `[level, slope, curvature]` (beta1, beta2, beta3) in decimal
/// yield units such as `[0.06, -0.02, 0.01]`; exactly three numbers.
/// @param tenors - Maturities in years, each finite and non-negative; output order matches this array.
/// @returns One decimal yield per tenor, in the same order as `tenors`.
///
/// # Errors
///
/// Throws a `TypeError` (`kind: "invalid_type"`) if `factors` or `tenors` is
/// not an array of numbers, and a `FinstackError` (`kind: "validation"`) if
/// `factors` does not hold exactly three entries, `lambda` is non-finite or
/// non-positive, any factor loading is non-finite, or any tenor is non-finite
/// or negative.
#[wasm_bindgen(js_name = nelsonSiegelYields)]
pub fn nelson_siegel_yields(
    lambda: JsValue,
    factors: JsValue,
    tenors: JsValue,
) -> Result<Box<[f64]>, JsValue> {
    let lambda = js_f64(&lambda, "lambda")?;
    let factors = js_f64_seq(&factors, "factors")?;
    let tenors = js_f64_seq(&tenors, "tenors")?;
    // Shape conversion to the Rust `[f64; 3]`, as PyO3 does for Python.
    let factors = <[f64; 3]>::try_from(factors.as_slice()).map_err(|_| {
        to_js_err(Error::Validation(format!(
            "factors must hold exactly 3 entries [level, slope, curvature], got {}",
            factors.len()
        )))
    })?;
    finstack_quant_models::rates::dtsm::nelson_siegel_yields(lambda, factors, &tenors)
        .map(Vec::into_boxed_slice)
        .map_err(to_js_err)
}

/// Rows of a dense matrix as nested vectors (host conversion only).
macro_rules! matrix_rows {
    ($matrix:expr) => {
        $matrix
            .row_iter()
            .map(|row| row.iter().copied().collect::<Vec<f64>>())
            .collect::<Vec<Vec<f64>>>()
    };
}

/// A panel of yield observations: one row per date, one column per tenor.
#[wasm_bindgen(js_name = YieldPanel)]
pub struct JsYieldPanel {
    pub(crate) inner: YieldPanel,
}

json_round_trip!(JsYieldPanel, YieldPanel);

#[wasm_bindgen(js_class = YieldPanel)]
impl JsYieldPanel {
    /// Panel from tenors and yield rows.
    /// @param tenors - Maturities in years, strictly increasing and positive, one per column.
    /// @param yields - Yields as nested rows: one `number[]` per date, each with one decimal yield per tenor.
    /// @param dates - Optional observation dates as ISO-8601 strings, ascending, one per row.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the rows are ragged, the dimensions do
    /// not match, a value is non-finite, or a date does not parse.
    #[wasm_bindgen(constructor)]
    pub fn new(
        tenors: JsValue,
        yields: JsValue,
        dates: Option<JsValue>,
    ) -> Result<JsYieldPanel, JsValue> {
        let tenors = js_f64_seq(&tenors, "tenors")?;
        let yields = js_f64_matrix(&yields, "yields")?;
        let dates = js_opt_string_seq(dates.as_ref(), "dates")?
            .map(|dates| {
                dates
                    .iter()
                    .map(|date| parse_iso_date(date))
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;
        YieldPanel::from_rows(tenors, yields, dates)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Maturities in years, one per column.
    #[wasm_bindgen(getter)]
    pub fn tenors(&self) -> Box<[f64]> {
        self.inner.tenors.clone().into_boxed_slice()
    }

    /// Observation dates as ISO-8601 strings, or `undefined` when the panel has none.
    #[wasm_bindgen(getter)]
    pub fn dates(&self) -> Option<Vec<String>> {
        self.inner
            .dates
            .as_ref()
            .map(|dates| dates.iter().map(ToString::to_string).collect())
    }

    /// Yields as nested rows: one `number[]` per date.
    #[wasm_bindgen(getter)]
    pub fn yields(&self) -> Result<JsValue, JsValue> {
        to_js_value(&matrix_rows!(self.inner.yields))
    }

    /// Number of observation dates (rows).
    #[wasm_bindgen(getter, js_name = numDates)]
    pub fn num_dates(&self) -> usize {
        self.inner.num_dates()
    }

    /// Number of tenors (columns).
    #[wasm_bindgen(getter, js_name = numTenors)]
    pub fn num_tenors(&self) -> usize {
        self.inner.num_tenors()
    }

    /// First differences of the yields across dates.
    /// @returns Nested rows with `numDates - 1` rows of per-tenor yield changes.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the rows cannot be converted.
    #[wasm_bindgen(js_name = yieldChanges)]
    pub fn yield_changes(&self) -> Result<JsValue, JsValue> {
        to_js_value(&matrix_rows!(self.inner.yield_changes()))
    }
}

/// Diebold-Li (2006) dynamic Nelson-Siegel model with VAR(1) factor dynamics.
#[wasm_bindgen(js_name = DieboldLi)]
pub struct JsDieboldLi {
    pub(crate) inner: DieboldLi,
}

json_round_trip!(JsDieboldLi, DieboldLi);

#[wasm_bindgen(js_class = DieboldLi)]
impl JsDieboldLi {
    /// Unfitted model with a decay parameter.
    /// @param lambda - Optional exponential decay parameter for tenors in years; positive. Omitted uses the Rust default (0.7308).
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if `lambda` is not positive and finite.
    #[wasm_bindgen(constructor)]
    pub fn new(lambda: Option<JsValue>) -> Result<JsDieboldLi, JsValue> {
        dtsm::diebold_li_model(js_opt_f64(lambda.as_ref(), "lambda")?)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Exponential decay parameter (Python `lambda_`).
    #[wasm_bindgen(getter)]
    pub fn lambda(&self) -> f64 {
        self.inner.lambda()
    }

    /// Tenors in years of the fitted panel (empty before factor extraction).
    #[wasm_bindgen(getter)]
    pub fn tenors(&self) -> Box<[f64]> {
        self.inner.tenors().into()
    }

    /// Extracted level, slope and curvature factors as a `FactorTimeSeries` object, or `undefined` before extraction.
    #[wasm_bindgen(getter)]
    pub fn factors(&self) -> Result<JsValue, JsValue> {
        match self.inner.factors() {
            Some(factors) => to_js_value(factors),
            None => Ok(JsValue::UNDEFINED),
        }
    }

    /// VAR(1) transition matrix as 3 by 3 nested rows, or `undefined` before `fitVar`.
    #[wasm_bindgen(getter)]
    pub fn phi(&self) -> Result<JsValue, JsValue> {
        match self.inner.phi() {
            Some(phi) => to_js_value(&matrix_rows!(phi)),
            None => Ok(JsValue::UNDEFINED),
        }
    }

    /// VAR(1) intercept `[level, slope, curvature]`, or `undefined` before `fitVar`.
    #[wasm_bindgen(getter)]
    pub fn mu(&self) -> Option<Box<[f64]>> {
        self.inner
            .mu()
            .map(|mu| mu.iter().copied().collect::<Vec<f64>>().into_boxed_slice())
    }

    /// VAR(1) innovation covariance as 3 by 3 nested rows, or `undefined` before `fitVar`.
    #[wasm_bindgen(getter, js_name = qCov)]
    pub fn q_cov(&self) -> Result<JsValue, JsValue> {
        match self.inner.q_cov() {
            Some(q_cov) => to_js_value(&matrix_rows!(q_cov)),
            None => Ok(JsValue::UNDEFINED),
        }
    }

    /// Nelson-Siegel loading matrix for the fitted tenors.
    /// @returns Nested rows, one per tenor, each `[level, slope, curvature]` loading.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the rows cannot be converted.
    #[wasm_bindgen(js_name = loadingMatrix)]
    pub fn loading_matrix(&self) -> Result<JsValue, JsValue> {
        to_js_value(&matrix_rows!(self.inner.loading_matrix()))
    }

    /// Extract the factor time series by cross-sectional OLS on each date.
    /// @param panel - `YieldPanel` handle of observed yields.
    /// @returns A new model carrying the extracted factors.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the panel has too few tenors or dates.
    #[wasm_bindgen(js_name = extractFactors)]
    pub fn extract_factors(&self, panel: &JsYieldPanel) -> Result<JsDieboldLi, JsValue> {
        self.inner
            .clone()
            .extract_factors(&panel.inner)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Fit the VAR(1) dynamics of the extracted factors.
    /// @returns A new model carrying `phi`, `mu` and `qCov`.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if factors have not been extracted or there
    /// are too few observations, and a `computation` error if the regression
    /// is singular.
    #[wasm_bindgen(js_name = fitVar)]
    pub fn fit_var(&self) -> Result<JsDieboldLi, JsValue> {
        self.inner
            .clone()
            .fit_var()
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Extract the factors and fit their VAR(1) dynamics in one call.
    /// @param panel - `YieldPanel` handle of observed yields.
    /// @returns A new fully fitted model.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the panel is too small, and a
    /// `computation` error if the regression is singular.
    pub fn fit(&self, panel: &JsYieldPanel) -> Result<JsDieboldLi, JsValue> {
        self.inner
            .clone()
            .fit(&panel.inner)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Forecast the yield curve a number of periods ahead.
    /// @param horizon - Forecast horizon in observation periods; a positive safe integer.
    /// @returns The `YieldForecast` object (`horizon`, `yields`, `tenors`, `factors`, `lower_95`, `upper_95`).
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the model is not fitted or `horizon` is zero.
    pub fn forecast(&self, horizon: JsValue) -> Result<JsValue, JsValue> {
        let forecast = self
            .inner
            .forecast(js_uint(&horizon, "horizon")?)
            .map_err(to_js_err)?;
        to_js_value(&forecast)
    }
}

/// Principal-component analysis of yield-curve changes.
#[wasm_bindgen(js_name = YieldPca)]
pub struct JsYieldPca {
    pub(crate) inner: YieldPca,
}

json_round_trip!(JsYieldPca, YieldPca);

#[wasm_bindgen(js_class = YieldPca)]
impl JsYieldPca {
    /// Fit the PCA to the yield changes of a panel.
    /// @param panel - `YieldPanel` handle of observed yields.
    /// @returns The fitted PCA.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the panel has too few dates or tenors,
    /// and a `computation` error if the eigendecomposition fails.
    pub fn fit(panel: &JsYieldPanel) -> Result<JsYieldPca, JsValue> {
        YieldPca::fit(&panel.inner)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Fit the PCA directly to a matrix of yield changes.
    /// @param yield_changes - Yield changes as nested rows: one `number[]` per date, one decimal change per tenor.
    /// @returns The fitted PCA.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the rows are ragged or too few, and a
    /// `computation` error if the eigendecomposition fails.
    #[wasm_bindgen(js_name = fitYieldChanges)]
    pub fn fit_yield_changes(yield_changes: JsValue) -> Result<JsYieldPca, JsValue> {
        YieldPca::fit_yield_changes(js_f64_matrix(&yield_changes, "yieldChanges")?)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Number of principal components (equal to the number of tenors).
    #[wasm_bindgen(getter, js_name = numComponents)]
    pub fn num_components(&self) -> usize {
        self.inner.num_components()
    }

    /// Eigenvalues of the covariance of yield changes, largest first.
    #[wasm_bindgen(getter)]
    pub fn eigenvalues(&self) -> Box<[f64]> {
        self.inner.eigenvalues().into()
    }

    /// Loadings as nested rows: one row per tenor, one column per component.
    #[wasm_bindgen(getter)]
    pub fn loadings(&self) -> Result<JsValue, JsValue> {
        to_js_value(&matrix_rows!(self.inner.loadings()))
    }

    /// Scores as nested rows: one row per date, one column per component.
    #[wasm_bindgen(getter)]
    pub fn scores(&self) -> Result<JsValue, JsValue> {
        to_js_value(&matrix_rows!(self.inner.scores()))
    }

    /// Tenors in years, one per loading row.
    #[wasm_bindgen(getter)]
    pub fn tenors(&self) -> Box<[f64]> {
        self.inner.tenors().into()
    }

    /// Fraction of total variance explained by each component.
    #[wasm_bindgen(getter, js_name = varianceExplained)]
    pub fn variance_explained(&self) -> Box<[f64]> {
        self.inner.variance_explained().into()
    }

    /// Cumulative fraction of variance explained by the leading components.
    #[wasm_bindgen(getter, js_name = cumulativeVariance)]
    pub fn cumulative_variance(&self) -> Box<[f64]> {
        self.inner.cumulative_variance().into()
    }

    /// Mean yield change per tenor removed before the decomposition.
    #[wasm_bindgen(getter, js_name = meanChange)]
    pub fn mean_change(&self) -> Box<[f64]> {
        self.inner
            .mean_change()
            .iter()
            .copied()
            .collect::<Vec<f64>>()
            .into_boxed_slice()
    }

    /// Loading vector of one component across tenors.
    /// @param k - Zero-based component index.
    /// @returns One loading per tenor.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if `k` is not below `numComponents`.
    pub fn loading(&self, k: JsValue) -> Result<Box<[f64]>, JsValue> {
        self.inner
            .loading(js_uint(&k, "k")?)
            .map(|v| v.iter().copied().collect::<Vec<f64>>().into_boxed_slice())
            .map_err(to_js_err)
    }

    /// Smallest number of leading components whose cumulative variance reaches a threshold.
    /// @param threshold - Target cumulative explained-variance fraction, from 0 through 1.
    /// @returns The number of components needed.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if `threshold` is not a number.
    #[wasm_bindgen(js_name = componentsForThreshold)]
    pub fn components_for_threshold(&self, threshold: JsValue) -> Result<usize, JsValue> {
        Ok(self
            .inner
            .components_for_threshold(js_f64(&threshold, "threshold")?))
    }

    /// Yield-change scenario from component shocks in standard deviations.
    /// @param shocks - One shock per leading component, in standard deviations of that component's score.
    /// @returns The yield change per tenor, as decimals.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if more shocks than components are given.
    pub fn scenario(&self, shocks: JsValue) -> Result<Box<[f64]>, JsValue> {
        self.inner
            .scenario(&js_f64_seq(&shocks, "shocks")?)
            .map(Vec::into_boxed_slice)
            .map_err(to_js_err)
    }

    /// Apply a component-shock scenario to a base yield curve.
    /// @param base_yields - Base yields per tenor, as decimals; one per tenor.
    /// @param shocks - One shock per leading component, in standard deviations of that component's score.
    /// @returns The shocked yields per tenor.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the lengths do not match the model.
    #[wasm_bindgen(js_name = applyScenario)]
    pub fn apply_scenario(
        &self,
        base_yields: JsValue,
        shocks: JsValue,
    ) -> Result<Box<[f64]>, JsValue> {
        self.inner
            .apply_scenario(
                &js_f64_seq(&base_yields, "baseYields")?,
                &js_f64_seq(&shocks, "shocks")?,
            )
            .map(Vec::into_boxed_slice)
            .map_err(to_js_err)
    }

    /// Reconstruct the yield changes from the leading components.
    /// @param num_components - Number of leading components to keep.
    /// @returns Nested rows: one row per date, one reconstructed change per tenor.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if `numComponents` is zero or exceeds the model.
    pub fn reconstruct(&self, num_components: JsValue) -> Result<JsValue, JsValue> {
        let matrix = self
            .inner
            .reconstruct(js_uint(&num_components, "numComponents")?)
            .map_err(to_js_err)?;
        to_js_value(&matrix_rows!(matrix))
    }

    /// Reporting view restricted to the leading components.
    /// @param n_components - Number of leading components to keep.
    /// @returns The `YieldPcaView` object (`loadings`, `scores`, `eigenvalues`, `explained_variance_ratio`, `cumulative_variance`, `mean_change`, `tenors`).
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if `nComponents` is zero or exceeds the model.
    pub fn truncated(&self, n_components: JsValue) -> Result<JsValue, JsValue> {
        let view = self
            .inner
            .truncated(js_uint(&n_components, "nComponents")?)
            .map_err(to_js_err)?;
        to_js_value(&view)
    }
}

/// Extract Diebold-Li level, slope and curvature factors from a yield matrix.
/// @param tenors - Maturities in years, one per column of `yieldsMatrix`.
/// @param yields_matrix - Yields as nested rows: one `number[]` per date, one decimal yield per tenor.
/// @param lambda - Optional decay parameter; omitted uses the Rust default (0.7308).
/// @returns The `FactorTimeSeries` object (`factors`, `residuals`, `r_squared`, `r_squared_avg`, `dates`).
///
/// # Errors
///
/// Throws a `validation` error if the inputs are ragged, too small or non-finite.
#[wasm_bindgen(js_name = dieboldLiFitFactors)]
pub fn diebold_li_fit_factors(
    tenors: JsValue,
    yields_matrix: JsValue,
    lambda: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let factors = dtsm::diebold_li_fit_factors(
        js_f64_seq(&tenors, "tenors")?,
        js_f64_matrix(&yields_matrix, "yieldsMatrix")?,
        js_opt_f64(lambda.as_ref(), "lambda")?,
    )
    .map_err(to_js_err)?;
    to_js_value(&factors)
}

/// Fit Diebold-Li to a yield matrix and forecast the curve.
/// @param tenors - Maturities in years, one per column of `yieldsMatrix`.
/// @param yields_matrix - Yields as nested rows: one `number[]` per date, one decimal yield per tenor.
/// @param horizon - Forecast horizon in observation periods; a positive safe integer.
/// @param lambda - Optional decay parameter; omitted uses the Rust default (0.7308).
/// @returns The `YieldForecast` object (`horizon`, `yields`, `tenors`, `factors`, `lower_95`, `upper_95`).
///
/// # Errors
///
/// Throws a `validation` error if the inputs are ragged, too small or
/// non-finite, and a `computation` error if the VAR regression is singular.
#[wasm_bindgen(js_name = dieboldLiForecast)]
pub fn diebold_li_forecast(
    tenors: JsValue,
    yields_matrix: JsValue,
    horizon: JsValue,
    lambda: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let forecast = dtsm::diebold_li_forecast(
        js_f64_seq(&tenors, "tenors")?,
        js_f64_matrix(&yields_matrix, "yieldsMatrix")?,
        js_uint(&horizon, "horizon")?,
        js_opt_f64(lambda.as_ref(), "lambda")?,
    )
    .map_err(to_js_err)?;
    to_js_value(&forecast)
}

/// Fit a yield-curve PCA and return the leading components.
/// @param yield_changes - Yield changes as nested rows: one `number[]` per date, one decimal change per tenor.
/// @param n_components - Optional number of leading components to keep; omitted uses the Rust default (3).
/// @returns The `YieldPcaView` object (`loadings`, `scores`, `eigenvalues`, `explained_variance_ratio`, `cumulative_variance`, `mean_change`, `tenors`).
///
/// # Errors
///
/// Throws a `validation` error if the rows are ragged or too few or
/// `nComponents` exceeds the number of tenors.
#[wasm_bindgen(js_name = yieldPcaFit)]
pub fn yield_pca_fit(
    yield_changes: JsValue,
    n_components: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let n_components =
        js_opt_uint(n_components.as_ref(), "nComponents")?.unwrap_or(dtsm::DEFAULT_PCA_COMPONENTS);
    let view = YieldPca::fit_yield_changes(js_f64_matrix(&yield_changes, "yieldChanges")?)
        .and_then(|pca| pca.truncated(n_components))
        .map_err(to_js_err)?;
    to_js_value(&view)
}

/// Yield-change scenario from a shock to one principal component.
/// @param yield_changes - Yield changes as nested rows: one `number[]` per date, one decimal change per tenor.
/// @param component_index - Zero-based index of the shocked component.
/// @param sigma_shock - Shock size in standard deviations of that component's score.
/// @param n_components - Optional number of leading components retained; omitted uses the Rust default (3).
/// @returns The yield change per tenor, as decimals.
///
/// # Errors
///
/// Throws a `validation` error if the rows are ragged or too few or
/// `componentIndex` is not below `nComponents`.
#[wasm_bindgen(js_name = yieldPcaScenario)]
pub fn yield_pca_scenario(
    yield_changes: JsValue,
    component_index: JsValue,
    sigma_shock: JsValue,
    n_components: Option<JsValue>,
) -> Result<Box<[f64]>, JsValue> {
    let n_components =
        js_opt_uint(n_components.as_ref(), "nComponents")?.unwrap_or(dtsm::DEFAULT_PCA_COMPONENTS);
    YieldPca::scenario_from_yield_changes(
        js_f64_matrix(&yield_changes, "yieldChanges")?,
        js_uint(&component_index, "componentIndex")?,
        js_f64(&sigma_shock, "sigmaShock")?,
        n_components,
    )
    .map(Vec::into_boxed_slice)
    .map_err(to_js_err)
}
