import type { core, models } from "finstack-quant-wasm";
import { serializeHost } from "@/lib/finstack/codec.mjs";
import type {
  CubeSlice,
  FxStrikeSurface,
  MigrationMatrixRequest,
  ModelsApi,
} from "../finstack-contract";
import type { ServiceContext } from "./context";
/** models.volatility evaluators, the core surface handles they sample and models.credit.migration. */
export type ModelsNative = {
  core: Pick<typeof core, "FxDeltaVolSurface" | "VolCube">;
  models: {
    volatility: Pick<
      typeof models.volatility,
      | "getFxDeltaVol"
      | "getCubeVol"
      | "getCubeNormalVol"
      | "materializeFxDeltaSurface"
      | "materializeCubeTenorSlice"
      | "materializeCubeTenorSliceNormal"
      | "materializeCubeExpirySlice"
      | "materializeCubeExpirySliceNormal"
      | "SabrParameters"
      | "SabrSmile"
    >;
    credit: {
      migration: Pick<
        typeof models.credit.migration,
        "TransitionMatrix" | "GeneratorMatrix" | "project"
      >;
    };
  };
};
type TransitionMatrix = InstanceType<
  ModelsNative["models"]["credit"]["migration"]["TransitionMatrix"]
>;
/** Labels and default state of a matrix's scale; the getter returns a fresh handle, freed here. */
function scaleOf(matrix: TransitionMatrix) {
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
const orNull = (values: Float64Array | undefined) =>
  values === undefined ? null : Array.from(values);
/** ModelsApi over the models facade namespace. */
export function createModelsService(
  native: ModelsNative,
  context: ServiceContext,
): ModelsApi {
  const migration = native.models.credit.migration;
  const labelled = (request: MigrationMatrixRequest) =>
    migration.TransitionMatrix.fromLabeled(
      request.labels,
      request.labels,
      request.rows.map((row) => [...row]),
      request.horizon,
    );
  return {
    sampleCube(request) {
      return context.result(() => {
        const evaluate =
          request.convention === "normal"
            ? native.models.volatility.getCubeNormalVol
            : request.convention === "black_lognormal"
              ? native.models.volatility.getCubeVol
              : null;
        if (!evaluate)
          throw new RangeError("Unsupported cube output convention");
        const handle = native.core.VolCube.fromJson(
          serializeHost(request.cube),
        );
        try {
          return request.coordinates.map((point) =>
            evaluate(handle, point.expiry, point.tenor, point.strike),
          );
        } finally {
          handle.free();
        }
      });
    },
    sampleFxDelta(request) {
      return context.result(() => {
        const handle = native.core.FxDeltaVolSurface.fromJson(
          serializeHost(request.surface),
        );
        try {
          return request.coordinates.map((c) =>
            native.models.volatility.getFxDeltaVol(
              handle,
              c.expiry,
              c.strike,
              c.forward,
            ),
          );
        } finally {
          handle.free();
        }
      });
    },
    materializeFxSurface(request) {
      return context.result(() => {
        const handle = native.core.FxDeltaVolSurface.fromJson(
          serializeHost(request.surface),
        );
        try {
          const surface = native.models.volatility.materializeFxDeltaSurface(
            handle,
            request.spot,
            request.domesticRate,
            request.foreignRate,
          );
          try {
            return JSON.parse(surface.toJson()) as FxStrikeSurface;
          } finally {
            surface.free();
          }
        } finally {
          handle.free();
        }
      });
    },
    cubeSlice(request) {
      return context.result(() => {
        const v = native.models.volatility,
          normal = request.convention === "normal";
        if (!normal && request.convention !== "black_lognormal")
          throw new RangeError("Unsupported cube output convention");
        if (request.axis !== "tenor" && request.axis !== "expiry")
          throw new RangeError("Unsupported cube slice axis");
        const cube = native.core.VolCube.fromJson(serializeHost(request.cube));
        try {
          const strikes = [...request.strikes];
          const slice =
            request.axis === "tenor"
              ? (normal
                  ? v.materializeCubeTenorSliceNormal
                  : v.materializeCubeTenorSlice)(cube, request.at, strikes)
              : (normal
                  ? v.materializeCubeExpirySliceNormal
                  : v.materializeCubeExpirySlice)(cube, request.at, strikes);
          try {
            const json = JSON.parse(slice.toJson());
            return (
              request.axis === "tenor"
                ? { axis: "tenor", surface: json }
                : { axis: "expiry", slice: json }
            ) as CubeSlice;
          } finally {
            slice.free();
          }
        } finally {
          cube.free();
        }
      });
    },
    cubeSmile(request) {
      return context.result(() => {
        const cube = native.core.VolCube.fromJson(serializeHost(request.cube));
        try {
          const parameters = cube.paramsAt(
            request.expiryIndex,
            request.tenorIndex,
          );
          const forward = cube.forwardAt(
            request.expiryIndex,
            request.tenorIndex,
          );
          const expiry = cube.expiries[request.expiryIndex]!;
          const params =
            native.models.volatility.SabrParameters.fromJson(parameters);
          try {
            const smile = new native.models.volatility.SabrSmile(
              params,
              forward,
              expiry,
            );
            try {
              const strikes = [...request.strikes];
              return {
                parameters,
                forward,
                expiry,
                vols: Array.from(smile.generateSmile(strikes)),
                arbitrage: smile.validateNoArbitrage(strikes, request.rate),
                repaired:
                  request.repairIterations == null
                    ? null
                    : Array.from(
                        smile.repairArbitrage(
                          strikes,
                          request.rate,
                          request.repairIterations,
                        ),
                      ),
              };
            } finally {
              smile.free();
            }
          } finally {
            params.free();
          }
        } finally {
          cube.free();
        }
      });
    },
    migrationMatrix(request) {
      return context.result(() => {
        const matrix = labelled(request);
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
      });
    },
    migrationProjection(request) {
      return context.result(() => {
        const matrix = labelled(request);
        try {
          // An omitted tolerance stays unset so Rust applies its default.
          const generator = migration.GeneratorMatrix.fromTransitionMatrix(
            matrix,
            request.roundTripTol ?? undefined,
          );
          try {
            return {
              ...scaleOf(matrix),
              generator: {
                rows: generator.toRows(),
                roundTripError: generator.roundTripError,
                regularizationL1: generator.regularizationL1,
              },
              projections: request.horizons.map((t) => {
                const projected = migration.project(generator, t);
                try {
                  return {
                    horizon: projected.horizon,
                    defaultProbabilities: orNull(
                      projected.defaultProbabilities(),
                    ),
                  };
                } finally {
                  projected.free();
                }
              }),
            };
          } finally {
            generator.free();
          }
        } finally {
          matrix.free();
        }
      });
    },
  };
}
