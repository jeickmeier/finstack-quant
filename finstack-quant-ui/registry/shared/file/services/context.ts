import type { core, MarketContext } from "finstack-quant-wasm";
import { errorValue, type Envelope } from "../finstack-contract";
/** State every crate slice shares: one lazy initialization, envelope wrapping and the market handle cache. */
export interface ServiceContext {
  /** Run `operation` after initialization; any failure becomes a cloneable error envelope. */
  result<T>(operation: () => T | Promise<T>): Promise<Envelope<T>>;
  /** Canonical market JSON and its live native handle, from a four-entry LRU that frees evictions. */
  market(json: string): { json: string; handle: MarketContext };
}
/** Build the shared context; `initialize` starts native initialization once. */
export function createContext(native: {
  initialize(wasmUrl?: string): Promise<unknown>;
  core: Pick<typeof core, "MarketContext">;
}): ServiceContext & { initialize(url?: string): Promise<unknown> } {
  let ready: Promise<unknown> | undefined;
  const markets = new Map<string, { json: string; handle: MarketContext }>();
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
  function market(json: string) {
    let entry = markets.get(json);
    if (!entry) {
      const handle = native.core.MarketContext.fromJson(json);
      let canonical: string;
      try {
        canonical = handle.toJson();
      } catch (error) {
        handle.free();
        throw error;
      }
      entry = markets.get(canonical);
      if (entry) handle.free();
      else entry = { json: canonical, handle };
    }
    markets.delete(entry.json);
    markets.set(entry.json, entry);
    if (markets.size > 4) {
      const oldest = markets.entries().next().value!;
      markets.delete(oldest[0]);
      oldest[1].handle.free();
    }
    return entry;
  }
  return { initialize, result, market };
}
