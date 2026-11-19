import * as wasm from '../pkg/finstack_quant_wasm.js';
import { schema } from './statements/schema.js';

/**
 * Namespaced TypeScript entry points for statements calculations and types.
 * @example
 * ```typescript
 * import init, { statements } from "finstack-quant-wasm";
 * await init();
 * console.log(statements.parseFormula("revenue-expenses")); // "revenue - expenses"
 * ```
 */
export const statements = {
  Evaluator: wasm.Evaluator,
  Registry: wasm.Registry,
  ModelBuilder: wasm.ModelBuilder,
  MixedNodeBuilder: wasm.MixedNodeBuilder,
  validateFinancialModelJson: wasm.validateFinancialModelJson,
  modelNodeIds: wasm.modelNodeIds,
  applyModelEdits: wasm.applyModelEdits,
  validateCheckSuiteSpecJson: wasm.validateCheckSuiteSpecJson,
  validateCapitalStructureSpecJson: wasm.validateCapitalStructureSpecJson,
  validateWaterfallSpecJson: wasm.validateWaterfallSpecJson,
  validateEcfSweepSpecJson: wasm.validateEcfSweepSpecJson,
  validatePikToggleSpecJson: wasm.validatePikToggleSpecJson,
  nodeToDatedSchedule: wasm.nodeToDatedSchedule,
  nodeToPricingFlows: wasm.nodeToPricingFlows,
  monteCarloBreachProbability: wasm.monteCarloBreachProbability,
  monteCarloPercentileByPeriod: wasm.monteCarloPercentileByPeriod,
  statementResultToTableLong: wasm.statementResultToTableLong,
  statementResultToTableWide: wasm.statementResultToTableWide,
  financialModelContentHash: wasm.financialModelContentHash,
  parseFormula: wasm.parseFormula,
  parseAndCompile: wasm.parseAndCompile,
  statementResultGet: wasm.statementResultGet,
  statementResultGetMoney: wasm.statementResultGetMoney,
  statementResultGetCell: wasm.statementResultGetCell,
  statementResultGetScalar: wasm.statementResultGetScalar,
  statementResultGetOr: wasm.statementResultGetOr,
  statementResultAllPeriods: wasm.statementResultAllPeriods,
  statementResultRollup: wasm.statementResultRollup,
  rollupSpecDeclaredMethods: wasm.rollupSpecDeclaredMethods,
  statementResultNumericSeries: wasm.statementResultNumericSeries,
  statementResultNodeIds: wasm.statementResultNodeIds,
  financialModelHasNode: wasm.financialModelHasNode,
  financialModelGetNode: wasm.financialModelGetNode,
  checkReportHasErrors: wasm.checkReportHasErrors,
  checkReportHasWarnings: wasm.checkReportHasWarnings,
  checkReportFindingsBySeverity: wasm.checkReportFindingsBySeverity,
  capitalStructureCashflowsGetComponent: wasm.capitalStructureCashflowsGetComponent,
  capitalStructureCashflowsGetMoney: wasm.capitalStructureCashflowsGetMoney,
  normalizationConfigAddAdjustment: wasm.normalizationConfigAddAdjustment,
  validateNormalizationConfigJson: wasm.validateNormalizationConfigJson,
  normalize: wasm.normalize,
  mergeIntoResults: wasm.mergeIntoResults,
  checkSuiteSpecBuiltinCheckNames: wasm.checkSuiteSpecBuiltinCheckNames,
  checkConfigValidate: wasm.checkConfigValidate,
  schema,
};
