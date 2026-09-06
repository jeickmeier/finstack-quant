import * as wasm from '../pkg/finstack_quant_wasm.js';

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
};
