//! Structural Monte Carlo simulated-path WASM handle.

use crate::utils::input::js_uint;
use crate::utils::to_js_value;
use finstack_quant_models::credit::SimulatedPaths;
use wasm_bindgen::prelude::*;

/// Monte Carlo asset-value paths from `MertonModel.simulatePaths`.
///
/// `assetValues` is row-major: path `p` occupies indices
/// `p * valuesPerPath .. (p + 1) * valuesPerPath`, where
/// `valuesPerPath == numSteps + 1` (the grid includes `t = 0`).
///
/// @example
/// ```typescript
/// import init, { models } from "finstack-quant-wasm";
/// await init();
/// const model = new models.credit.MertonModel(100, 0.25, 60, 0.03);
/// const paths = model.simulatePaths(4, 12, 1.0, 42, false);
/// const copy = models.credit.SimulatedPaths.fromJson(paths.toJson());
/// console.log(copy.numPaths, copy.valuesPerPath, copy.get(0, 0));
/// ```
#[wasm_bindgen(js_name = SimulatedPaths)]
pub struct JsSimulatedPaths {
    pub(crate) inner: SimulatedPaths,
}

json_round_trip!(JsSimulatedPaths, SimulatedPaths);

wasm_getters!(JsSimulatedPaths, SimulatedPaths |s| {
    /// Time grid in years from 0 to the simulation horizon.
    times -> Box<[f64]> = s.inner.times.clone().into_boxed_slice();
    /// Asset values in row-major order (path by path).
    asset_values as assetValues -> Box<[f64]> = s.inner.asset_values.clone().into_boxed_slice();
    /// Number of simulated paths.
    num_paths as numPaths -> usize = s.inner.num_paths;
    /// Number of time steps between grid points.
    num_steps as numSteps -> usize = s.inner.num_steps;
    /// Number of stored values per path (`numSteps + 1`, including `t = 0`).
    values_per_path as valuesPerPath -> usize = s.inner.values_per_path();
});

#[wasm_bindgen(js_class = SimulatedPaths)]
impl JsSimulatedPaths {
    /// One asset value by path and time-grid index.
    /// @param pathIdx - Zero-based path index.
    /// @param timeIdx - Zero-based time-grid index (index 0 is `t = 0`).
    /// @returns The asset value, or `undefined` when either index is out of range.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if an index is not a safe non-negative integer.
    pub fn get(
        &self,
        #[wasm_bindgen(js_name = "pathIdx", unchecked_param_type = "number")] path_idx: JsValue,
        #[wasm_bindgen(js_name = "timeIdx", unchecked_param_type = "number")] time_idx: JsValue,
    ) -> Result<Option<f64>, JsValue> {
        let path_idx: usize = js_uint(&path_idx, "pathIdx")?;
        let time_idx: usize = js_uint(&time_idx, "timeIdx")?;
        Ok(self.inner.get(path_idx, time_idx))
    }

    /// The contiguous asset-value row of one path.
    /// @param pathIdx - Zero-based path index.
    /// @returns The path's values on the time grid, or `undefined` when the index is out of range.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if `pathIdx` is not a safe non-negative integer.
    pub fn path(
        &self,
        #[wasm_bindgen(js_name = "pathIdx", unchecked_param_type = "number")] path_idx: JsValue,
    ) -> Result<Option<Box<[f64]>>, JsValue> {
        let path_idx: usize = js_uint(&path_idx, "pathIdx")?;
        Ok(self.inner.path(path_idx).map(Box::from))
    }

    /// Materialize the paths as one row of values per path.
    /// @returns An array with one `number[]` per path, each of length `valuesPerPath`.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the rows cannot be converted.
    #[wasm_bindgen(js_name = toNested, unchecked_return_type = "number[][]")]
    pub fn to_nested(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.to_nested())
    }
}
