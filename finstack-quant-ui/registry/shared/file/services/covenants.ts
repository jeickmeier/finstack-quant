import type { covenants, statements_analytics } from "finstack-quant-wasm";
import type { CovenantsApi } from "../finstack-contract";
import type { ServiceContext } from "./context";
/** covenants and statements_analytics facade members this slice reads. */
export type CovenantsNative = {
  covenants: Pick<
    typeof covenants,
    | "covLite"
    | "lboStandard"
    | "realEstate"
    | "projectFinance"
    | "validateCovenantEngineJson"
    | "CovenantEngine"
  >;
  statements_analytics: Pick<typeof statements_analytics, "evaluateCovenants">;
};
/** CovenantsApi over the covenants facade namespace and its statement evaluation. */
export function createCovenantsService(
  native: CovenantsNative,
  context: ServiceContext,
): CovenantsApi {
  return {
    covenantTemplate(request) {
      return context.result(() => {
        const api = native.covenants;
        switch (request.kind) {
          case "covLite":
            return api.covLite(request.maxLeverage, request.maxSeniorLeverage);
          case "lboStandard":
            return api.lboStandard(
              request.initialLeverage,
              request.interestCoverage,
              request.fixedChargeCoverage,
              request.maxCapex,
            );
          case "realEstate":
            return api.realEstate(
              request.minDscr,
              request.minDebtYield,
              request.maxLtv,
            );
          case "projectFinance":
            return api.projectFinance(
              request.minDscr,
              request.distributionLockupDscr,
              request.minLiquidity,
              request.maxNetLeverage,
            );
        }
      });
    },
    validateCovenantEngine(json) {
      return context.result(() =>
        native.covenants.validateCovenantEngineJson(json),
      );
    },
    evaluateCovenants(request) {
      return context.result(() => {
        const engine = native.covenants.CovenantEngine.fromJson(
          request.engineJson,
        );
        try {
          return native.statements_analytics.evaluateCovenants(
            request.resultsJson,
            engine,
            request.modelJson,
          );
        } finally {
          engine.free();
        }
      });
    },
  };
}
