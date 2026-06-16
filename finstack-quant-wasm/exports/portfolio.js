import * as wasm from '../pkg/finstack_quant_wasm.js';

export const portfolio = {
  Portfolio: wasm.Portfolio,
  parsePortfolioSpec: wasm.parsePortfolioSpec,
  brinsonFachler: wasm.brinsonFachler,
  carinoLink: wasm.carinoLink,
  twrrModifiedDietz: wasm.twrrModifiedDietz,
  twrrLinked: wasm.twrrLinked,
  mwrXirr: wasm.mwrXirr,
  buildPortfolioFromSpec: wasm.buildPortfolioFromSpec,
  portfolioResultTotalValue: wasm.portfolioResultTotalValue,
  portfolioResultGetMetric: wasm.portfolioResultGetMetric,
  aggregateMetrics: wasm.aggregateMetrics,
  valuePortfolio: wasm.valuePortfolio,
  valuePortfolioBuilt: wasm.valuePortfolioBuilt,
  aggregateFullCashflows: wasm.aggregateFullCashflows,
  aggregateFullCashflowsBuilt: wasm.aggregateFullCashflowsBuilt,
  applyScenarioAndRevalue: wasm.applyScenarioAndRevalue,
  applyScenarioAndRevalueBuilt: wasm.applyScenarioAndRevalueBuilt,
  optimizePortfolio: wasm.optimizePortfolio,
  replayPortfolio: wasm.replayPortfolio,
  parametricVarDecomposition: wasm.parametricVarDecomposition,
  parametricEsDecomposition: wasm.parametricEsDecomposition,
  historicalVarDecomposition: wasm.historicalVarDecomposition,
  evaluateRiskBudget: wasm.evaluateRiskBudget,
  // ⚠️ BLOCKING: prefer computeFactorSensitivitiesWithMarket for repeated calls
  // so large MarketContext JSON is parsed once into Market.
  computeFactorSensitivities: wasm.computeFactorSensitivities,
  computeFactorSensitivitiesWithMarket: wasm.computeFactorSensitivitiesWithMarket,
  computePnlProfiles: wasm.computePnlProfiles,
  computePnlProfilesWithMarket: wasm.computePnlProfilesWithMarket,
  // ⚠️ BLOCKING: validate sensitivity/covariance dimensions before calling;
  // malformed matrices throw instead of returning partial decompositions.
  decomposeFactorRisk: wasm.decomposeFactorRisk,
  rollEffectiveSpread: wasm.rollEffectiveSpread,
  amihudIlliquidity: wasm.amihudIlliquidity,
  daysToLiquidate: wasm.daysToLiquidate,
  liquidityTier: wasm.liquidityTier,
  lvarBangia: wasm.lvarBangia,
  almgrenChrissImpact: wasm.almgrenChrissImpact,
  kyleLambda: wasm.kyleLambda,
};
