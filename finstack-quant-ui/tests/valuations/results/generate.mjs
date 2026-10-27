import { createRequire } from "node:module";
import { pricingCases } from "../../browser/cases.mjs";
import { checking, emit } from "../../../scripts/fixture-output.mjs";
const native = createRequire(import.meta.url)(
  "../../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
const request = (await pricingCases()).bond;
request.metrics = ["ytm", "dv01", "duration_mod", "bucketed_dv01"];
const result = native.priceInstrument(
  request.instrumentJson,
  request.marketJson,
  request.asOf,
  request.model,
  request.metrics,
  request.metricPricingOverrides,
  request.marketHistory,
);
const fixture = {
  request,
  result,
  groups: native.listStandardMetricsGrouped(),
  metadata: native.metricMetadata(Object.keys(result.measures)),
  comparisonMetadata: native.metricMetadata([
    "ytm",
    "bucketed_cs01::SYNTHETIC-CREDIT::1y",
    "bucketed_cs01::SYNTHETIC-CREDIT::3y",
    "bucketed_cs01::SYNTHETIC-CREDIT::5y",
    "bucketed_cs01::SYNTHETIC-OTHER::1y",
    "bucketed_dv01::USD-OIS::30y",
    "bucketed_dv01::USD-OIS::10y",
  ]),
};
await emit(
  new URL("../../../src/fixtures/results/bond.json", import.meta.url),
  JSON.stringify(fixture),
);
console.log(
  `${checking ? "Checked" : "Generated"} the native bond valuation fixture and metric groups.`,
);
