import * as wasm from '../pkg/finstack_quant_wasm.js';

export const features = {
  transformTimeseries: wasm.transformTimeseries,
  transformCrossSectional: wasm.transformCrossSectional,
  transformPanel: wasm.transformPanel,
};
