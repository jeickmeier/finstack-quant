import * as wasm from '../../pkg/finstack_quant_wasm.js';

export const instruments = {
  validateInstrumentJson: wasm.validateInstrumentJson,
  priceInstrument: wasm.priceInstrument,
  priceInstrumentWithMetrics: wasm.priceInstrumentWithMetrics,
  priceInstrumentWithMarket: wasm.priceInstrumentWithMarket,
  priceInstrumentWithMetricsAndMarket: wasm.priceInstrumentWithMetricsAndMarket,
  instrumentCashflowsWithMarket: wasm.instrumentCashflowsWithMarket,
  listStandardMetrics: wasm.listStandardMetrics,
  listStandardMetricsGrouped: wasm.listStandardMetricsGrouped,
  structuredCreditTrancheDiscountMargin: wasm.structuredCreditTrancheDiscountMargin,
  structuredCreditTrancheBreakevenCdr: wasm.structuredCreditTrancheBreakevenCdr,
  structuredCreditTrancheOas: wasm.structuredCreditTrancheOas,
  structuredCreditTrancheScenarioTable: wasm.structuredCreditTrancheScenarioTable,
  structuredCreditTrancheMetrics: wasm.structuredCreditTrancheMetrics,
};
