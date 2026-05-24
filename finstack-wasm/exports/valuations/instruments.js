import * as wasm from '../../pkg/finstack_wasm.js';

export const instruments = {
  validateInstrumentJson: wasm.validateInstrumentJson,
  priceInstrument: wasm.priceInstrument,
  priceInstrumentWithMetrics: wasm.priceInstrumentWithMetrics,
  priceInstrumentWithMarket: wasm.priceInstrumentWithMarket,
  priceInstrumentWithMetricsAndMarket: wasm.priceInstrumentWithMetricsAndMarket,
  instrumentCashflowsWithMarket: wasm.instrumentCashflowsWithMarket,
  listStandardMetrics: wasm.listStandardMetrics,
  listStandardMetricsGrouped: wasm.listStandardMetricsGrouped,
};
