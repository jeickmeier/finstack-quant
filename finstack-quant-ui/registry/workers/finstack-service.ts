import type {
  core,
  valuations,
  models,
  calibration,
  Market,
} from "finstack-quant-wasm";
import { exportValuation } from "@/lib/finstack/host";
import { errorValue, type Envelope, type WorkerApi } from "./finstack-contract";
/** Dependencies are the published facade; injection permits the same service in a real Node worker. */
export function createService(native: {
  initialize: (wasmUrl?: string) => Promise<unknown>;
  core: Pick<
    typeof core,
    "availableCalendars" | "FxDeltaVolSurface" | "VolCube"
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
  const markets = new Map<string, Market>();
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
    validate(instrumentJson) {
      return result(() => instruments.validateInstrumentJson(instrumentJson));
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
        const c = request.cube;
        const evaluate =
          request.convention === "normal"
            ? native.models.volatility.getCubeNormalVol
            : request.convention === "black_lognormal"
              ? native.models.volatility.getCubeVol
              : null;
        if (!evaluate)
          throw new RangeError("Unsupported cube output convention");
        const handle = new native.core.VolCube(
          c.id,
          c.expiries,
          c.tenors,
          c.params.flatMap((p) => [
            p.alpha,
            p.beta,
            p.rho,
            p.nu,
            p.shift ?? NaN,
          ]),
          c.forwards,
          c.interpolation_mode,
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
        const s = request.surface;
        const handle = new native.core.FxDeltaVolSurface(
          s.id,
          s.expiries,
          s.atm_vols,
          s.rr_25d,
          s.bf_25d,
          s.rr_10d ?? undefined,
          s.bf_10d ?? undefined,
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
        exportValuation(value, native.valuations.validateValuationResultJson),
      ),
  };
}
