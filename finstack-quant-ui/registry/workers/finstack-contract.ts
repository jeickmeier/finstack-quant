import type { MarketContextStateWire } from "@/lib/finstack/generated/types/market_context_state";
import type {
  ValuationResult,
  CalibrationResultEnvelope,
} from "finstack-quant-wasm";
/** Complete immutable facade request; JSON strings retain wide integer tokens. */
export interface PriceRequest {
  readonly instrumentJson: string;
  readonly marketJson: string;
  readonly asOf: string;
  readonly model?: string | null;
  readonly metrics?: readonly string[] | null;
  readonly pricingOptions?: string | null;
  readonly marketHistory?: string | null;
}
export interface CashflowRequest {
  readonly instrumentJson: string;
  readonly marketJson: string;
  readonly asOf: string;
  readonly model: string;
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
export interface ValidationRequest {
  readonly instrumentJson: string;
  readonly revision: string | number;
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
export interface WorkerApi {
  initialize(
    wasmUrl?: string,
  ): Promise<Envelope<{ state: "ready"; worker: boolean }>>;
  price(request: PriceRequest): Promise<Envelope<ValuationResult>>;
  validate(
    request: ValidationRequest,
  ): Promise<Envelope<{ revision: string | number; json: string }>>;
  /** Canonicalize the complete market through its native constructor. */
  validateMarket(marketJson: string): Promise<Envelope<string>>;
  /** Full envelope includes prior_market, quote sets, data and every plan setting. */
  validateCalibration(envelopeJson: string): Promise<Envelope<string>>;
  dryRun(envelopeJson: string): Promise<Envelope<string>>;
  calibrate(envelopeJson: string): Promise<Envelope<CalibrationResultEnvelope>>;
  sampleCube(request: CubeSampleRequest): Promise<Envelope<number[]>>;
  sampleFxDelta(request: FxDeltaSampleRequest): Promise<Envelope<number[]>>;
  cashflows(request: CashflowRequest): Promise<Envelope<string>>;
  models(): Promise<Envelope<Record<string, string[]>>>;
  metrics(): Promise<Envelope<Record<string, string[]>>>;
  calendars(): Promise<Envelope<string[]>>;
  exportResult(value: ValuationResult): Promise<Envelope<string>>;
  dispose(): Promise<Envelope<void>>;
}
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
