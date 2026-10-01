import * as wasm from '../pkg/finstack_quant_wasm.js';

// Rust `Portfolio::from_materialization` / `validate_materialization` borrow a
// cache; the published statics make it optional. wasm-bindgen 0.2.126 cannot
// express an optional borrowed handle (`Option<&T>`), and an owned
// `Option<InstrumentArtifactCache>` would consume the caller's reusable JS
// handle, so the omitted case lives here. Omitted, `undefined` or `null` means
// a per-call cache with the Rust default bounds (the Python `cache=None`
// twin). The statics are patched in place so the namespace keeps exporting
// the one `Portfolio` class that wasm-bindgen instantiates. The `= undefined`
// default keeps `Function.length` at the declared required arity (1).
const withCache =
  (call) =>
  (bundle, cache = undefined) => {
    if (cache != null) {
      return call(bundle, cache);
    }
    const ephemeral = new wasm.InstrumentArtifactCache();
    try {
      return call(bundle, ephemeral);
    } finally {
      ephemeral.free();
    }
  };
wasm.Portfolio.fromMaterialization = withCache(
  wasm.Portfolio.fromMaterialization.bind(wasm.Portfolio)
);
wasm.Portfolio.validateMaterialization = withCache(
  wasm.Portfolio.validateMaterialization.bind(wasm.Portfolio)
);

export const portfolio = {
  InstrumentArtifactCache: wasm.InstrumentArtifactCache,
  Portfolio: wasm.Portfolio,
  parsePortfolioSpecJson: wasm.parsePortfolioSpecJson,
  brinsonFachler: wasm.brinsonFachler,
  carinoLink: wasm.carinoLink,
  carinoLinkFromSectorPeriods: wasm.carinoLinkFromSectorPeriods,
  campisiAttribution: wasm.campisiAttribution,
  // ⚠️ campisiCarinoLink links precomputed results and carries no shared
  // period_years, so it is the correct entry point for unequal-length
  // periods (e.g. act/365 months). campisiCarinoLinkFromSnapshots applies one
  // config to every period and is only correct for equal-length periods.
  campisiCarinoLink: wasm.campisiCarinoLink,
  campisiCarinoLinkFromSnapshots: wasm.campisiCarinoLinkFromSnapshots,
  campisiReconciliationCheck: wasm.campisiReconciliationCheck,
  cellReturnsFromReference: wasm.cellReturnsFromReference,
  cellReturnsFromCurves: wasm.cellReturnsFromCurves,
  excessReturns: wasm.excessReturns,
  gridAttribution: wasm.gridAttribution,
  gridCarinoLink: wasm.gridCarinoLink,
  factorBrinsonAttribution: wasm.factorBrinsonAttribution,
  twrrModifiedDietz: wasm.twrrModifiedDietz,
  twrrLinked: wasm.twrrLinked,
  mwrXirr: wasm.mwrXirr,
  buildPortfolioFromSpecJson: wasm.buildPortfolioFromSpecJson,
  aggregateMetrics: wasm.aggregateMetrics,
  portfolioMetricsSeries: wasm.portfolioMetricsSeries,
  valuePortfolio: wasm.valuePortfolio,
  valuePortfolioBuilt: wasm.valuePortfolioBuilt,
  aggregateFullCashflows: wasm.aggregateFullCashflows,
  aggregateFullCashflowsBuilt: wasm.aggregateFullCashflowsBuilt,
  netInCurrencyByDate: wasm.netInCurrencyByDate,
  collapseToBaseByDateKind: wasm.collapseToBaseByDateKind,
  applyScenarioAndRevalue: wasm.applyScenarioAndRevalue,
  applyScenarioAndRevalueBuilt: wasm.applyScenarioAndRevalueBuilt,
  scenarioPnl: wasm.scenarioPnl,
  scenarioPnlBuilt: wasm.scenarioPnlBuilt,
  optimizePortfolio: wasm.optimizePortfolio,
  rebalanceFromSpec: wasm.rebalanceFromSpec,
  replayPortfolio: wasm.replayPortfolio,
  // ⚠️ BLOCKING: prefer computeFactorSensitivitiesWithMarket for repeated calls
  // so large MarketContext JSON is parsed once into a core.MarketContext handle.
  computeFactorSensitivities: wasm.computeFactorSensitivities,
  computeFactorSensitivitiesWithMarket: wasm.computeFactorSensitivitiesWithMarket,
  computePnlProfiles: wasm.computePnlProfiles,
  computePnlProfilesWithMarket: wasm.computePnlProfilesWithMarket,
  // Takes the computeFactorSensitivities wire object; malformed dimensions
  // throw a validation Error.
  decomposeFactorRisk: wasm.decomposeFactorRisk,
  PortfolioBuilder: wasm.PortfolioBuilder,
  allocateWeights: wasm.allocateWeights,
  allocateWeightsJson: wasm.allocateWeightsJson,
  validateAllocationJson: wasm.validateAllocationJson,
  factorStress: wasm.factorStress,
  positionWhatIf: wasm.positionWhatIf,
  buildCreditVolReport: wasm.buildCreditVolReport,
  scenarioPnlBatch: wasm.scenarioPnlBatch,
  attributePortfolioPnl: wasm.attributePortfolioPnl,
  // Rust methods on result types; WASM results are plain objects, so each
  // takes the object (or its JSON) as the first argument.
  portfolioAttributionExplainText: wasm.portfolioAttributionExplainText,
  portfolioAttributionReconciliationCheck: wasm.portfolioAttributionReconciliationCheck,
  portfolioValuationGetPositionValue: wasm.portfolioValuationGetPositionValue,
  portfolioValuationGetEntityValue: wasm.portfolioValuationGetEntityValue,
  portfolioMetricsGetMetric: wasm.portfolioMetricsGetMetric,
  portfolioMetricsGetPositionMetrics: wasm.portfolioMetricsGetPositionMetrics,
  portfolioMetricsGetTotal: wasm.portfolioMetricsGetTotal,
  portfolioOptimizationResultNewPositionTrades: wasm.portfolioOptimizationResultNewPositionTrades,
  portfolioOptimizationResultBindingConstraints: wasm.portfolioOptimizationResultBindingConstraints,
};
