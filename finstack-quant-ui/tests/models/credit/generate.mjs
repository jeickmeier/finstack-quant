import { createRequire } from "node:module";
import { checking, emit } from "../../../scripts/fixture-output.mjs";
const native = createRequire(import.meta.url)(
  "../../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
// An illustrative one-year rating migration matrix, AAA to CCC plus an
// absorbing default (rows in the shape of published agency averages; not a
// sourced data set). Each row sums to one.
const labels = ["AAA", "AA", "A", "BBB", "BB", "B", "CCC", "D"];
const rows = [
  [0.9081, 0.0833, 0.0068, 0.0006, 0.0008, 0.0002, 0.0001, 0.0001],
  [0.007, 0.9065, 0.0779, 0.0064, 0.0006, 0.001, 0.0004, 0.0002],
  [0.0009, 0.0227, 0.9105, 0.0552, 0.0074, 0.0021, 0.0006, 0.0006],
  [0.0002, 0.0033, 0.0595, 0.8693, 0.053, 0.0102, 0.0027, 0.0018],
  [0.0003, 0.0014, 0.0067, 0.0773, 0.8053, 0.0884, 0.01, 0.0106],
  [0, 0.0011, 0.0024, 0.0043, 0.0648, 0.8346, 0.0407, 0.0521],
  [0.0022, 0, 0.0022, 0.013, 0.0238, 0.1124, 0.6486, 0.1978],
  [0, 0, 0, 0, 0, 0, 0, 1],
];
// roundTripTol is omitted: Rust's default tolerance applies.
const request = { labels, rows, horizon: 1, horizons: [1, 2, 3, 5, 7, 10] };
const optional = (value) => value ?? undefined;
const orNull = (value) => (value === undefined ? null : Array.from(value));
// The worker's ModelsApi.migrationMatrix and migrationProjection over the raw
// package (models.credit.migration re-exports these raw classes unchanged).
function scaleOf(matrix) {
  const scale = matrix.scale;
  try {
    return {
      labels: scale.labels(),
      defaultState: scale.defaultState() ?? null,
    };
  } finally {
    scale.free();
  }
}
function migrationMatrix({ labels, rows, horizon }) {
  const matrix = native.TransitionMatrix.fromLabeled(
    labels,
    labels,
    rows,
    horizon,
  );
  try {
    return {
      ...scaleOf(matrix),
      horizon: matrix.horizon,
      rows: matrix.toRows(),
      defaultProbabilities: orNull(matrix.defaultProbabilities()),
    };
  } finally {
    matrix.free();
  }
}
function migrationProjection(request) {
  const matrix = native.TransitionMatrix.fromLabeled(
    request.labels,
    request.labels,
    request.rows,
    request.horizon,
  );
  let generator;
  try {
    generator = native.GeneratorMatrix.fromTransitionMatrix(
      matrix,
      optional(request.roundTripTol),
    );
    return {
      ...scaleOf(matrix),
      generator: {
        rows: generator.toRows(),
        roundTripError: generator.roundTripError,
        regularizationL1: generator.regularizationL1,
      },
      projections: request.horizons.map((t) => {
        const projected = native.project(generator, t);
        try {
          return {
            horizon: projected.horizon,
            defaultProbabilities: orNull(projected.defaultProbabilities()),
          };
        } finally {
          projected.free();
        }
      }),
    };
  } finally {
    generator?.free();
    matrix.free();
  }
}
// fixture-output writes null for NaN and ±Infinity, so the pinned values must be finite.
function assertFinite(value, path) {
  if (typeof value === "number") {
    if (!Number.isFinite(value)) throw new Error(`${path} is ${value}`);
  } else if (value && typeof value === "object")
    for (const [key, child] of Object.entries(value))
      assertFinite(child, `${path}.${key}`);
}
const matrix = migrationMatrix(request);
const projection = migrationProjection(request);
assertFinite({ matrix, projection }, "migration");
await emit(
  new URL("migration.json", import.meta.url),
  JSON.stringify({
    description:
      "Native models.credit.migration results for an illustrative eight-state one-year matrix (AAA to CCC plus an absorbing default): the transition matrix, its regularized generator (Rust default round-trip tolerance) and cumulative default probabilities projected to each horizon.",
    request,
    matrix,
    projection,
  }),
);
console.log(
  `${checking ? "Checked" : "Generated"} the native migration matrix and projection.`,
);
