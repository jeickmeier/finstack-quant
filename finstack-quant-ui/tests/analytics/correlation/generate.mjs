import { checking, emit } from "../../../scripts/fixture-output.mjs";
import { native } from "../../../scripts/fixture-inputs.mjs";
import { panel, failures } from "./panels.mjs";
// The worker's AnalyticsApi.correlationMatrix over the raw package: a
// Performance of the request's panel, getTickerNames and correlationMatrix,
// then free.
function correlation(request) {
  const performance = new native.Performance(request.panel);
  try {
    const result = performance.correlationMatrix();
    return {
      tickerNames: result.tickers,
      matrix: result.matrix,
      nObs: result.n_obs,
      startDate: result.start_date,
      endDate: result.end_date,
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
    "Native analytics.Performance correlation matrix of a host panel of four sleeves' daily decimal returns over 60 business days from 2024-01-02 (tests/analytics/correlation/panels.mjs), with the shared sample count and return-ending bounds, and the native errors of a zero-variance series, an interior missing return and two series with no overlapping observations. Gaps are null; the panels live in panels.mjs and only the valid one is stored.",
  request: { panel },
  result: correlation({ panel }),
  errors: Object.fromEntries(
    Object.entries(failures).map(([name, failed]) => [
      name,
      failure({ panel: failed }),
    ]),
  ),
};
await emit(
  new URL("cases.json", import.meta.url),
  `${JSON.stringify(value, null, 2)}\n`,
);
console.log(
  `${checking ? "Checked" : "Generated"} the native correlation matrix fixture.`,
);
