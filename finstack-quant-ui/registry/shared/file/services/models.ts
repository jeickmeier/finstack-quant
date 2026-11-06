import type { core, models } from "finstack-quant-wasm";
import { serializeHost } from "@/lib/finstack/codec.mjs";
import type {
  ArbitrageReport,
  CubeSlice,
  FxStrikeSurface,
  MigrationMatrixRequest,
  ModelsApi,
  SeniorityRecoveryStats,
} from "../finstack-contract";
import type { ServiceContext } from "./context";
/** The closed-form option calculation, models.volatility evaluators and the surface arbitrage check, the core surface handles they read, models.credit.migration, the Merton model, the credit-score report, the liability-management analyses, the recovery waterfall, workout LGD and seniority recovery statistics, and the models.factor.risk position decompositions. */
export type ModelsNative = {
  core: Pick<typeof core, "FxDeltaVolSurface" | "VolCube" | "VolSurface">;
  models: Pick<typeof models, "optionCalculation"> & {
    volatility: Pick<
      typeof models.volatility,
      | "getFxDeltaVol"
      | "getCubeVol"
      | "materializeFxDeltaSurface"
      | "materializeCubeTenorSlice"
      | "materializeCubeExpirySlice"
      | "SabrParameters"
      | "SabrSmile"
      | "checkSurface"
    >;
    credit: Pick<typeof models.credit, "MertonModel" | "mertonReport"> & {
      migration: Pick<
        typeof models.credit.migration,
        "TransitionMatrix" | "GeneratorMatrix" | "project"
      >;
      liabilityManagement: Pick<
        typeof models.credit.liabilityManagement,
        "analyzeExchangeOffer" | "analyzeLme" | "tenderRecommendationHurdle"
      >;
      lgd: Pick<
        typeof models.credit.lgd,
        "WorkoutLgd" | "seniorityRecoveryStats" | "seniorityCalibrationIds"
      >;
      recoveryWaterfall: Pick<
        typeof models.credit.recoveryWaterfall,
        "allocateRecovery"
      >;
      scoring: Pick<typeof models.credit.scoring, "creditScoreReport">;
    };
    factor: {
      risk: Pick<
        typeof models.factor.risk,
        "positionRiskReport" | "decomposeFactors"
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
        const handle = native.core.VolCube.fromJson(
          serializeHost(request.cube),
        );
        try {
          return request.coordinates.map((point) =>
            native.models.volatility.getCubeVol(
              handle,
              point.expiry,
              point.tenor,
              point.strike,
              request.convention,
            ),
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
        const v = native.models.volatility;
        if (request.axis !== "tenor" && request.axis !== "expiry")
          throw new RangeError("Unsupported cube slice axis");
        const cube = native.core.VolCube.fromJson(serializeHost(request.cube));
        try {
          const strikes = [...request.strikes];
          const slice =
            request.axis === "tenor"
              ? v.materializeCubeTenorSlice(
                  cube,
                  request.at,
                  strikes,
                  request.convention,
                )
              : v.materializeCubeExpirySlice(
                  cube,
                  request.at,
                  strikes,
                  request.convention,
                );
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
    checkSurface(request) {
      return context.result((): ArbitrageReport => {
        const surface = native.core.VolSurface.fromJson(
          serializeHost(request.surface),
        );
        try {
          return native.models.volatility.checkSurface(surface, request.config);
        } finally {
          surface.free();
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
    validateMertonModel(json) {
      return context.result(() => {
        const model = native.models.credit.MertonModel.fromJson(json);
        try {
          return model.toJson();
        } finally {
          model.free();
        }
      });
    },
    mertonReport(request) {
      return context.result(() => native.models.credit.mertonReport(request));
    },
    creditScore(request) {
      return context.result(() =>
        native.models.credit.scoring.creditScoreReport(request),
      );
    },
    analyzeExchangeOffer(request) {
      return context.result(() =>
        native.models.credit.liabilityManagement.analyzeExchangeOffer(request),
      );
    },
    analyzeLme(request) {
      return context.result(() =>
        native.models.credit.liabilityManagement.analyzeLme(request),
      );
    },
    tenderRecommendationHurdle: () =>
      context.result(() =>
        native.models.credit.liabilityManagement.tenderRecommendationHurdle(),
      ),
    allocateRecovery(request) {
      return context.result(() =>
        native.models.credit.recoveryWaterfall.allocateRecovery(
          request.estateValue,
          request.claims,
        ),
      );
    },
    workoutLgd(request) {
      return context.result(() => {
        const model = native.models.credit.lgd.WorkoutLgd.fromJson(
          serializeHost(request.model),
        );
        try {
          return model.evaluate(request.ead);
        } finally {
          model.free();
        }
      });
    },
    seniorityRecoveryStats(request) {
      return context.result((): SeniorityRecoveryStats => {
        const recovery = native.models.credit.lgd.seniorityRecoveryStats(
          request.seniority,
          request.calibrationId,
        );
        try {
          return {
            mean: recovery.mean,
            std_dev: recovery.stdDev,
            variance: recovery.variance,
            alpha: recovery.alpha,
            beta_param: recovery.betaParam,
            mode: recovery.mode,
            mean_lgd: recovery.meanLgd,
          };
        } finally {
          recovery.free();
        }
      });
    },
    seniorityCalibrationIds: () =>
      context.result(() => native.models.credit.lgd.seniorityCalibrationIds()),
    optionCalculator(request) {
      return context.result(() => native.models.optionCalculation(request));
    },
    positionRisk(request) {
      return context.result(() =>
        native.models.factor.risk.positionRiskReport(request),
      );
    },
    decomposeFactors(request) {
      return context.result(() =>
        // The JSON passes through; an omitted risk measure stays unset (Rust: variance).
        native.models.factor.risk.decomposeFactors(
          request.sensitivitiesJson,
          request.covarianceJson,
          request.riskMeasureJson ?? undefined,
        ),
      );
    },
  };
}
