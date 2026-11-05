//! WASM bindings for credit migration: rating scales, transition matrices,
//! CTMC generators and seeded path simulation.
//!
//! Mirrors `finstack-quant-py/src/bindings/models/credit/migration.rs`.

use crate::utils::input::{
    from_js_json, js_f64, js_f64_matrix, js_f64_seq, js_opt_f64, js_string, js_string_seq, js_u64,
    js_uint,
};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_core::math::linalg::flatten_square_matrix;
use finstack_quant_models::credit::migration::{
    self, GeneratorMatrix, MigrationSimulator, RatingPath, RatingScale, TransitionMatrix,
};
use wasm_bindgen::prelude::*;

/// Square-matrix input for an `n`-state scale, as flat row-major numbers or as
/// an array of `n` rows of `n` numbers.
fn js_matrix_data(value: &JsValue, n: usize, label: &str) -> Result<Vec<f64>, JsValue> {
    let nested = js_sys::Array::is_array(value)
        && js_sys::Array::is_array(&js_sys::Array::from(value).get(0));
    if !nested {
        return js_f64_seq(value, label);
    }
    flatten_square_matrix(js_f64_matrix(value, label)?, n, label).map_err(to_js_err)
}

/// Ordered set of rating states, optionally with an absorbing default state.
///
/// @example
/// ```typescript
/// import init, { models } from "finstack-quant-wasm";
/// await init();
/// const scale = models.credit.migration.RatingScale.custom(["A", "B", "D"]);
/// console.log(scale.nStates, scale.labels(), scale.defaultState());
/// scale.free();
/// ```
#[wasm_bindgen(js_name = RatingScale)]
#[derive(Clone)]
pub struct JsRatingScale {
    pub(crate) inner: RatingScale,
}

json_round_trip!(JsRatingScale, RatingScale);

#[wasm_bindgen(js_class = RatingScale)]
impl JsRatingScale {
    /// Standard letter scale `AAA, AA, A, BBB, BB, B, CCC, D`.
    /// @returns The standard eight-state scale with `D` as the default state.
    pub fn standard() -> JsRatingScale {
        Self {
            inner: RatingScale::standard(),
        }
    }

    /// Standard letter scale with an additional not-rated (`NR`) state.
    /// @returns The standard scale extended with `NR`.
    #[wasm_bindgen(js_name = standardWithNr)]
    pub fn standard_with_nr() -> JsRatingScale {
        Self {
            inner: RatingScale::standard_with_nr(),
        }
    }

    /// Notched scale (`AA+`, `AA`, `AA-`, ...) with `D` as the default state.
    /// @returns The notched rating scale.
    pub fn notched() -> JsRatingScale {
        Self {
            inner: RatingScale::notched(),
        }
    }

    /// Custom scale from explicit labels, best credit first.
    /// @param labels - Distinct state labels in order, at least two; a label `"D"` is treated as the default state.
    /// @returns The custom rating scale.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if fewer than two labels are given or a
    /// label repeats.
    pub fn custom(
        #[wasm_bindgen(unchecked_param_type = "string[]")] labels: JsValue,
    ) -> Result<JsRatingScale, JsValue> {
        RatingScale::custom(js_string_seq(&labels, "labels")?)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Custom scale with an explicitly named default state.
    /// @param labels - Distinct state labels in order, at least two.
    /// @param defaultLabel - Label of the absorbing default state; must be one of `labels`.
    /// @returns The custom rating scale.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the labels are invalid, and a
    /// `not_found` error if `defaultLabel` is not among them.
    #[wasm_bindgen(js_name = customWithDefault)]
    pub fn custom_with_default(
        #[wasm_bindgen(unchecked_param_type = "string[]")] labels: JsValue,
        #[wasm_bindgen(js_name = "defaultLabel", unchecked_param_type = "string")]
        default_label: JsValue,
    ) -> Result<JsRatingScale, JsValue> {
        RatingScale::custom_with_default(
            js_string_seq(&labels, "labels")?,
            js_string(&default_label, "defaultLabel")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Number of states in the scale.
    #[wasm_bindgen(getter, js_name = nStates)]
    pub fn n_states(&self) -> usize {
        self.inner.n_states()
    }

    /// Zero-based index of a state label.
    /// @param label - State label to look up, such as `"BBB"`.
    /// @returns The index, or `undefined` when the label is not in the scale.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if `label` is not a string.
    #[wasm_bindgen(js_name = indexOf)]
    pub fn index_of(
        &self,
        #[wasm_bindgen(unchecked_param_type = "string")] label: JsValue,
    ) -> Result<Option<usize>, JsValue> {
        Ok(self.inner.index_of(&js_string(&label, "label")?))
    }

    /// Zero-based index of a state label that must exist.
    /// @param label - State label to look up, such as `"BBB"`.
    /// @returns The index of the label.
    ///
    /// # Errors
    ///
    /// Throws a `not_found` error if the label is not in the scale.
    #[wasm_bindgen(js_name = indexOfRequired)]
    pub fn index_of_required(
        &self,
        #[wasm_bindgen(unchecked_param_type = "string")] label: JsValue,
    ) -> Result<usize, JsValue> {
        self.inner
            .index_of_required(&js_string(&label, "label")?)
            .map_err(to_js_err)
    }

    /// Label of a state index.
    /// @param index - Zero-based state index.
    /// @returns The label, or `undefined` when the index is out of range.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if `index` is not a safe non-negative integer.
    #[wasm_bindgen(js_name = labelOf)]
    pub fn label_of(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] index: JsValue,
    ) -> Result<Option<String>, JsValue> {
        let index: usize = js_uint(&index, "index")?;
        Ok(self.inner.label_of(index).map(str::to_owned))
    }

    /// Index of the absorbing default state.
    /// @returns The default-state index, or `undefined` when the scale has none.
    #[wasm_bindgen(js_name = defaultState)]
    pub fn default_state(&self) -> Option<usize> {
        self.inner.default_state()
    }

    /// State labels in scale order.
    /// @returns The labels, best credit first.
    pub fn labels(&self) -> Vec<String> {
        self.inner.labels().to_vec()
    }

    /// Moody's WARF factor of a rating label.
    /// @param label - Rating label in this scale, such as `"BBB"`.
    /// @returns The weighted-average rating factor of that rating.
    ///
    /// # Errors
    ///
    /// Throws a `not_found` error if the label is unknown or has no WARF factor.
    pub fn warf(
        &self,
        #[wasm_bindgen(unchecked_param_type = "string")] label: JsValue,
    ) -> Result<f64, JsValue> {
        self.inner
            .warf(&js_string(&label, "label")?)
            .map_err(to_js_err)
    }

    /// Rating whose WARF band contains a portfolio WARF.
    /// @param warf - Weighted-average rating factor; finite and non-negative.
    /// @returns The label of the rating equivalent to that WARF.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if `warf` is invalid or the scale has no
    /// WARF mapping.
    #[wasm_bindgen(js_name = ratingFromWarf)]
    pub fn rating_from_warf(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] warf: JsValue,
    ) -> Result<String, JsValue> {
        self.inner
            .rating_from_warf(js_f64(&warf, "warf")?)
            .map(str::to_owned)
            .map_err(to_js_err)
    }
}

/// Rating transition matrix over a fixed horizon; rows sum to one.
///
/// @example
/// ```typescript
/// import init, { models } from "finstack-quant-wasm";
/// await init();
/// const scale = models.credit.migration.RatingScale.custom(["A", "B", "D"]);
/// const matrix = new models.credit.migration.TransitionMatrix(
///   scale,
///   [[0.9, 0.08, 0.02], [0.1, 0.8, 0.1], [0, 0, 1]],
///   1.0,
/// );
/// console.log(matrix.probability("A", "D"), matrix.defaultProbabilities());
/// ```
#[wasm_bindgen(js_name = TransitionMatrix)]
pub struct JsTransitionMatrix {
    pub(crate) inner: TransitionMatrix,
}

json_round_trip!(JsTransitionMatrix, TransitionMatrix);

#[wasm_bindgen(js_class = TransitionMatrix)]
impl JsTransitionMatrix {
    /// Transition matrix from probabilities.
    /// @param scale - `RatingScale` handle defining the state order.
    /// @param data - Transition probabilities, either flat row-major (`nStates * nStates` numbers) or as an array of rows; each row sums to one.
    /// @param horizon - Horizon the probabilities apply to, in years; positive.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the dimensions do not match the scale, an
    /// entry is outside `[0, 1]`, a row does not sum to one, the default state
    /// is not absorbing, or `horizon` is not positive.
    #[wasm_bindgen(constructor)]
    pub fn new(
        scale: &JsRatingScale,
        #[wasm_bindgen(unchecked_param_type = "NumericArray | NumericArray[]")] data: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] horizon: JsValue,
    ) -> Result<JsTransitionMatrix, JsValue> {
        let data = js_matrix_data(&data, scale.inner.n_states(), "data")?;
        TransitionMatrix::new(scale.inner.clone(), &data, js_f64(&horizon, "horizon")?)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Construct a transition matrix from ordered origin and destination labels.
    /// @param rowLabels - Ordered origin labels, one per row.
    /// @param columnLabels - Ordered destination labels; must equal `rowLabels` exactly.
    /// @param data - Flat row-major probabilities or square nested rows; each row sums to one.
    /// @param horizon - Positive finite horizon in years covered by the probabilities.
    /// @param scale - Optional `RatingScale` JSON text or plain object, such as `scale.toJson()`; its labels must match both vectors in order. Omit to infer a custom scale with the last label absorbing.
    /// @returns The validated transition-matrix handle; no entries are reordered.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` for invalid host input types, or a `validation`
    /// error for differing label orders, invalid labels, matrix dimensions,
    /// probabilities, horizon or absorbing default row.
    #[wasm_bindgen(js_name = fromLabeled)]
    pub fn from_labeled(
        #[wasm_bindgen(js_name = "rowLabels", unchecked_param_type = "readonly string[]")]
        row_labels: JsValue,
        #[wasm_bindgen(js_name = "columnLabels", unchecked_param_type = "readonly string[]")]
        column_labels: JsValue,
        #[wasm_bindgen(unchecked_param_type = "NumericArray | readonly NumericArray[]")]
        data: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] horizon: JsValue,
        #[wasm_bindgen(unchecked_optional_param_type = "JsonInput")] scale: Option<JsValue>,
    ) -> Result<JsTransitionMatrix, JsValue> {
        let row_labels = js_string_seq(&row_labels, "rowLabels")?;
        let column_labels = js_string_seq(&column_labels, "columnLabels")?;
        let data = js_matrix_data(&data, row_labels.len(), "data")?;
        let scale = scale
            .filter(|value| !value.is_null() && !value.is_undefined())
            .map(|value| from_js_json::<RatingScale>(&value, "scale"))
            .transpose()?;
        TransitionMatrix::from_labeled(
            &row_labels,
            &column_labels,
            &data,
            js_f64(&horizon, "horizon")?,
            scale,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Transition probability between two labelled states.
    /// @param from - Label of the starting state.
    /// @param to - Label of the ending state.
    /// @returns The probability of moving from `from` to `to` over the horizon.
    ///
    /// # Errors
    ///
    /// Throws a `not_found` error if either label is not in the scale.
    pub fn probability(
        &self,
        #[wasm_bindgen(unchecked_param_type = "string")] from: JsValue,
        #[wasm_bindgen(unchecked_param_type = "string")] to: JsValue,
    ) -> Result<f64, JsValue> {
        self.inner
            .probability(&js_string(&from, "from")?, &js_string(&to, "to")?)
            .map_err(to_js_err)
    }

    /// Transition probability between two state indices.
    /// @param from - Zero-based index of the starting state.
    /// @param to - Zero-based index of the ending state.
    /// @returns The probability of moving from `from` to `to` over the horizon.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if either index is outside the scale.
    #[wasm_bindgen(js_name = probabilityByIndex)]
    pub fn probability_by_index(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] from: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] to: JsValue,
    ) -> Result<f64, JsValue> {
        self.inner
            .probability_by_index(js_uint(&from, "from")?, js_uint(&to, "to")?)
            .map_err(to_js_err)
    }

    /// One row of transition probabilities, indexed by destination state.
    /// @param from - Label of the starting state.
    /// @returns The probabilities of ending in each state, in scale order.
    ///
    /// # Errors
    ///
    /// Throws a `not_found` error if the label is not in the scale.
    pub fn row(
        &self,
        #[wasm_bindgen(unchecked_param_type = "string")] from: JsValue,
    ) -> Result<Box<[f64]>, JsValue> {
        self.inner
            .row(&js_string(&from, "from")?)
            .map(Vec::into_boxed_slice)
            .map_err(to_js_err)
    }

    /// Chain this matrix with another over the same scale (matrix product).
    /// @param other - `TransitionMatrix` applied after this one; its horizon is added.
    /// @returns The composed transition matrix over the combined horizon.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the two matrices use different scales.
    pub fn compose(&self, other: &JsTransitionMatrix) -> Result<JsTransitionMatrix, JsValue> {
        self.inner
            .compose(&other.inner)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Transition probabilities as one row per starting state.
    /// @returns An array of `number[]` rows in scale order.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the rows cannot be converted.
    #[wasm_bindgen(js_name = toRows, unchecked_return_type = "number[][]")]
    pub fn to_rows(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.to_rows())
    }

    /// Horizon the probabilities apply to, in years.
    #[wasm_bindgen(getter)]
    pub fn horizon(&self) -> f64 {
        self.inner.horizon()
    }

    /// Number of states in the scale.
    #[wasm_bindgen(getter, js_name = nStates)]
    pub fn n_states(&self) -> usize {
        self.inner.n_states()
    }

    /// Rating scale that orders the rows and columns.
    #[wasm_bindgen(getter)]
    pub fn scale(&self) -> JsRatingScale {
        JsRatingScale {
            inner: self.inner.scale().clone(),
        }
    }

    /// Default probability of each state over the horizon.
    /// @returns One probability per state in scale order, or `undefined` when the scale has no default state.
    #[wasm_bindgen(js_name = defaultProbabilities)]
    pub fn default_probabilities(&self) -> Option<Box<[f64]>> {
        self.inner
            .default_probabilities()
            .map(Vec::into_boxed_slice)
    }
}

/// Continuous-time Markov-chain generator (intensity) matrix; rows sum to zero.
///
/// @example
/// ```typescript
/// import init, { models } from "finstack-quant-wasm";
/// await init();
/// const scale = models.credit.migration.RatingScale.custom(["A", "B", "D"]);
/// const generator = new models.credit.migration.GeneratorMatrix(scale, [
///   [-0.1, 0.08, 0.02],
///   [0.1, -0.2, 0.1],
///   [0, 0, 0],
/// ]);
/// console.log(generator.exitRate("A"), models.credit.migration.project(generator, 2).probability("A", "D"));
/// ```
#[wasm_bindgen(js_name = GeneratorMatrix)]
pub struct JsGeneratorMatrix {
    pub(crate) inner: GeneratorMatrix,
}

json_round_trip!(JsGeneratorMatrix, GeneratorMatrix);

#[wasm_bindgen(js_class = GeneratorMatrix)]
impl JsGeneratorMatrix {
    /// Generator matrix from transition intensities.
    /// @param scale - `RatingScale` handle defining the state order.
    /// @param data - Intensities per year, either flat row-major (`nStates * nStates` numbers) or as an array of rows; off-diagonals non-negative and each row summing to zero.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the dimensions do not match the scale or
    /// the entries do not form a valid generator.
    #[wasm_bindgen(constructor)]
    pub fn new(
        scale: &JsRatingScale,
        #[wasm_bindgen(unchecked_param_type = "NumericArray | NumericArray[]")] data: JsValue,
    ) -> Result<JsGeneratorMatrix, JsValue> {
        let data = js_matrix_data(&data, scale.inner.n_states(), "data")?;
        GeneratorMatrix::new(scale.inner.clone(), &data)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Estimate the generator of a transition matrix by matrix logarithm with
    /// regularization (Israel-Rosenthal-Wei).
    /// @param p - `TransitionMatrix` whose generator to extract.
    /// @param roundTripTol - Optional largest accepted absolute difference between `exp(Q * horizon)` and `p`; omitted or `null` uses the default `1e-2`.
    /// @returns The regularized generator; `roundTripError` reports the fit.
    ///
    /// # Errors
    ///
    /// Throws a `computation` error if no valid generator exists or the round
    /// trip exceeds the tolerance, and a `TypeError` if `roundTripTol` is not a number.
    #[wasm_bindgen(js_name = fromTransitionMatrix)]
    pub fn from_transition_matrix(
        p: &JsTransitionMatrix,
        #[wasm_bindgen(
            js_name = "roundTripTol",
            unchecked_optional_param_type = "number | null"
        )]
        round_trip_tol: Option<JsValue>,
    ) -> Result<JsGeneratorMatrix, JsValue> {
        let round_trip_tol = js_opt_f64(round_trip_tol.as_ref(), "roundTripTol")?;
        GeneratorMatrix::from_transition_matrix(&p.inner, round_trip_tol)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Transition intensity between two labelled states.
    /// @param from - Label of the starting state.
    /// @param to - Label of the ending state.
    /// @returns The intensity per year of moving from `from` to `to`.
    ///
    /// # Errors
    ///
    /// Throws a `not_found` error if either label is not in the scale.
    pub fn intensity(
        &self,
        #[wasm_bindgen(unchecked_param_type = "string")] from: JsValue,
        #[wasm_bindgen(unchecked_param_type = "string")] to: JsValue,
    ) -> Result<f64, JsValue> {
        self.inner
            .intensity(&js_string(&from, "from")?, &js_string(&to, "to")?)
            .map_err(to_js_err)
    }

    /// Total intensity per year of leaving a state.
    /// @param state - Label of the state.
    /// @returns The exit rate, the negated diagonal entry.
    ///
    /// # Errors
    ///
    /// Throws a `not_found` error if the label is not in the scale.
    #[wasm_bindgen(js_name = exitRate)]
    pub fn exit_rate(
        &self,
        #[wasm_bindgen(unchecked_param_type = "string")] state: JsValue,
    ) -> Result<f64, JsValue> {
        self.inner
            .exit_rate(&js_string(&state, "state")?)
            .map_err(to_js_err)
    }

    /// Intensities as one row per starting state.
    /// @returns An array of `number[]` rows in scale order.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the rows cannot be converted.
    #[wasm_bindgen(js_name = toRows, unchecked_return_type = "number[][]")]
    pub fn to_rows(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.to_rows())
    }

    /// Number of states in the scale.
    #[wasm_bindgen(getter, js_name = nStates)]
    pub fn n_states(&self) -> usize {
        self.inner.n_states()
    }

    /// Rating scale that orders the rows and columns.
    #[wasm_bindgen(getter)]
    pub fn scale(&self) -> JsRatingScale {
        JsRatingScale {
            inner: self.inner.scale().clone(),
        }
    }

    /// L1 size of the regularization applied when the generator was estimated (0 when built directly).
    #[wasm_bindgen(getter, js_name = regularizationL1)]
    pub fn regularization_l1(&self) -> f64 {
        self.inner.regularization_l1()
    }

    /// Largest absolute round-trip error of the estimated generator (0 when built directly).
    #[wasm_bindgen(getter, js_name = roundTripError)]
    pub fn round_trip_error(&self) -> f64 {
        self.inner.round_trip_error()
    }
}

/// One simulated rating trajectory: piecewise-constant, right-continuous.
///
/// @example
/// ```typescript
/// import init, { models } from "finstack-quant-wasm";
/// await init();
/// const scale = models.credit.migration.RatingScale.custom(["A", "B", "D"]);
/// const generator = new models.credit.migration.GeneratorMatrix(scale, [-0.1, 0.08, 0.02, 0.1, -0.2, 0.1, 0, 0, 0]);
/// const path = new models.credit.migration.MigrationSimulator(generator, 5).simulate(0, 1, 7).paths[0];
/// const copy = models.credit.migration.RatingPath.fromJson(path.toJson());
/// console.log(copy.labelAt(5), copy.defaulted());
/// ```
#[wasm_bindgen(js_name = RatingPath)]
#[derive(Clone)]
pub struct JsRatingPath {
    pub(crate) inner: RatingPath,
}

json_round_trip!(JsRatingPath, RatingPath);

#[wasm_bindgen(js_class = RatingPath)]
impl JsRatingPath {
    /// State index at a time; the initial state before 0 and the terminal state after the horizon.
    /// @param t - Time in years from the start of the path.
    /// @returns The zero-based state index occupied at `t`.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if `t` is not a number.
    #[wasm_bindgen(js_name = stateAt)]
    pub fn state_at(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] t: JsValue,
    ) -> Result<usize, JsValue> {
        Ok(self.inner.state_at(js_f64(&t, "t")?))
    }

    /// State label at a time.
    /// @param t - Time in years from the start of the path.
    /// @returns The label of the state occupied at `t`.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if `t` is not a number.
    #[wasm_bindgen(js_name = labelAt)]
    pub fn label_at(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] t: JsValue,
    ) -> Result<String, JsValue> {
        Ok(self.inner.label_at(js_f64(&t, "t")?).to_owned())
    }

    /// Whether the path reached the default state.
    /// @returns `true` when the path defaulted within the horizon.
    pub fn defaulted(&self) -> bool {
        self.inner.defaulted()
    }

    /// Time of default in years.
    /// @returns The default time, or `undefined` when the path did not default.
    #[wasm_bindgen(js_name = defaultTime)]
    pub fn default_time(&self) -> Option<f64> {
        self.inner.default_time()
    }

    /// Number of rating changes after the initial state.
    /// @returns The count of transitions along the path.
    #[wasm_bindgen(js_name = nTransitions)]
    pub fn n_transitions(&self) -> usize {
        self.inner.n_transitions()
    }

    /// Transition events as `[time, stateIndex]` pairs, starting with `[0, initialState]`.
    /// @returns The events in time order.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the events cannot be converted.
    #[wasm_bindgen(unchecked_return_type = "[number, number][]")]
    pub fn transitions(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.transitions())
    }

    /// Simulation horizon in years.
    #[wasm_bindgen(getter)]
    pub fn horizon(&self) -> f64 {
        self.inner.horizon()
    }

    /// Rating scale the state indices refer to.
    #[wasm_bindgen(getter)]
    pub fn scale(&self) -> JsRatingScale {
        JsRatingScale {
            inner: self.inner.scale().clone(),
        }
    }
}

/// A batch of simulated rating paths from `MigrationSimulator.simulate`.
///
/// @example
/// ```typescript
/// import init, { models } from "finstack-quant-wasm";
/// await init();
/// const scale = models.credit.migration.RatingScale.custom(["A", "B", "D"]);
/// const generator = new models.credit.migration.GeneratorMatrix(scale, [-0.1, 0.08, 0.02, 0.1, -0.2, 0.1, 0, 0, 0]);
/// const batch = new models.credit.migration.MigrationSimulator(generator, 5).simulate(0, 100, 7);
/// const copy = models.credit.migration.RatingPaths.fromJson(batch.toJson());
/// console.log(copy.length, copy.defaultRate);
/// ```
#[wasm_bindgen(js_name = RatingPaths)]
pub struct JsRatingPaths {
    pub(crate) inner: Vec<RatingPath>,
}

json_round_trip!(JsRatingPaths, RatingPaths);

#[wasm_bindgen(js_class = RatingPaths)]
impl JsRatingPaths {
    /// The simulated paths, in simulation order.
    #[wasm_bindgen(getter)]
    pub fn paths(&self) -> Vec<JsRatingPath> {
        self.inner
            .iter()
            .cloned()
            .map(|inner| JsRatingPath { inner })
            .collect()
    }

    /// Fraction of paths that ended in default (0 for an empty batch).
    #[wasm_bindgen(getter, js_name = defaultRate)]
    pub fn default_rate(&self) -> f64 {
        migration::default_rate(&self.inner)
    }

    /// Number of paths in the batch (twin of Python `len(paths)`).
    #[wasm_bindgen(getter)]
    pub fn length(&self) -> usize {
        self.inner.len()
    }
}

/// Gillespie simulator of rating migration under a generator matrix.
///
/// @example
/// ```typescript
/// import init, { models } from "finstack-quant-wasm";
/// await init();
/// const scale = models.credit.migration.RatingScale.custom(["A", "B", "D"]);
/// const generator = new models.credit.migration.GeneratorMatrix(scale, [-0.1, 0.08, 0.02, 0.1, -0.2, 0.1, 0, 0, 0]);
/// const simulator = new models.credit.migration.MigrationSimulator(generator, 5);
/// console.log(simulator.simulate(0, 100, 7).defaultRate);
/// console.log(simulator.empiricalMatrix(200, 7).probability("A", "D"));
/// ```
#[wasm_bindgen(js_name = MigrationSimulator)]
pub struct JsMigrationSimulator {
    pub(crate) inner: MigrationSimulator,
}

json_round_trip!(JsMigrationSimulator, MigrationSimulator);

#[wasm_bindgen(js_class = MigrationSimulator)]
impl JsMigrationSimulator {
    /// Simulator over a fixed horizon.
    /// @param generator - `GeneratorMatrix` handle driving the migration.
    /// @param horizon - Simulation horizon in years; positive and finite.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if `horizon` is not positive and finite.
    #[wasm_bindgen(constructor)]
    pub fn new(
        generator: &JsGeneratorMatrix,
        #[wasm_bindgen(unchecked_param_type = "number")] horizon: JsValue,
    ) -> Result<JsMigrationSimulator, JsValue> {
        MigrationSimulator::new(generator.inner.clone(), js_f64(&horizon, "horizon")?)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Simulate independent rating paths from one starting state.
    /// @param initialState - Zero-based starting state index.
    /// @param nPaths - Number of paths; a safe non-negative integer.
    /// @param seed - Seed of the Rust PCG64 generator, as a safe integer or `bigint`; equal seeds give equal paths in every host.
    /// @returns The simulated `RatingPaths` batch.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if `initialState` is outside the scale.
    pub fn simulate(
        &self,
        #[wasm_bindgen(js_name = "initialState", unchecked_param_type = "number")]
        initial_state: JsValue,
        #[wasm_bindgen(js_name = "nPaths", unchecked_param_type = "number")] n_paths: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number | bigint")] seed: JsValue,
    ) -> Result<JsRatingPaths, JsValue> {
        self.inner
            .simulate(
                js_uint(&initial_state, "initialState")?,
                js_uint(&n_paths, "nPaths")?,
                js_u64(&seed, "seed")?,
            )
            .map(|inner| JsRatingPaths { inner })
            .map_err(to_js_err)
    }

    /// Estimate the transition matrix over the horizon by simulation.
    /// @param nPathsPerState - Paths simulated from every starting state; positive.
    /// @param seed - Seed of the Rust PCG64 generator, as a safe integer or `bigint`.
    /// @returns The empirical `TransitionMatrix`.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if `nPathsPerState` is zero.
    #[wasm_bindgen(js_name = empiricalMatrix)]
    pub fn empirical_matrix(
        &self,
        #[wasm_bindgen(js_name = "nPathsPerState", unchecked_param_type = "number")]
        n_paths_per_state: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number | bigint")] seed: JsValue,
    ) -> Result<JsTransitionMatrix, JsValue> {
        self.inner
            .empirical_matrix(
                js_uint(&n_paths_per_state, "nPathsPerState")?,
                js_u64(&seed, "seed")?,
            )
            .map(|inner| JsTransitionMatrix { inner })
            .map_err(to_js_err)
    }

    /// Simulation horizon in years.
    #[wasm_bindgen(getter)]
    pub fn horizon(&self) -> f64 {
        self.inner.horizon()
    }

    /// Generator matrix driving the simulation.
    #[wasm_bindgen(getter)]
    pub fn generator(&self) -> JsGeneratorMatrix {
        JsGeneratorMatrix {
            inner: self.inner.generator().clone(),
        }
    }
}

/// Transition matrix implied by a generator over a horizon, `exp(Q * t)`.
/// @param generator - `GeneratorMatrix` handle to exponentiate.
/// @param t - Horizon in years; positive and finite.
/// @returns The projected `TransitionMatrix` with horizon `t`.
///
/// # Errors
///
/// Throws a `validation` error if `t` is not positive and finite, and a
/// `computation` error if the matrix exponential fails.
#[wasm_bindgen(js_name = project)]
pub fn project(
    generator: &JsGeneratorMatrix,
    #[wasm_bindgen(unchecked_param_type = "number")] t: JsValue,
) -> Result<JsTransitionMatrix, JsValue> {
    migration::project(&generator.inner, js_f64(&t, "t")?)
        .map(|inner| JsTransitionMatrix { inner })
        .map_err(to_js_err)
}
