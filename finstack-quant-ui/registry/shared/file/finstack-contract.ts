import type { MarketContextStateWire } from "@/lib/finstack/generated/types/market_context_state";
import type { RoundingStamp } from "@/lib/finstack/format/format";
import type {
  ValuationResult,
  CalibrationResultEnvelope,
  CalibrationEnvelope,
  CalibrationValidationReport,
  MetricMetadata,
  MoneyValue,
  ScenarioTable,
  ScenarioResults,
  StatementResult,
  Explanation,
  DependencyTree,
  CheckReport,
} from "finstack-quant-wasm";
/** Complete immutable facade request; JSON strings retain wide integer tokens. */
export interface PriceRequest {
  readonly instrumentJson: string;
  readonly marketJson: string;
  readonly asOf: string;
  readonly model?: string | null;
  readonly metrics?: readonly string[] | null;
  readonly metricPricingOverrides?: string | null;
  readonly marketHistory?: string | null;
}
/** Exact source amount plus the returned rounding stamp; both are snapshotted verbatim and never recomputed. */
export interface MoneyFormatRequest {
  readonly value: MoneyValue;
  readonly rounding?: RoundingStamp;
}
export interface ScenarioRequest {
  readonly instrumentJson: string;
  readonly trancheId: string;
  readonly marketJson: string;
  readonly asOf: string;
  /** Complete native ScenarioGrid JSON; no model inputs are inferred. */
  readonly gridJson: string;
}
export interface CashflowRequest {
  readonly instrumentJson: string;
  readonly marketJson: string;
  readonly asOf: string;
  readonly model: string;
}
/** Exact statement evaluation identity; market and date are supplied together. */
export interface StatementRequest {
  readonly modelJson: string;
  readonly marketJson?: string;
  readonly asOf?: string;
}
/** Match the model and its already evaluated result for diagnostics. */
export interface StatementExplanationRequest {
  readonly modelJson: string;
  readonly resultsJson: string;
  readonly nodeId: string;
  readonly period: string;
}
/** Root node whose dependency tree is traced in modelJson. */
export interface StatementTraceRequest {
  readonly modelJson: string;
  readonly nodeId: string;
}
export interface StatementChecksRequest {
  readonly modelJson: string;
  readonly resultsJson: string;
  readonly kind: "suite" | "three-statement" | "credit-underwriting";
  readonly configJson: string;
}
/** Native ScenarioSet JSON evaluated against one model; market and date are supplied together. */
export interface StatementScenarioRequest {
  readonly modelJson: string;
  readonly scenarioSetJson: string;
  readonly marketJson?: string;
  readonly asOf?: string;
}
/** Roll an evaluated result of `modelJson` up to the native RollupSpec's frequency. */
export interface StatementRollupRequest {
  readonly modelJson: string;
  readonly resultsJson: string;
  readonly specJson: string;
}
/** Scenario name to its native statement result. */
export type StatementScenarioResults = ScenarioResults;
/** Omitted pricing models resolve to the native "default" (see pricing.rs price_instrument). */
export function resolveModel(model?: string | null): string {
  return model ?? "default";
}
export interface FxDeltaSampleRequest {
  readonly surface: MarketContextStateWire["fx_delta_vol_surfaces"][number];
  /** Explicit years, strike and quote/base forward per sample, in original order. */
  readonly coordinates: readonly {
    expiry: number;
    strike: number;
    forward: number;
  }[];
}
export interface CubeSampleRequest {
  readonly cube: MarketContextStateWire["vol_cubes"][number];
  readonly coordinates: readonly {
    expiry: number;
    tenor: number;
    strike: number;
  }[];
  readonly convention: "normal" | "black_lognormal";
}
/** Explicit cloneable exception data; never Comlink's lossy default thrown-error transfer. */
export interface ErrorValue {
  name: string;
  message: string;
  kind?: unknown;
  [field: string]: unknown;
}
export type Envelope<T> =
  { ok: true; value: T } | { ok: false; error: ErrorValue };
/** core: money display, market canonicalization and calendars. */
export interface CoreApi {
  /** core.Currency + core.Money.fromDecimalStr(...).formatWith: native display text for an exact amount under its returned rounding stamp; raw text keeping every supplied digit when the stamp has no scale for the currency. */
  formatMoney(request: MoneyFormatRequest): Promise<Envelope<string>>;
  /** core.MarketContext.fromJson + toJson: canonicalize the complete market and cache its handle. */
  validateMarket(marketJson: string): Promise<Envelope<string>>;
  /** core.availableCalendars. */
  calendars(): Promise<Envelope<string[]>>;
}
/** valuations: instrument validation, pricing, cashflows, scenarios, registries and export. */
export interface ValuationsApi {
  /** valuations.instruments.validateInstrumentJson + priceInstrumentWithMarket over the cached market handle. */
  price(request: PriceRequest): Promise<Envelope<ValuationResult>>;
  /** valuations.instruments.validateInstrumentJson: validate and canonicalize instrument JSON. */
  validate(instrumentJson: string): Promise<Envelope<string>>;
  /** valuations.instruments.validateInstrumentJson + instrumentCashflowsWithMarket over the cached market handle, returned as JSON text. */
  cashflows(request: CashflowRequest): Promise<Envelope<string>>;
  /** valuations.instruments.StructuredCredit.fromJson + trancheScenarioTable. */
  scenarioTable(request: ScenarioRequest): Promise<Envelope<ScenarioTable>>;
  /** valuations.instruments.listModelsGrouped. */
  models(): Promise<Envelope<Record<string, string[]>>>;
  /** valuations.instruments.listStandardMetricsGrouped. */
  metrics(): Promise<Envelope<Record<string, string[]>>>;
  /** valuations.instruments.metricMetadata: native per-key interpretation (units, decoded coordinates, groups) for metric keys. */
  metricMetadata(keys: readonly string[]): Promise<Envelope<MetricMetadata[]>>;
  /** valuations.valuationResultToJson over the host-serialized result: native canonical JSON. */
  exportResult(value: ValuationResult): Promise<Envelope<string>>;
}
/** calibration: envelope validation, static diagnostics and calibration runs. */
export interface CalibrationApi {
  /** calibration.validateCalibration: full envelope includes prior_market, quote sets, data and every plan setting. */
  validateCalibration(
    envelopeJson: string,
  ): Promise<Envelope<CalibrationEnvelope>>;
  /** calibration.dryRun: structured static diagnostics. */
  dryRun(envelopeJson: string): Promise<Envelope<CalibrationValidationReport>>;
  /** calibration.calibrate. */
  calibrate(envelopeJson: string): Promise<Envelope<CalibrationResultEnvelope>>;
}
/** models: volatility sampling over core surface handles. */
export interface ModelsApi {
  /** core.VolCube.fromJson + models.volatility.getCubeNormalVol or getCubeVol, per coordinate. */
  sampleCube(request: CubeSampleRequest): Promise<Envelope<number[]>>;
  /** core.FxDeltaVolSurface.fromJson + models.volatility.getFxDeltaVol, per coordinate. */
  sampleFxDelta(request: FxDeltaSampleRequest): Promise<Envelope<number[]>>;
}
/** statements and statements_analytics: model validation, evaluation and diagnostics. */
export interface StatementsApi {
  /** statements.validateFinancialModelJson. */
  validateStatementModel(modelJson: string): Promise<Envelope<string>>;
  /** statements.parseAndCompile + parseFormula. */
  validateStatementFormula(formula: string): Promise<Envelope<string>>;
  /** statements.validateCheckSuiteSpecJson: validate and canonicalize CheckSuiteSpec JSON. */
  validateCheckSuiteSpec(json: string): Promise<Envelope<string>>;
  /** statements.Evaluator evaluate, or evaluateWithMarket when market and date are supplied. */
  evaluateStatement(
    request: StatementRequest,
  ): Promise<Envelope<StatementResult>>;
  /** statements_analytics.explainFormula. */
  explainStatement(
    request: StatementExplanationRequest,
  ): Promise<Envelope<Explanation>>;
  /** statements_analytics.DependencyTracer.dependencyTree. */
  traceStatement(
    request: StatementTraceRequest,
  ): Promise<Envelope<DependencyTree>>;
  /** statements_analytics.runChecks, runThreeStatementChecks or runCreditUnderwritingChecks, by request kind. */
  runStatementChecks(
    request: StatementChecksRequest,
  ): Promise<Envelope<CheckReport>>;
  /** statements_analytics.evaluateScenarioSet, or evaluateScenarioSetWithMarket to price the capital structure from the market. */
  evaluateStatementScenarios(
    request: StatementScenarioRequest,
  ): Promise<Envelope<StatementScenarioResults>>;
  /** statements.statementResultRollup: an evaluated result at a coarser frequency. */
  rollupStatement(
    request: StatementRollupRequest,
  ): Promise<Envelope<StatementResult>>;
}
/** Every worker method, grouped by the crate whose facade namespace it calls. */
export type WorkerApi = {
  /** Facade default init (module_or_path = wasmUrl), started once; resolves when ready. */
  initialize(
    wasmUrl?: string,
  ): Promise<Envelope<{ state: "ready"; worker: boolean }>>;
} & CoreApi &
  ValuationsApi &
  CalibrationApi &
  ModelsApi &
  StatementsApi;
/** Preserve native exception fields and recursively preserve Error causes. */
export function errorValue(error: unknown): ErrorValue {
  const object =
    error != null && typeof error === "object"
      ? (error as Record<string, unknown>)
      : {};
  const value: ErrorValue = {
    name: typeof object.name === "string" ? object.name : "Error",
    message:
      typeof object.message === "string" ? object.message : String(error),
    kind: object.kind,
  };
  for (const key of Object.getOwnPropertyNames(object)) {
    if (
      [
        "name",
        "message",
        "stack",
        "__proto__",
        "constructor",
        "prototype",
      ].includes(key)
    )
      continue;
    value[key] =
      object[key] instanceof Error ? errorValue(object[key]) : object[key];
  }
  return value;
}
/** Error consumed by Query; original facade fields remain on the exception and payload. */
export class FinstackError extends Error {
  readonly payload: ErrorValue;
  constructor(payload: ErrorValue) {
    super(payload.message);
    this.payload = payload;
    Object.assign(this, payload);
    if (
      payload.cause &&
      typeof payload.cause === "object" &&
      "message" in payload.cause
    )
      this.cause = new FinstackError(payload.cause as ErrorValue);
  }
}
export function unwrap<T>(result: Envelope<T>): T {
  if (!result.ok) throw new FinstackError(result.error);
  return result.value;
}
