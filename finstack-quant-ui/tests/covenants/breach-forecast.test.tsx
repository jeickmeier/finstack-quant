// @vitest-environment jsdom
import { createRequire } from "node:module";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { afterEach, expect, it, vi } from "vitest";
import {
  cleanup,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { FinstackQueryProvider } from "@/hooks/shared/use-finstack/use-finstack";
import {
  forecastBreachesOptions,
  useForecastBreaches,
} from "@/hooks/covenants/use-forecast-breaches/use-forecast-breaches";
import { CovenantBreachForecast } from "@/components/finstack/covenants/components/covenant-breach-forecast/covenant-breach-forecast";
import type { FutureBreachWire } from "@/lib/finstack/generated/types/future_breach";
import { unwrap } from "@/workers/finstack-contract";
import { serializeHost } from "../../src/codec.mjs";
import { useWorkerHarness } from "../shared/worker/use-worker-harness";
import { budget } from "../shared/budget";
import captured from "../../src/fixtures/covenants/analyst-breach-forecast.json";

const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
const read = (name: string) =>
  readFileSync(
    resolve(import.meta.dirname, "../../src/fixtures/", name),
    "utf8",
  );
const resultsJson = read("statements/analyst-result.json");
const engineJson = read("covenants/analyst-covenants.json");
const tightJson = read("covenants/analyst-covenants-tight.json");
const configJson = read("covenants/analyst-forecast-config.json");
// JSON imports widen string unions; the fixture holds native rows.
const forecast = captured as unknown as Record<
  "deterministic" | "stochastic",
  FutureBreachWire[]
>;
vi.mock(
  "@/hooks/shared/use-finstack/client",
  () => import("../shared/worker/client"),
);
type Resources = Record<
  "covenantEngineConstructed" | "covenantEngineFreed",
  number
>;
const harness = useWorkerHarness(["forecastBreaches"]);
const resources = () =>
  (harness.proxy as unknown as { resources(): Promise<Resources> }).resources();
async function counted<T>(call: () => Promise<T>) {
  const before = await resources();
  const value = await call();
  const after = await resources();
  return {
    value,
    engine: [
      after.covenantEngineConstructed - before.covenantEngineConstructed,
      after.covenantEngineFreed - before.covenantEngineFreed,
    ],
  };
}
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
const request = (configJson?: string) => ({
  engineJson: tightJson,
  resultsJson,
  ...(configJson === undefined ? {} : { configJson }),
});

it("captures Rust's deterministic and stochastic breach forecasts of the tightened engine", () => {
  const run = (config?: string) => {
    const engine = native.CovenantEngine.fromJson(tightJson);
    try {
      return config === undefined
        ? native.statementsAnalyticsForecastBreaches(resultsJson, engine)
        : native.statementsAnalyticsForecastBreaches(
            resultsJson,
            engine,
            config,
          );
    } finally {
      engine.free();
    }
  };
  expect(serializeHost(run())).toBe(serializeHost(forecast.deterministic));
  expect(serializeHost(run(configJson))).toBe(
    serializeHost(forecast.stochastic),
  );
  // Deterministic: the breached dates only, probability 1, by date then id.
  expect(
    forecast.deterministic.map((row) => [
      row.breach_date,
      row.covenant_id,
      row.breach_probability,
    ]),
  ).toEqual([
    ["2024-03-31", "min_interest_coverage", 1],
    ["2024-06-30", "min_interest_coverage", 1],
    ["2024-09-30", "min_interest_coverage", 1],
    ["2024-12-31", "max_debt_to_ebitda", 1],
    ["2025-03-31", "max_debt_to_ebitda", 1],
  ]);
  // Stochastic: passing dates whose probability reaches the 0.25 threshold too.
  expect(forecast.stochastic.length).toBeGreaterThan(
    forecast.deterministic.length,
  );
  expect(forecast.stochastic.some((row) => row.headroom! > 0)).toBe(true);
  for (const row of forecast.stochastic)
    expect(row.breach_probability).toBeGreaterThanOrEqual(0.25);
});

it("forecasts through the worker with and without a config, freeing the engine", async () => {
  const plain = await counted(
    async () => await harness.proxy.forecastBreaches(request()),
  );
  expect(serializeHost(unwrap(plain.value))).toBe(
    serializeHost(forecast.deterministic),
  );
  expect(plain.engine).toEqual([1, 1]);
  const stochastic = await counted(
    async () => await harness.proxy.forecastBreaches(request(configJson)),
  );
  expect(serializeHost(unwrap(stochastic.value))).toBe(
    serializeHost(forecast.stochastic),
  );
  expect(stochastic.engine).toEqual([1, 1]);
  // The analyst engine passes every date: Rust forecasts no breach.
  expect(
    unwrap(await harness.proxy.forecastBreaches({ ...request(), engineJson })),
  ).toEqual([]);
});

it("keeps a uint64 seed exact and returns native config and metric errors, freeing the engine", async () => {
  const seeded = (seed: string) =>
    configJson.replace('"random_seed": 7', `"random_seed": ${seed}`);
  expect(seeded("1")).not.toBe(configJson);
  const max = await harness.proxy.forecastBreaches(
    request(seeded("18446744073709551615")),
  );
  expect(max.ok).toBe(true);
  const over = await counted(
    async () =>
      await harness.proxy.forecastBreaches(
        request(seeded("18446744073709551616")),
      ),
  );
  expect(over.value.ok).toBe(false);
  if (!over.value.ok) expect(over.value.error.kind).toBe("validation");
  expect(over.engine).toEqual([1, 1]);
  // cov_lite is an incurrence package: the maintenance default forecasts
  // nothing, an incurrence scope reads total_leverage, which the model lacks.
  const covLite = JSON.stringify({
    specs: native.covenantTemplate({
      kind: "cov_lite",
      max_leverage: 5,
      max_senior_leverage: 3,
    }),
  });
  expect(
    unwrap(
      await harness.proxy.forecastBreaches({
        ...request(),
        engineJson: covLite,
      }),
    ),
  ).toEqual([]);
  const incurrence = await harness.proxy.forecastBreaches({
    ...request('{"scope":"incurrence"}'),
    engineJson: covLite,
  });
  expect(incurrence.ok).toBe(false);
  if (!incurrence.ok) {
    expect(incurrence.error.kind).toBe("not_found");
    expect(incurrence.error.message).toContain("total_leverage");
  }
});

it("keys the forecast on the worker session and the complete snapshotted request", () => {
  const value = request(configJson);
  const options = forecastBreachesOptions(null, 5, value);
  expect(options.queryKey).toEqual(["finstack", 5, "forecastBreaches", value]);
  expect(options.queryKey[3]).not.toBe(value);
  expect(forecastBreachesOptions(null, 5, null).enabled).toBe(false);
});

it("presents the returned breaches in supplied order with every digit, and the empty, loading and error states", () => {
  const { rerender } = render(
    <CovenantBreachForecast
      onSubmit={() => {}}
      breaches={forecast.deterministic}
    />,
  );
  const table = screen.getByRole("table", {
    name: "Forecast covenant breaches",
  });
  const rows = within(table).getAllByRole("row").slice(1);
  expect(rows).toHaveLength(5);
  expect(
    [...rows[0]!.querySelectorAll("th,td")].map((cell) => cell.textContent),
  ).toEqual([
    "2024-03-31",
    "min_interest_coverage",
    "Interest Coverage >= 5.50x",
    "4.500980666853461",
    "5.5",
    "-0.18163987875391624",
    "1",
  ]);
  rerender(<CovenantBreachForecast onSubmit={() => {}} breaches={[]} />);
  expect(screen.getByText("Rust forecast no breaches")).toBeTruthy();
  rerender(<CovenantBreachForecast onSubmit={() => {}} loading />);
  expect(screen.getByRole("status").textContent).toBe("Forecasting breaches…");
  rerender(<CovenantBreachForecast onSubmit={() => {}} error="native" />);
  expect(screen.getByRole("alert").textContent).toBe("native");
});

function Connected() {
  const [config, setConfig] = useState<string | null>(null);
  const breaches = useForecastBreaches(
    config === null ? null : { ...request(), configJson: config },
  );
  return (
    <CovenantBreachForecast
      defaultConfigJson={configJson}
      onSubmit={setConfig}
      breaches={breaches.data ?? null}
      error={breaches.error?.message}
    />
  );
}
it(
  "submits the edited config through the hook and renders Rust's stochastic forecast",
  async () => {
    render(
      <FinstackQueryProvider>
        <Connected />
      </FinstackQueryProvider>,
    );
    const button = await screen.findByRole("button", { name: "Forecast" });
    await waitFor(
      async () => {
        await userEvent.click(button);
        expect(
          screen.queryByRole("table", { name: "Forecast covenant breaches" }),
        ).toBeTruthy();
      },
      { timeout: budget(10000) },
    );
    const rows = within(
      screen.getByRole("table", { name: "Forecast covenant breaches" }),
    )
      .getAllByRole("row")
      .slice(1);
    expect(rows).toHaveLength(forecast.stochastic.length);
  },
  budget(30000),
);
