import * as wasm from '../../pkg/finstack_quant_wasm.js';

export const liquidity = {
  AlmgrenChrissModel: wasm.AlmgrenChrissModel,
  KyleLambdaModel: wasm.KyleLambdaModel,
  rollEffectiveSpread: wasm.rollEffectiveSpread,
  amihudIlliquidity: wasm.amihudIlliquidity,
  daysToLiquidate: wasm.daysToLiquidate,
  liquidityTier: wasm.liquidityTier,
  lvarBangia: wasm.lvarBangia,
  almgrenChrissImpact: wasm.almgrenChrissImpact,
  kyleLambda: wasm.kyleLambda,
};
