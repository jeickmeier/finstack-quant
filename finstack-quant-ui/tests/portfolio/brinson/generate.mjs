import { createRequire } from "node:module";
import { readFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { emit } from "../../../scripts/fixture-output.mjs";
const repository = new URL("../../../../", import.meta.url);
const native = createRequire(import.meta.url)(
  "../../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
const path = "finstack-quant/portfolio/tests/fixtures/host_parity.json";
const bytes = await readFile(new URL(path, repository));
const source = {
  path,
  sha256: createHash("sha256").update(bytes).digest("hex"),
};
// Host data, as a performance system would supply it: the cross-host golden's
// three periods of Tech, Energy and Cash rows, with Cash held only by the
// benchmark in the first two. Weights are decimal fractions summing to one on
// each side; returns are decimal period returns.
const periods = JSON.parse(bytes).inputs.brinson_periods;
const periodLabels = ["2024-Q1", "2024-Q2", "2024-Q3"];
const result = native.brinsonLinked(periods);
function failure(request) {
  try {
    native.brinsonLinked(request);
  } catch (error) {
    return {
      request,
      error: { name: error.name, kind: error.kind, message: error.message },
    };
  }
  throw new Error("Expected a native brinsonLinked error");
}
const errors = {
  // No periods: there is nothing to link.
  empty: failure([]),
  // The second period's portfolio weights sum to 0.9; Rust names the period.
  weights: failure([
    periods[0],
    periods[1].map((row, index) =>
      index === 0 ? { ...row, portfolio_weight: 0.5 } : row,
    ),
    periods[2],
  ]),
  // The third period lists its sectors in another order.
  reordered: failure([periods[0], periods[1], [...periods[2]].reverse()]),
};
await emit(
  new URL("cases.json", import.meta.url),
  JSON.stringify({
    description:
      "Native portfolio.brinsonLinked of host-supplied SectorPeriod rows over three periods (the portfolio host-parity golden): every period's Brinson-Fachler sector effects and totals, the Carino-linked effects by sector and by effect and the compounded portfolio and benchmark returns, with the native errors of no periods, weights not summing to one and a reordered period.",
    source,
    inputs: { periods, periodLabels },
    result,
    errors,
  }),
);
