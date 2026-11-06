import { it, expect } from "vitest";
import { createRequire } from "node:module";
import { readFileSync } from "node:fs";
import { createContext } from "@/workers/services/context";
import type { Portfolio } from "finstack-quant-wasm";
// The shared portfolio handle cache over the real Node package; every handle
// the cache builds or frees is counted.
const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
const { inputs } = JSON.parse(
  readFileSync(
    new URL(
      "../../../finstack-quant/portfolio/tests/fixtures/host_parity.json",
      import.meta.url,
    ),
    "utf8",
  ),
);
function harness(options: { failSerialization?: boolean } = {}) {
  const counts = { built: 0, freed: 0 };
  const Portfolio = {
    fromSpec(json: string): Portfolio {
      const handle = native.Portfolio.fromSpec(json);
      counts.built++;
      const free = handle.free.bind(handle);
      handle.free = () => {
        free();
        counts.freed++;
      };
      if (options.failSerialization)
        handle.toJson = () => {
          throw new TypeError("Portfolio serialization failed");
        };
      return handle;
    },
  };
  const context = createContext({
    initialize: async () => undefined,
    core: { MarketContext: native.MarketContext },
    portfolio: { Portfolio: Portfolio as never },
  });
  return { context, counts };
}
const spec = (id: string) => ({ ...inputs.portfolio_spec, id });
it("dedupes a reformatted spec onto one canonical entry", () => {
  const { context, counts } = harness();
  const compact = JSON.stringify(spec("BOOK"));
  const first = context.portfolio(compact);
  expect(first.json).toBe(native.parsePortfolioSpecJson(compact));
  const reformatted = context.portfolio(JSON.stringify(spec("BOOK"), null, 2));
  // The second handle serialized to the cached canonical form and was freed.
  expect(reformatted).toBe(first);
  expect(counts).toEqual({ built: 2, freed: 1 });
  // The canonical text is a direct hit; other text builds a probe handle,
  // finds the cached canonical entry and frees the probe.
  expect(context.portfolio(first.json)).toBe(first);
  expect(counts).toEqual({ built: 2, freed: 1 });
  expect(context.portfolio(compact)).toBe(first);
  expect(counts).toEqual({ built: 3, freed: 2 });
  expect(first.handle.positionIds).toEqual(["POS-0", "POS-1"]);
});
it("frees the least recently used portfolio when a fifth spec arrives", () => {
  const { context, counts } = harness();
  // Canonical texts, so every repeat request is a direct hit.
  const specs = Array.from({ length: 5 }, (_, i) =>
    native.parsePortfolioSpecJson(JSON.stringify(spec(`BOOK-${i}`))),
  );
  const entries = specs.slice(0, 4).map((json) => context.portfolio(json));
  // Touch BOOK-0 so BOOK-1 becomes the oldest.
  expect(context.portfolio(specs[0]!)).toBe(entries[0]);
  expect(counts).toEqual({ built: 4, freed: 0 });
  context.portfolio(specs[4]!);
  expect(counts).toEqual({ built: 5, freed: 1 });
  // BOOK-1 was evicted and freed: asking again rebuilds it and evicts BOOK-2.
  expect(context.portfolio(specs[1]!)).not.toBe(entries[1]);
  expect(counts).toEqual({ built: 6, freed: 2 });
  expect(context.portfolio(specs[0]!)).toBe(entries[0]);
  expect(counts.built).toBe(6);
});
it("returns cloneable failure envelopes and frees a handle that cannot serialize", async () => {
  const { context, counts } = harness();
  const malformed = await context.result(() => context.portfolio("{"));
  expect(malformed.ok).toBe(false);
  expect(structuredClone(malformed)).toEqual(malformed);
  expect(counts).toEqual({ built: 0, freed: 0 });
  const failing = harness({ failSerialization: true });
  for (let i = 1; i <= 2; i++) {
    const envelope = await failing.context.result(
      () => failing.context.portfolio(JSON.stringify(spec("BOOK"))).json,
    );
    expect(envelope.ok).toBe(false);
    expect(structuredClone(envelope)).toEqual(envelope);
    if (!envelope.ok) expect(envelope.error.message).toContain("serialization");
    expect(failing.counts).toEqual({ built: i, freed: i });
  }
});
