import * as wasm from '../../../pkg/finstack_quant_wasm.js';

export const scoring = {
  altmanZScore: wasm.altmanZScore,
  altmanZPrime: wasm.altmanZPrime,
  altmanZDoublePrime: wasm.altmanZDoublePrime,
  altmanEmScore: wasm.altmanEmScore,
  ohlsonOScore: wasm.ohlsonOScore,
  zmijewskiScore: wasm.zmijewskiScore,
};
