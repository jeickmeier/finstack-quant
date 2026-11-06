import * as wasm from '../pkg/finstack_quant_wasm.js';

/**
 * Vectorized panel feature transforms.
 *
 * `values` accepts finite numbers or `null`; non-finite values are treated as
 * missing by the Rust crate. Time-series transforms are grouped by `entity` and
 * sorted by `order`; cross-sectional transforms partition by `timeKey`.
 * @example
 * ```typescript
 * import init, { features } from "finstack-quant-wasm";
 * await init();
 * const changes = features.transformTimeseries(
 *   [1, 3, 6],
 *   ["ACME", "ACME", "ACME"],
 *   ["2026-01-01", "2026-01-02", "2026-01-03"],
 *   "diff"
 * );
 * console.log(changes);
 * ```
 */
export const features = {
  neutralize: wasm.neutralize,
  neutralizeAndZscore: wasm.neutralizeAndZscore,
  rankToWeights: wasm.rankToWeights,
  riskScaledWeights: wasm.riskScaledWeights,
  rollingRegressionResidual: wasm.rollingRegressionResidual,
  transformTimeseries: wasm.transformTimeseries,
  transformTimeseriesPairwise: wasm.transformTimeseriesPairwise,
  transformCrossSectional: wasm.transformCrossSectional,
  transformCrossSectionalGrouped: wasm.transformCrossSectionalGrouped,
  transformPanelJson: wasm.transformPanelJson,
  transformPanel: wasm.transformPanel,
  panelTransformResultGetColumn: wasm.panelTransformResultGetColumn,
  timeSeriesOpValues: wasm.timeSeriesOpValues,
  timeSeriesOpParamKeys: wasm.timeSeriesOpParamKeys,
  crossSectionalOpValues: wasm.crossSectionalOpValues,
  crossSectionalOpParamKeys: wasm.crossSectionalOpParamKeys,
  pairwiseOpValues: wasm.pairwiseOpValues,
  pairwiseOpParamKeys: wasm.pairwiseOpParamKeys,
};
