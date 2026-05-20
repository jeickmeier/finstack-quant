import * as wasm from '../pkg/finstack_wasm.js';
import { correlation } from './valuations/correlation.js';
import { fx } from './valuations/fx.js';

export const valuations = {
  correlation,
  credit: {
    mertonModelJson: wasm.mertonModelJson,
    creditGradesModelJson: wasm.creditGradesModelJson,
    mertonDefaultProbability: wasm.mertonDefaultProbability,
    mertonDistanceToDefault: wasm.mertonDistanceToDefault,
    mertonImpliedSpread: wasm.mertonImpliedSpread,
    dynamicRecoveryAtNotional: wasm.dynamicRecoveryAtNotional,
    endogenousHazardAtLeverage: wasm.endogenousHazardAtLeverage,
    endogenousHazardAfterPikAccrual: wasm.endogenousHazardAfterPikAccrual,
    dynamicRecoveryConstantJson: wasm.dynamicRecoveryConstantJson,
    endogenousHazardPowerLawJson: wasm.endogenousHazardPowerLawJson,
    creditStateJson: wasm.creditStateJson,
    toggleExerciseThresholdJson: wasm.toggleExerciseThresholdJson,
    toggleExerciseOptimalJson: wasm.toggleExerciseOptimalJson,
  },
  creditDerivatives: {
    creditDefaultSwapExampleJson: wasm.creditDefaultSwapExampleJson,
    cdsIndexExampleJson: wasm.cdsIndexExampleJson,
    cdsTrancheExampleJson: wasm.cdsTrancheExampleJson,
    cdsOptionExampleJson: wasm.cdsOptionExampleJson,
    validate: wasm.validateInstrumentJson,
    priceInstrument: wasm.priceInstrument,
    priceInstrumentWithMetrics: wasm.priceInstrumentWithMetrics,
    priceInstrumentWithMarket: wasm.priceInstrumentWithMarket,
    priceInstrumentWithMetricsAndMarket: wasm.priceInstrumentWithMetricsAndMarket,
    instrumentCashflowsWithMarket: wasm.instrumentCashflowsWithMarket,
  },
  fx,
  instruments: {
    validateInstrumentJson: wasm.validateInstrumentJson,
    priceInstrument: wasm.priceInstrument,
    priceInstrumentWithMetrics: wasm.priceInstrumentWithMetrics,
    priceInstrumentWithMarket: wasm.priceInstrumentWithMarket,
    priceInstrumentWithMetricsAndMarket: wasm.priceInstrumentWithMetricsAndMarket,
    instrumentCashflowsWithMarket: wasm.instrumentCashflowsWithMarket,
    listStandardMetrics: wasm.listStandardMetrics,
    listStandardMetricsGrouped: wasm.listStandardMetricsGrouped,
  },
  // Credit factor hierarchy
  CreditFactorModel: wasm.CreditFactorModel,
  CreditCalibrator: wasm.CreditCalibrator,
  LevelsAtDate: wasm.LevelsAtDate,
  PeriodDecomposition: wasm.PeriodDecomposition,
  FactorCovarianceForecast: wasm.FactorCovarianceForecast,
  decomposeLevels: wasm.decomposeLevels,
  decomposePeriod: wasm.decomposePeriod,
  validateValuationResultJson: wasm.validateValuationResultJson,
  // Calibration: build a MarketContext from raw quotes
  calibrate(envelope) {
    const json = typeof envelope === 'string' ? envelope : JSON.stringify(envelope);
    return JSON.parse(wasm.calibrate(json));
  },
  validateCalibrationJson(envelope) {
    const json = typeof envelope === 'string' ? envelope : JSON.stringify(envelope);
    return wasm.validateCalibrationJson(json);
  },
  dryRun(envelope) {
    const json = typeof envelope === 'string' ? envelope : JSON.stringify(envelope);
    return wasm.dryRun(json);
  },
  dependencyGraphJson(envelope) {
    const json = typeof envelope === 'string' ? envelope : JSON.stringify(envelope);
    return wasm.dependencyGraphJson(json);
  },
  validateInstrumentJson: wasm.validateInstrumentJson,
  WasmMarket: wasm.WasmMarket,
  priceInstrument: wasm.priceInstrument,
  priceInstrumentWithMetrics: wasm.priceInstrumentWithMetrics,
  priceInstrumentWithMarket: wasm.priceInstrumentWithMarket,
  priceInstrumentWithMetricsAndMarket: wasm.priceInstrumentWithMetricsAndMarket,
  instrumentCashflowsJson: wasm.instrumentCashflowsJson,
  instrumentCashflowsWithMarket: wasm.instrumentCashflowsWithMarket,
  listStandardMetrics: wasm.listStandardMetrics,
  listStandardMetricsGrouped: wasm.listStandardMetricsGrouped,
  bsPrice: wasm.bsPrice,
  bsGreeks: wasm.bsGreeks,
  bsImpliedVol: wasm.bsImpliedVol,
  black76ImpliedVol: wasm.black76ImpliedVol,
  barrierCall: wasm.barrierCall,
  asianOptionPrice: wasm.asianOptionPrice,
  lookbackOptionPrice: wasm.lookbackOptionPrice,
  quantoOptionPrice: wasm.quantoOptionPrice,
  SabrParameters: wasm.SabrParameters,
  SabrModel: wasm.SabrModel,
  SabrSmile: wasm.SabrSmile,
  SabrCalibrator: wasm.SabrCalibrator,
  bsCosPrice: wasm.bsCosPrice,
  vgCosPrice: wasm.vgCosPrice,
  mertonJumpCosPrice: wasm.mertonJumpCosPrice,
  tarnCouponProfile: wasm.tarnCouponProfile,
  snowballCouponProfile: wasm.snowballCouponProfile,
  cmsSpreadOptionIntrinsic: wasm.cmsSpreadOptionIntrinsic,
  callableRangeAccrualAccrued: wasm.callableRangeAccrualAccrued,
  attributePnl: wasm.attributePnl,
  attributePnlFromSpec: wasm.attributePnlFromSpec,
  validateAttributionJson: wasm.validateAttributionJson,
  defaultWaterfallOrder: wasm.defaultWaterfallOrder,
  defaultAttributionMetrics: wasm.defaultAttributionMetrics,
  computeFactorSensitivities: wasm.computeFactorSensitivities,
  computePnlProfiles: wasm.computePnlProfiles,
  decomposeFactorRisk: wasm.decomposeFactorRisk,
};
