import * as wasm from '../pkg/finstack_quant_wasm.js';

export const statements_analytics = {
  runSensitivity: wasm.runSensitivity,
  runVariance: wasm.runVariance,
  evaluateScenarioSet: wasm.evaluateScenarioSet,
  backtestForecast: wasm.backtestForecast,
  generateTornadoEntries: wasm.generateTornadoEntries,
  runMonteCarlo: wasm.runMonteCarlo,
  goalSeek: wasm.goalSeek,
  traceDependencies: wasm.traceDependencies,
  explainFormula: wasm.explainFormula,
  explainFormulaText: wasm.explainFormulaText,
  plSummaryReport: wasm.plSummaryReport,
  creditAssessmentReport: wasm.creditAssessmentReport,
  runChecks: wasm.runChecks,
  runThreeStatementChecks: wasm.runThreeStatementChecks,
  runCreditUnderwritingChecks: wasm.runCreditUnderwritingChecks,
  renderCheckReportText: wasm.renderCheckReportText,
  renderCheckReportHtml: wasm.renderCheckReportHtml,
  // Comps — comparable company analysis
  percentileRank: wasm.percentileRank,
  zScore: wasm.zScore,
  peerStats: wasm.peerStats,
  regressionFairValue: wasm.regressionFairValue,
  computeMultiple: wasm.computeMultiple,
  scoreRelativeValue: wasm.scoreRelativeValue,
};
