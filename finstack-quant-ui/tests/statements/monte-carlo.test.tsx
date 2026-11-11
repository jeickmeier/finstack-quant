// @vitest-environment jsdom
import { createRequire } from "node:module";
import { afterAll, afterEach, beforeAll, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { QueryClient, hashKey } from "@tanstack/react-query";
import type { MonteCarloResults } from "finstack-quant-wasm";
import stochastic from "../../src/fixtures/statements/analyst-model-stochastic.json";
import config from "../../src/fixtures/statements/analyst-monte-carlo-config.json";
import captured from "../../src/fixtures/statements/analyst-monte-carlo.json";
import breach from "../../src/fixtures/statements/analyst-monte-carlo-breach.json";
import { StatementMonteCarlo } from "@/components/finstack/statements/components/statement-monte-carlo/statement-monte-carlo";
import { evaluateMonteCarloOptions } from "@/hooks/statements/use-evaluate-monte-carlo/use-evaluate-monte-carlo";
import { monteCarloBreachProbabilityOptions } from "@/hooks/statements/use-monte-carlo-breach-probability/use-monte-carlo-breach-probability";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import {
  unwrap,
  type Envelope,
  type WorkerApi,
} from "@/workers/finstack-contract";
import { serializeHost } from "../../src/codec.mjs";
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
const modelJson: string = native.validateFinancialModelJson(
  JSON.stringify(stochastic),
);
const configJson = JSON.stringify(config);
const results = captured as unknown as MonteCarloResults;
let harness: Awaited<ReturnType<typeof startWorker>>;
beforeAll(async () => {
  harness = await startWorker();
});
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
  "simulates and tests a breach through the real worker exactly as the facade returns them",
  async () => {
    const { client, calls } = recordingClient();
    const query = new QueryClient();
    try {
      const simulated = await query.fetchQuery(
        evaluateMonteCarloOptions(client, 1, { modelJson, configJson }),
      );
      // The captured gallery fixture is this same native simulation.
      expect(JSON.parse(serializeHost(simulated))).toEqual(captured);
      expect(simulated.path_data ?? null).toBeNull();
      // Without path data Rust reports no probability: null, not zero.
      expect(
        await query.fetchQuery(
          monteCarloBreachProbabilityOptions(client, 1, {
            resultsJson: serializeHost(simulated),
            metric: breach.metric,
            threshold: breach.threshold,
          }),
        ),
      ).toBeNull();
      const withPaths = await query.fetchQuery(
        evaluateMonteCarloOptions(client, 1, {
          modelJson,
          configJson: JSON.stringify({ ...config, include_path_data: true }),
        }),
      );
      expect(
        await query.fetchQuery(
          monteCarloBreachProbabilityOptions(client, 1, {
            resultsJson: serializeHost(withPaths),
            metric: breach.metric,
            threshold: breach.threshold,
          }),
        ),
      ).toBe(breach.probability);
      expect(calls.map(([method]) => method)).toEqual([
        "evaluateMonteCarlo",
        "monteCarloBreachProbability",
        "evaluateMonteCarlo",
        "monteCarloBreachProbability",
      ]);
      // The full uint64 seed range reaches Rust as an exact integer token.
      const wide = await query.fetchQuery(
        evaluateMonteCarloOptions(client, 1, {
          modelJson,
          configJson: '{"n_paths":4,"seed":18446744073709551615}',
        }),
      );
      expect(wide.n_paths).toBe(4);
      // Native errors are returned unchanged.
      await expect(
        query.fetchQuery(
          evaluateMonteCarloOptions(client, 1, {
            modelJson,
            configJson: '{"n_paths":0,"seed":1}',
          }),
        ),
      ).rejects.toMatchObject({ kind: "validation" });
      await expect(
        query.fetchQuery(
          evaluateMonteCarloOptions(client, 1, {
            modelJson,
            configJson: '{"n_paths":1,"seed":1,"bogus":1}',
          }),
        ),
      ).rejects.toThrow(/unknown field `bogus`/);
    } finally {
      query.clear();
    }
  },
  budget(60000),
);

it("keys both queries by every request field and the worker session, and keeps a NaN threshold distinct", () => {
  const { client } = recordingClient();
  const request = { modelJson, configJson };
  const key = hashKey(evaluateMonteCarloOptions(client, 1, request).queryKey);
  expect(
    hashKey(evaluateMonteCarloOptions(client, 2, request).queryKey),
  ).not.toBe(key);
  for (const field of Object.keys(request))
    expect(
      hashKey(
        evaluateMonteCarloOptions(client, 1, { ...request, [field]: "x" })
          .queryKey,
      ),
    ).not.toBe(key);
  const threshold = (value: number) =>
    hashKey(
      monteCarloBreachProbabilityOptions(client, 1, {
        resultsJson: "{}",
        metric: "leverage",
        threshold: value,
      }).queryKey,
    );
  expect(threshold(Number.NaN)).not.toBe(threshold(null as never));
  expect(evaluateMonteCarloOptions(client, 1, null).enabled).toBe(false);
  expect(monteCarloBreachProbabilityOptions(client, 1, null).enabled).toBe(
    false,
  );
});

it("starts the config empty and submits the uint64 seed as an exact integer token", async () => {
  const submit = vi.fn<(json: string) => void>();
  render(<StatementMonteCarlo onSubmit={submit} results={null} />);
  const paths = screen.getByRole("textbox", { name: "N paths" });
  const seed = screen.getByRole("textbox", { name: "Seed" });
  // Rust's published example is not preloaded: no path count or seed is defaulted.
  expect((paths as HTMLInputElement).value).toBe("");
  expect((seed as HTMLInputElement).value).toBe("");
  fireEvent.change(paths, { target: { value: "200" } });
  fireEvent.change(seed, { target: { value: "18446744073709551615" } });
  const simulate = screen.getByRole("button", { name: "Simulate" });
  await waitFor(() =>
    expect((simulate as HTMLButtonElement).disabled).toBe(false),
  );
  fireEvent.click(simulate);
  await waitFor(() => expect(submit).toHaveBeenCalled());
  expect(submit.mock.lastCall![0]).toBe(
    '{"n_paths":200,"seed":18446744073709551615}',
  );
  // A seed above the uint64 range is rejected before submission.
  fireEvent.change(seed, { target: { value: "18446744073709551616" } });
  expect(
    (await screen.findAllByText(/expected bigint to be <=/)).length,
  ).toBeGreaterThan(0);
});

it("presents the returned percentiles of the chosen metric in Rust's order, with the breach readout as returned", () => {
  const onBreach = vi.fn();
  const { rerender } = render(
    <StatementMonteCarlo
      defaultConfig={config}
      onSubmit={() => {}}
      results={results}
      defaultBreach={{ metric: breach.metric, threshold: breach.threshold }}
      onBreach={onBreach}
      breachProbability={breach.probability}
    />,
  );
  expect(
    screen.getByRole("img", { name: "Leverage percentile fan" }),
  ).toBeTruthy();
  const table = screen.getByRole("table", {
    name: "Leverage percentiles by forecast period",
  });
  expect(
    within(table)
      .getAllByRole("columnheader")
      .map((cell) => cell.textContent),
  ).toEqual(["Period", "5%", "25%", "50%", "75%", "95%"]);
  const rows = within(table).getAllByRole("row").slice(1);
  expect(
    rows.map((row) => within(row).getByRole("rowheader").textContent),
  ).toEqual(results.forecast_periods);
  const q4 = results.percentile_results.leverage!.values["2025Q4"]!;
  expect(
    within(rows[2]!)
      .getAllByRole("cell")
      .map((cell) => cell.textContent),
  ).toEqual(q4.map(([, value]) => String(value)));
  const readout = screen.getByRole("region", { name: "Breach probability" });
  expect(readout.textContent).toContain("14%");
  expect(readout.textContent).toContain("(0.14)");
  fireEvent.click(within(readout).getByRole("button", { name: "Test breach" }));
  expect(onBreach).toHaveBeenLastCalledWith({
    metric: "leverage",
    threshold: 2.25,
  });
  // A cleared threshold is sent as NaN, never 0.
  fireEvent.change(
    within(readout).getByRole("textbox", { name: "Threshold" }),
    {
      target: { value: "" },
    },
  );
  fireEvent.click(within(readout).getByRole("button", { name: "Test breach" }));
  expect(onBreach.mock.lastCall![0].threshold).toBeNaN();
  rerender(
    <StatementMonteCarlo
      onSubmit={() => {}}
      results={results}
      onBreach={onBreach}
      breachProbability={null}
    />,
  );
  expect(
    screen.getByText(
      /Rust reports no probability: these results carry no path data/,
    ),
  ).toBeTruthy();
  expect(
    screen.getByText(
      `Simulation warnings (${(results.warnings ?? []).length})`,
    ),
  ).toBeTruthy();
});

it("shows a native simulation error as returned", () => {
  render(
    <StatementMonteCarlo
      onSubmit={() => {}}
      results={null}
      error="Validation error: n_paths must be positive"
    />,
  );
  expect(
    screen
      .getByText("Validation error: n_paths must be positive")
      .getAttribute("role"),
  ).toBe("alert");
});
