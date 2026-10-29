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
// Variation margin under the Rust USD regulatory CSA (zero threshold, USD
// 500,000 MTA, USD 10,000 rounding, daily calls on the usny calendar): the
// worker's MarginApi over the raw package (margin.VmCalculator and the CSA and
// VmResult functions re-export these raw exports unchanged).
function withCalculator(csaJson, use) {
  const calculator = new native.VmCalculator(csaJson);
  try {
    return use(calculator);
  } finally {
    calculator.free();
  }
}
function validateCsa(json) {
  native.csaSpecValidate(json);
  return withCalculator(json, (calculator) => JSON.stringify(calculator.csa));
}
function vm({ csaJson, exposure, postedCollateral, asOf }) {
  return withCalculator(csaJson, (calculator) => {
    const result = calculator.calculate(
      exposure,
      postedCollateral,
      calculator.csa.base_currency,
      asOf,
    );
    return {
      result,
      requiresCall: native.vmResultRequiresCall(result),
      netMargin: native.vmResultNetMargin(result),
    };
  });
}
const csaJson = JSON.stringify(native.csaSpecUsdRegulatory());
const vmAsOf = "2024-06-17";
const vmCases = [
  // Collect above the MTA, post above the MTA, and an exposure below the MTA.
  { name: "collect", exposure: 1_000_000, postedCollateral: 0 },
  { name: "post", exposure: -750_000, postedCollateral: 0 },
  { name: "below-mta", exposure: 100_000, postedCollateral: 0 },
];
const callsRequest = {
  csaJson,
  exposures: [
    ["2024-06-17", 1_000_000],
    ["2024-06-18", 1_200_000],
    ["2024-06-20", -500_000],
    ["2024-06-21", -450_000],
  ],
  initialCollateral: 0,
};
// 2024-06-19 (Juneteenth) is not a usny business day.
const datesRequest = { csaJson, start: "2024-06-14", end: "2024-06-28" };
await emit(
  new URL("../../src/fixtures/margin/vm.json", import.meta.url),
  JSON.stringify({
    description:
      "Native variation margin under margin.csaSpecUsdRegulatory() on 2024-06-17: the Rust CSA presets, the canonical CSA, three VmResults with Rust's call decision and net margin, a margin-call schedule over four exposures and the contractual call dates from 2024-06-14 to 2024-06-28.",
    presets: {
      usd: native.csaSpecUsdRegulatory(),
      eur: native.csaSpecEurRegulatory(),
      gbp: native.csaSpecRegulatory("GBP", "CSA-GBP", "GBP-SONIA"),
    },
    csaJson,
    canonical: validateCsa(csaJson),
    vm: Object.fromEntries(
      vmCases.map(({ name, exposure, postedCollateral }) => {
        const request = { csaJson, exposure, postedCollateral, asOf: vmAsOf };
        return [name, { request, report: vm(request) }];
      }),
    ),
    calls: {
      request: callsRequest,
      result: withCalculator(csaJson, (calculator) =>
        calculator.generateMarginCalls(
          callsRequest.exposures,
          callsRequest.initialCollateral,
        ),
      ),
    },
    callDates: {
      request: datesRequest,
      result: withCalculator(csaJson, (calculator) =>
        calculator.marginCallDates(datesRequest.start, datesRequest.end),
      ),
    },
  }),
);
console.log(
  `${checking ? "Checked" : "Generated"} ${vmCases.length} native variation margin results.`,
);
