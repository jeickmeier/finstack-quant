import type { covenants, statements_analytics } from "finstack-quant-wasm";
import type { CovenantsApi } from "../finstack-contract";
import type { ServiceContext } from "./context";
/** covenants and statements_analytics facade members this slice reads. */
export type CovenantsNative = {
  covenants: Pick<
    typeof covenants,
    "covenantTemplate" | "validateCovenantEngineJson" | "CovenantEngine"
  >;
  statements_analytics: Pick<
    typeof statements_analytics,
    "evaluateCovenants" | "forecastBreaches"
  >;
};
/** CovenantsApi over the covenants facade namespace and its statement evaluation. */
export function createCovenantsService(
  native: CovenantsNative,
  context: ServiceContext,
): CovenantsApi {
  return {
    covenantTemplate(request) {
      return context.result(() => native.covenants.covenantTemplate(request));
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
          );
        } finally {
          engine.free();
        }
      });
    },
    forecastBreaches(request) {
      return context.result(() => {
        const engine = native.covenants.CovenantEngine.fromJson(
          request.engineJson,
        );
        try {
          // The config crosses as JSON text, which keeps a uint64 random_seed exact.
          return native.statements_analytics.forecastBreaches(
            request.resultsJson,
            engine,
            request.configJson,
          );
        } finally {
          engine.free();
        }
      });
    },
  };
}
