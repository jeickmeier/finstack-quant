import { createRequire } from "node:module";
import { checking, emit } from "../../../scripts/fixture-output.mjs";
import { panel, failures } from "./panels.mjs";
const native = createRequire(import.meta.url)(
  "../../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
// The worker's AnalyticsApi.correlationMatrix over the raw package:
// Performance.fromReturns of the host panel, getTickerNames and
// correlationMatrix, then free.
function correlation(request) {
  const performance = native.Performance.fromReturns(
    request.dates,
    request.values,
    request.tickerNames,
    undefined,
    request.frequency ?? undefined,
  );
  try {
    const result = performance.correlationMatrix();
    return {
      tickerNames: performance.getTickerNames(),
      matrix: result.matrix.map((row) => Array.from(row)),
      repaired: result.repaired,
    };
  } finally {
    performance.free();
  }
}
function failure(request) {
  try {
    correlation(request);
  } catch (error) {
    return { name: error.name, kind: error.kind, message: error.message };
  }
  throw new Error("Expected a native correlationMatrix error");
}
const value = {
  description:
    "Native analytics.Performance correlation matrix of a host panel of four sleeves' daily decimal returns over 60 business days from 2024-01-02 (tests/analytics/correlation/panels.mjs), with Rust's repaired flag, and the native errors of a zero-variance series, an interior missing return and two series with no overlapping observations. The panels are rebuilt from panels.mjs because JSON cannot carry their NaN gaps; only the valid panel is stored.",
  request: panel,
  result: correlation(panel),
  errors: Object.fromEntries(
    Object.entries(failures).map(([name, request]) => [name, failure(request)]),
  ),
};
await emit(
  new URL("cases.json", import.meta.url),
  `${JSON.stringify(value, null, 2)}\n`,
);
console.log(
  `${checking ? "Checked" : "Generated"} the native correlation matrix fixture.`,
);
