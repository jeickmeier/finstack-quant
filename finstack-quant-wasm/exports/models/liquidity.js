import * as wasm from '../../pkg/finstack_quant_wasm.js';

/**
 * Product-independent liquidity risk and market-impact models.
 *
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * console.log(models.liquidity.daysToLiquidate(1_000_000, 250_000, 0.20));
 * ```
 */
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
  liquidityProfileSpread: wasm.liquidityProfileSpread,
  liquidityProfileRelativeSpread: wasm.liquidityProfileRelativeSpread,
  liquidityProfileHalfSpread: wasm.liquidityProfileHalfSpread,
  liquidityProfileRelativeSpreadVolatility: wasm.liquidityProfileRelativeSpreadVolatility,
  tradeParamsEffectiveReferencePrice: wasm.tradeParamsEffectiveReferencePrice,
};
