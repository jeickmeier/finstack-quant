import * as wasm from '../pkg/finstack_wasm.js';

const credit = {
  CreditFactorModel: wasm.CreditFactorModel,
  CreditCalibrator: wasm.CreditCalibrator,
  LevelsAtDate: wasm.LevelsAtDate,
  PeriodDecomposition: wasm.PeriodDecomposition,
  FactorCovarianceForecast: wasm.FactorCovarianceForecast,
  decomposeLevels: wasm.decomposeLevels,
  decomposePeriod: wasm.decomposePeriod,
};

export const factor_model = {
  credit,
  // Root aliases mirror finstack_factor_model crate-root re-exports.
  CreditFactorModel: credit.CreditFactorModel,
  CreditCalibrator: credit.CreditCalibrator,
  LevelsAtDate: credit.LevelsAtDate,
  PeriodDecomposition: credit.PeriodDecomposition,
  FactorCovarianceForecast: credit.FactorCovarianceForecast,
  decomposeLevels: credit.decomposeLevels,
  decomposePeriod: credit.decomposePeriod,
};
