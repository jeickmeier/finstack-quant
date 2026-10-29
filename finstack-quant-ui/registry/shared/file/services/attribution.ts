import type { attribution, PnlAttribution } from "finstack-quant-wasm";
import type { AttributionApi } from "../finstack-contract";
import type { ServiceContext } from "./context";
/** attribution facade members this slice reads. */
export type AttributionNative = {
  attribution: Pick<
    typeof attribution,
    | "attributePnlMany"
    | "defaultWaterfallOrder"
    | "pnlAttributionExplainText"
    | "pnlAttributionPctOfTotal"
    | "pnlAttributionResidualWithinTolerance"
  >;
};
/** The PnlAttribution buckets whose share of total Rust reports, in the Rust struct's field order. */
const factorFields = [
  "carry",
  "rates_curves_pnl",
  "credit_curves_pnl",
  "inflation_curves_pnl",
  "correlations_pnl",
  "fx_pnl",
  "fx_translation_pnl",
  "vol_pnl",
  "cross_factor_pnl",
  "model_params_pnl",
  "market_scalars_pnl",
  "residual",
] as const satisfies readonly (keyof PnlAttribution)[];
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
        return api
          .attributePnlMany(inputs, [...request.instrumentJsons])
          .map((attribution) => ({
            attribution,
            explain: api.pnlAttributionExplainText(attribution),
            residualWithinTolerance:
              api.pnlAttributionResidualWithinTolerance(attribution),
            // Rust's share per bucket; the decimal amount crosses as the number its signature takes.
            pctOfTotal: Object.fromEntries(
              factorFields.map((field) => [
                field,
                api.pnlAttributionPctOfTotal(
                  attribution,
                  Number(attribution[field].amount),
                ) ?? null,
              ]),
            ),
          }));
      });
    },
    defaultWaterfallOrder() {
      return context.result(() => api.defaultWaterfallOrder());
    },
  };
}
