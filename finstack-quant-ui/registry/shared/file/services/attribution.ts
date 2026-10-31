import type { attribution } from "finstack-quant-wasm";
import type { AttributionApi } from "../finstack-contract";
import type { ServiceContext } from "./context";
/** attribution facade members this slice reads. */
export type AttributionNative = {
  attribution: Pick<
    typeof attribution,
    | "attributePnlMany"
    | "defaultWaterfallOrder"
    | "pnlAttributionExplainText"
    | "pnlAttributionPctOfTotalByBucket"
    | "pnlAttributionResidualWithinTolerance"
  >;
};
/** AttributionApi over the attribution facade namespace and the shared market cache. */
export function createAttributionService(
  native: AttributionNative,
  context: ServiceContext,
): AttributionApi {
  const api = native.attribution;
  return {
    attributePnlMany(request) {
      return context.result(() => {
        // Canonical market texts are spliced into AttributionInputs, never parsed;
        // every option beyond the method stays unset so Rust applies its defaults.
        const inputs = `{"as_of_t0":${JSON.stringify(request.asOfT0)},"as_of_t1":${JSON.stringify(request.asOfT1)},"market_t0":${context.market(request.marketT0Json).json},"market_t1":${context.market(request.marketT1Json).json},"method":${JSON.stringify(request.method)}}`;
        // finstack-compose: Rust's own text, check and shares per returned attribution
        return api
          .attributePnlMany(inputs, [...request.instrumentJsons])
          .map((attribution) => ({
            attribution,
            explain: api.pnlAttributionExplainText(attribution),
            residualWithinTolerance:
              api.pnlAttributionResidualWithinTolerance(attribution),
            buckets: api.pnlAttributionPctOfTotalByBucket(attribution),
          }));
      });
    },
    defaultWaterfallOrder() {
      return context.result(() => api.defaultWaterfallOrder());
    },
  };
}
