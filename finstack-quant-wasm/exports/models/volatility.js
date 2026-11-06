import * as wasm from '../../pkg/finstack_quant_wasm.js';

/**
 * Product-independent volatility models and evaluators.
 *
 * Core owns the serializable surface, cube, and FX-delta artifacts. This
 * namespace owns SABR behavior and all currently bound artifact evaluation.
 * @example
 * ```typescript
 * import init, { core, models } from "finstack-quant-wasm";
 * await init();
 * const cube = new core.VolCube(
 *   "USD-SWAPTION", [1], [5], [{ alpha: 0.03, beta: 0.5, rho: -0.2, nu: 0.4 }], [0.03]
 * );
 * console.log(models.volatility.getCubeVol(cube, 1, 5, 0.03, "lognormal"));
 * cube.free();
 * ```
 */
export const volatility = {
  SabrCalibrator: wasm.SabrCalibrator,
  SabrParameters: wasm.SabrParameters,
  SabrSmile: wasm.SabrSmile,
  calibrateSvi: wasm.calibrateSvi,
  convertAtmVolatility: wasm.convertAtmVolatility,
  deltaToStrike: wasm.deltaToStrike,
  getCubeVol: wasm.getCubeVol,
  getCubeVolClamped: wasm.getCubeVolClamped,
  getCubeExpirySliceVol: wasm.getCubeExpirySliceVol,
  getCubeExpirySliceVolClamped: wasm.getCubeExpirySliceVolClamped,
  getFxDeltaPillarVols: wasm.getFxDeltaPillarVols,
  getFxDeltaVol: wasm.getFxDeltaVol,
  impliedVolBachelier: wasm.impliedVolBachelier,
  impliedVolBlack: wasm.impliedVolBlack,
  localVolFromImpliedVol: wasm.localVolFromImpliedVol,
  localVolFromImpliedVolSmoothed: wasm.localVolFromImpliedVolSmoothed,
  localVolValue: wasm.localVolValue,
  strikeToDelta: wasm.strikeToDelta,
  sviImpliedVol: wasm.sviImpliedVol,
  sviTotalVariance: wasm.sviTotalVariance,
  sviDurrlemanG: wasm.sviDurrlemanG,
  checkButterflyGrid: wasm.checkButterflyGrid,
  checkCalendarSpreadGrid: wasm.checkCalendarSpreadGrid,
  checkLocalVolDensityGrid: wasm.checkLocalVolDensityGrid,
  checkSurface: wasm.checkSurface,
  checkSurfaceGrid: wasm.checkSurfaceGrid,
  getSurfaceVol: wasm.getSurfaceVol,
  getSurfaceVolClamped: wasm.getSurfaceVolClamped,
  materializeCubeTenorSlice: wasm.materializeCubeTenorSlice,
  materializeCubeExpirySlice: wasm.materializeCubeExpirySlice,
  materializeFxDeltaSurface: wasm.materializeFxDeltaSurface,
};
