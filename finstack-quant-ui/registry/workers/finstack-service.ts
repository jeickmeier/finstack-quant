import type { core, valuations, Market } from "finstack-quant-wasm";
import { serializeHost } from "@/lib/finstack/codec.mjs";
import { errorValue, type Envelope, type WorkerApi } from "./finstack-contract";
/** Dependencies are the published facade; injection permits the same service in a real Node worker. */
export function createService(native: {
  initialize: (wasmUrl?: string) => Promise<unknown>;
  core: Pick<typeof core, "availableCalendars">;
  valuations: Pick<
    typeof valuations,
    "Market" | "instruments" | "validateValuationResultJson"
  >;
}): WorkerApi {
  let ready: Promise<unknown> | undefined;
  let disposed = false;
  const markets = new Map<string, Market>();
  const initialize = (url?: string) =>
    (ready ??= Promise.resolve().then(() => native.initialize(url)));
  async function result<T>(
    operation: () => T | Promise<T>,
  ): Promise<Envelope<T>> {
    try {
      await initialize();
      if (disposed) throw new Error("Worker has been disposed");
      return { ok: true, value: await operation() };
    } catch (error) {
      return { ok: false, error: errorValue(error) };
    }
  }
  function market(json: string) {
    let handle = markets.get(json);
    if (handle) markets.delete(json);
    else handle = new native.valuations.Market(json);
    markets.set(json, handle);
    if (markets.size > 4) {
      const oldest = markets.entries().next().value!;
      markets.delete(oldest[0]);
      oldest[1].free();
    }
    return handle;
  }
  const instruments = native.valuations.instruments;
  return {
    initialize(url) {
      initialize(url);
      return result(() => ({
        state: "ready" as const,
        worker: typeof document === "undefined",
      }));
    },
    price(request) {
      return result(() =>
        instruments.priceInstrumentWithMarket(
          request.instrumentJson,
          market(request.marketJson),
          request.asOf,
          // pricing.rs price_instrument uses exactly this default for an absent model.
          request.model ?? "default",
          request.metrics == null ? request.metrics : [...request.metrics],
          request.pricingOptions,
          request.marketHistory,
        ),
      );
    },
    validate(request) {
      return result(() => ({
        revision: request.revision,
        json: instruments.validateInstrumentJson(request.instrumentJson),
      }));
    },
    validateMarket(json) {
      return result(() => {
        const handle = new native.valuations.Market(json);
        try {
          return handle.toJson();
        } finally {
          handle.free();
        }
      });
    },
    cashflows(request) {
      return result(() =>
        instruments.instrumentCashflowsWithMarketJson(
          request.instrumentJson,
          market(request.marketJson),
          request.asOf,
          request.model,
        ),
      );
    },
    models: () => result(() => instruments.listModelsGrouped()),
    metrics: () => result(() => instruments.listStandardMetricsGrouped()),
    calendars: () => result(() => native.core.availableCalendars()),
    exportResult: (value) =>
      result(() =>
        native.valuations.validateValuationResultJson(serializeHost(value)),
      ),
    async dispose() {
      disposed = true;
      for (const handle of markets.values()) handle.free();
      markets.clear();
      return { ok: true, value: undefined };
    },
  };
}
