import * as wasm from '../../../pkg/finstack_quant_wasm.js';

export const pd = {
  MasterScale: wasm.MasterScale,
  baselIrbPdFloor: wasm.baselIrbPdFloor,
  applyBaselIrbPdFloor: wasm.applyBaselIrbPdFloor,
  pitToTtc: wasm.pitToTtc,
  ttcToPit: wasm.ttcToPit,
  centralTendency: wasm.centralTendency,
};
