import { createRequire } from "node:module";
import { readFile, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";
const native = createRequire(import.meta.url)(
  "../../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
const instrumentSource =
    "finstack-quant/valuations/tests/instruments/json_examples/structured_credit.json",
  marketSource =
    "finstack-quant-ui/tests/valuations/instruments/pricing-market.json";
const instrumentJson = await readFile(
    new URL(`../../../../${instrumentSource}`, import.meta.url),
    "utf8",
  ),
  marketJson = await readFile(
    new URL(`../../../../${marketSource}`, import.meta.url),
    "utf8",
  );
const instrument = JSON.parse(instrumentJson),
  factor = structuredClone(instrument);
factor.instrument.spec.tranches.tranches[0].current_balance.amount = "50000000";
factor.instrument.spec.pool.assets[0].balance.amount = "50000000";
const grid = {
    cprs: [0, 0.05, 0.15],
    cdrs: [0, 0.03],
    severities: [0.2, 0.6],
    recovery_lag: null,
  },
  asOf = instrument.instrument.spec.closing_date,
  trancheId = "CLONOTES-A";
const cases = [instrument, factor].map((input, index) => {
  const instrumentJson = native.validateInstrumentJson(JSON.stringify(input));
  const table = native.structuredCreditTrancheScenarioTable(
    instrumentJson,
    trancheId,
    marketJson,
    asOf,
    JSON.stringify(grid),
  );
  return {
    name: index ? "factor-adjusted" : "original",
    instrument: JSON.parse(instrumentJson),
    table,
  };
});
await writeFile(
  new URL("./cases.json", import.meta.url),
  JSON.stringify(
    {
      instrumentSource,
      instrumentSha256: createHash("sha256")
        .update(instrumentJson)
        .digest("hex"),
      marketSource,
      marketSha256: createHash("sha256").update(marketJson).digest("hex"),
      asOf,
      trancheId,
      grid,
      cases,
    },
    null,
    2,
  ) + "\n",
);
console.log(
  cases.map((c) => ({
    name: c.name,
    prices: c.table.cells.map((cell) => cell.price),
  })),
);
