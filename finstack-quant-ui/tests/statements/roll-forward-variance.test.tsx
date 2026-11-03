// @vitest-environment jsdom
import { createRequire } from "node:module";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { afterAll, afterEach, beforeAll, expect, it, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { QueryClient, hashKey } from "@tanstack/react-query";
import { z } from "zod";
import type { ForecastMetrics } from "finstack-quant-wasm";
import model from "../../src/fixtures/statements/analyst-model.json";
import reported from "../../src/fixtures/statements/analyst-reported-2025Q4.json";
import capturedVariance from "../../src/fixtures/statements/analyst-variance.json";
import capturedBridge from "../../src/fixtures/statements/analyst-bridge.json";
import capturedBacktest from "../../src/fixtures/statements/analyst-backtest.json";
import varianceConfigSchema from "../../src/generated/schemas/variance_config.json";
import { VarianceBridgeChart } from "@/components/finstack/statements/components/variance-bridge-chart/variance-bridge-chart";
import { ForecastBacktest } from "@/components/finstack/statements/components/forecast-backtest/forecast-backtest";
import { runVarianceOptions } from "@/hooks/statements/use-run-variance/use-run-variance";
import { varianceBridgeOptions } from "@/hooks/statements/use-variance-bridge/use-variance-bridge";
import { backtestForecastOptions } from "@/hooks/statements/use-backtest-forecast/use-backtest-forecast";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import {
  unwrap,
  type BridgeChart,
  type Envelope,
  type WorkerApi,
} from "@/workers/finstack-contract";
import { serializeHost } from "../../src/codec.mjs";
import { converterSchema } from "../../src/schema.mjs";
import { startWorker } from "../shared/worker/harness.mjs";
import { budget } from "../shared/budget";

// Figures are drawn by the shared chart in the browser gates; here each one
// is a labelled placeholder.
vi.mock(
  "@/components/finstack/shared/chart/finstack-chart/finstack-chart",
  async (importOriginal) => ({
    ...(await importOriginal<object>()),
    FinstackChart: ({ ariaLabel }: { ariaLabel: string }) => (
      <div role="img" aria-label={ariaLabel} />
    ),
  }),
);
const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
/** Host-serialized native evaluation of a model fixture. */
function evaluated(spec: unknown): string {
  const evaluator = new native.Evaluator();
  try {
    return serializeHost(
      evaluator.evaluate(
        native.validateFinancialModelJson(JSON.stringify(spec)),
      ),
    );
  } finally {
    evaluator.free();
  }
}
const baseJson = evaluated(model);
const comparisonJson = evaluated(reported);
const rolled = ["2025Q2", "2025Q3", "2025Q4"];
const labels = ["Before roll forward", "After roll forward"] as const;
const configJson = JSON.stringify({
  baseline_label: labels[0],
  comparison_label: labels[1],
  metrics: ["revenue", "opex", "ebitda", "leverage"],
  periods: rolled,
});
const bridgeRequest = {
  baseJson,
  comparisonJson,
  targetMetric: "ebitda",
  period: "2025Q2",
  drivers: ["revenue", "cogs", "opex"],
  baselineLabel: labels[0],
  comparisonLabel: labels[1],
};
const revenue = (json: string) =>
  rolled.map((period) => JSON.parse(json).nodes.revenue[period] as number);
const backtestRequest = {
  actual: revenue(comparisonJson),
  forecast: revenue(baseJson),
};
let harness: Awaited<ReturnType<typeof startWorker>>;
beforeAll(async () => {
  harness = await startWorker();
}, budget(30000));
afterAll(async () => harness?.close());
afterEach(cleanup);

/** Client over the real worker that records every operation and argument it forwards. */
function recordingClient() {
  const calls: [keyof WorkerApi, unknown][] = [];
  const client = {
    call: async (method: keyof WorkerApi, argument: unknown) => {
      calls.push([method, argument]);
      const invoke = harness.proxy[method] as unknown as (
        argument: unknown,
      ) => Promise<Envelope<unknown>>;
      return unwrap(await invoke(argument));
    },
  } as unknown as FinstackClient;
  return { client, calls };
}

it(
  "compares, bridges and backtests through the real worker exactly as the facade returns them",
  async () => {
    const { client, calls } = recordingClient();
    const query = new QueryClient();
    try {
      const variance = await query.fetchQuery(
        runVarianceOptions(client, 1, { baseJson, comparisonJson, configJson }),
      );
      // The captured fixtures are these same native results.
      expect(JSON.parse(serializeHost(variance))).toEqual(capturedVariance);
      expect(serializeHost(variance)).toBe(
        serializeHost(native.runVariance(baseJson, comparisonJson, configJson)),
      );
      const bridge = await query.fetchQuery(
        varianceBridgeOptions(client, 1, bridgeRequest),
      );
      expect(JSON.parse(serializeHost(bridge))).toEqual(capturedBridge);
      expect(bridge.steps.map((step) => step.driver)).toEqual(
        bridgeRequest.drivers,
      );
      const backtest = await query.fetchQuery(
        backtestForecastOptions(client, 1, backtestRequest),
      );
      expect(JSON.parse(serializeHost(backtest))).toEqual(capturedBacktest);
      expect(calls.map(([method]) => method)).toEqual([
        "runVariance",
        "varianceBridge",
        "backtestForecast",
      ]);
      // Native errors are returned unchanged.
      await expect(
        query.fetchQuery(
          runVarianceOptions(client, 1, {
            baseJson,
            comparisonJson,
            configJson: JSON.stringify({
              ...JSON.parse(configJson),
              metrics: [],
            }),
          }),
        ),
      ).rejects.toThrow(/metrics cannot be empty/);
      await expect(
        query.fetchQuery(
          varianceBridgeOptions(client, 1, {
            ...bridgeRequest,
            drivers: ["leverage"],
          }),
        ),
      ).rejects.toThrow(/target metric type and currency/);
      await expect(
        query.fetchQuery(
          backtestForecastOptions(client, 1, {
            actual: [Number.NaN],
            forecast: [1],
          }),
        ),
      ).rejects.toThrow(/must be finite/);
      // A metric Rust leaves undefined stays a non-finite number, not zero.
      const undefinedMape = await query.fetchQuery(
        backtestForecastOptions(client, 1, {
          actual: [0, 0],
          forecast: [1, 2],
        }),
      );
      expect(undefinedMape.mape).toBeNull();
    } finally {
      query.clear();
    }
  },
  budget(30000),
);

it("keys each query by every request field and the worker session", () => {
  const { client } = recordingClient();
  const variance = { baseJson, comparisonJson, configJson };
  const key = hashKey(runVarianceOptions(client, 1, variance).queryKey);
  expect(hashKey(runVarianceOptions(client, 2, variance).queryKey)).not.toBe(
    key,
  );
  for (const field of Object.keys(variance))
    expect(
      hashKey(
        runVarianceOptions(client, 1, { ...variance, [field]: "x" }).queryKey,
      ),
    ).not.toBe(key);
  const bridgeKey = hashKey(
    varianceBridgeOptions(client, 1, bridgeRequest).queryKey,
  );
  for (const field of Object.keys(bridgeRequest))
    expect(
      hashKey(
        varianceBridgeOptions(client, 1, { ...bridgeRequest, [field]: ["x"] })
          .queryKey,
      ),
    ).not.toBe(bridgeKey);
  const nan = (value: number) =>
    hashKey(
      backtestForecastOptions(client, 1, { actual: [value], forecast: [1] })
        .queryKey,
    );
  expect(nan(Number.NaN)).not.toBe(nan(null as never));
  for (const options of [
    runVarianceOptions,
    varianceBridgeOptions,
    backtestForecastOptions,
  ])
    expect(options(client, 1, null).enabled).toBe(false);
});

it("sends a VarianceConfig the generated contract accepts", () => {
  const config = JSON.parse(configJson);
  expect(
    z.fromJSONSchema(converterSchema(varianceConfigSchema)).parse(config),
  ).toEqual(config);
});

it("draws each returned value as its own bar in Rust's order, with no running sum", async () => {
  const bridge = capturedBridge as unknown as BridgeChart;
  render(<VarianceBridgeChart bridge={bridge} />);
  const table = screen.getByRole("table", { name: "EBITDA bridge, 2025Q2" });
  expect(
    within(table)
      .getAllByRole("row")
      .slice(1)
      .map((row) => row.textContent),
  ).toEqual([
    "Before roll forwardLevel54,144,512.5",
    "RevenueDriver contribution3,651,000",
    "CogsDriver contribution1,232,212.5",
    "OpexDriver contribution1,105,800",
    "UnexplainedUnexplained-4,676,025",
    "After roll forwardLevel55,457,500",
  ]);
  // Every bar starts at zero; nothing is accumulated in the chart code.
  const source = await readFile(
    resolve(
      import.meta.dirname,
      "../../registry/statements/components/variance-bridge-chart/variance-bridge-chart.tsx",
    ),
    "utf8",
  );
  expect(source).toContain("y1: 0,");
  expect(source).not.toMatch(/reduce\(|\+=|y1: \(/);
});

it("presents the returned forecast metrics, with null or non-finite ones as n/m", () => {
  const { rerender } = render(
    <ForecastBacktest
      metrics={capturedBacktest as unknown as ForecastMetrics}
      caption="Revenue backtest"
    />,
  );
  const tile = (name: string) =>
    screen.getByRole("region", { name }).querySelector("p")!.textContent;
  expect(
    ["Mae", "Mape", "Mape effective n", "N", "Rmse", "Smape"].map(tile),
  ).toEqual([
    "2,864,625.0099999807",
    "1.2927065631167498",
    "3",
    "3",
    "2,918,113.400292748",
    "1.2933821298938597",
  ]);
  rerender(
    <ForecastBacktest
      metrics={{
        ...(capturedBacktest as unknown as ForecastMetrics),
        mape: null,
        rmse: Number.NaN,
      }}
      caption="Revenue backtest"
    />,
  );
  expect(tile("Mape")).toBe("n/m");
  expect(tile("Rmse")).toBe("n/m");
  rerender(
    <ForecastBacktest
      metrics={null}
      caption="Revenue backtest"
      error="Forecast error: bad series"
    />,
  );
  expect(screen.getByRole("alert").textContent).toBe(
    "Forecast error: bad series",
  );
});
