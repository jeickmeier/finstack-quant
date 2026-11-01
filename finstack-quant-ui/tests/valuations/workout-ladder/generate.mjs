import { readFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { createRequire } from "node:module";
import { emit } from "../../../scripts/fixture-output.mjs";
const repository = new URL("../../../../", import.meta.url);
const native = createRequire(import.meta.url)(
  "../../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
const paths = [
  "finstack-quant/valuations/tests/instruments/json_examples/bond.json",
  "finstack-quant-ui/tests/valuations/instruments/pricing-market.json",
];
const read = async (path) =>
  JSON.parse(await readFile(new URL(path, repository), "utf8"));
const [bullet, baseMarket] = await Promise.all(paths.map(read));
const marketJson = JSON.stringify(baseMarket);
const asOf = "2024-07-15";
const withSpec = (spec) => ({
  ...bullet,
  instrument: {
    ...bullet.instrument,
    spec: { ...bullet.instrument.spec, ...spec },
  },
});
const quote = (clean) => ({
  instrument_pricing_overrides: {
    market_quotes: { quoted_clean_price_pct: clean },
  },
});
const window = (date, price_pct_of_par, make_whole) => ({
  start: date,
  end: date,
  price_pct_of_par,
  ...(make_whole ? { make_whole } : {}),
});
const stepDown = {
  calls: [
    window("2026-01-15", 102),
    window("2028-01-15", 101),
    window("2030-01-15", 100),
  ],
  puts: [],
};
/** The native ladder and the ytw metric of one bond, the value its worst row must hold. */
function ladderCase(spec) {
  const instrumentJson = JSON.stringify(withSpec(spec));
  const bond = native.Bond.fromJson(instrumentJson);
  try {
    return {
      request: { instrumentJson, marketJson, asOf },
      ladder: native.bondWorkoutLadder(bond, marketJson, asOf),
      // JSON has no infinity: a null here is the metric's negative infinity.
      ytw: native.priceInstrument(instrumentJson, marketJson, asOf, "default", [
        "ytw",
      ]).measures.ytw,
    };
  } finally {
    bond.free();
  }
}
const cases = {
  // Above every strike the first call is worst.
  premium: ladderCase({ call_put: stepDown, ...quote(106) }),
  // Below par, running to maturity is worst.
  discount: ladderCase({ call_put: stepDown, ...quote(90) }),
  // At a premium the holder's put is worst: ytw walks puts as well as calls.
  putable: ladderCase({
    call_put: { calls: [], puts: [window("2027-01-15", 100)] },
    ...quote(108),
  }),
  // A make-whole call redeems at Rust's make-whole price, above its strike.
  makeWhole: ladderCase({
    call_put: {
      calls: [
        window("2027-01-15", 100, {
          reference_curve_id: "USD-TREASURY",
          spread_bp: 25,
        }),
      ],
      puts: [],
    },
    ...quote(103),
  }),
  // No exercise rights and no quote: one maturity row at the model price.
  bullet: ladderCase({}),
  // A call on the quote date below the price has no finite yield and is worst.
  quoteDateCall: ladderCase({
    call_put: { calls: [window("2024-07-16", 99)], puts: [] },
    ...quote(100),
  }),
};
// A bond with exercise rights needs a quote; the native error is shown as returned.
const unquoted = {
  instrumentJson: JSON.stringify(withSpec({ call_put: stepDown })),
  marketJson,
  asOf,
};
let unquotedError = null;
const unquotedBond = native.Bond.fromJson(unquoted.instrumentJson);
try {
  native.bondWorkoutLadder(unquotedBond, marketJson, asOf);
} catch (error) {
  unquotedError = {
    name: error.name,
    kind: error.kind,
    message: error.message,
  };
} finally {
  unquotedBond.free();
}
const sources = await Promise.all(
  paths.map(async (path) => ({
    path,
    sha256: createHash("sha256")
      .update(await readFile(new URL(path, repository)))
      .digest("hex"),
  })),
);
await emit(
  new URL("cases.json", import.meta.url),
  JSON.stringify({
    description:
      "Native bondWorkoutLadder results of callable, putable, make-whole and plain bonds, each with its ytw metric, and the native error of a callable bond with no quote.",
    sources,
    metadata: native.metricMetadata(["ytw", "i_spread"]),
    unquoted: { request: unquoted, error: unquotedError },
    cases,
  }),
);
