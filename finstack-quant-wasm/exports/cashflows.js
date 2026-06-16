import * as wasm from '../pkg/finstack_quant_wasm.js';

export const cashflows = {
  accruedInterestJson: wasm.accruedInterestJson,
  bondFromCashflowsJson: wasm.bondFromCashflowsJson,
  buildCashflowScheduleEnvelopeJson: wasm.buildCashflowScheduleEnvelopeJson,
  buildCashflowScheduleJson: wasm.buildCashflowScheduleJson,
  datedFlowsJson: wasm.datedFlowsJson,
  validateCashflowScheduleEnvelopeJson: wasm.validateCashflowScheduleEnvelopeJson,
  validateCashflowScheduleJson: wasm.validateCashflowScheduleJson,
};
