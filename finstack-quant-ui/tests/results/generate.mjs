import { createRequire } from "node:module";
import { readFile, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { pricingCases } from "../browser/cases.mjs";
const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
const request = (await pricingCases()).bond;
request.metrics = ["ytm", "dv01", "duration_mod", "bucketed_dv01"];
const result = native.priceInstrument(
  request.instrumentJson,
  request.marketJson,
  request.asOf,
  request.model,
  request.metrics,
  request.pricingOptions,
  request.marketHistory,
);
const root = new URL("../../src/fixtures/results/", import.meta.url);
const fixture = {
  request,
  result,
  groups: native.listStandardMetricsGrouped(),
  facadeWasmSha256: createHash("sha256")
    .update(
      await readFile(
        new URL(
          "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm_bg.wasm",
          import.meta.url,
        ),
      ),
    )
    .digest("hex"),
};
await writeFile(
  new URL("bond.json", root),
  JSON.stringify(fixture, null, 2) + "\n",
);
console.log("Generated native bond valuation fixture and metric groups.");
