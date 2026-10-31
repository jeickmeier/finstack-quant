import { createRequire } from "node:module";
import { readFile } from "node:fs/promises";
import { emit } from "../../../scripts/fixture-output.mjs";
const native = createRequire(import.meta.url)(
  "../../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
// The market-browser snapshot, cut to one curve of each lookup kind and one
// kind with no lookup, plus a stored surface, which that snapshot does not hold.
const { supplemental } = JSON.parse(
  await readFile(
    new URL("../market-browser/cases.json", import.meta.url),
    "utf8",
  ),
);
const kept = ["USD-OIS", "FORWARD", "HZ", "PRICE"];
const handle = native.MarketContext.fromJson(
  JSON.stringify({
    ...supplemental,
    hierarchy: null,
    credit_indices: [],
    curves: supplemental.curves.filter((curve) => kept.includes(curve.id)),
    surfaces: [
      {
        id: "EQ-VOL",
        expiries: [0.5, 1, 2],
        strikes: [80, 100, 120],
        secondary_axis: "strike",
        quote_type: "black_lognormal",
        interpolation_mode: "total_variance",
        vols_row_major: [0.3, 0.25, 0.27, 0.29, 0.24, 0.26, 0.28, 0.23, 0.25],
      },
    ],
  }),
);
const market = JSON.parse(handle.toJson());
const attempt = (call) => {
  try {
    return { ok: true, value: call() };
  } catch (error) {
    return {
      ok: false,
      error: { name: error.name, message: error.message, kind: error.kind },
    };
  }
};
// The worker's CoreApi.curveQuery over the raw package.
function lookup(request) {
  if (request.kind === "discount") {
    const curve = handle.getDiscount(request.id);
    try {
      return {
        kind: "discount",
        points: request.points.map((t) => ({
          t,
          df: attempt(() => curve.df(t)),
          zeroRate: attempt(() => curve.zeroRate(t)),
        })),
        forwards: request.forwards.map(([t1, t2]) => ({
          t1,
          t2,
          rate: attempt(() => curve.forward(t1, t2)),
        })),
      };
    } finally {
      curve.free();
    }
  }
  if (request.kind === "forward") {
    const curve = handle.getForward(request.id);
    try {
      return {
        kind: "forward",
        points: request.points.map((t) => ({
          t,
          rate: attempt(() => curve.rate(t)),
        })),
      };
    } finally {
      curve.free();
    }
  }
  if (request.kind === "hazard") {
    const curve = handle.getHazard(request.id);
    try {
      return {
        kind: "hazard",
        points: request.points.map((t) => ({
          t,
          sp: attempt(() => curve.sp(t)),
          hazardRate: attempt(() => curve.hazardRate(t)),
        })),
      };
    } finally {
      curve.free();
    }
  }
  const surface = handle.getSurface(request.id);
  try {
    return {
      kind: "surface",
      points: request.points.map(({ expiry, strike }) => ({
        expiry,
        strike,
        vol: attempt(() => native.getSurfaceVol(surface, expiry, strike)),
      })),
    };
  } finally {
    surface.free();
  }
}
const lookups = {
  discount: {
    kind: "discount",
    id: "USD-OIS",
    points: [0.5, 1, 5, "2030-06-15"],
    forwards: [
      [1, 2],
      [2, 5],
    ],
  },
  forward: {
    kind: "forward",
    id: "FORWARD",
    points: [0.25, 2, "2028-01-15"],
  },
  hazard: { kind: "hazard", id: "HZ", points: [1, 3, "2030-06-15"] },
  surface: {
    kind: "surface",
    id: "EQ-VOL",
    points: [
      { expiry: 0.75, strike: 100 },
      { expiry: 1.5, strike: 90 },
    ],
  },
  // Each point keeps its own native error beside the rest.
  rejected_points: {
    kind: "discount",
    id: "USD-OIS",
    points: [1, "not-a-date"],
    forwards: [
      [2, 1],
      [1, 2],
    ],
  },
  rejected_coordinate: {
    kind: "surface",
    id: "EQ-VOL",
    points: [
      { expiry: 1, strike: 100 },
      { expiry: 10, strike: 100 },
    ],
  },
};
const cases = Object.fromEntries(
  Object.entries(lookups).map(([name, request]) => [
    name,
    { request, result: lookup(request) },
  ]),
);
// An id the market does not hold fails the whole lookup.
const unknownId = attempt(() => handle.getDiscount("NO-SUCH-CURVE"));
try {
  await emit(
    new URL("./cases.json", import.meta.url),
    JSON.stringify({ market, cases, unknownId }),
  );
} finally {
  handle.free();
}
