import * as wasm from '../pkg/finstack_quant_wasm.js';

/**
 * Namespaced TypeScript entry points for statements analytics calculations and types.
 * @example
 * ```typescript
 * import init, { statements_analytics } from "finstack-quant-wasm";
 * await init();
 * console.log(
 *   statements_analytics.wacc({ equity_weight: 0.6, cost_of_equity: 0.1, debt_weight: 0.4, cost_of_debt: 0.05, tax_rate: 0.25 })
 * );
 * ```
 */
export const statements_analytics = {
  DependencyTracer: wasm.DependencyTracer,
  CorkscrewExtension: wasm.CorkscrewExtension,
  CreditScorecardExtension: wasm.CreditScorecardExtension,
  runSensitivity: wasm.runSensitivity,
  runVariance: wasm.runVariance,
  evaluateScenarioSet: wasm.evaluateScenarioSet,
  evaluateScenarioSetWithMarket: wasm.evaluateScenarioSetWithMarket,
  scenarioComparisonTable: wasm.scenarioComparisonTable,
  backtestForecast: wasm.backtestForecast,
  generateTornadoEntries: wasm.generateTornadoEntries,
  dcfSensitivity: wasm.dcfSensitivity,
  evaluateLbo: wasm.evaluateLbo,
  wacc: wasm.wacc,
  goalSeek: wasm.goalSeek,
  explainFormula: wasm.explainFormula,
  explainFormulaText: wasm.explainFormulaText,
  plSummaryReportText: wasm.plSummaryReportText,
  creditAssessmentReportText: wasm.creditAssessmentReportText,
  creditAssessment: wasm.creditAssessment,
  capitalStructureSummary: wasm.capitalStructureSummary,
  capitalStructureSummaryTable: wasm.capitalStructureSummaryTable,
  evaluateCovenants: wasm.evaluateCovenants,
  // Raw export is prefixed: `forecastBreaches` is also the covenants
  // namespace's forecast over dated metric rows, so the raw names must differ.
  forecastBreaches: wasm.statementsAnalyticsForecastBreaches,
  runChecks: wasm.runChecks,
  runThreeStatementChecks: wasm.runThreeStatementChecks,
  runCreditUnderwritingChecks: wasm.runCreditUnderwritingChecks,
  renderCheckReportText: wasm.renderCheckReportText,
  renderCheckReportHtml: wasm.renderCheckReportHtml,
  percentileRank: wasm.percentileRank,
  zScore: wasm.zScore,
  peerStats: wasm.peerStats,
  regressionFairValue: wasm.regressionFairValue,
  computeMultiple: wasm.computeMultiple,
  scoreRelativeValue: wasm.scoreRelativeValue,
  relativeValueReport: wasm.relativeValueReport,
  relativeValueReportTable: wasm.relativeValueReportTable,
  validateScorecardConfig: wasm.validateScorecardConfig,
  classifyStage: wasm.classifyStage,
  computeEcl: wasm.computeEcl,
  computeEclWeighted: wasm.computeEclWeighted,
  addVintageBuildup: wasm.addVintageBuildup,
  addRollForward: wasm.addRollForward,
  addRollForwardWithOpening: wasm.addRollForwardWithOpening,
  addNoiBuildup: wasm.addNoiBuildup,
  addNcfBuildup: wasm.addNcfBuildup,
  addRentRoll: wasm.addRentRoll,
  addPropertyOperatingStatement: wasm.addPropertyOperatingStatement,
  validateLeaseSpecJson: wasm.validateLeaseSpecJson,
  validateRenewalSpecJson: wasm.validateRenewalSpecJson,
  evaluateDcf: wasm.evaluateDcf,
  runCorporateAnalysis: wasm.runCorporateAnalysis,
  scenarioDiff: wasm.scenarioDiff,
  varianceBridge: wasm.varianceBridge,
  plSummaryReport: wasm.plSummaryReport,
  companyMetricsGet: wasm.companyMetricsGet,
  peerFilterAccepts: wasm.peerFilterAccepts,
  peerSetFromUniverse: wasm.peerSetFromUniverse,
  scenarioResultsGet: wasm.scenarioResultsGet,
  scenarioSetTrace: wasm.scenarioSetTrace,
  sensitivityResultGetParameterValue: wasm.sensitivityResultGetParameterValue,
  sensitivityResultGetValue: wasm.sensitivityResultGetValue,
  explanationToText: wasm.explanationToText,
  forecastMetricsSummaryText: wasm.forecastMetricsSummaryText,
};
