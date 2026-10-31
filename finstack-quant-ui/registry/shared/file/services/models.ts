import type { core, models } from "finstack-quant-wasm";
import { serializeHost } from "@/lib/finstack/codec.mjs";
import type {
  ArbitrageReport,
  CreditScoreRequest,
  CubeSlice,
  FxStrikeSurface,
  MigrationMatrixRequest,
  ModelsApi,
  OptionCalculatorRequest,
  OptionCalculatorResult,
  PositionRisk,
  ScoringResult,
} from "../finstack-contract";
import { attempt, type ServiceContext } from "./context";
/** The closed-form option functions, models.volatility evaluators and the surface arbitrage check, the core surface handles they read, models.credit.migration, the Merton model, the credit-scoring functions, the PD master scale and the models.factor.risk position decompositions. */
export type ModelsNative = {
  core: Pick<typeof core, "FxDeltaVolSurface" | "VolCube" | "VolSurface">;
  models: Pick<
    typeof models,
    | "bsPrice"
    | "bsGreeks"
    | "bsImpliedVol"
    | "black76Price"
    | "black76Greeks"
    | "black76ImpliedVol"
    | "bachelierPrice"
    | "bachelierGreeks"
  > & {
    volatility: Pick<
      typeof models.volatility,
      | "impliedVolBachelier"
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
      | "checkSurface"
    >;
    credit: Pick<typeof models.credit, "MertonModel" | "mertonReport"> & {
      migration: Pick<
        typeof models.credit.migration,
        "TransitionMatrix" | "GeneratorMatrix" | "project"
      >;
      pd: Pick<typeof models.credit.pd, "MasterScale">;
      scoring: Pick<
        typeof models.credit.scoring,
        | "altmanZScore"
        | "altmanZPrime"
        | "altmanZDoublePrime"
        | "altmanEmScore"
        | "ohlsonOScore"
        | "zmijewskiScore"
      >;
    };
    factor: {
      risk: Pick<
        typeof models.factor.risk,
        | "parametricVarDecomposition"
        | "historicalVarDecomposition"
        | "evaluateRiskBudgetFromDecomposition"
        | "buildStressAttribution"
        | "decomposeFactors"
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
/**
 * One scoring function with the request's named ratios in its positional
 * argument order (conversion only: the facade takes numbers, not the Rust
 * input struct; tests/models/credit/scorecard.test.tsx pins every order).
 * Values pass through unchanged, so Rust rejects a missing or non-finite one.
 */
function score(
  scoring: ModelsNative["models"]["credit"]["scoring"],
  request: CreditScoreRequest,
): ScoringResult {
  switch (request.model) {
    case "altmanZScore": {
      const r = request.input;
      return scoring.altmanZScore(
        r.working_capital_to_total_assets,
        r.retained_earnings_to_total_assets,
        r.ebit_to_total_assets,
        r.market_equity_to_total_liabilities,
        r.sales_to_total_assets,
      );
    }
    case "altmanZPrime": {
      const r = request.input;
      return scoring.altmanZPrime(
        r.working_capital_to_total_assets,
        r.retained_earnings_to_total_assets,
        r.ebit_to_total_assets,
        r.book_equity_to_total_liabilities,
        r.sales_to_total_assets,
      );
    }
    case "altmanZDoublePrime":
    case "altmanEmScore": {
      const r = request.input;
      return (
        request.model === "altmanEmScore"
          ? scoring.altmanEmScore
          : scoring.altmanZDoublePrime
      )(
        r.working_capital_to_total_assets,
        r.retained_earnings_to_total_assets,
        r.ebit_to_total_assets,
        r.book_equity_to_total_liabilities,
      );
    }
    case "ohlsonOScore": {
      const r = request.input;
      return scoring.ohlsonOScore(
        r.log_total_assets_adjusted,
        r.total_liabilities_to_total_assets,
        r.working_capital_to_total_assets,
        r.current_liabilities_to_current_assets,
        r.liabilities_exceed_assets,
        r.net_income_to_total_assets,
        r.funds_from_operations_to_total_liabilities,
        r.negative_net_income_two_years,
        r.net_income_change,
      );
    }
    case "zmijewskiScore": {
      const r = request.input;
      return scoring.zmijewskiScore(
        r.net_income_to_total_assets,
        r.total_liabilities_to_total_assets,
        r.current_assets_to_current_liabilities,
      );
    }
    default:
      throw new RangeError("Unsupported scoring model");
  }
}
/**
 * One closed-form function of the request's model with the named terms in the
 * facade's positional order (conversion only; tests/models/options pins every
 * order). Values pass through unchanged, so Rust rejects an invalid one.
 */
function optionPrice(
  native: ModelsNative["models"],
  r: OptionCalculatorRequest,
  vol: number,
) {
  switch (r.model) {
    case "black_scholes":
      return native.bsPrice(
        r.spot,
        r.strike,
        r.rate,
        r.divYield,
        vol,
        r.expiry,
        r.isCall,
      );
    case "black76":
      return native.black76Price(
        r.forward,
        r.strike,
        r.df,
        vol,
        r.expiry,
        r.isCall,
      );
    case "bachelier":
      return native.bachelierPrice(
        r.forward,
        r.strike,
        vol,
        r.expiry,
        r.isCall,
      );
    default:
      throw new RangeError("Unsupported option model");
  }
}
function optionImpliedVol(
  native: ModelsNative["models"],
  r: OptionCalculatorRequest,
  price: number,
) {
  switch (r.model) {
    case "black_scholes":
      return native.bsImpliedVol(
        r.spot,
        r.strike,
        r.rate,
        r.divYield,
        r.expiry,
        price,
        r.isCall,
      );
    case "black76":
      return native.black76ImpliedVol(
        r.forward,
        r.strike,
        r.df,
        r.expiry,
        price,
        r.isCall,
      );
    case "bachelier":
      return native.volatility.impliedVolBachelier(
        price,
        r.forward,
        r.strike,
        r.expiry,
        r.isCall,
      );
    default:
      throw new RangeError("Unsupported option model");
  }
}
function optionGreeks(
  native: ModelsNative["models"],
  r: OptionCalculatorRequest,
  vol: number,
) {
  switch (r.model) {
    case "black_scholes":
      // An omitted denominator stays unset so Rust applies its default.
      return native.bsGreeks(
        r.spot,
        r.strike,
        r.rate,
        r.divYield,
        vol,
        r.expiry,
        r.isCall,
        r.thetaDaysPerYear ?? undefined,
      );
    case "black76":
      return native.black76Greeks(r.forward, r.strike, vol, r.expiry, r.isCall);
    case "bachelier":
      return native.bachelierGreeks(
        r.forward,
        r.strike,
        vol,
        r.expiry,
        r.isCall,
      );
    default:
      throw new RangeError("Unsupported option model");
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
        // finstack-compose: dispatch on request.convention; evaluate is a native function, not a result
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
    checkSurface(request) {
      return context.result((): ArbitrageReport => {
        const surface = native.core.VolSurface.fromJson(
          serializeHost(request.surface),
        );
        try {
          // An omitted tolerance stays unset so Rust applies its default.
          return native.models.volatility.checkSurface(surface, {
            forward_prices: [...request.forwards],
            ...(request.tolerance == null
              ? {}
              : { tolerance: request.tolerance }),
          });
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
      return context.result(() => {
        const result = score(native.models.credit.scoring, request);
        const scale = native.models.credit.pd.MasterScale.defaultCalibration();
        try {
          return {
            model: request.model,
            result,
            grade: attempt(() => scale.mapScore(result)),
          };
        } finally {
          scale.free();
        }
      });
    },
    optionCalculator(request) {
      return context.result((): OptionCalculatorResult => {
        const { model } = request;
        if (request.mode === "price")
          return {
            model,
            mode: "price",
            price: optionPrice(native.models, request, request.vol),
            greeks: attempt(() =>
              optionGreeks(native.models, request, request.vol),
            ),
          };
        // The solved volatility only passes along to the Greeks call.
        const impliedVol = optionImpliedVol(
          native.models,
          request,
          request.price,
        );
        return {
          model,
          mode: "implied_vol",
          impliedVol,
          greeks: attempt(() =>
            optionGreeks(native.models, request, impliedVol),
          ),
        };
      });
    },
    positionRisk(request) {
      return context.result((): PositionRisk => {
        const risk = native.models.factor.risk;
        const { source } = request;
        const ids = [...request.positionIds];
        // Omitted options stay unset so Rust applies its own defaults.
        const confidence = request.confidence ?? undefined;
        const rows = (matrix: readonly (readonly number[])[]) =>
          matrix.map((row) => [...row]);
        let decomposition;
        if (source.kind === "parametric")
          decomposition = risk.parametricVarDecomposition(
            ids,
            [...source.weights],
            rows(source.covariance),
            confidence,
            source.computeIncremental ?? undefined,
          );
        else if (source.kind === "historical")
          decomposition = risk.historicalVarDecomposition(
            ids,
            rows(source.pnls),
            confidence,
          );
        else throw new RangeError("Unsupported position risk source");
        const { budget } = request;
        return {
          decomposition,
          budget:
            budget == null
              ? null
              : attempt(() =>
                  risk.evaluateRiskBudgetFromDecomposition(
                    decomposition,
                    [...budget.targets],
                    budget.utilizationThreshold ?? undefined,
                  ),
                ),
          // Tail attribution needs scenario P&L, which only a historical source has.
          stress:
            source.kind === "historical"
              ? attempt(() =>
                  risk.buildStressAttribution(
                    ids,
                    rows(source.pnls),
                    confidence,
                  ),
                )
              : null,
        };
      });
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
