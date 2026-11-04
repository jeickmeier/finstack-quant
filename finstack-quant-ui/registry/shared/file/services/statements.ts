import type { statements, statements_analytics } from "finstack-quant-wasm";
import type { StatementsApi } from "../finstack-contract";
import type { ServiceContext } from "./context";
/** statements and statements_analytics facade members this slice reads. */
export type StatementsNative = {
  statements: Pick<
    typeof statements,
    | "validateFinancialModelJson"
    | "validateCheckSuiteSpecJson"
    | "parseAndCompile"
    | "parseFormula"
    | "Evaluator"
    | "statementResultRollup"
    | "rollupSpecDeclaredMethods"
    | "monteCarloBreachProbability"
    | "statementResultToTableWide"
    | "statementResultToTableLong"
    | "validateNormalizationConfigJson"
    | "normalize"
  >;
  statements_analytics: Pick<
    typeof statements_analytics,
    | "explainFormula"
    | "DependencyTracer"
    | "runChecks"
    | "runThreeStatementChecks"
    | "runCreditUnderwritingChecks"
    | "evaluateScenarioSet"
    | "evaluateScenarioSetWithMarket"
    | "scenarioDiff"
    | "scenarioSetTrace"
    | "creditAssessment"
    | "plSummaryReport"
    | "evaluateDcf"
    | "dcfSensitivity"
    | "evaluateLbo"
    | "capitalStructureSummary"
    | "relativeValueReport"
    | "parameterSpecWithPercentages"
    | "runSensitivity"
    | "generateTornadoEntries"
    | "goalSeek"
    | "runVariance"
    | "varianceBridge"
    | "backtestForecast"
    | "wacc"
  >;
};
/** StatementsApi over the statements and statements_analytics facade namespaces. */
export function createStatementsService(
  native: StatementsNative,
  context: ServiceContext,
): StatementsApi {
  return {
    validateStatementModel(json) {
      return context.result(() =>
        native.statements.validateFinancialModelJson(json),
      );
    },
    validateStatementFormula(formula) {
      return context.result(() => {
        native.statements.parseAndCompile(formula);
        return native.statements.parseFormula(formula);
      });
    },
    validateCheckSuiteSpec(json) {
      return context.result(() =>
        native.statements.validateCheckSuiteSpecJson(json),
      );
    },
    evaluateStatement(request) {
      return context.result(() => {
        const evaluator = new native.statements.Evaluator();
        try {
          if (request.marketJson !== undefined || request.asOf !== undefined) {
            if (request.marketJson === undefined || request.asOf === undefined)
              throw new TypeError(
                "Statement market JSON and as-of date must be supplied together",
              );
            return evaluator.evaluateWithMarket(
              request.modelJson,
              request.marketJson,
              request.asOf,
            );
          }
          return evaluator.evaluate(request.modelJson);
        } finally {
          evaluator.free();
        }
      });
    },
    explainStatement(request) {
      return context.result(() =>
        native.statements_analytics.explainFormula(
          request.modelJson,
          request.resultsJson,
          request.nodeId,
          request.period,
        ),
      );
    },
    traceStatement(request) {
      return context.result(() => {
        const tracer = new native.statements_analytics.DependencyTracer(
          request.modelJson,
        );
        try {
          return tracer.dependencyTree(request.nodeId);
        } finally {
          tracer.free();
        }
      });
    },
    runStatementChecks(request) {
      return context.result(() => {
        const { modelJson, resultsJson, configJson, kind } = request;
        switch (kind) {
          case "suite":
            return native.statements_analytics.runChecks(
              modelJson,
              native.statements.validateCheckSuiteSpecJson(configJson),
              resultsJson,
            );
          case "three-statement":
            return native.statements_analytics.runThreeStatementChecks(
              modelJson,
              configJson,
              resultsJson,
            );
          case "credit-underwriting":
            return native.statements_analytics.runCreditUnderwritingChecks(
              modelJson,
              configJson,
              resultsJson,
            );
          default: {
            const unhandled: never = kind;
            throw new RangeError(String(unhandled));
          }
        }
      });
    },
    evaluateStatementScenarios(request) {
      return context.result(() => {
        const { modelJson, scenarioSetJson, marketJson, asOf } = request;
        if (marketJson === undefined && asOf === undefined)
          return native.statements_analytics.evaluateScenarioSet(
            modelJson,
            scenarioSetJson,
          );
        if (marketJson === undefined || asOf === undefined)
          throw new TypeError(
            "Statement market JSON and as-of date must be supplied together",
          );
        return native.statements_analytics.evaluateScenarioSetWithMarket(
          modelJson,
          scenarioSetJson,
          marketJson,
          asOf,
        );
      });
    },
    diffStatementScenarios(request) {
      return context.result(() =>
        request.pairs.map((pair) =>
          native.statements_analytics.scenarioDiff(
            request.scenarioSetJson,
            request.resultsJson,
            pair.baseline,
            pair.comparison,
            [...request.metrics],
            [...request.periods],
          ),
        ),
      );
    },
    traceStatementScenario(request) {
      return context.result(() =>
        native.statements_analytics.scenarioSetTrace(
          request.scenarioSetJson,
          request.scenario,
        ),
      );
    },
    creditAssessment(request) {
      return context.result(() =>
        native.statements_analytics.creditAssessment(
          request.resultsJson,
          request.period,
        ),
      );
    },
    plSummaryReport(request) {
      return context.result(() =>
        native.statements_analytics.plSummaryReport(
          request.resultsJson,
          [...request.lineItems],
          [...request.periods],
        ),
      );
    },
    validateNormalizationConfig(json) {
      return context.result(() =>
        native.statements.validateNormalizationConfigJson(json),
      );
    },
    normalize(request) {
      return context.result(() =>
        native.statements.normalize(request.resultJson, request.configJson),
      );
    },
    statementResultToTableWide(resultJson) {
      return context.result(() =>
        native.statements.statementResultToTableWide(resultJson),
      );
    },
    statementResultToTableLong(resultJson) {
      return context.result(() =>
        native.statements.statementResultToTableLong(resultJson),
      );
    },
    wacc(request) {
      return context.result(() =>
        native.statements_analytics.wacc(
          request.equityWeight,
          request.costOfEquity,
          request.debtWeight,
          request.costOfDebt,
          request.taxRate,
        ),
      );
    },
    evaluateDcf(request) {
      return context.result(() =>
        native.statements_analytics.evaluateDcf(
          request.modelJson,
          request.wacc,
          request.terminalValue,
          request.ufcfNode,
          request.netDebtOverride,
          request.optionsJson,
          request.marketJson,
          request.asOf,
        ),
      );
    },
    dcfSensitivity(request) {
      return context.result(() =>
        native.statements_analytics.dcfSensitivity(
          request.modelJson,
          request.wacc,
          request.terminalValue,
          request.ufcfNode,
          request.netDebtOverride,
          request.optionsJson,
        ),
      );
    },
    evaluateLbo(request) {
      return context.result(() =>
        native.statements_analytics.evaluateLbo(
          request.modelJson,
          request.configJson,
        ),
      );
    },
    capitalStructureSummary(request) {
      return context.result(() =>
        native.statements_analytics.capitalStructureSummary(
          request.resultJson,
          request.modelJson,
          request.configJson,
        ),
      );
    },
    relativeValueReport(request) {
      return context.result(() =>
        native.statements_analytics.relativeValueReport(
          request.subjectJson,
          request.universeJson,
          request.configJson,
        ),
      );
    },
    runSensitivity(request) {
      return context.result(() => {
        const api = native.statements_analytics;
        const config = {
          mode: request.mode,
          parameters: request.parameters.map((parameter) =>
            api.parameterSpecWithPercentages(
              parameter.nodeId,
              parameter.periodId,
              parameter.baseValue,
              [...parameter.pctRange],
            ),
          ),
          target_metrics: [request.targetMetric],
        };
        const result = api.runSensitivity(
          request.modelJson,
          JSON.stringify(config),
        );
        return {
          result,
          tornado: api.generateTornadoEntries(
            result,
            request.targetMetric,
            request.period,
          ),
        };
      });
    },
    goalSeek(request) {
      return context.result(() =>
        native.statements_analytics.goalSeek(
          request.modelJson,
          request.targetNode,
          request.targetPeriod,
          request.targetValue,
          request.driverNode,
          request.driverPeriod,
          request.updateModel,
          request.bounds === undefined
            ? undefined
            : [request.bounds[0], request.bounds[1]],
        ),
      );
    },
    runVariance(request) {
      return context.result(() =>
        native.statements_analytics.runVariance(
          request.baseJson,
          request.comparisonJson,
          request.configJson,
        ),
      );
    },
    varianceBridge(request) {
      return context.result(() =>
        native.statements_analytics.varianceBridge(
          request.baseJson,
          request.comparisonJson,
          request.targetMetric,
          request.period,
          [...request.drivers],
          request.baselineLabel,
          request.comparisonLabel,
        ),
      );
    },
    backtestForecast(request) {
      return context.result(() =>
        native.statements_analytics.backtestForecast(
          [...request.actual],
          [...request.forecast],
        ),
      );
    },
    evaluateMonteCarlo(request) {
      return context.result(() => {
        const evaluator = new native.statements.Evaluator();
        try {
          return evaluator.evaluateMonteCarlo(
            request.modelJson,
            request.configJson,
          );
        } finally {
          evaluator.free();
        }
      });
    },
    monteCarloBreachProbability(request) {
      // Rust's None (no path data for the metric) crosses as undefined; the envelope carries null.
      return context.result(
        () =>
          native.statements.monteCarloBreachProbability(
            request.resultsJson,
            request.metric,
            request.threshold,
          ) ?? null,
      );
    },
    rollupStatement(request) {
      return context.result(() =>
        native.statements.statementResultRollup(
          request.resultsJson,
          request.modelJson,
          request.specJson,
        ),
      );
    },
    defaultRollupSpec(request) {
      return context.result(() =>
        native.statements.rollupSpecDeclaredMethods(
          request.modelJson,
          request.frequency,
        ),
      );
    },
  };
}
