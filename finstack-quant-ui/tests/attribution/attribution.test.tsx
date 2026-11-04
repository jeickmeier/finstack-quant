import { createRequire } from "node:module";
import { afterAll, beforeAll, expect, it, vi } from "vitest";
import { attributePnlManyOptions } from "@/hooks/attribution/use-attribute-pnl-many/use-attribute-pnl-many";
import {
  unwrap,
  type AttributePnlManyRequest,
} from "@/workers/finstack-contract";
import { createContext } from "@/workers/services/context";
import { createAttributionService } from "@/workers/services/attribution";
import { startWorker } from "../shared/worker/harness.mjs";
import { budget } from "../shared/budget";
import fixture from "./cases.json";
import marketScenarios from "../scenarios/application/cases.json";
import bond from "../../src/fixtures/results/bond.json";
const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
let harness: Awaited<ReturnType<typeof startWorker>>;
beforeAll(async () => {
  harness = await startWorker();
});
afterAll(async () => harness?.close());
const base = bond.request;
type Method = AttributePnlManyRequest["method"];
const workbench = (scenario: string, name: string) =>
  fixture.workbench.find(
    (entry) => entry.scenario === scenario && entry.name === name,
  )!;
/** The scenario's T1 market and date, from the real worker's applyMarketScenario. */
async function t1(scenario: string) {
  const { scenarioJson } = marketScenarios.cases.find(
    (entry) => entry.name === scenario,
  )!;
  return unwrap(
    await harness.proxy.applyMarketScenario({
      scenarioJson,
      marketJson: base.marketJson,
      asOf: base.asOf,
      instrumentJson: base.instrumentJson,
    }),
  );
}
async function request(
  scenario: string,
  name: string,
): Promise<AttributePnlManyRequest> {
  const shocked = await t1(scenario);
  return {
    instrumentJsons: [base.instrumentJson],
    marketT0Json: base.marketJson,
    marketT1Json: shocked.marketJson,
    asOfT0: base.asOf,
    asOfT1: shocked.asOf,
    method: workbench(scenario, name).method as Method,
  };
}
it.each(
  fixture.workbench
    .filter((entry) => entry.ok)
    .map((entry) => [entry.scenario, entry.name]),
)(
  "attributes the bond over %s by %s through the real worker exactly as the native fixture",
  async (scenario, name) => {
    const expected = workbench(scenario!, name!);
    const supplied = await request(scenario!, name!);
    expect(supplied.asOfT1).toBe(expected.asOfT1);
    const [result, ...rest] = unwrap(
      await harness.proxy.attributePnlMany(supplied),
    );
    expect(rest).toEqual([]);
    // The fixture stores JSON, where Rust's -0 reads as 0.
    expect(JSON.parse(JSON.stringify(result!.attribution))).toEqual(
      expected.attribution,
    );
    expect(result!.explain).toBe(expected.explain);
    expect(result!.residualWithinTolerance).toBe(
      expected.residualWithinTolerance,
    );
    // Rust returns -0 for a zero share; JSON reads it as 0.
    expect(JSON.parse(JSON.stringify(result!.buckets))).toEqual(
      expected.buckets,
    );
  },
  budget(20000),
);
it("pins Rust's bond figures: an all-rates parallel shock, and carry plus rates over the rolled waterfall", () => {
  const parallel = workbench("usd-ois-50bp", "parallel");
  expect(parallel.attribution!.total_pnl.amount).toBe("-4895.657693227");
  expect(parallel.attribution!.rates_curves_pnl.amount).toBe("-4895.657693227");
  expect(parallel.attribution!.waterfall_steps).toBeUndefined();
  expect(
    parallel.buckets!.find((bucket) => bucket.field === "rates_curves_pnl")!
      .pct_of_total,
  ).toBe(100);
  const sameDate = workbench("usd-ois-50bp", "waterfall");
  expect(sameDate.attribution!.waterfall_steps).toHaveLength(9);
  const rolled = workbench("roll-1m-25bp", "waterfall");
  expect(rolled.asOfT1).toBe("2025-02-03");
  expect(rolled.attribution!.carry.amount).toBe("4613.745264130");
  expect(rolled.attribution!.rates_curves_pnl.amount).toBe("-2238.722127162");
  // Rust chains the steps in its default order: each pv_after is the next pv_before.
  const steps = rolled.attribution!.waterfall_steps!;
  expect(steps.map((step) => step.factor)).toEqual(
    native.defaultWaterfallOrder(),
  );
  for (let index = 1; index < steps.length; index++)
    expect(steps[index]!.pv_before).toEqual(steps[index - 1]!.pv_after);
  expect(rolled.residualWithinTolerance).toBe(true);
});
it("returns Rust's metrics_based note for the workbench hazard curve without a calibration recipe", async () => {
  const [result] = unwrap(
    await harness.proxy.attributePnlMany(
      await request("usd-ois-50bp", "metrics_based"),
    ),
  );
  // Rust drops the quote-space credit metrics and says so; credit P&L stays in the residual.
  expect(result!.attribution.meta.notes).toEqual(
    workbench("usd-ois-50bp", "metrics_based").attribution.meta.notes,
  );
  expect(
    result!.attribution.meta.notes.some((note) =>
      note.includes("calibration recipe"),
    ),
  ).toBe(true);
  expect(result!.attribution.credit_curves_pnl.amount).toBe("0");
});
it("returns Rust's default waterfall order", async () => {
  expect(unwrap(await harness.proxy.defaultWaterfallOrder())).toEqual(
    native.defaultWaterfallOrder(),
  );
});
it("splices canonical market texts into AttributionInputs and sets nothing beyond the method", async () => {
  const attributePnlMany = vi.fn(native.attributePnlMany);
  const residual = vi.fn(native.pnlAttributionResidualWithinTolerance);
  const explain = vi.fn(native.pnlAttributionExplainText);
  const service = createAttributionService(
    {
      attribution: {
        ...native,
        attributePnlMany,
        pnlAttributionResidualWithinTolerance: residual,
        pnlAttributionExplainText: explain,
      },
    },
    createContext({
      initialize: async () => undefined,
      core: { MarketContext: native.MarketContext },
      portfolio: { Portfolio: native.Portfolio },
    }),
  );
  const supplied = await request("usd-ois-50bp", "parallel");
  // A reformatted market text reaches Rust as the cached canonical form.
  const reformatted = JSON.stringify(JSON.parse(base.marketJson), null, 2);
  unwrap(
    await service.attributePnlMany({ ...supplied, marketT0Json: reformatted }),
  );
  const [inputs, instruments] = attributePnlMany.mock.calls[0]!;
  expect(instruments).toEqual([base.instrumentJson]);
  const market = native.MarketContext.fromJson(base.marketJson);
  let canonical: string;
  try {
    canonical = market.toJson();
  } finally {
    market.free();
  }
  expect(inputs).toBe(
    `{"as_of_t0":"${base.asOf}","as_of_t1":"${supplied.asOfT1}","market_t0":${canonical},"market_t1":${supplied.marketT1Json},"method":"parallel"}`,
  );
  // Rust's own tolerances and its default zero-factor hiding.
  expect(residual.mock.calls[0]!.slice(1)).toEqual([]);
  expect(explain.mock.calls[0]!.slice(1)).toEqual([]);
});
it("keys the attribution on the worker session and the complete snapshotted request", async () => {
  const supplied = await request("roll-1m-25bp", "waterfall");
  const options = attributePnlManyOptions(null, 5, supplied);
  expect(options.queryKey).toEqual([
    "finstack",
    5,
    "attributePnlMany",
    supplied,
  ]);
  expect(options.queryKey[3]).not.toBe(supplied);
  expect(attributePnlManyOptions(null, 5, null).enabled).toBe(false);
});
