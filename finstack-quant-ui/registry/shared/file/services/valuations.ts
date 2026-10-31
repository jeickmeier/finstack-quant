import type { valuations } from "finstack-quant-wasm";
import { exportValuation } from "@/lib/finstack/host";
import { resolveModel, type ValuationsApi } from "../finstack-contract";
import { attempt, type ServiceContext } from "./context";
/** valuations facade members this slice reads. */
export type ValuationsNative = {
  valuations: Pick<typeof valuations, "valuationResultToJson"> & {
    instruments: Pick<
      typeof valuations.instruments,
      | "validateInstrumentJson"
      | "priceInstrumentWithMarket"
      | "instrumentCashflowsWithMarket"
      | "trancheScenarioTable"
      | "trancheMetrics"
      | "trancheOas"
      | "trancheDiscountMargin"
      | "trancheBreakevenCdr"
      | "StructuredCredit"
      | "listModelsGrouped"
      | "metricsForInstrument"
      | "metricMetadata"
      | "priceDriverFields"
      | "trancheMetricFieldUnits"
      | "trancheSolveFieldUnits"
    >;
  };
};
/** ValuationsApi over the valuations facade namespace and the shared market cache. */
export function createValuationsService(
  native: ValuationsNative,
  context: ServiceContext,
): ValuationsApi {
  const instruments = native.valuations.instruments;
  type Deal = ReturnType<typeof instruments.StructuredCredit.fromJson>;
  /** Load the deal once per call and free its handle whatever the native call does. */
  function withDeal<T>(instrumentJson: string, call: (deal: Deal) => T): T {
    const deal = instruments.StructuredCredit.fromJson(instrumentJson);
    try {
      return call(deal);
    } finally {
      deal.free();
    }
  }
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
    metricValues(request) {
      return context.result(() => {
        const instrument = instruments.validateInstrumentJson(
          request.instrumentJson,
          request.metricPricingOverrides,
        );
        const market = context.market(request.marketJson).handle;
        return request.metrics.map((metric) => ({
          metric,
          value: attempt(
            () =>
              instruments.priceInstrumentWithMarket(
                instrument,
                market,
                request.asOf,
                resolveModel(request.model),
                [metric],
                undefined,
                request.marketHistory,
              ).measures[metric],
          ),
        }));
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
        return instruments.instrumentCashflowsWithMarket(
          instrument,
          context.market(request.marketJson).handle,
          request.asOf,
          resolveModel(request.model),
        );
      });
    },
    newStandardDeal(request) {
      return context.result(() => {
        const deal = instruments.StructuredCredit.newStandard(
          request.dealType,
          request.paramsJson,
        );
        try {
          return deal.toJson();
        } finally {
          deal.free();
        }
      });
    },
    dealTranches(instrumentJson) {
      return context.result(() =>
        withDeal(instrumentJson, (deal) => deal.tranches),
      );
    },
    scenarioTable(request) {
      return context.result(() =>
        withDeal(request.instrumentJson, (deal) =>
          instruments.trancheScenarioTable(
            deal,
            request.trancheId,
            request.marketJson,
            request.asOf,
            request.gridJson,
          ),
        ),
      );
    },
    trancheMetrics(request) {
      return context.result(() =>
        withDeal(request.instrumentJson, (deal) =>
          instruments.trancheMetrics(
            deal,
            request.trancheId,
            request.marketJson,
            request.asOf,
            request.marketPricePct,
            request.metrics == null ? request.metrics : [...request.metrics],
          ),
        ),
      );
    },
    trancheOas(request) {
      return context.result(() =>
        withDeal(request.instrumentJson, (deal) =>
          instruments.trancheOas(
            deal,
            request.trancheId,
            request.marketJson,
            request.asOf,
            request.marketPricePct,
            request.configJson,
          ),
        ),
      );
    },
    trancheDiscountMargin(request) {
      return context.result(() =>
        withDeal(request.instrumentJson, (deal) =>
          instruments.trancheDiscountMargin(
            deal,
            request.trancheId,
            request.marketJson,
            request.asOf,
            request.marketPricePct,
          ),
        ),
      );
    },
    trancheBreakevenCdr(request) {
      return context.result(() =>
        withDeal(request.instrumentJson, (deal) =>
          instruments.trancheBreakevenCdr(
            deal,
            request.trancheId,
            request.marketJson,
            request.asOf,
          ),
        ),
      );
    },
    models: () => context.result(() => instruments.listModelsGrouped()),
    metrics: (instrumentType) =>
      context.result(() => instruments.metricsForInstrument(instrumentType)),
    metricMetadata: (keys) =>
      context.result(() => instruments.metricMetadata([...keys])),
    priceDriverFields: () =>
      context.result(() => instruments.priceDriverFields()),
    trancheMetricFieldUnits: (currency) =>
      context.result(() => instruments.trancheMetricFieldUnits(currency)),
    trancheSolveFieldUnits: () =>
      context.result(() => instruments.trancheSolveFieldUnits()),
    exportResult: (value) =>
      context.result(() =>
        exportValuation(value, native.valuations.valuationResultToJson),
      ),
  };
}
