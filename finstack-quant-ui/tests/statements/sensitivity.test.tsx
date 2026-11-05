// @vitest-environment jsdom
import { createRequire } from "node:module";
import { afterAll, afterEach, beforeAll, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from "@testing-library/react";
import { QueryClient, hashKey } from "@tanstack/react-query";
import type { StatementResult } from "finstack-quant-wasm";
import model from "../../src/fixtures/statements/analyst-model.json";
import evaluated from "../../src/fixtures/statements/analyst-result.json";
import run from "../../src/fixtures/statements/analyst-sensitivity-run.json";
import captured from "../../src/fixtures/statements/analyst-sensitivity.json";
import {
  StatementSensitivity,
  type StatementSensitivitySubmission,
} from "@/components/finstack/statements/components/statement-sensitivity/statement-sensitivity";
import {
  evaluatedNumber,
  sensitivityModes,
} from "@/components/finstack/statements/components/statement-sensitivity/sensitivity";
import { runSensitivityOptions } from "@/hooks/statements/use-run-sensitivity/use-run-sensitivity";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import {
  unwrap,
  type Envelope,
  type RunSensitivityRequest,
  type StatementSensitivity as Sensitivity,
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
  JSON.stringify(model),
);
const result = evaluated as unknown as StatementResult;
const data = captured as unknown as Sensitivity;
const nodeIds = Object.keys(model.nodes);
const periods = model.periods.map((period) => period.id);
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
type Config = RunSensitivityRequest["config"];
type Parameter = Config["parameters"][number];
/** The fixture run as the worker request, base values read from the evaluated cells. */
function fixtureRequest(): RunSensitivityRequest {
  return {
    modelJson,
    config: {
      mode: run.mode as Config["mode"],
      parameters: run.parameters.map((parameter) => ({
        node_id: parameter.nodeId,
        period_id: parameter.periodId,
        base_value: evaluatedNumber(
          result,
          parameter.nodeId,
          parameter.periodId,
        ),
        pct_range: parameter.pctRange,
      })),
      target_metrics: [run.targetMetric],
    },
    targetMetric: run.targetMetric,
    period: run.period,
  };
}
/** The request with its first parameter changed. */
function withParameter(
  request: RunSensitivityRequest,
  change: Partial<Parameter>,
): RunSensitivityRequest {
  const [first, ...rest] = request.config.parameters;
  return {
    ...request,
    config: {
      ...request.config,
      parameters: [{ ...first!, ...change }, ...rest],
    },
  };
}

it(
  "runs the sensitivity and its tornado through the real worker exactly as the facade returns them",
  async () => {
    const { client, calls } = recordingClient();
    const query = new QueryClient();
    try {
      const request = fixtureRequest();
      const returned = await query.fetchQuery(
        runSensitivityOptions(client, 1, request),
      );
      // The captured gallery fixture is this same native run.
      expect(JSON.parse(serializeHost(returned))).toEqual(captured);
      const direct = native.runSensitivity(
        modelJson,
        JSON.stringify(request.config),
      );
      expect(serializeHost(returned.result)).toBe(serializeHost(direct));
      expect(returned.tornado).toEqual(
        native.generateTornadoEntries(direct, run.targetMetric, run.period),
      );
      expect(calls).toEqual([["runSensitivity", request]]);
      // A full-grid run has scenarios but no tornado (Rust cannot rank it).
      const grid = await query.fetchQuery(
        runSensitivityOptions(client, 1, {
          ...request,
          config: { ...request.config, mode: "full_grid" },
        }),
      );
      expect(grid.tornado).toBeNull();
      expect(grid.result.config.mode).toBe("full_grid");
      expect(grid.result.scenarios).toHaveLength(4);
      // A NaN base value or move reaches the binding, which rejects it.
      const fails = (failing: RunSensitivityRequest) =>
        query.fetchQuery(runSensitivityOptions(client, 1, failing));
      await expect(
        fails(withParameter(request, { base_value: Number.NaN })),
      ).rejects.toMatchObject({ kind: "invalid_type" });
      await expect(
        fails(withParameter(request, { pct_range: [Number.NaN] })),
      ).rejects.toMatchObject({ kind: "invalid_type" });
      // Both perturbation forms, an unknown mode and a missing node are native errors.
      await expect(
        fails(withParameter(request, { perturbations: [1] })),
      ).rejects.toThrow(/exactly one of `perturbations` or `pct_range`/);
      await expect(
        fails({
          ...request,
          config: { ...request.config, mode: "" as Config["mode"] },
        }),
      ).rejects.toThrow(/unknown variant/);
      await expect(
        fails(withParameter(request, { node_id: "missing" })),
      ).rejects.toMatchObject({ name: "FinstackError" });
    } finally {
      query.clear();
    }
  },
  budget(30000),
);

it("keys the run by every request field and the worker session, and keeps NaN distinct", () => {
  const { client } = recordingClient();
  const request = fixtureRequest();
  const key = hashKey(runSensitivityOptions(client, 1, request).queryKey);
  expect(hashKey(runSensitivityOptions(client, 2, request).queryKey)).not.toBe(
    key,
  );
  for (const field of ["modelJson", "targetMetric", "period"])
    expect(
      hashKey(
        runSensitivityOptions(client, 1, { ...request, [field]: "changed" })
          .queryKey,
      ),
    ).not.toBe(key);
  expect(
    hashKey(
      runSensitivityOptions(client, 1, {
        ...request,
        config: { ...request.config, mode: "full_grid" },
      }).queryKey,
    ),
  ).not.toBe(key);
  const withBase = (base_value: number) =>
    hashKey(
      runSensitivityOptions(client, 1, withParameter(request, { base_value }))
        .queryKey,
    );
  expect(withBase(Number.NaN)).not.toBe(withBase(null as never));
  expect(runSensitivityOptions(client, 1, null).enabled).toBe(false);
});

it("offers the generated SensitivityMode values in Rust's order", () => {
  expect(sensitivityModes).toEqual(["diagonal", "full_grid", "tornado"]);
});

it("submits the run with each base value read from the evaluated cell, and a cleared move as NaN", () => {
  const submit = vi.fn<(submission: StatementSensitivitySubmission) => void>();
  render(
    <StatementSensitivity
      result={result}
      nodeIds={nodeIds}
      periods={periods}
      defaultValues={run}
      onSubmit={submit}
      data={null}
    />,
  );
  const button = screen.getByRole("button", { name: "Run sensitivity" });
  fireEvent.click(button);
  const { modelJson: _model, ...expected } = fixtureRequest();
  expect(submit).toHaveBeenLastCalledWith(expected);
  const first = screen.getByRole("group", { name: "Parameter 1" });
  fireEvent.change(
    within(first).getByRole("textbox", { name: "Percentage moves" }),
    { target: { value: "-5, " } },
  );
  fireEvent.click(screen.getByRole("button", { name: "Add parameter" }));
  fireEvent.click(button);
  const parameters = submit.mock.lastCall![0].config.parameters;
  expect(parameters[0]!.pct_range).toEqual([-5, Number.NaN]);
  // A new row starts empty: no node, period or move is defaulted.
  expect(parameters[2]).toEqual({
    node_id: "",
    period_id: "",
    base_value: Number.NaN,
    pct_range: [Number.NaN],
  });
});

it("starts empty without defaults", () => {
  const submit = vi.fn<(submission: StatementSensitivitySubmission) => void>();
  render(
    <StatementSensitivity
      result={result}
      nodeIds={nodeIds}
      periods={periods}
      onSubmit={submit}
      data={null}
    />,
  );
  expect(screen.getByText("No sensitivity run available.")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Run sensitivity" }));
  expect(submit.mock.lastCall![0]).toEqual({
    config: {
      mode: "",
      parameters: [
        {
          node_id: "",
          period_id: "",
          base_value: Number.NaN,
          pct_range: [Number.NaN],
        },
      ],
      target_metrics: [""],
    },
    targetMetric: "",
    period: "",
  });
});

it("presents the native tornado and the target metric in every returned scenario in Rust's order", () => {
  render(
    <StatementSensitivity
      result={result}
      nodeIds={nodeIds}
      periods={periods}
      defaultValues={run}
      onSubmit={() => {}}
      data={data}
    />,
  );
  const tornado = screen.getByRole("table", { name: "Sensitivity tornado" });
  expect(
    within(tornado)
      .getAllByRole("row")
      .slice(1)
      .map((row) =>
        within(row)
          .getAllByRole("cell")
          .map((cell) => cell.textContent),
      ),
  ).toEqual([
    ["Revenue · 2025Q4", "-15,329,233.726987481", "15,329,233.726987481"],
    ["Opex · 2025Q4", "9,190,853.399280012", "-9,190,853.399279982"],
  ]);
  const scenarios = screen.getByRole("table", {
    name: "Returned sensitivity scenarios",
  });
  const rows = within(scenarios).getAllByRole("row").slice(1);
  expect(rows).toHaveLength(data.result.scenarios.length + 1);
  expect(
    within(rows[0]!)
      .getAllByRole("cell")
      .map((cell) => cell.textContent),
  ).toEqual(["Unperturbed", "61,383,803.27707498"]);
  expect(within(rows[1]!).getByRole("rowheader").textContent).toBe(
    "Scenario 1",
  );
  expect(
    within(rows[1]!)
      .getAllByRole("cell")
      .map((cell) => cell.textContent),
  ).toEqual(["Revenue · 2025Q4 = 208,246,194.02699998", "46,054,569.5500875"]);
});

it("shows a native error and notes a full-grid run has no tornado", () => {
  const { rerender } = render(
    <StatementSensitivity
      result={result}
      nodeIds={nodeIds}
      periods={periods}
      onSubmit={() => {}}
      data={null}
      error="Evaluation error: bad run"
    />,
  );
  expect(screen.getByRole("alert").textContent).toBe(
    "Evaluation error: bad run",
  );
  rerender(
    <StatementSensitivity
      result={result}
      nodeIds={nodeIds}
      periods={periods}
      onSubmit={() => {}}
      data={{ ...data, tornado: null }}
    />,
  );
  expect(
    screen.queryByRole("table", { name: "Sensitivity tornado" }),
  ).toBeNull();
  expect(
    screen.getByText(/a full-grid run combines the parameters/),
  ).toBeTruthy();
});
