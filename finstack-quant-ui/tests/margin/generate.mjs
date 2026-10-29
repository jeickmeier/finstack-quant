import { createRequire } from "node:module";
import { readFile } from "node:fs/promises";
import { checking, emit } from "../../scripts/fixture-output.mjs";
const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
// The Rust example of the simm_sensitivities_json contract (IR, equity and FX
// deltas in USD) and a single USD 5Y IR delta of 1000.
const schema = JSON.parse(
  await readFile(
    new URL(
      "../../../finstack-quant/margin/schemas/margin/1/simm_sensitivities_json.schema.json",
      import.meta.url,
    ),
    "utf8",
  ),
);
const asOf = "2025-01-15";
const cases = [
  {
    name: "example",
    sensitivitiesJson: JSON.stringify(schema.examples[0]),
  },
  {
    name: "ir-5y",
    sensitivitiesJson: JSON.stringify({
      base_currency: "USD",
      ir_delta: [["USD", "5Y", 1000]],
    }),
  },
];
// The worker's MarginApi over the raw package (margin.SimmSensitivities and
// margin.SimmCalculator re-export these raw classes unchanged). Version and
// MPOR are left unset so Rust's defaults apply.
function validate(json) {
  const sensitivities = native.SimmSensitivities.fromJson(json);
  try {
    sensitivities.validate();
    return sensitivities.toJson();
  } finally {
    sensitivities.free();
  }
}
function simmIm({ sensitivitiesJson, asOf }) {
  const sensitivities = native.SimmSensitivities.fromJson(sensitivitiesJson);
  try {
    const calculator = new native.SimmCalculator(undefined, undefined);
    try {
      return calculator.calculateFromSensitivities(
        sensitivities,
        sensitivities.baseCurrency,
        asOf,
      );
    } finally {
      calculator.free();
    }
  } finally {
    sensitivities.free();
  }
}
const output = Object.fromEntries(
  cases.map(({ name, sensitivitiesJson }) => {
    const request = { sensitivitiesJson, asOf };
    return [
      name,
      {
        request,
        canonical: validate(sensitivitiesJson),
        result: simmIm(request),
      },
    ];
  }),
);
await emit(
  new URL("../../src/fixtures/margin/simm.json", import.meta.url),
  JSON.stringify({
    description:
      "Native indicative SIMM (default version and MPOR) on 2025-01-15: the simm_sensitivities_json schema example and a single USD 5Y IR delta of 1000, each with its canonical sensitivities text and returned ImResult.",
    ...output,
  }),
);
console.log(
  `${checking ? "Checked" : "Generated"} ${cases.length} native SIMM results.`,
);
