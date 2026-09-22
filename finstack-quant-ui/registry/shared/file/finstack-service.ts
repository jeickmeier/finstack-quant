import type {
  core,
  valuations,
  models,
  calibration,
  Market,
} from "finstack-quant-wasm";
import { exportValuation } from "@/lib/finstack/host";
import { serializeHost } from "@/lib/finstack/codec.mjs";
import { formatMoney } from "@/lib/finstack/format/format";
import {
  errorValue,
  resolveModel,
  type Envelope,
  type WorkerApi,
} from "./finstack-contract";
/** Dependencies are the published facade; injection permits the same service in a real Node worker. */
export function createService(native: {
  initialize: (wasmUrl?: string) => Promise<unknown>;
  core: Pick<
    typeof core,
    "availableCalendars" | "FxDeltaVolSurface" | "Money" | "VolCube"
  >;
  models: {
    volatility: Pick<
      typeof models.volatility,
      "getFxDeltaVol" | "getCubeVol" | "getCubeNormalVol"
    >;
  };
  calibration: Pick<
    typeof calibration,
    "calibrate" | "dryRun" | "validateCalibrationJson"
  >;
  valuations: Pick<
    typeof valuations,
    "Market" | "instruments" | "validateValuationResultJson"
  >;
}): WorkerApi {
  let ready: Promise<unknown> | undefined;
  const markets = new Map<string, { json: string; handle: Market }>();
  const initialize = (url?: string) =>
    (ready ??= Promise.resolve().then(() => native.initialize(url)));
  async function result<T>(
    operation: () => T | Promise<T>,
  ): Promise<Envelope<T>> {
    try {
      await initialize();
      return { ok: true, value: await operation() };
    } catch (error) {
      return { ok: false, error: errorValue(error) };
    }
  }
  function market(json: string) {
    let entry = markets.get(json);
    if (!entry) {
      const handle = new native.valuations.Market(json);
      let canonical: string;
      try {
        canonical = handle.toJson();
      } catch (error) {
        handle.free();
        throw error;
      }
      entry = markets.get(canonical);
      if (entry) handle.free();
      else entry = { json: canonical, handle };
    }
    markets.delete(entry.json);
    markets.set(entry.json, entry);
    if (markets.size > 4) {
      const oldest = markets.entries().next().value!;
      markets.delete(oldest[0]);
      oldest[1].handle.free();
    }
    return entry;
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
      return result(() => {
        const instrument = instruments.validateInstrumentJson(
          request.instrumentJson,
          request.pricingOptions,
        );
        return instruments.priceInstrumentWithMarket(
          instrument,
          market(request.marketJson).handle,
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
      return result(() => instruments.validateInstrumentJson(instrumentJson));
    },
    formatMoney(request) {
      return result(() =>
        formatMoney(request.value, request.rounding, native.core),
      );
    },
    validateMarket(json) {
      return result(() => market(json).json);
    },
    validateCalibration(json) {
      return result(() => native.calibration.validateCalibrationJson(json));
    },
    dryRun(json) {
      return result(() => native.calibration.dryRun(json));
    },
    calibrate(json) {
      return result(() => native.calibration.calibrate(json));
    },
    sampleCube(request) {
      return result(() => {
        const evaluate =
          request.convention === "normal"
            ? native.models.volatility.getCubeNormalVol
            : request.convention === "black_lognormal"
              ? native.models.volatility.getCubeVol
              : null;
        if (!evaluate)
          throw new RangeError("Unsupported cube output convention");
        const handle = native.core.VolCube.fromJson(
          serializeHost(request.cube),
        );
        try {
          return request.coordinates.map((point) =>
            evaluate(handle, point.expiry, point.tenor, point.strike),
          );
        } finally {
          handle.free();
        }
      });
    },
    sampleFxDelta(request) {
      return result(() => {
        const handle = native.core.FxDeltaVolSurface.fromJson(
          serializeHost(request.surface),
        );
        try {
          return request.coordinates.map((c) =>
            native.models.volatility.getFxDeltaVol(
              handle,
              c.expiry,
              c.strike,
              c.forward,
            ),
          );
        } finally {
          handle.free();
        }
      });
    },
    scenarioTable(request) {
      return result(() =>
        instruments.structuredCreditTrancheScenarioTable(
          request.instrumentJson,
          request.trancheId,
          request.marketJson,
          request.asOf,
          request.gridJson,
        ),
      );
    },
    cashflows(request) {
      return result(() => {
        const instrument = instruments.validateInstrumentJson(
          request.instrumentJson,
        );
        return instruments.instrumentCashflowsWithMarketJson(
          instrument,
          market(request.marketJson).handle,
          request.asOf,
          resolveModel(request.model),
        );
      });
    },
    models: () => result(() => instruments.listModelsGrouped()),
    metrics: () => result(() => instruments.listStandardMetricsGrouped()),
    metricMetadata: (keys) =>
      result(() => instruments.metricMetadata([...keys])),
    calendars: () => result(() => native.core.availableCalendars()),
    exportResult: (value) =>
      result(() =>
        exportValuation(value, native.valuations.validateValuationResultJson),
      ),
  };
}
