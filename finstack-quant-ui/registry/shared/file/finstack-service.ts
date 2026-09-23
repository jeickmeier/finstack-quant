import type {
  core,
  valuations,
  models,
  calibration,
  statements,
  statements_analytics,
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
  statements: Pick<
    typeof statements,
    "validateFinancialModelJson" | "modelNodeIds" | "validateFormula" | "parseFormulaText" | "evaluateModel" | "evaluateModelWithMarket"
  >;
  statements_analytics: Pick<
    typeof statements_analytics,
    "explainFormula" | "traceDependencies" | "runChecks" | "runThreeStatementChecks" | "runCreditUnderwritingChecks"
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
    validateStatementModel(json) {
      return result(() => native.statements.validateFinancialModelJson(json));
    },
    statementNodeIds(json) {
      return result(() => native.statements.modelNodeIds(json));
    },
    validateStatementFormula(formula) {
      return result(() => {
        native.statements.validateFormula(formula);
        return native.statements.parseFormulaText(formula);
      });
    },
    evaluateStatement(request) {
      return result(() => {
        if (request.marketJson !== undefined || request.asOf !== undefined) {
          if (request.marketJson === undefined || request.asOf === undefined)
            throw new TypeError("Statement market JSON and as-of date must be supplied together");
          return native.statements.evaluateModelWithMarket(
            request.modelJson,
            request.marketJson,
            request.asOf,
          );
        }
        return native.statements.evaluateModel(request.modelJson);
      });
    },
    explainStatement(request) {
      return result(() => native.statements_analytics.explainFormula(
        request.modelJson,
        request.resultsJson,
        request.nodeId,
        request.period,
      ));
    },
    traceStatement(modelJson, nodeId) {
      return result(() => native.statements_analytics.traceDependencies(modelJson, nodeId));
    },
    runStatementChecks(request) {
      return result(() => {
        const { modelJson, resultsJson, configJson } = request;
        switch (request.kind) {
          case "suite":
            return native.statements_analytics.runChecks(modelJson, configJson, resultsJson);
          case "three-statement":
            return native.statements_analytics.runThreeStatementChecks(modelJson, configJson, resultsJson);
          case "credit-underwriting":
            return native.statements_analytics.runCreditUnderwritingChecks(modelJson, configJson, resultsJson);
        }
      });
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
