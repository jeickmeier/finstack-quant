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
} from "@/workers/finstack-contract";
import { scenarioOptions } from "@/hooks/valuations/use-scenario-table/use-scenario-table";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import fixture from "./scenario-table/cases.json";
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
}, 30000);
afterAll(async () => worker?.close());
it("preserves every native scenario cell and failure through the real worker", async () => {
  for (const c of fixture.cases) {
    const request = {
      instrumentJson: JSON.stringify(c.instrument),
      marketJson,
      asOf: fixture.asOf,
      trancheId: fixture.trancheId,
      gridJson: JSON.stringify(fixture.grid),
    };
    expect(unwrap(await worker.proxy.scenarioTable(request))).toEqual(
      native.structuredCreditTrancheScenarioTable(
        request.instrumentJson,
        request.trancheId,
        request.marketJson,
        request.asOf,
        request.gridJson,
      ),
    );
    let error;
    try {
      native.structuredCreditTrancheScenarioTable(
        request.instrumentJson,
        "missing",
        request.marketJson,
        request.asOf,
        request.gridJson,
      );
    } catch (e) {
      error = errorValue(e);
    }
    expect(
      await worker.proxy.scenarioTable({ ...request, trancheId: "missing" }),
    ).toEqual({ ok: false, error });
  }
}, 30000);
it("snapshots every request input and isolates worker sessions", async () => {
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
}, 30000);
