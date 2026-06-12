import * as wasm from '../pkg/finstack_wasm.js';

export const cashflows = {
  buildCashflowScheduleJson: wasm.buildCashflowScheduleJson,
  validateCashflowScheduleJson: wasm.validateCashflowScheduleJson,
  datedFlowsJson: wasm.datedFlowsJson,
  accruedInterestJson: wasm.accruedInterestJson,
  bondFromCashflowsJson: wasm.bondFromCashflowsJson,
};
