import { expose } from "comlink";
import { serializeHost } from "finstack-quant-ui/codec";
import init, { valuations } from "finstack-quant-wasm";

// Test fixture only. Production request lifecycle belongs to PR-009.
function errorValue(error) {
  const value = {
    name: error?.name ?? "Error",
    message: error?.message ?? String(error),
    kind: error?.kind,
  };
  for (const key of [
    "report",
    "cause",
    "stage",
    "step_id",
    "solver_diagnostics",
    "details",
  ]) {
    if (error?.[key] !== undefined)
      value[key] =
        error[key] instanceof Error ? errorValue(error[key]) : error[key];
  }
  return value;
}
async function result(call) {
  try {
    return { ok: true, value: await call() };
  } catch (error) {
    return { ok: false, error: errorValue(error) };
  }
}
let ready;
const api = {
  initialize(wasmUrl) {
    ready ??= init(wasmUrl ? { module_or_path: wasmUrl } : undefined);
    return result(async () => {
      await ready;
      return { state: "ready", worker: typeof document === "undefined" };
    });
  },
  price(request) {
    return result(async () => {
      await ready;
      return valuations.instruments.priceInstrument(
        request.instrumentJson,
        request.marketJson,
        request.asOf,
        request.model,
        request.metrics,
        request.pricingOptions,
        request.marketHistory,
      );
    });
  },
  cashflows(request) {
    return result(async () => {
      await ready;
      return valuations.instruments.instrumentCashflowsJson(
        request.instrumentJson,
        request.marketJson,
        request.asOf,
        request.model,
      );
    });
  },
  exportResult(value) {
    return result(() =>
      valuations.validateValuationResultJson(serializeHost(value)),
    );
  },
};
expose(api);
