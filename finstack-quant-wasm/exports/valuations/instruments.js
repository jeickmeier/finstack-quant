import * as wasm from '../../pkg/finstack_quant_wasm.js';

export const instruments = {
  bondFromCashflowsJson: wasm.bondFromCashflowsJson,
  validateInstrumentJson: wasm.validateInstrumentJson,
  priceInstrument: wasm.priceInstrument,
  priceInstrumentWithMetrics: wasm.priceInstrumentWithMetrics,
  priceInstrumentWithMarket: wasm.priceInstrumentWithMarket,
  priceInstrumentWithMetricsAndMarket: wasm.priceInstrumentWithMetricsAndMarket,
  instrumentCashflowsJson: wasm.instrumentCashflowsJson,
  instrumentCashflowsWithMarket: wasm.instrumentCashflowsWithMarket,
  listModels: wasm.listModels,
  listModelsGrouped: wasm.listModelsGrouped,
  listStandardMetrics: wasm.listStandardMetrics,
  listStandardMetricsGrouped: wasm.listStandardMetricsGrouped,
  structuredCreditTrancheDiscountMargin: wasm.structuredCreditTrancheDiscountMargin,
  structuredCreditTrancheBreakevenCdr: wasm.structuredCreditTrancheBreakevenCdr,
  structuredCreditTrancheOas: wasm.structuredCreditTrancheOas,
  structuredCreditTrancheScenarioTable: wasm.structuredCreditTrancheScenarioTable,
  structuredCreditTrancheMetrics: wasm.structuredCreditTrancheMetrics,
};
