import * as wasm from '../../pkg/finstack_quant_wasm.js';

export const instruments = {
  Bond: wasm.Bond,
  TermLoan: wasm.TermLoan,
  RevolvingCredit: wasm.RevolvingCredit,
  bondFromCashflowsJson: wasm.bondFromCashflowsJson,
  validateInstrumentJson: wasm.validateInstrumentJson,
  priceInstrument: wasm.priceInstrument,
  priceInstrumentWithMarket: wasm.priceInstrumentWithMarket,
  instrumentCashflowsJson: wasm.instrumentCashflowsJson,
  instrumentCashflowsWithMarketJson: wasm.instrumentCashflowsWithMarketJson,
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
