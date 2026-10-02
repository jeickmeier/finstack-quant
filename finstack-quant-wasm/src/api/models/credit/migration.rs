//! WASM bindings for credit migration: rating scales, transition matrices,
//! CTMC generators and seeded path simulation.
//!
//! Mirrors `finstack-quant-py/src/bindings/models/credit/migration.rs`.

use crate::utils::input::{
    js_f64, js_f64_matrix, js_f64_seq, js_string, js_string_seq, js_u64, js_uint,
};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_models::credit::migration::{
    self, projection, GeneratorMatrix, MigrationSimulator, RatingPath, RatingScale,
    TransitionMatrix,
};
use wasm_bindgen::prelude::*;

/// Square-matrix input as flat row-major numbers or as an array of rows.
fn js_matrix_data(value: &JsValue, label: &str) -> Result<Vec<f64>, JsValue> {
    let nested = js_sys::Array::is_array(value)
        && js_sys::Array::is_array(&js_sys::Array::from(value).get(0));
    if !nested {
        return js_f64_seq(value, label);
    }
    let rows = js_f64_matrix(value, label)?;
    let width = rows.first().map_or(0, Vec::len);
    if rows.iter().any(|row| row.len() != width) {
        return Err(to_js_err(format!(
            "{label}: matrix rows must all have the same length"
        )));
    }
    Ok(rows.into_iter().flatten().collect())
}

/// Ordered set of rating states, optionally with an absorbing default state.
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
    pub fn custom(labels: JsValue) -> Result<JsRatingScale, JsValue> {
        RatingScale::custom(js_string_seq(&labels, "labels")?)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Custom scale with an explicitly named default state.
    /// @param labels - Distinct state labels in order, at least two.
    /// @param default_label - Label of the absorbing default state; must be one of `labels`.
    /// @returns The custom rating scale.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the labels are invalid, and a
    /// `not_found` error if `defaultLabel` is not among them.
    #[wasm_bindgen(js_name = customWithDefault)]
    pub fn custom_with_default(
        labels: JsValue,
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
    pub fn index_of(&self, label: JsValue) -> Result<Option<usize>, JsValue> {
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
    pub fn index_of_required(&self, label: JsValue) -> Result<usize, JsValue> {
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
    pub fn label_of(&self, index: JsValue) -> Result<Option<String>, JsValue> {
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
    pub fn warf(&self, label: JsValue) -> Result<f64, JsValue> {
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
    pub fn rating_from_warf(&self, warf: JsValue) -> Result<String, JsValue> {
        self.inner
            .rating_from_warf(js_f64(&warf, "warf")?)
            .map(str::to_owned)
            .map_err(to_js_err)
    }
}

/// Rating transition matrix over a fixed horizon; rows sum to one.
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
        data: JsValue,
        horizon: JsValue,
    ) -> Result<JsTransitionMatrix, JsValue> {
        let data = js_matrix_data(&data, "data")?;
        TransitionMatrix::new(scale.inner.clone(), &data, js_f64(&horizon, "horizon")?)
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
    pub fn probability(&self, from: JsValue, to: JsValue) -> Result<f64, JsValue> {
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
    pub fn probability_by_index(&self, from: JsValue, to: JsValue) -> Result<f64, JsValue> {
        self.inner
            .try_probability_by_index(js_uint(&from, "from")?, js_uint(&to, "to")?)
            .map_err(to_js_err)
    }

    /// One row of transition probabilities, indexed by destination state.
    /// @param from - Label of the starting state.
    /// @returns The probabilities of ending in each state, in scale order.
    ///
    /// # Errors
    ///
    /// Throws a `not_found` error if the label is not in the scale.
    pub fn row(&self, from: JsValue) -> Result<Box<[f64]>, JsValue> {
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
    #[wasm_bindgen(js_name = toMatrix)]
    pub fn to_matrix(&self) -> Result<JsValue, JsValue> {
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
    pub fn new(scale: &JsRatingScale, data: JsValue) -> Result<JsGeneratorMatrix, JsValue> {
        let data = js_matrix_data(&data, "data")?;
        GeneratorMatrix::new(scale.inner.clone(), &data)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Estimate the generator of a transition matrix by matrix logarithm with
    /// regularization (Israel-Rosenthal-Wei).
    /// @param p - `TransitionMatrix` whose generator to extract.
    /// @returns The regularized generator; `roundTripError` reports the fit.
    ///
    /// # Errors
    ///
    /// Throws a `computation` error if no valid generator exists or the round
    /// trip exceeds the default tolerance.
    #[wasm_bindgen(js_name = fromTransitionMatrix)]
    pub fn from_transition_matrix(p: &JsTransitionMatrix) -> Result<JsGeneratorMatrix, JsValue> {
        GeneratorMatrix::from_transition_matrix(&p.inner)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Estimate the generator of a transition matrix with an explicit round-trip tolerance.
    /// @param p - `TransitionMatrix` whose generator to extract.
    /// @param round_trip_tol - Largest accepted absolute difference between `exp(Q * horizon)` and `p`.
    /// @returns The regularized generator.
    ///
    /// # Errors
    ///
    /// Throws a `computation` error if no valid generator exists or the round
    /// trip exceeds `roundTripTol`.
    #[wasm_bindgen(js_name = fromTransitionMatrixWithTol)]
    pub fn from_transition_matrix_with_tol(
        p: &JsTransitionMatrix,
        round_trip_tol: JsValue,
    ) -> Result<JsGeneratorMatrix, JsValue> {
        GeneratorMatrix::from_transition_matrix_with_tol(
            &p.inner,
            js_f64(&round_trip_tol, "roundTripTol")?,
        )
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
    pub fn intensity(&self, from: JsValue, to: JsValue) -> Result<f64, JsValue> {
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
    pub fn exit_rate(&self, state: JsValue) -> Result<f64, JsValue> {
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
    #[wasm_bindgen(js_name = toMatrix)]
    pub fn to_matrix(&self) -> Result<JsValue, JsValue> {
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
    pub fn state_at(&self, t: JsValue) -> Result<usize, JsValue> {
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
    pub fn label_at(&self, t: JsValue) -> Result<String, JsValue> {
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
        horizon: JsValue,
    ) -> Result<JsMigrationSimulator, JsValue> {
        MigrationSimulator::new(generator.inner.clone(), js_f64(&horizon, "horizon")?)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Simulate independent rating paths from one starting state.
    /// @param initial_state - Zero-based starting state index.
    /// @param n_paths - Number of paths; a safe non-negative integer.
    /// @param seed - Seed of the Rust PCG64 generator, as a safe integer or `bigint`; equal seeds give equal paths in every host.
    /// @returns The simulated `RatingPaths` batch.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if `initialState` is outside the scale.
    pub fn simulate(
        &self,
        initial_state: JsValue,
        n_paths: JsValue,
        seed: JsValue,
    ) -> Result<JsRatingPaths, JsValue> {
        self.inner
            .simulate_seeded(
                js_uint(&initial_state, "initialState")?,
                js_uint(&n_paths, "nPaths")?,
                js_u64(&seed, "seed")?,
            )
            .map(|inner| JsRatingPaths { inner })
            .map_err(to_js_err)
    }

    /// Estimate the transition matrix over the horizon by simulation.
    /// @param n_paths_per_state - Paths simulated from every starting state; positive.
    /// @param seed - Seed of the Rust PCG64 generator, as a safe integer or `bigint`.
    /// @returns The empirical `TransitionMatrix`.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if `nPathsPerState` is zero.
    #[wasm_bindgen(js_name = empiricalMatrix)]
    pub fn empirical_matrix(
        &self,
        n_paths_per_state: JsValue,
        seed: JsValue,
    ) -> Result<JsTransitionMatrix, JsValue> {
        self.inner
            .empirical_matrix_seeded(
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
pub fn project(generator: &JsGeneratorMatrix, t: JsValue) -> Result<JsTransitionMatrix, JsValue> {
    projection::project(&generator.inner, js_f64(&t, "t")?)
        .map(|inner| JsTransitionMatrix { inner })
        .map_err(to_js_err)
}
