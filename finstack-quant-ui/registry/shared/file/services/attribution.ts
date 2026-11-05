import type { attribution } from "finstack-quant-wasm";
import { serializeHost } from "@/lib/finstack/codec.mjs";
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
        // Canonical market texts are spliced into the Rust AttributionInputs,
        // never parsed; every other field passes through unchanged.
        const fields = serializeHost(request.inputs).slice(1);
        const inputs = `{"market_t0":${context.market(request.marketT0Json).json},"market_t1":${context.market(request.marketT1Json).json}${fields === "}" ? "" : ","}${fields}`;
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
