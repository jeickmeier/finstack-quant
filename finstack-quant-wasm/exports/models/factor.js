import * as wasm from '../../pkg/finstack_quant_wasm.js';
import { schema } from './factor/schema.js';

const credit = {
  CreditFactorModel: wasm.CreditFactorModel,
  CreditCalibrator: wasm.CreditCalibrator,
  LevelsAtDate: wasm.LevelsAtDate,
  PeriodDecomposition: wasm.PeriodDecomposition,
  FactorCovarianceForecast: wasm.FactorCovarianceForecast,
  VolHorizon: wasm.VolHorizon,
  decomposeLevels: wasm.decomposeLevels,
  decomposePeriod: wasm.decomposePeriod,
  factorVariance: wasm.factorVariance,
  factorCovariance: wasm.factorCovariance,
  factorCorrelation: wasm.factorCorrelation,
  factorCovarianceRows: wasm.factorCovarianceRows,
  validateFactorModelConfig: wasm.validateFactorModelConfig,
};

const risk = {
  DecompositionConfig: wasm.DecompositionConfig,
  parametricVarDecomposition: wasm.parametricVarDecomposition,
  parametricEsDecomposition: wasm.parametricEsDecomposition,
  historicalVarDecomposition: wasm.historicalVarDecomposition,
  evaluateRiskBudget: wasm.evaluateRiskBudget,
  buildStressAttribution: wasm.buildStressAttribution,
  positionComponentVar: wasm.positionComponentVar,
  defaultUtilizationThreshold: wasm.defaultUtilizationThreshold,
};

export const factor = {
  credit,
  risk,
  schema,
};
