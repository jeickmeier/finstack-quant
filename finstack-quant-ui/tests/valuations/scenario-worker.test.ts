import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { createRequire } from "node:module";
import { beforeAll, afterAll, it, expect } from "vitest";
import { QueryClient } from "@tanstack/react-query";
import { startWorker } from "../shared/worker/harness.mjs";
import {
  unwrap,
  errorValue,
  type ScenarioRequest,
  type StandardDealType,
} from "@/workers/finstack-contract";
import { scenarioOptions } from "@/hooks/valuations/use-scenario-table/use-scenario-table";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import fixture from "./scenario-table/cases.json";
import tranches from "./scenario-table/tranche-analytics.json";
import standardDeals from "./scenario-table/standard-deals.json";
import { budget } from "../shared/budget";
const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
const marketJson = readFileSync(
  resolve(import.meta.dirname, "../../..", fixture.marketSource),
  "utf8",
);
let worker: Awaited<ReturnType<typeof startWorker>>;
beforeAll(async () => {
  worker = await startWorker();
}, budget(30000));
afterAll(async () => worker?.close());
it(
  "preserves every native scenario cell and failure through the real worker",
  async () => {
    for (const c of fixture.cases) {
      const request = {
        instrumentJson: JSON.stringify(c.instrument),
        marketJson,
        asOf: fixture.asOf,
        trancheId: fixture.trancheId,
        gridJson: JSON.stringify(fixture.grid),
      };
      const deal = native.StructuredCredit.fromJson(request.instrumentJson);
      try {
        expect(unwrap(await worker.proxy.scenarioTable(request))).toEqual(
          native.trancheScenarioTable(
            deal,
            request.trancheId,
            request.marketJson,
            request.asOf,
            request.gridJson,
          ),
        );
        let error;
        try {
          native.trancheScenarioTable(
            deal,
            "missing",
            request.marketJson,
            request.asOf,
            request.gridJson,
          );
        } catch (e) {
          error = errorValue(e);
        }
        expect(
          await worker.proxy.scenarioTable({
            ...request,
            trancheId: "missing",
          }),
        ).toEqual({ ok: false, error });
      } finally {
        deal.free();
      }
    }
  },
  budget(30000),
);
it(
  "builds each standard deal family and lists its native tranches through the real worker",
  async () => {
    const paramsJson = JSON.stringify(standardDeals.params);
    for (const c of standardDeals.cases) {
      const built = unwrap(
        await worker.proxy.newStandardDeal({
          dealType: c.family as StandardDealType,
          paramsJson,
        }),
      );
      expect(built).toBe(c.instrumentJson);
      expect(unwrap(await worker.proxy.dealTranches(built))).toEqual(
        c.tranches,
      );
      // Each family simulates on the UI market; the fixture holds the native table.
      expect(
        unwrap(
          await worker.proxy.scenarioTable({
            instrumentJson: built,
            trancheId: c.tranches.tranches[0]!.id,
            marketJson,
            asOf: standardDeals.asOf,
            gridJson: JSON.stringify(standardDeals.grid),
          }),
        ),
      ).toEqual(c.table);
    }
    let error;
    try {
      native.StructuredCredit.newStandard("cbo", paramsJson);
    } catch (e) {
      error = errorValue(e);
    }
    expect(
      await worker.proxy.newStandardDeal({
        dealType: "cbo" as StandardDealType,
        paramsJson,
      }),
    ).toEqual({ ok: false, error });
  },
  budget(30000),
);
it(
  "snapshots every request input and isolates worker sessions",
  async () => {
    const request = {
      instrumentJson: JSON.stringify(fixture.cases[0]!.instrument),
      marketJson,
      asOf: fixture.asOf,
      trancheId: fixture.trancheId,
      gridJson: JSON.stringify(fixture.grid),
    };
    let calls = 0;
    const client = {
      call: async (_method: string, value: ScenarioRequest) => {
        calls++;
        return unwrap(await worker.proxy.scenarioTable(value));
      },
    } as unknown as FinstackClient;
    const query = new QueryClient();
    try {
      const options = scenarioOptions(client, 1, request);
      request.trancheId = "changed after snapshot";
      await query.fetchQuery(options);
      await query.fetchQuery(options);
      expect(calls).toBe(1);
      expect(scenarioOptions(client, 2, request).queryKey).not.toEqual(
        options.queryKey,
      );
      for (const key of Object.keys(request) as (keyof ScenarioRequest)[])
        expect(
          scenarioOptions(client, 1, { ...request, [key]: "changed" }).queryKey,
        ).not.toEqual(options.queryKey);
      expect(scenarioOptions(client, 1, null).enabled).toBe(false);
    } finally {
      query.clear();
    }
  },
  budget(30000),
);
it(
  "returns the captured native tranche analytics and errors through the real worker",
  async () => {
    for (const entry of tranches.cases) {
      const request = {
        instrumentJson: JSON.stringify(entry.instrument),
        marketJson: JSON.stringify(entry.market),
        asOf: entry.asOf,
        trancheId: entry.trancheId,
      };
      const priced = { ...request, marketPricePct: entry.marketPricePct };
      const results = entry.results as Record<
        string,
        {
          value?: unknown;
          error?: { name: string; kind: string; message: string };
        }
      >;
      const expectResult = (
        actual: { ok: boolean; value?: unknown; error?: unknown },
        expected: (typeof results)[string],
      ) =>
        expected.error
          ? expect(actual).toMatchObject({ ok: false, error: expected.error })
          : expect(actual).toEqual({ ok: true, value: expected.value });
      expectResult(
        await worker.proxy.trancheMetrics(request),
        results.defaultMetrics!,
      );
      expectResult(
        await worker.proxy.trancheMetrics({
          ...priced,
          metrics: entry.metrics,
        }),
        results.metrics!,
      );
      expectResult(
        await worker.proxy.trancheBreakevenCdr(request),
        results.breakevenCdr!,
      );
      expectResult(
        await worker.proxy.trancheDiscountMargin(priced),
        results.discountMargin!,
      );
      expectResult(await worker.proxy.trancheOas(priced), results.oas!);
    }
  },
  budget(30000),
);
