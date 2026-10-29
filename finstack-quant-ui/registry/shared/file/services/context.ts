import type {
  core,
  MarketContext,
  portfolio,
  Portfolio,
} from "finstack-quant-wasm";
import { errorValue, type Envelope } from "../finstack-contract";
/** A canonical JSON document and the live native handle built from it. */
export interface CachedHandle<H> {
  json: string;
  handle: H;
}
/** State every crate slice shares: one lazy initialization, envelope wrapping and the handle caches. */
export interface ServiceContext {
  /** Run `operation` after initialization; any failure becomes a cloneable error envelope. */
  result<T>(operation: () => T | Promise<T>): Promise<Envelope<T>>;
  /** Canonical market JSON and its live native handle, from a four-entry LRU that frees evictions. */
  market(json: string): CachedHandle<MarketContext>;
  /** Canonical portfolio spec JSON (Portfolio#toJson) and its live native handle, from a four-entry LRU that frees evictions. */
  portfolio(specJson: string): CachedHandle<Portfolio>;
}
/** Facade members the shared context reads. */
export type ContextNative = {
  initialize(wasmUrl?: string): Promise<unknown>;
  core: Pick<typeof core, "MarketContext">;
  portfolio: Pick<typeof portfolio, "Portfolio">;
};
/**
 * A four-entry LRU of native handles keyed on their canonical JSON. A handle
 * whose canonical form is already cached is freed in favour of the cached
 * one; a handle that cannot serialize is freed before the error propagates;
 * the least recently used entry is freed on eviction.
 */
function handleCache<H extends { toJson(): string; free(): void }>(
  build: (json: string) => H,
): (json: string) => CachedHandle<H> {
  const entries = new Map<string, CachedHandle<H>>();
  return (json) => {
    let entry = entries.get(json);
    if (!entry) {
      const handle = build(json);
      let canonical: string;
      try {
        canonical = handle.toJson();
      } catch (error) {
        handle.free();
        throw error;
      }
      entry = entries.get(canonical);
      if (entry) handle.free();
      else entry = { json: canonical, handle };
    }
    entries.delete(entry.json);
    entries.set(entry.json, entry);
    if (entries.size > 4) {
      const oldest = entries.entries().next().value!;
      entries.delete(oldest[0]);
      oldest[1].handle.free();
    }
    return entry;
  };
}
/** Build the shared context; `initialize` starts native initialization once. */
export function createContext(
  native: ContextNative,
): ServiceContext & { initialize(url?: string): Promise<unknown> } {
  let ready: Promise<unknown> | undefined;
  const initialize = (url?: string) =>
    (ready ??= Promise.resolve().then(() => native.initialize(url)));
  async function result<T>(
    operation: () => T | Promise<T>,
  ): Promise<Envelope<T>> {
    try {
      await initialize();
      return { ok: true, value: await operation() };
    } catch (error) {
      return { ok: false, error: errorValue(error) };
    }
  }
  const market = handleCache((json) =>
    native.core.MarketContext.fromJson(json),
  );
  const portfolio = handleCache((json) =>
    native.portfolio.Portfolio.fromSpec(json),
  );
  return { initialize, result, market, portfolio };
}
