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
};

const risk = {
  positionRiskReport: wasm.positionRiskReport,
  evaluateRiskBudget: wasm.evaluateRiskBudget,
  evaluateRiskBudgetFromDecomposition: wasm.evaluateRiskBudgetFromDecomposition,
  buildStressAttribution: wasm.buildStressAttribution,
  positionComponentVar: wasm.positionComponentVar,
  decomposeFactors: wasm.decomposeFactors,
  defaultUtilizationThreshold: wasm.defaultUtilizationThreshold,
};

export const factor = {
  factorVariance: wasm.factorVariance,
  factorCovariance: wasm.factorCovariance,
  factorCorrelation: wasm.factorCorrelation,
  factorCovarianceRows: wasm.factorCovarianceRows,
  validateFactorModelConfig: wasm.validateFactorModelConfig,
  credit,
  risk,
  schema,
};
