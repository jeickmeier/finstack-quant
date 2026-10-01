import * as wasm from '../pkg/finstack_quant_wasm.js';

export const cashflows = {
  absToSmm: wasm.absToSmm,
  accruedInterest: wasm.accruedInterest,
  buildCashflowScheduleJson: wasm.buildCashflowScheduleJson,
  cdrToMdr: wasm.cdrToMdr,
  cprToSmm: wasm.cprToSmm,
  datedFlowsJson: wasm.datedFlowsJson,
  mdrToCdr: wasm.mdrToCdr,
  scheduleCalendarYearLadder: wasm.scheduleCalendarYearLadder,
  scheduleOutstandingByDate: wasm.scheduleOutstandingByDate,
  scheduleWal: wasm.scheduleWal,
  smmToCpr: wasm.smmToCpr,
  validateCashflowScheduleJson: wasm.validateCashflowScheduleJson,
};
