import * as wasm from '../../../pkg/finstack_quant_wasm.js';

/**
 * PD calibration: master scales, PIT/TTC conversion, central tendency and the Basel IRB PD floor.
 * Mirrors Rust `credit::pd`.
 *
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * console.log(models.credit.pd.pitToTtc(0.02, 0.15, -1.0), models.credit.pd.baselIrbPdFloor());
 * ```
 */
export const pd = {
  MasterScale: wasm.MasterScale,
  baselIrbPdFloor: wasm.baselIrbPdFloor,
  applyBaselIrbPdFloor: wasm.applyBaselIrbPdFloor,
  pitToTtc: wasm.pitToTtc,
  ttcToPit: wasm.ttcToPit,
  centralTendency: wasm.centralTendency,
};
