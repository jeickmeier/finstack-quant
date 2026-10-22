import * as wasm from '../../../pkg/finstack_quant_wasm.js';

export const recoveryWaterfall = {
  allocateRecovery: wasm.allocateRecovery,
  recoveryClaimTotalClaim: wasm.recoveryClaimTotalClaim,
};
