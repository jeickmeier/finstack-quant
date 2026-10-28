import type { valuations } from "finstack-quant-wasm";
import { exportValuation } from "@/lib/finstack/host";
import { resolveModel, type ValuationsApi } from "../finstack-contract";
import type { ServiceContext } from "./context";
/** valuations facade members this slice reads. */
export type ValuationsNative = {
  valuations: Pick<typeof valuations, "instruments" | "valuationResultToJson">;
};
/** ValuationsApi over the valuations facade namespace and the shared market cache. */
export function createValuationsService(
  native: ValuationsNative,
  context: ServiceContext,
): ValuationsApi {
  const instruments = native.valuations.instruments;
  return {
    price(request) {
      return context.result(() => {
        const instrument = instruments.validateInstrumentJson(
          request.instrumentJson,
          request.metricPricingOverrides,
        );
        return instruments.priceInstrumentWithMarket(
          instrument,
          context.market(request.marketJson).handle,
          request.asOf,
          // Omitted models resolve centrally; pricing.rs price_instrument uses "default".
          resolveModel(request.model),
          request.metrics == null ? request.metrics : [...request.metrics],
          undefined,
          request.marketHistory,
        );
      });
    },
    validate(instrumentJson) {
      return context.result(() =>
        instruments.validateInstrumentJson(instrumentJson),
      );
    },
    cashflows(request) {
      return context.result(() => {
        const instrument = instruments.validateInstrumentJson(
          request.instrumentJson,
        );
        return JSON.stringify(
          instruments.instrumentCashflowsWithMarket(
            instrument,
            context.market(request.marketJson).handle,
            request.asOf,
            resolveModel(request.model),
          ),
        );
      });
    },
    scenarioTable(request) {
      return context.result(() => {
        const deal = instruments.StructuredCredit.fromJson(
          request.instrumentJson,
        );
        try {
          return instruments.trancheScenarioTable(
            deal,
            request.trancheId,
            request.marketJson,
            request.asOf,
            request.gridJson,
          );
        } finally {
          deal.free();
        }
      });
    },
    models: () => context.result(() => instruments.listModelsGrouped()),
    metrics: () =>
      context.result(() => instruments.listStandardMetricsGrouped()),
    metricMetadata: (keys) =>
      context.result(() => instruments.metricMetadata([...keys])),
    exportResult: (value) =>
      context.result(() =>
        exportValuation(value, native.valuations.valuationResultToJson),
      ),
  };
}
