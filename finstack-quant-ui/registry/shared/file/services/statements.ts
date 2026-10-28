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
        const { modelJson, resultsJson, configJson } = request;
        switch (request.kind) {
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
    rollupStatement(request) {
      return context.result(() =>
        native.statements.statementResultRollup(
          request.resultsJson,
          request.modelJson,
          request.specJson,
        ),
      );
    },
  };
}
