import * as wasm from '../pkg/finstack_quant_wasm.js';
import { correlation } from './models/correlation.js';
import { credit } from './models/credit.js';
import { factor } from './models/factor.js';
import { liquidity } from './models/liquidity.js';
import { monteCarlo } from './models/monteCarlo.js';
import { rates } from './models/rates.js';
import { volatility } from './models/volatility.js';

/**
 * Namespaced TypeScript entry points for reusable quantitative models.
 *
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * console.log(models.bsPrice(100, 100, 0.03, 0, 0.2, 1, true));
 * ```
 */
export const models = {
  correlation,
  credit,
  factor,
  liquidity,
  monteCarlo,
  rates,
  volatility,
  optionCalculation: wasm.optionCalculation,
  bsPrice: wasm.bsPrice,
  vanillaExpiryPayoff: wasm.vanillaExpiryPayoff,
  bsGreeks: wasm.bsGreeks,
  bsGreeksIsValid: wasm.bsGreeksIsValid,
  bsImpliedVol: wasm.bsImpliedVol,
  black76ImpliedVol: wasm.black76ImpliedVol,
  black76Price: wasm.black76Price,
  black76Greeks: wasm.black76Greeks,
  bachelierPrice: wasm.bachelierPrice,
  bachelierGreeks: wasm.bachelierGreeks,
  blackShiftedPrice: wasm.blackShiftedPrice,
  blackShiftedVega: wasm.blackShiftedVega,
  barrierOptionPrice: wasm.barrierOptionPrice,
  asianOptionPrice: wasm.asianOptionPrice,
  lookbackOptionPrice: wasm.lookbackOptionPrice,
  quantoOptionPrice: wasm.quantoOptionPrice,
  hestonPrice: wasm.hestonPrice,
  bsCosPrice: wasm.bsCosPrice,
  vgCosPrice: wasm.vgCosPrice,
  mertonJumpCosPrice: wasm.mertonJumpCosPrice,
};
