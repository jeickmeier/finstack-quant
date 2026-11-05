import * as wasm from '../../pkg/finstack_quant_wasm.js';
import { schema } from './factor/schema.js';

/**
 * Namespaced TypeScript entry points for factor model credit calculations and types.
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const config = JSON.stringify({
 *   policy: "globally_off",
 *   hierarchy: { levels: [] },
 *   min_bucket_size_per_level: { per_level: [] },
 *   vol_model: "sample",
 *   covariance_strategy: "diagonal",
 *   beta_shrinkage: "none",
 *   use_returns_or_levels: "returns",
 *   panel_frequency: "monthly",
 *   bucket_weighting: "equal"
 * });
 * const calibrator = new models.factor.credit.CreditCalibrator(config);
 * calibrator.free();
 * ```
 */
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

/**
 * Product-independent factor and position risk decomposition kernels.
 *
 * The position risk report and risk-budget functions return the canonical Rust
 * result types (`PositionRiskReport`, `RiskBudgetResult`), the same objects the
 * Python functions return.
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const report = models.factor.risk.positionRiskReport({
 *   position_ids: ["A", "B"],
 *   source: { kind: "parametric", weights: [0.6, 0.4], covariance: [[0.04, 0.01], [0.01, 0.09]] },
 * });
 * console.log(report.decomposition.portfolio_var, report.stress); // ..., null
 * ```
 */
const risk = {
  positionRiskReport: wasm.positionRiskReport,
  evaluateRiskBudget: wasm.evaluateRiskBudget,
  evaluateRiskBudgetFromDecomposition: wasm.evaluateRiskBudgetFromDecomposition,
  buildStressAttribution: wasm.buildStressAttribution,
  positionComponentVar: wasm.positionComponentVar,
  decomposeFactors: wasm.decomposeFactors,
  defaultUtilizationThreshold: wasm.defaultUtilizationThreshold,
};

/**
 * Namespaced TypeScript entry points for factor model calculations and types.
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const credit = models.factor.credit;
 * const config = JSON.stringify({
 *   policy: "globally_off",
 *   hierarchy: { levels: [] },
 *   min_bucket_size_per_level: { per_level: [] },
 *   vol_model: "sample",
 *   covariance_strategy: "diagonal",
 *   beta_shrinkage: "none",
 *   use_returns_or_levels: "returns",
 *   panel_frequency: "monthly",
 *   bucket_weighting: "equal"
 * });
 * const calibrator = new credit.CreditCalibrator(config);
 * calibrator.free();
 * ```
 */
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
